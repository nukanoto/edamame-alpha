//! `edamame-core`: 統合オリジン/コントローラープロセス。

use clap::Parser;
use tokio::net::TcpListener;

use edamame_alpha::config::CoreConfig;
use edamame_alpha::core::registry::EvalParams;
use edamame_alpha::core::storage::CanonicalStore;
use edamame_alpha::core::{CoreState, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CoreConfig::parse();
    config.validate()?;
    let window = config.window()?;
    let eval = EvalParams {
        heartbeat_timeout_ms: config.heartbeat_timeout_ms,
        overload_threshold: config.overload_threshold,
    };

    let store = CanonicalStore::new(config.storage_dir.clone());
    let state = CoreState::new(store, window, eval);
    let app = router(state);

    let listener = TcpListener::bind(config.bind).await?;
    println!("edamame-core listening on {}", config.bind);
    axum::serve(listener, app).await?;
    Ok(())
}
