//! HTTP extractors for trusted gateway headers and request context.

use axum::Json;
use axum::extract::FromRequestParts;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use http::request::Parts;
use method_library_contracts::{ActorContext, ErrorResponse, GatewayHeaders, RequestMeta};
use method_library_domain::content::{ActorKind, IdempotencyKey, RequestHash, Timestamp};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

const REQUEST_ID_HEADER: &str = "x-request-id";
const TRACE_ID_HEADER: &str = "x-trace-id";
const IDEMPOTENCY_KEY_HEADER: &str = "x-idempotency-key";
const ACTOR_ID_HEADER: &str = "x-actor-id";
const ACTOR_KIND_HEADER: &str = "x-actor-kind";
const TRUSTED_BY_HEADER: &str = "x-gateway-trusted-by";

/// Trusted gateway context extracted from inbound HTTP headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayContextExtractor {
    /// Trusted gateway headers.
    pub headers: GatewayHeaders,
    /// Request-scoped actor context.
    pub actor: ActorContext,
}

impl GatewayContextExtractor {
    /// Builds request metadata from the extracted gateway headers.
    pub fn request_meta(
        &self,
        request_hash: RequestHash,
        received_at: Timestamp,
    ) -> Result<RequestMeta, MethodLibraryError> {
        RequestMeta::from_gateway_headers(&self.headers, request_hash, received_at)
    }
}

/// Rejection returned when trusted gateway headers cannot be extracted.
#[derive(Debug, Clone)]
pub struct GatewayContextRejection {
    error: MethodLibraryError,
}

impl GatewayContextRejection {
    fn new(error: MethodLibraryError) -> Self {
        Self { error }
    }
}

impl IntoResponse for GatewayContextRejection {
    fn into_response(self) -> Response {
        let request_id = String::new();
        let trace_id = String::new();
        let response = ErrorResponse::from_domain_error(request_id, trace_id, self.error);
        (StatusCode::BAD_REQUEST, Json(response)).into_response()
    }
}

impl<S> FromRequestParts<S> for GatewayContextExtractor
where
    S: Send + Sync,
{
    type Rejection = GatewayContextRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let headers =
            parse_gateway_headers(&parts.headers).map_err(GatewayContextRejection::new)?;
        let actor =
            ActorContext::from_gateway_headers(&headers).map_err(GatewayContextRejection::new)?;

        Ok(Self { headers, actor })
    }
}

fn parse_gateway_headers(headers: &http::HeaderMap) -> Result<GatewayHeaders, MethodLibraryError> {
    let request_id = required_header(headers, REQUEST_ID_HEADER)?;
    let trace_id = required_header(headers, TRACE_ID_HEADER)?;
    let actor_id = required_header(headers, ACTOR_ID_HEADER)?;
    let actor_kind = parse_actor_kind(required_header(headers, ACTOR_KIND_HEADER)?.as_str())?;
    let trusted_by = required_header(headers, TRUSTED_BY_HEADER)?;
    let idempotency_key = optional_header(headers, IDEMPOTENCY_KEY_HEADER)?;

    Ok(GatewayHeaders {
        request_id,
        trace_id,
        idempotency_key,
        actor_id,
        actor_kind,
        trusted_by,
    })
}

fn required_header(headers: &http::HeaderMap, name: &str) -> Result<String, MethodLibraryError> {
    let value = headers
        .get(name)
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::GatewayContextMissing,
                format!("missing {name} header"),
            )
        })?
        .to_str()
        .map_err(|_| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::GatewayContextInvalid,
                format!("{name} header must be valid UTF-8"),
            )
        })?
        .trim()
        .to_string();

    if value.is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::GatewayContextInvalid,
            format!("{name} header must not be empty"),
        ));
    }

    Ok(value)
}

fn optional_header(
    headers: &http::HeaderMap,
    name: &str,
) -> Result<Option<IdempotencyKey>, MethodLibraryError> {
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };

    let value = value
        .to_str()
        .map_err(|_| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::GatewayContextInvalid,
                format!("{name} header must be valid UTF-8"),
            )
        })?
        .trim()
        .to_string();

    if value.is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::GatewayContextInvalid,
            format!("{name} header must not be empty"),
        ));
    }

    Ok(Some(value))
}

fn parse_actor_kind(value: &str) -> Result<ActorKind, MethodLibraryError> {
    match value {
        "human" => Ok(ActorKind::Human),
        "ai_member" => Ok(ActorKind::AiMember),
        "system" => Ok(ActorKind::System),
        other => Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::GatewayContextInvalid,
            format!("unsupported actor kind: {other}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::extract::FromRequestParts;
    use axum::response::IntoResponse;
    use http::Request;
    use http::StatusCode;

    use super::GatewayContextExtractor;
    use method_library_domain::content::Timestamp;

    #[tokio::test]
    async fn extracts_gateway_context_from_headers() {
        let request = Request::builder()
            .uri("/healthz")
            .header("x-request-id", "req-1")
            .header("x-trace-id", "trace-1")
            .header("x-idempotency-key", "idem-1")
            .header("x-actor-id", "actor-1")
            .header("x-actor-kind", "human")
            .header("x-gateway-trusted-by", "gateway")
            .body(Body::empty())
            .expect("request should build");

        let (mut parts, _) = request.into_parts();
        let extractor = GatewayContextExtractor::from_request_parts(&mut parts, &())
            .await
            .expect("gateway headers should extract");

        assert_eq!(extractor.actor.actor_id, "actor-1");
        assert_eq!(extractor.headers.idempotency_key.as_deref(), Some("idem-1"));

        let meta = extractor
            .request_meta(
                "hash-1".to_string(),
                Timestamp::from_unix_timestamp(1_747_247_200).expect("timestamp should build"),
            )
            .expect("request meta should build");

        assert_eq!(meta.request_id, "req-1");
    }

    #[tokio::test]
    async fn rejects_missing_gateway_headers() {
        let request = Request::builder()
            .uri("/healthz")
            .header("x-trace-id", "trace-1")
            .header("x-actor-id", "actor-1")
            .header("x-actor-kind", "human")
            .header("x-gateway-trusted-by", "gateway")
            .body(Body::empty())
            .expect("request should build");

        let (mut parts, _) = request.into_parts();
        let error = GatewayContextExtractor::from_request_parts(&mut parts, &())
            .await
            .expect_err("missing request id should fail");

        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
