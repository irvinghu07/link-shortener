use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose, Engine};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use url::Url;

use crate::utils::internal_error;

const DEFAULT_CACHE_CONTROL_HEADER_VALUE: &str =
    "public, max-age=300, s-maxage=300, stale-while-revalidate=300, stale-if-error=300";

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub id: String,
    pub target_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTarget {
    pub target_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CountedLinkStatistics {
    pub amount: Option<i64>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
}

fn generate_id() -> String {
    let random_number: u32 = rand::thread_rng().gen_range(0..u32::MAX);
    general_purpose::URL_SAFE_NO_PAD.encode(random_number.to_string())
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Service is healthy")
}

pub async fn redirect(
    State(pool): State<PgPool>,
    Path(requested_link): Path<String>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let select_timeout = tokio::time::Duration::from_millis(300);
    let link = tokio::time::timeout(
        select_timeout,
        sqlx::query_as!(
            Link,
            "SELECT id, target_url FROM links WHERE id = $1",
            requested_link
        )
        .fetch_optional(&pool),
    )
    .await
    .map_err(internal_error)?
    .map_err(internal_error)?
    .ok_or_else(|| "Not Found".to_string())
    .map_err(|err| (StatusCode::NOT_FOUND, err))?;

    tracing::debug!(
        "Redirecting link id {} to {}",
        requested_link,
        link.target_url
    );
    let referer_header = headers
        .get("referer")
        .map(|v| v.to_str().unwrap_or_default().to_string());
    let user_agent_header = headers
        .get("user-agent")
        .map(|v| v.to_str().unwrap_or_default().to_string());

    let statistic_duration = tokio::time::Duration::from_millis(300);
    let saved_statistics = tokio::time::timeout(
        statistic_duration,
        sqlx::query(
            r#"
                INSERT INTO link_statistics(link_id, referer, user_agent) 
                VALUES ($1, $2, $3)
            "#,
        )
        .bind(&requested_link)
        .bind(&referer_header)
        .bind(&user_agent_header)
        .execute(&pool),
    )
    .await;

    match saved_statistics {
        Err(elasped) => tracing::error!("Saving new link click resulted in a timeout: {}", elasped),
        Ok(Err(err)) => tracing::error!(
            "Saving a new link click failed with the following error: {}",
            err
        ),
        _ => tracing::debug!(
            "Persisted new link click for link with id {}, referer {}, and user agent {}",
            requested_link,
            referer_header.unwrap_or_default(),
            user_agent_header.unwrap_or_default()
        ),
    }
    Ok(Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header("Location", link.target_url)
        .header("Cache-Control", DEFAULT_CACHE_CONTROL_HEADER_VALUE)
        .body(Body::empty())
        .expect("This response should always be constructable"))
}

pub async fn create_link(
    State(pool): State<PgPool>,
    Json(new_link): Json<LinkTarget>,
) -> Result<Json<Link>, (StatusCode, String)> {
    let url: String = Url::parse(&new_link.target_url)
        .map_err(|_| (StatusCode::CONFLICT, "Url Malformed".into()))?
        .to_string();
    let new_link_id = generate_id();
    let insert_link_timeout = tokio::time::Duration::from_millis(300);
    let new_link = tokio::time::timeout(
        insert_link_timeout,
        sqlx::query_as!(
            Link,
            r#"
            WITH inserted_link AS (
                INSERT INTO links (id, target_url)
                VALUES ($1, $2)
                RETURNING id, target_url
            )
            SELECT id, target_url FROM inserted_link
            "#,
            &new_link_id,
            &url
        )
        .fetch_one(&pool),
    )
    .await
    .map_err(internal_error)?
    .map_err(internal_error)?;
    tracing::debug!("Created new link with id {} targeting {}", new_link_id, url);
    Ok(Json(new_link))
}

pub async fn update_link(
    State(pool): State<PgPool>,
    Path(id): Path<String>,
    Json(update_link): Json<LinkTarget>,
) -> Result<Json<Link>, (StatusCode, String)> {
    let url: String = Url::parse(&update_link.target_url)
        .map_err(|_| (StatusCode::CONFLICT, "Url Malformed".into()))?
        .to_string();
    let update_link_timeout = tokio::time::Duration::from_millis(300);
    let updated_link = tokio::time::timeout(
        update_link_timeout,
        sqlx::query_as!(
            Link,
            r#"
            WITH updated_link AS (
                UPDATE links
                SET target_url = $1
                WHERE id = $2
                RETURNING id, target_url
            )
            SELECT id, target_url FROM updated_link
            "#,
            &url,
            &id
        )
        .fetch_one(&pool),
    )
    .await
    .map_err(internal_error)?
    .map_err(internal_error)?;
    tracing::debug!("Updated link with id {} targeting {}", id, url);
    Ok(Json(updated_link))
}

pub async fn get_link_statistics(
    State(pool): State<PgPool>,
    Path(link_id): Path<String>,
) -> Result<Json<Vec<CountedLinkStatistics>>, (StatusCode, String)> {
    let fetch_statistics_timeout = tokio::time::Duration::from_millis(300);
    let link_statistics = tokio::time::timeout(
        fetch_statistics_timeout,
        sqlx::query_as!(
            CountedLinkStatistics,
            r#"
                SELECT COUNT(*) AS amount, referer, user_agent
                FROM link_statistics
                GROUP BY link_id, referer, user_agent
                HAVING link_id = $1
            "#,
            &link_id
        )
        .fetch_all(&pool),
    )
    .await
    .map_err(internal_error)?
    .map_err(internal_error)?;
    tracing::debug!("Statistics for link with id {} requested", link_id);
    Ok(Json(link_statistics))
}
