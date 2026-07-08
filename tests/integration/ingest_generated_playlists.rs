//! マルチレンディションセグメントインジェストと生成プレイリスト取得の統合テスト
//! （タスク T019）。

#[path = "../support/mod.rs"]
mod support;

use support::{default_eval, default_window, sample_bytes, spawn_core};

async fn publish(client: &reqwest::Client, base: &str, rendition: &str, seq: u64, bw: u64) {
    let url =
        format!("{base}/segments/live/{rendition}/{seq}.ts?bandwidth={bw}&resolution=1280x720");
    let resp = client
        .put(url)
        .body(sample_bytes(&format!("{rendition}-{seq}")))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn publishes_and_serves_generated_master_and_media_playlists() {
    let core = spawn_core(default_window(), default_eval()).await;
    let client = reqwest::Client::new();

    publish(&client, &core.base_url, "720p", 101, 3_000_000).await;
    publish(&client, &core.base_url, "720p", 102, 3_000_000).await;
    publish(&client, &core.base_url, "1080p", 101, 6_000_000).await;
    publish(&client, &core.base_url, "1080p", 102, 6_000_000).await;

    // ACTIVE ノードがない場合、マスタープレイリストは利用不可（オリジンURLが漏出しない）。
    let unavailable = client
        .get(format!("{}/channels/live/master.m3u8", core.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(unavailable.status().as_u16(), 503);

    // ノードを登録し、フルライブウィンドウを保持していると報告して ACTIVE にする。
    let node_base = "http://127.0.0.1:18080";
    client
        .put(format!("{}/nodes/cache-a", core.base_url))
        .json(&serde_json::json!({ "base_url": node_base }))
        .send()
        .await
        .unwrap();
    let resp = client
        .post(format!("{}/nodes/cache-a/heartbeat", core.base_url))
        .json(&serde_json::json!({
            "health": "ok",
            "load_score": 0.1,
            "retained_segments": [
                { "channel_id": "live", "rendition": "720p", "sequence": 101 },
                { "channel_id": "live", "rendition": "720p", "sequence": 102 },
                { "channel_id": "live", "rendition": "1080p", "sequence": 101 },
                { "channel_id": "live", "rendition": "1080p", "sequence": 102 }
            ]
        }))
        .send()
        .await
        .unwrap();
    let state: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(state["state"], "ACTIVE");

    // マスタープレイリストは両方のレンディションメディアプレイリストを参照する。
    let master = client
        .get(format!("{}/channels/live/master.m3u8", core.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(master.status().as_u16(), 200);
    let master_body = master.text().await.unwrap();
    assert!(master_body.contains("720p.m3u8"));
    assert!(master_body.contains("1080p.m3u8"));

    // メディアプレイリストはセグメントURLを単一の ACTIVE ノードに書き換える。
    let media = client
        .get(format!("{}/channels/live/720p.m3u8", core.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(media.status().as_u16(), 200);
    let media_body = media.text().await.unwrap();
    assert!(media_body.contains(&format!("{node_base}/segments/live/720p/101.ts")));
    assert!(media_body.contains(&format!("{node_base}/segments/live/720p/102.ts")));
}
