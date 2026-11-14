//! Le rotte dei contest dei modi testuali: elenco dei file `.contest`, lettura, salvataggio, cancellazione e scelta del contest attivo
//! (uno solo; «Nessun contest» = nessuno). Il contest attivo arriva alla pagina con le sue macro e il suo elenco dei domini.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::AppState;
use crate::routes::{ApiError, Authed};
use contest::ContestService;
use contest::elenco::ContestTesto;

fn service(state: &AppState) -> Result<Arc<ContestService>, ApiError> {
    state.contest.clone().ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "contest_unavailable"))
}

/// L'elenco dei contest (nome e note), quello attivo e i file di macro e di domini che si possono scegliere.
#[utoipa::path(get, path = "/api/v1/contest-testo", responses((status = 200, description = "Elenco dei contest dei modi testuali")))]
pub(crate) async fn elenco(State(state): State<AppState>, _: Authed) -> Result<Json<Value>, ApiError> {
    let s = service(&state)?;
    let e = s.elenco();
    let voci: Vec<Value> = e.elenco().into_iter().map(|c| json!({ "id": c.id, "name": c.name, "notes": c.notes, "start": c.start, "end": c.end })).collect();
    Ok(Json(json!({ "attivo": e.attivo_id(), "contest": voci, "macros": e.file_disponibili("macros", ".mc"), "domini": e.file_disponibili("dom", ".dom") })))
}

/// Il contest attivo con le sue macro e i domini (`null` se nessuno).
#[utoipa::path(get, path = "/api/v1/contest-testo/attivo", responses((status = 200, description = "Contest attivo o null")))]
pub(crate) async fn attivo(State(state): State<AppState>, _: Authed) -> Result<Json<Value>, ApiError> {
    let s = service(&state)?;
    Ok(Json(json!(s.elenco().attivo())))
}

/// Un contest.
#[utoipa::path(get, path = "/api/v1/contest-testo/voce/{id}", responses((status = 200, description = "Il contest"), (status = 404, description = "Sconosciuto (contest_unknown)")))]
pub(crate) async fn leggi(State(state): State<AppState>, _: Authed, Path(id): Path<String>) -> Result<Json<ContestTesto>, ApiError> {
    let s = service(&state)?;
    s.elenco().leggi(&id).map(Json).ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "contest_unknown"))
}

fn salva_in(state: &AppState, c: ContestTesto) -> Result<Json<Value>, ApiError> {
    let s = service(state)?;
    let id = s.elenco().salva(c).map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "bad_contest").with_message(e))?;
    Ok(Json(json!({ "ok": true, "id": id })))
}

/// Salva un contest esistente.
#[utoipa::path(put, path = "/api/v1/contest-testo/voce/{id}", responses((status = 200, description = "Salvato"), (status = 400, description = "Valori non validi (bad_contest)")))]
pub(crate) async fn salva(State(state): State<AppState>, _: Authed, Path(id): Path<String>, Json(mut c): Json<ContestTesto>) -> Result<Json<Value>, ApiError> {
    c.id = id;
    salva_in(&state, c)
}

/// Aggiunge un contest nuovo (il nome del file viene dal nome del contest).
#[utoipa::path(post, path = "/api/v1/contest-testo/voce", responses((status = 200, description = "Aggiunto, con il suo id"), (status = 400, description = "Valori non validi (bad_contest)")))]
pub(crate) async fn nuovo(State(state): State<AppState>, _: Authed, Json(mut c): Json<ContestTesto>) -> Result<Json<Value>, ApiError> {
    c.id = String::new();
    salva_in(&state, c)
}

/// Cancella un contest.
#[utoipa::path(delete, path = "/api/v1/contest-testo/voce/{id}", responses((status = 200, description = "Cancellato"), (status = 404, description = "Sconosciuto (contest_unknown)")))]
pub(crate) async fn elimina(State(state): State<AppState>, _: Authed, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    let s = service(&state)?;
    s.elenco().elimina(&id).map_err(|e| ApiError::new(StatusCode::NOT_FOUND, "contest_unknown").with_message(e))?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(crate) struct SelezionaBody {
    id: Option<String>,
}

/// Sceglie il contest attivo (`id` nullo = «Nessun contest»).
#[utoipa::path(post, path = "/api/v1/contest-testo/seleziona", responses((status = 200, description = "Scelto"), (status = 404, description = "Sconosciuto (contest_unknown)")))]
pub(crate) async fn seleziona(State(state): State<AppState>, _: Authed, Json(body): Json<SelezionaBody>) -> Result<Json<Value>, ApiError> {
    let s = service(&state)?;
    s.elenco().seleziona(body.id.as_deref()).map_err(|e| ApiError::new(StatusCode::NOT_FOUND, "contest_unknown").with_message(e))?;
    Ok(Json(json!({ "ok": true, "attivo": s.elenco().attivo_id() })))
}
