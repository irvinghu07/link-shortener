use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::IntoResponse,
};
use metrics::counter;
use sha3::{Digest, Sha3_256};
use sqlx::PgPool;

use crate::utils::internal_error;

struct Settings {
    #[allow(dead_code)]
    id: String,
    encrypted_global_api_key: String,
}

pub async fn auth(
    State(pool): State<PgPool>,
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let labels = [("uri", format!("{}!", req.uri()))];
    let api_key = req
        .headers()
        .get("x-api")
        .map(|v| v.to_str().unwrap_or_default())
        .ok_or_else(|| {
            tracing::error!("Unauthorized call to API: No key header received");
            counter!("unauthorized_calls_count", &labels).increment(1);

            (StatusCode::UNAUTHORIZED, "Unauthorized".into())
        })?;
    let fetch_setting_timeout = tokio::time::Duration::from_millis(300);
    let setting: Settings = tokio::time::timeout(
        fetch_setting_timeout,
        sqlx::query_as!(
            Settings,
            "SELECT id, encrypted_global_api_key FROM settings WHERE id = $1",
            "DEFUALT_SETTINGS"
        )
        .fetch_one(&pool),
    )
    .await
    .map_err(internal_error)?
    .map_err(internal_error)?;

    let mut hasher = Sha3_256::new();
    hasher.update(api_key.as_bytes());
    let provided_api_key = hasher.finalize();

    if setting.encrypted_global_api_key != format!("{provided_api_key:x}") {
        tracing::error!("Unauthorized call to API: Incorrect key supplied");
        counter!("unauthorized_calls_count", &labels).increment(1);
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }
    Ok(next.run(req).await)
}
