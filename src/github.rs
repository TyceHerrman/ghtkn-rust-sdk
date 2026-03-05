//! GitHub API client for authenticated requests.
//!
//! Provides a thin wrapper around the GitHub REST API for operations needed by
//! the token management flow (currently just fetching the authenticated user).

use serde::Deserialize;

/// A GitHub user returned by the `/user` endpoint.
#[derive(Debug, Deserialize)]
pub struct User {
    pub login: String,
}

/// GitHub API client for authenticated requests.
///
/// Configured with a bearer token and appropriate headers for the GitHub API.
pub struct GitHubClient {
    http_client: reqwest::Client,
    /// Base URL for the GitHub API. Defaults to `https://api.github.com`.
    /// Override for testing with a mock server.
    api_base_url: String,
}

impl GitHubClient {
    /// Create a new GitHub API client authenticated with the given token.
    ///
    /// The client is configured with:
    /// - `Authorization: Bearer <token>` header
    /// - `Accept: application/vnd.github+json` header
    /// - `ghtkn-rust-sdk` user agent
    pub fn new(token: &str) -> Self {
        Self::build(token, "https://api.github.com".to_string())
    }

    /// Create a new GitHub API client with a custom base URL (for testing).
    pub fn with_base_url(token: &str, api_base_url: String) -> Self {
        Self::build(token, api_base_url)
    }

    /// Build the client with the given token and base URL.
    fn build(token: &str, api_base_url: String) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}")
                .parse()
                .expect("valid authorization header value"),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/vnd.github+json"
                .parse()
                .expect("valid accept header value"),
        );
        let http_client = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent("ghtkn-rust-sdk")
            .build()
            .expect("failed to build reqwest client");
        Self {
            http_client,
            api_base_url,
        }
    }

    /// Fetch the authenticated user's profile (`GET /user`).
    pub async fn get_user(&self) -> crate::Result<User> {
        let url = format!("{}/user", self.api_base_url);
        self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| crate::Error::GitHub(format!("request user: {e}")))?
            .json::<User>()
            .await
            .map_err(|e| crate::Error::GitHub(format!("decode user response: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn test_get_user() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"login": "octocat"})),
            )
            .mount(&server)
            .await;

        let client = GitHubClient::with_base_url("test_token", server.uri());
        let user = client.get_user().await.unwrap();

        assert_eq!(user.login, "octocat");
    }

    #[tokio::test]
    async fn test_get_user_error() {
        let server = MockServer::start().await;

        // Return a 500 with an HTML body that cannot be deserialized as User.
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&server)
            .await;

        let client = GitHubClient::with_base_url("test_token", server.uri());
        let result = client.get_user().await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, crate::Error::GitHub(_)),
            "expected Error::GitHub, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_get_user_with_extra_fields() {
        let server = MockServer::start().await;

        // GitHub returns many fields; we only care about `login`.
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "login": "testuser",
                "id": 12345,
                "avatar_url": "https://avatars.githubusercontent.com/u/12345",
                "name": "Test User"
            })))
            .mount(&server)
            .await;

        let client = GitHubClient::with_base_url("test_token", server.uri());
        let user = client.get_user().await.unwrap();

        assert_eq!(user.login, "testuser");
    }
}
