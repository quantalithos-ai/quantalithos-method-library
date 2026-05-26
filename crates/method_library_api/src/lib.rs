//! HTTP bootstrap surface for the method-library service.

use axum::{Json, Router, routing::get};
use method_library_contracts::PlaceholderContract;

async fn healthcheck() -> Json<PlaceholderContract> {
    Json(PlaceholderContract::default())
}

/// Builds the minimal router used during the bootstrap phase.
#[must_use]
pub fn router() -> Router {
    Router::new().route("/healthz", get(healthcheck))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::router;

    #[tokio::test]
    async fn responds_to_healthcheck() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
