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

/// Paese, continente e zone di un nominativo dal database DXCC (`None` se manca).
fn info_paese(state: &AppState, call: &str) -> Option<contest::punti_testo::InfoPaese> {
    let v = state.dxcc.as_ref()?(call)?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let n = |k: &str| v.get(k).and_then(Value::as_u64).and_then(|x| u8::try_from(x).ok()).unwrap_or(0);
    Some(contest::punti_testo::InfoPaese { dxcc: s("prefix"), continente: s("continent"), cq: n("cq_zone"), itu: n("itu_zone") })
}

/// Il contest attivo valutato sui QSO del log: contest, QSO valutati, totali e nominativo della stazione.
async fn valuta_attivo(state: &AppState) -> Result<(ContestTesto, Vec<contest::punti_testo::Valutato>, contest::punti_testo::Punteggio, String), ApiError> {
    use contest::punti_testo::{QsoTesto, qso_del_contest, scambi_da_commento, valuta};
    let s = service(state)?;
    let attivo = s.elenco().attivo().ok_or_else(|| ApiError::new(StatusCode::CONFLICT, "contest_off"))?;
    let book = state.logbook.clone();
    let entries = tokio::task::spawn_blocking(move || book.entries())
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "log_task"))?
        .map_err(|e| {
            tracing::warn!("log non letto: {e}");
            ApiError::new(StatusCode::BAD_GATEWAY, "log_read")
        })?;
    let qsos: Vec<QsoTesto> = entries
        .iter()
        .filter_map(|e| {
            let ms = contest::elenco::data_ms(&format!("{}{:0<6}", e.date, e.time))?;
            let (exch_sent, exch_rcvd) = scambi_da_commento(&e.comment);
            Some(QsoTesto { call: e.call.clone(), band: e.band.clone(), mode: e.mode.clone(), freq_hz: e.freq_hz, time_ms: ms, rst_sent: e.rst_sent.clone(), rst_rcvd: e.rst_rcvd.clone(), exch_sent, exch_rcvd })
        })
        .collect();
    let ora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    let del_contest = qso_del_contest(&attivo.contest, &qsos, ora);
    let (call, _) = s.stazione();
    let mio = info_paese(state, &call);
    let paese = |c: &str| info_paese(state, c);
    let (valutati, tot) = valuta(&attivo.contest, &del_contest, mio.as_ref(), &paese, attivo.domain.as_ref());
    Ok((attivo.contest, valutati, tot, call))
}

/// Il punteggio del contest attivo dei modi testuali: QSO, doppi, punti, moltiplicatori, punteggio e righe per banda.
#[utoipa::path(get, path = "/api/v1/contest-testo/punteggio", responses((status = 200, description = "Punteggio del contest attivo"), (status = 409, description = "Nessun contest attivo (contest_off)")))]
pub(crate) async fn punteggio(State(state): State<AppState>, _: Authed) -> Result<Json<Value>, ApiError> {
    let (_, _, tot, _) = valuta_attivo(&state).await?;
    Ok(Json(json!(tot)))
}

/// Il file Cabrillo 3.0 del contest attivo dei modi testuali, con il nome proposto.
#[utoipa::path(get, path = "/api/v1/contest-testo/cabrillo", responses((status = 200, description = "`{testo, nome, qso}`"), (status = 409, description = "Nessun contest attivo (contest_off)")))]
pub(crate) async fn cabrillo(State(state): State<AppState>, _: Authed) -> Result<Json<Value>, ApiError> {
    let (c, valutati, tot, call) = valuta_attivo(&state).await?;
    let s = service(&state)?;
    let testo = contest::punti_testo::cabrillo(&c, &call, &valutati, &tot, s.versione());
    let nome = format!("{}-{}.log", call.trim().to_ascii_uppercase().replace('/', "_"), c.id.replace(' ', "_"));
    Ok(Json(json!({ "testo": testo, "nome": nome, "qso": valutati.len() })))
}
