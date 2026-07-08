//! ドキュメント化されたコアおよびノードHTTPサーフェスの契約テスト
//! （タスク T018, T032, T044）。

#[path = "../support/mod.rs"]
mod support;

use serde_json::Value;
use support::{default_eval, default_window, sample_bytes, spawn_core, spawn_node};

#[tokio::test]
async fn segment_ingest_status_codes() {
    let core = spawn_core(default_window(), default_eval()).await;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/segments/live/720p/101.ts?bandwidth=3000000&resolution=1280x720",
        core.base_url
    );

    // 新規セグメント -> 201 Created。
    let created = client
        .put(&url)
        .body(sample_bytes("101"))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status().as_u16(), 201);

    // 同一のバイト列 -> 200 OK（冪等）。
    let idempotent = client
        .put(&url)
        .body(sample_bytes("101"))
        .send()
        .await
        .unwrap();
    assert_eq!(idempotent.status().as_u16(), 200);

    // 同一識別子に対する異なるバイト列 -> 409 Conflict。
    let conflict = client
        .put(&url)
        .body(sample_bytes("different"))
        .send()
        .await
        .unwrap();
    assert_eq!(conflict.status().as_u16(), 409);

    // 無効なファイル名（拡張子なし） -> 400 Bad Request。
    let bad = client
        .put(format!("{}/segments/live/720p/badname", core.base_url))
        .body(sample_bytes("x"))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status().as_u16(), 400);
}

#[tokio::test]
async fn node_registration_status_codes() {
    let core = spawn_core(default_window(), default_eval()).await;
    let client = reqwest::Client::new();
    let url = format!("{}/nodes/cache-a", core.base_url);
    let body = serde_json::json!({ "base_url": "http://127.0.0.1:9001", "capacity_hint": 100 });

    let created = client.put(&url).json(&body).send().await.unwrap();
    assert_eq!(created.status().as_u16(), 201);

    let updated = client.put(&url).json(&body).send().await.unwrap();
    assert_eq!(updated.status().as_u16(), 200);
}

#[tokio::test]
async fn heartbeat_returns_state_and_prewarm_work() {
    let core = spawn_core(default_window(), default_eval()).await;
    let client = reqwest::Client::new();

    client
        .put(format!("{}/nodes/cache-a", core.base_url))
        .json(&serde_json::json!({ "base_url": "http://127.0.0.1:9001" }))
        .send()
        .await
        .unwrap();
    client
        .put(format!("{}/segments/live/720p/101.ts", core.base_url))
        .body(sample_bytes("101"))
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!("{}/nodes/cache-a/heartbeat", core.base_url))
        .json(&serde_json::json!({
            "health": "ok",
            "load_score": 0.1,
            "retained_segments": []
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["state"], "COLD");
    let prewarm = body["prewarm"].as_array().unwrap();
    assert_eq!(prewarm.len(), 1);
    assert_eq!(prewarm[0]["channel_id"], "live");
    assert_eq!(prewarm[0]["rendition"], "720p");
    assert_eq!(prewarm[0]["filename"], "101.ts");
}

#[tokio::test]
async fn playlists_unavailable_without_active_node() {
    let core = spawn_core(default_window(), default_eval()).await;
    let client = reqwest::Client::new();

    let master = client
        .get(format!("{}/channels/live/master.m3u8", core.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(master.status().as_u16(), 503);

    let media = client
        .get(format!("{}/channels/live/720p.m3u8", core.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(media.status().as_u16(), 503);
}

#[tokio::test]
async fn node_prewarm_then_serve_and_miss() {
    let core = spawn_core(default_window(), default_eval()).await;
    let node = spawn_node("cache-a", &core.base_url, 12).await;
    let client = reqwest::Client::new();

    // コアに正規セグメントを公開する。
    client
        .put(format!("{}/segments/live/720p/101.ts", core.base_url))
        .body(sample_bytes("101"))
        .send()
        .await
        .unwrap();

    // ノードにコアからプリウォームするよう指示 -> 202 Accepted。
    let origin_url = format!("{}/segments/live/720p/101.ts", core.base_url);
    let prewarm = client
        .put(format!("{}/prewarm/live/720p/101.ts", node.base_url))
        .json(&serde_json::json!({ "origin_url": origin_url }))
        .send()
        .await
        .unwrap();
    assert_eq!(prewarm.status().as_u16(), 202);

    // 保持されたセグメントが配信される -> 200、オリジナルのバイト列付き。
    let served = client
        .get(format!("{}/segments/live/720p/101.ts", node.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(served.status().as_u16(), 200);
    assert_eq!(served.bytes().await.unwrap().as_ref(), sample_bytes("101"));

    // ノードが保持していないセグメント -> 404 Not Found。
    let miss = client
        .get(format!("{}/segments/live/720p/999.ts", node.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(miss.status().as_u16(), 404);
}
