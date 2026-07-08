//! `edamame-core` と `edamame-node` の引数パースと検証の契約テスト
//! （タスク T047）。

use clap::Parser;

use edamame_alpha::config::{CoreConfig, NodeConfig};

#[test]
fn core_parses_documented_arguments() {
    let config = CoreConfig::try_parse_from([
        "edamame-core",
        "--bind",
        "127.0.0.1:9000",
        "--storage-dir",
        ".edamame/core",
        "--live-window",
        "6",
        "--retention-window",
        "12",
        "--heartbeat-timeout-ms",
        "10000",
        "--overload-threshold",
        "0.80",
    ])
    .unwrap();

    assert_eq!(config.live_window, 6);
    assert_eq!(config.retention_window, 12);
    config.validate().unwrap();
}

#[test]
fn core_refuses_retention_smaller_than_live_window() {
    let config = CoreConfig::try_parse_from([
        "edamame-core",
        "--bind",
        "127.0.0.1:9000",
        "--storage-dir",
        ".edamame/core",
        "--live-window",
        "12",
        "--retention-window",
        "6",
        "--heartbeat-timeout-ms",
        "10000",
        "--overload-threshold",
        "0.80",
    ])
    .unwrap();

    assert!(config.validate().is_err());
}

#[test]
fn core_rejects_out_of_range_overload_threshold() {
    let config = CoreConfig::try_parse_from([
        "edamame-core",
        "--bind",
        "127.0.0.1:9000",
        "--storage-dir",
        ".edamame/core",
        "--live-window",
        "6",
        "--retention-window",
        "12",
        "--heartbeat-timeout-ms",
        "10000",
        "--overload-threshold",
        "1.5",
    ])
    .unwrap();

    assert!(config.validate().is_err());
}

#[test]
fn node_parses_documented_arguments() {
    let config = NodeConfig::try_parse_from([
        "edamame-node",
        "--node-id",
        "cache-a",
        "--bind",
        "127.0.0.1:9001",
        "--core-url",
        "http://127.0.0.1:9000",
        "--cache-dir",
        ".edamame/cache-a",
        "--heartbeat-interval-ms",
        "1000",
    ])
    .unwrap();

    assert_eq!(config.node_id, "cache-a");
    assert_eq!(config.core_url, "http://127.0.0.1:9000");
}

#[test]
fn node_requires_core_url() {
    let result = NodeConfig::try_parse_from([
        "edamame-node",
        "--node-id",
        "cache-a",
        "--bind",
        "127.0.0.1:9001",
        "--cache-dir",
        ".edamame/cache-a",
        "--heartbeat-interval-ms",
        "1000",
    ]);
    assert!(result.is_err());
}
