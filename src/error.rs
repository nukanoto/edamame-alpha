//! 共有エラー型とHTTPレスポンスへのマッピング。

use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// ドキュメント化されたHTTPステータスコードに直接マッピングされるエラー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdamameError {
    /// 無効な識別子、ファイル名、シーケンス、またはメタデータ (`400`)。
    BadRequest(String),
    /// 同じ識別子が異なるバイト列で存在するか、メタデータが競合している (`409`)。
    Conflict(String),
    /// リクエストされたリソースが不明 (`404`)。
    NotFound(String),
    /// 生成されたプレイリストが無効な状態を参照する (`422`)。
    Unprocessable(String),
    /// 新しいクライアントに利用可能なACTIVEキャッシュノードがない (`503`)。
    Unavailable(String),
    /// プリウォーム中に正規セグメントを取得できなかった (`502`)。
    BadGateway(String),
    /// 予期しない内部エラー (`500`)。
    Internal(String),
}

impl EdamameError {
    /// このエラーがマッピングされるHTTPステータスコード。
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            EdamameError::BadRequest(_) => StatusCode::BAD_REQUEST,
            EdamameError::Conflict(_) => StatusCode::CONFLICT,
            EdamameError::NotFound(_) => StatusCode::NOT_FOUND,
            EdamameError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            EdamameError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            EdamameError::BadGateway(_) => StatusCode::BAD_GATEWAY,
            EdamameError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// このエラーが持つ人間が読めるメッセージ。
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            EdamameError::BadRequest(m)
            | EdamameError::Conflict(m)
            | EdamameError::NotFound(m)
            | EdamameError::Unprocessable(m)
            | EdamameError::Unavailable(m)
            | EdamameError::BadGateway(m)
            | EdamameError::Internal(m) => m,
        }
    }
}

impl fmt::Display for EdamameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message(), self.status().as_u16())
    }
}

impl std::error::Error for EdamameError {}

impl IntoResponse for EdamameError {
    fn into_response(self) -> Response {
        (self.status(), self.message().to_owned()).into_response()
    }
}
