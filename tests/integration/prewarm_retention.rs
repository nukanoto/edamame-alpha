//! ノードのプリウォームプル、保持配信、エビクション、ミスレスポンスの統合テスト
//! （タスク T045）。

#[path = "../support/mod.rs"]
mod support;

use support::{default_eval, default_window, sample_bytes, spawn_core, spawn_node};

#[tokio::test]
async fn node_prewarms_serves_and_evicts_past_retention() {
    let core = spawn_core(default_window(), default_eval()).await;
    // 保持ウィンドウ 2 は短いシーケンス内でエビクションを観測可能にする。
    let node = spawn_node("cache-a", &core.base_url, 2).await;
    let client = reqwest::Client::new();

    // コアに4つの正規セグメントを公開する。
    for seq in 1..=4u64 {
        client
            .put(format!("{}/segments/live/720p/{seq}.ts", core.base_url))
            .body(sample_bytes(&format!("720p-{seq}")))
            .send()
            .await
            .unwrap();
    }

    // コアからプルしてすべて4つをノードにプリウォームする。
    for seq in 1..=4u64 {
        let origin_url = format!("{}/segments/live/720p/{seq}.ts", core.base_url);
        let resp = client
            .put(format!("{}/prewarm/live/720p/{seq}.ts", node.base_url))
            .json(&serde_json::json!({ "origin_url": origin_url }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 202);
    }

    // live_edge = 4, retention = 2 => シーケンス 1 がエビクションされる。
    let evicted = client
        .get(format!("{}/segments/live/720p/1.ts", node.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(evicted.status().as_u16(), 404);

    // ライブエッジは保持され、オリジナルのバイト列で配信される。
    let retained = client
        .get(format!("{}/segments/live/720p/4.ts", node.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(retained.status().as_u16(), 200);
    assert_eq!(
        retained.bytes().await.unwrap().as_ref(),
        sample_bytes("720p-4")
    );

    // プリウォームされていないセグメントはミスとなる。
    let miss = client
        .get(format!("{}/segments/live/720p/9.ts", node.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(miss.status().as_u16(), 404);
}
