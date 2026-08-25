//! Engine binary. Loads config, builds state, serves the axum router.

use engine::api;
use engine::config::Registry;
use engine::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).init();

    let config_dir =
        std::env::var("CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
    let registry = Registry::load(&PathBuf::from(&config_dir))
        .unwrap_or_else(|e| panic!("failed to load config from {config_dir}: {e}"));
    tracing::info!(
        "loaded registry: {} questions, {} predicates, {} policies",
        registry.questions.len(),
        registry.predicates.len(),
        registry.policies.len()
    );

    let state = AppState::new(registry);
    let app = api::router(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8081);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("engine listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
