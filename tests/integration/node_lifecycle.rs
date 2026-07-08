//! コアHTTPサーフェスを通じたキャッシュノードライフサイクル遷移の統合テスト:
//! COLD -> ACTIVE -> DRAINING -> ACTIVE, タイムアウト -> DEAD, 再開 -> COLD
//! （タスク T033）。

#[path = "../support/mod.rs"]
mod support;

use std::time::Duration;

use edamame_alpha::core::registry::EvalParams;
use serde_json::{Value, json};
use support::{default_window, sample_bytes, spawn_core};

async fn heartbeat(client: &reqwest::Client, base: &str, load: f64, retained: Value) -> String {
    let resp = client
        .post(format!("{base}/nodes/cache-a/heartbeat"))
        .json(&json!({
            "health": "ok",
            "load_score": load,
            "retained_segments": retained
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.unwrap();
    body["state"].as_str().unwrap().to_owned()
}

fn full_window() -> Value {
    json!([
        { "channel_id": "live", "rendition": "720p", "sequence": 101 },
        { "channel_id": "live", "rendition": "720p", "sequence": 102 }
    ])
}

#[tokio::test]
async fn node_transitions_through_lifecycle_states() {
    let eval = EvalParams {
        heartbeat_timeout_ms: 200,
        overload_threshold: 0.80,
    };
    let core = spawn_core(default_window(), eval).await;
    let client = reqwest::Client::new();

    // ライブウィンドウを公開し、プリウォーム要件が存在するようにする。
    for seq in [101u64, 102] {
        client
            .put(format!("{}/segments/live/720p/{seq}.ts", core.base_url))
            .body(sample_bytes(&format!("720p-{seq}")))
            .send()
            .await
            .unwrap();
    }

    client
        .put(format!("{}/nodes/cache-a", core.base_url))
        .json(&json!({ "base_url": "http://127.0.0.1:18081" }))
        .send()
        .await
        .unwrap();

    // 保持が空 -> COLD のまま。
    assert_eq!(
        heartbeat(&client, &core.base_url, 0.1, json!([])).await,
        "COLD"
    );
    // ライブウィンドウを保持 -> ACTIVE。
    assert_eq!(
        heartbeat(&client, &core.base_url, 0.1, full_window()).await,
        "ACTIVE"
    );
    // 過負荷 -> DRAINING。
    assert_eq!(
        heartbeat(&client, &core.base_url, 0.95, full_window()).await,
        "DRAINING"
    );
    // 回復 -> ACTIVE。
    assert_eq!(
        heartbeat(&client, &core.base_url, 0.1, full_window()).await,
        "ACTIVE"
    );

    // タイムアウトを超えるまで十分にハートビートを停止し、プレイリストリクエストで
    // レジストリを評価する: 唯一のノードは DEAD なので ACTIVE ノードは残らない。
    tokio::time::sleep(Duration::from_millis(350)).await;
    let media = client
        .get(format!("{}/channels/live/720p.m3u8", core.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(media.status().as_u16(), 503);

    // 再開したハートビートはノードを COLD に戻す; ACTIVE になる前に再度プリウォームが必要。
    assert_eq!(
        heartbeat(&client, &core.base_url, 0.1, full_window()).await,
        "COLD"
    );
}
