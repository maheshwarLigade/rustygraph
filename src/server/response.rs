use serde::Serialize;
use tiny_http::StatusCode;

pub struct HttpResponse {
    pub status: StatusCode,
    pub content_type: &'static str,
    pub body: String,
}

impl HttpResponse {
    pub fn ok(body: &str) -> Self {
        Self {
            status: StatusCode(200),
            content_type: "application/json",
            body: body.to_string(),
        }
    }

    pub fn html(body: &str) -> Self {
        Self {
            status: StatusCode(200),
            content_type: "text/html; charset=utf-8",
            body: body.to_string(),
        }
    }

    pub fn json<T: Serialize>(val: &T) -> Self {
        match serde_json::to_string(val) {
            Ok(body) => Self {
                status: StatusCode(200),
                content_type: "application/json",
                body,
            },
            Err(e) => Self::error(StatusCode(500), &e.to_string()),
        }
    }

    pub fn error(status: StatusCode, msg: &str) -> Self {
        let err_json = serde_json::json!({
            "success": false,
            "error": msg
        });
        Self {
            status,
            content_type: "application/json",
            body: err_json.to_string(),
        }
    }
}
