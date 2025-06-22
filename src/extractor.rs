use crate::{error::Error, models::OciIdentifier};
use axum::{
    extract::{FromRequestParts, MatchedPath, Path},
    http::request::Parts,
};
use std::str::FromStr;
use tracing::info;

pub struct OciId(pub OciIdentifier);

// First, let's implement the Axum extractor for OciIdentifier
impl<S> FromRequestParts<S> for OciId
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(oci_path): Path<String> = Path::from_request_parts(&mut parts.clone(), state)
            .await
            .map_err(|_| Error::InvalidPath)?;

        info!("Path in oci-extractor: {:?}", oci_path);

        // URL decode the path since OCI identifiers might contain special characters
        let decoded_path = urlencoding::decode(&oci_path)?;

        info!("Decoded Path: {0}", decoded_path);

        // Parse the OCI identifier
        let id = OciIdentifier::from_str(&decoded_path)?;

        info!("Oci Parameter: {:?}", id);
        Ok(OciId(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::{Path, Request},
        http::{Method, StatusCode},
    };
    use tower::util::ServiceExt;

    use axum::{response::Json, routing::get, Router};
    use serde_json::json;

    // Handler function that uses the OCI extractor
    async fn get_package(oci: OciId) -> Result<Json<serde_json::Value>, StatusCode> {
        // Your business logic here
        println!("Requested package: {}", oci.0.to_string());
        println!("Registry: {:?}", oci.0.registry);
        println!("Namespace: {}", oci.0.namespace);
        println!("Repository: {}", oci.0.repository);
        println!("Tag: {:?}", oci.0.tag);

        Ok(Json(json!({
            "oci_identifier": oci.0.to_string(),
            "registry": oci.0.registry.as_ref().map(|r| r.to_string()),
            "namespace": oci.0.namespace,
            "repository": oci.0.repository,
            "tag": oci.0.tag.to_string(),
            "has_registry": oci.0.has_registry(),
            "full_path": oci.0.full_repository_path()
        })))
    }

    // Alternative approach: Using Path extractor with catch-all
    async fn get_package_with_path(
        Path(oci_path): Path<String>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        // URL decode the path
        let decoded_path = urlencoding::decode(&oci_path)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid URL encoding".to_string()))?;

        let oci = OciIdentifier::from_str(&decoded_path).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid OCI identifier: {}", e),
            )
        })?;

        Ok(Json(json!({
            "oci_identifier": oci.to_string(),
            "namespace": oci.namespace,
            "repository": oci.repository,
            "tag": oci.tag.to_string()
        })))
    }

    // Router setup
    pub fn create_router() -> Router {
        Router::new()
            // Using custom extractor - matches /api/v0/anything
            .route("/api/v0/{*path}", get(get_package))
            // Alternative using Path extractor
            .route("/api/v1/{*oci_path}", get(get_package_with_path))
    }
    #[tokio::test]
    async fn test_oci_extractor_simple() {
        let app = create_router();

        // Ö:153Test simple OCI identifier
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v0/nginx/nginx:latest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_oci_extractor_with_registry() {
        let app = create_router();

        // Test with URL encoding for registry URLs
        let encoded_oci = urlencoding::encode("gcr.io/google-containers/pause:3.9");
        let uri = format!("/api/v0/{}", encoded_oci);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_path_extractor_alternative() {
        let app = create_router();

        // Test the alternative path-based approach
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/nginx/nginx:latest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
