use crate::route::{
    create_link, get_link_statistics as statistics, health_check, redirect, update_link,
};

use crate::auth::auth;
use axum::{
    middleware,
    routing::{get, patch, post},
    Router,
};
use axum_prometheus::PrometheusMetricLayer;
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod route;
mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "link_shortener=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_link: String = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_conn = PgPoolOptions::new().connect(&db_link).await?;

    let (prometheus_layer, metrics_handle) = PrometheusMetricLayer::pair();
    let app = Router::new()
        .route("/create", post(create_link))
        .route("/:id/statistics", get(statistics))
        .route_layer(middleware::from_fn_with_state(db_conn.clone(), auth))
        .route(
            "/:id",
            patch(update_link)
                .route_layer(middleware::from_fn_with_state(db_conn.clone(), auth))
                .get(redirect),
        )
        .route("/metrics", get(|| async move { metrics_handle.render() }))
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .layer(prometheus_layer)
        .with_state(db_conn);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Could not initialize server");
    tracing::debug!(
        "Listenning on port: {}",
        listener
            .local_addr()
            .expect("Could not convert listener address to local address")
    );
    axum::serve(listener, app)
        .await
        .expect("Could not start server");
    Ok(())
}
