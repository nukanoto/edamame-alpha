//! `edamame-node`: キャッシュノードデータプレーンエージェント。

use clap::Parser;
use tokio::net::TcpListener;

use edamame_alpha::config::NodeConfig;
use edamame_alpha::node::cache::{DEFAULT_RETENTION_WINDOW, NodeCache};
use edamame_alpha::node::{NodeState, heartbeat, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = NodeConfig::parse();
    let base_url = format!("http://{}", config.bind);
    let cache = NodeCache::new(config.cache_dir.clone(), DEFAULT_RETENTION_WINDOW);
    let state = NodeState::new(
        config.node_id.clone(),
        config.core_url.clone(),
        base_url,
        cache,
    );

    if let Err(err) = heartbeat::register(&state).await {
        eprintln!("warning: initial registration failed: {err}");
    }

    let loop_state = state.clone();
    let interval = config.heartbeat_interval_ms;
    tokio::spawn(async move {
        heartbeat::run_loop(loop_state, interval).await;
    });

    let app = router(state);
    let listener = TcpListener::bind(config.bind).await?;
    println!(
        "edamame-node {} listening on {}",
        config.node_id, config.bind
    );
    axum::serve(listener, app).await?;
    Ok(())
}
