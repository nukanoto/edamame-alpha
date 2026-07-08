//! Edamame MVP コアライブラリ。
//!
//! このクレートは、オリジン・コントローラー・キャッシュノードの責務を
//! 別々のモジュールに分けつつ、`edamame-core` と `edamame-node` という
//! バイナリに組み立てられるようにしています。ドメインロジックは具体構造体上の
//! 直接関数として表現され、ルーティング・配置・状態遷移・ライブウィンドウの
//! アルゴリズムを容易に検査・テストできるようにしています。

pub mod config;
pub mod core;
pub mod domain;
pub mod error;
pub mod node;

use std::time::{SystemTime, UNIX_EPOCH};

/// Unixエポックからの現在の壁時計時間（ミリ秒）。
///
/// 状態評価は暗黙的に時計を読み取ることはありません。呼び出し側はここで
/// 返された値を明示的な入力として渡すことで、同一の `(レジストリ状態, 現在時刻)`
/// に対して単一の評価パスが決定的になるようにしています（仕様 FR-042 参照）。
#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
