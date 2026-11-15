mod common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::*;
use remote_api::AppState;

fn sstv_state(name: &str) -> AppState {
    let dir = std::env::temp_dir().join(format!("aurora-sstv-tune-api-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    state().with_sstv(Arc::new(mode_sstv::SstvService::open(&dir)))
}

async fn send(s: &AppState, method: Method, uri: &str, body: Option<&str>) -> axum::response::Response {
    let cookie = cookie_for(s);
    let mut r = req(method, uri).cookie(&cookie);
    if let Some(b) = body {
        r = r.json(b);
    }
    call(&router(s), r).await
}

#[tokio::test]
async fn la_correzione_di_sintonia_si_imposta_a_passi_e_si_ferma_al_limite() {
    let s = sstv_state("passi");
    status_is(&call(&router(&s), req(Method::POST, "/api/v1/sstv/rx/tune").json(r#"{"steps":1}"#)).await, StatusCode::UNAUTHORIZED);
    let v = body_json(send(&s, Method::POST, "/api/v1/sstv/rx/tune", Some(r#"{"steps":16}"#)).await).await;
    assert_eq!(v["tune_hz"], 100.0);
    let g = body_json(send(&s, Method::GET, "/api/v1/sstv", None).await).await;
    assert_eq!(g["rx"]["tune_hz"], 100.0);
    let v = body_json(send(&s, Method::POST, "/api/v1/sstv/rx/tune", Some(r#"{"steps":-99}"#)).await).await;
    assert_eq!(v["tune_hz"], -200.0);
}
