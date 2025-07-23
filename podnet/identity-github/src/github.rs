use anyhow::{anyhow, Result};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use pod2::backends::plonky2::primitives::ec::curve::Point as PublicKey;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitHubOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

pub struct GitHubOAuthClient {
    client: BasicClient,
    http_client: Client,
}

impl GitHubOAuthClient {
    pub fn new(config: GitHubOAuthConfig) -> Result<Self> {
        let client = BasicClient::new(
            ClientId::new(config.client_id),
            Some(ClientSecret::new(config.client_secret)),
            AuthUrl::new("https://github.com/login/oauth/authorize".to_string())?,
            Some(TokenUrl::new("https://github.com/login/oauth/access_token".to_string())?),
        )
        .set_redirect_uri(RedirectUrl::new(config.redirect_uri)?);

        let http_client = Client::new();

        Ok(Self { client, http_client })
    }

    pub fn get_authorization_url(&self, public_key: &PublicKey) -> Result<(Url, CsrfToken)> {
        // Use the public key as state to associate OAuth flow with user
        let public_key_json = serde_json::to_string(public_key)?;
        let csrf_token = CsrfToken::new(public_key_json);

        let (auth_url, _) = self
            .client
            .authorize_url(|| csrf_token.clone())
            .add_scope(Scope::new("user:email".to_string()))
            .url();

        Ok((auth_url, csrf_token))
    }

    pub async fn exchange_code(&self, code: AuthorizationCode) -> Result<String> {
        let token_result = self
            .client
            .exchange_code(code)
            .request_async(async_http_client)
            .await?;

        Ok(token_result.access_token().secret().clone())
    }

    pub async fn get_user_info(&self, access_token: &str) -> Result<GitHubUser> {
        let response = self
            .http_client
            .get("https://api.github.com/user")
            .bearer_auth(access_token)
            .header("User-Agent", "pod2-identity-github/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get GitHub user info: {}",
                response.status()
            ));
        }

        let user: GitHubUser = response.json().await?;
        Ok(user)
    }

    pub async fn get_ssh_keys(&self, username: &str) -> Result<Vec<String>> {
        let url = format!("https://github.com/{}.keys", username);
        
        let response = self
            .http_client
            .get(&url)
            .header("User-Agent", "pod2-identity-github/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get SSH keys for {}: {}",
                username,
                response.status()
            ));
        }

        let keys_text = response.text().await?;
        let keys: Vec<String> = keys_text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .collect();

        Ok(keys)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
    pub public_key: PublicKey,
}

pub fn parse_oauth_state(state: &str) -> Result<PublicKey> {
    let public_key: PublicKey = serde_json::from_str(state)?;
    Ok(public_key)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthUrlResponse {
    pub auth_url: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct IdentityRequest {
    pub access_token: String,
    pub github_user: GitHubUser,
    pub user_challenge_signature: String, // User signs a challenge containing GitHub info
    pub public_key: PublicKey,
}