use axum::{extract::Query, http::StatusCode, response::Html, routing::get, Router};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use ticks::{AccessToken, Authorization};
use tokio::sync::Mutex;

const REDIRECT_URI: &str = "http://localhost:8080/callback";

#[derive(Debug, Deserialize, Clone)]
pub struct AuthCallback {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub fn get_client_id() -> Option<(String, String)> {
    let client_id = std::env::var("TICKTICK_CLIENT_ID").ok()?;
    let client_secret = std::env::var("TICKTICK_CLIENT_SECRET").ok()?;

    if client_id.is_empty() || client_secret.is_empty() {
        return None;
    }

    Some((client_id, client_secret))
}

fn get_token_cache_path() -> PathBuf {
    let mut path = dirs::home_dir().expect("Could not determine home directory");
    path.push(".automatick");
    std::fs::create_dir_all(&path).ok();
    path.push("token.json");
    path
}

pub fn load_cached_token() -> Option<AccessToken> {
    let path = get_token_cache_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(token) = serde_json::from_str::<AccessToken>(&content) {
                return Some(token);
            }
        }
    }
    None
}

pub fn save_token_cache(token: &AccessToken) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_token_cache_path();
    let json = serde_json::to_string(token)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn clear_token_cache() {
    let path = get_token_cache_path();
    let _ = std::fs::remove_file(path);
}

pub async fn perform_authorization(
    client_id: String,
    client_secret: String,
) -> Option<AccessToken> {
    let redirect_uri = REDIRECT_URI.to_string();
    let auth_result = Authorization::begin_auth(client_id.clone(), redirect_uri.clone());
    let awaiting_auth = match auth_result {
        Ok(auth) => auth,
        Err(_e) => {
            return None;
        }
    };
    let auth_code = Arc::new(Mutex::new(None::<String>));
    let auth_state = Arc::new(Mutex::new(None::<String>));

    let auth_code_clone = auth_code.clone();
    let auth_state_clone = auth_state.clone();

    let callback_handler = move |Query(callback): Query<AuthCallback>| {
        let code_storage = auth_code_clone.clone();
        let state_storage = auth_state_clone.clone();
        async move {
            if let Some(error) = callback.error {
                return (
                    StatusCode::BAD_REQUEST,
                    Html(format!(
                        "<html><body><h1>✗ Authorization Error</h1><p>{}</p></body></html>",
                        error
                    )),
                );
            }
            if let Some(code) = callback.code {
                *code_storage.lock().await = Some(code);
                if let Some(state) = callback.state {
                    *state_storage.lock().await = Some(state);
                }
                (
                    StatusCode::OK,
                    Html("<html><body><h1>✓ Authorization Successful!</h1><p>You can now return to your terminal.</p></body></html>".to_string()),
                )
            } else {
                (
                    StatusCode::BAD_REQUEST,
                    Html("<html><body><h1>✗ No Authorization Code</h1><p>No code received in callback.</p></body></html>".to_string()),
                )
            }
        }
    };
    let app = Router::new().route("/callback", get(callback_handler));
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:8080").await {
        Ok(l) => l,
        Err(_e) => {
            return None;
        }
    };
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let auth_url = awaiting_auth.get_url().to_string();
    let _ = open::that(&auth_url);
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(300); // 5 minute timeout
    let received_code = loop {
        if let Some(code) = auth_code.lock().await.take() {
            break code;
        }
        if start.elapsed() > timeout {
            return None;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    };
    let received_state = auth_state.lock().await.take().unwrap_or_default();
    match awaiting_auth
        .finish_auth(client_secret, received_code, received_state)
        .await
    {
        Ok(token) => Some(token),
        Err(_e) => None,
    }
}

pub async fn get_access_token(client_id: String, client_secret: String) -> Option<AccessToken> {
    if let Some(token) = load_cached_token() {
        return Some(token);
    }
    match perform_authorization(client_id, client_secret).await {
        Some(token) => {
            let _ = save_token_cache(&token);
            Some(token)
        }
        None => None,
    }
}
