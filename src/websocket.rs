use axum::{
    async_trait,
    body::Bytes,
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::Response,
};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use std::future::Future;

pub type WebSocket = web_socket::WebSocket<TokioIo<Upgraded>>;
pub use web_socket;

pub struct WebSocketUpgrade {
    sec_websocket_key: HeaderValue,
    on_upgrade: hyper::upgrade::OnUpgrade,
}

impl WebSocketUpgrade {
    pub fn on_upgrade<C, Fut>(self, callback: C) -> Response
    where
        C: FnOnce(WebSocket) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        tokio::spawn(async move {
            if let Ok(upgraded) = self.on_upgrade.await {
                callback(WebSocket::server(TokioIo::new(upgraded))).await;
            }
        });
        Response::builder()
            .status(StatusCode::SWITCHING_PROTOCOLS)
            .header(header::CONNECTION, HeaderValue::from_static("upgrade"))
            .header(header::UPGRADE, HeaderValue::from_static("websocket"))
            .header(
                header::SEC_WEBSOCKET_ACCEPT,
                sign(self.sec_websocket_key.as_bytes()),
            )
            .body(axum::body::Body::empty())
            .unwrap()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for WebSocketUpgrade
where
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if parts.method != Method::GET {
            return Err(());
        }
        if !header_contains(&parts.headers, header::CONNECTION, "upgrade") {
            return Err(());
        }
        if !header_eq(&parts.headers, header::UPGRADE, "websocket") {
            return Err(());
        }
        if !header_eq(&parts.headers, header::SEC_WEBSOCKET_VERSION, "13") {
            return Err(());
        }
        Ok(Self {
            sec_websocket_key: parts
                .headers
                .get(header::SEC_WEBSOCKET_KEY)
                .ok_or(())?
                .clone(),
            on_upgrade: parts
                .extensions
                .remove::<hyper::upgrade::OnUpgrade>()
                .ok_or(())?,
        })
    }
}

fn sign(key: &[u8]) -> HeaderValue {
    use base64::engine::Engine as _;
    use sha1::{Digest, Sha1};

    let mut sha1 = Sha1::default();
    sha1.update(key);
    sha1.update(&b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"[..]);
    let b64 = Bytes::from(base64::engine::general_purpose::STANDARD.encode(sha1.finalize()));
    HeaderValue::from_maybe_shared(b64).expect("base64 is a valid value")
}

fn header_eq(headers: &HeaderMap, key: HeaderName, value: &'static str) -> bool {
    if let Some(header) = headers.get(&key) {
        header.as_bytes().eq_ignore_ascii_case(value.as_bytes())
    } else {
        false
    }
}

fn header_contains(headers: &HeaderMap, key: HeaderName, value: &'static str) -> bool {
    let header = if let Some(header) = headers.get(&key) {
        header
    } else {
        return false;
    };
    if let Ok(header) = std::str::from_utf8(header.as_bytes()) {
        header.to_ascii_lowercase().contains(value)
    } else {
        false
    }
}