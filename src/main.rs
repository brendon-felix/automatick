mod action;
mod app;
mod auth;
mod modal;
mod tasks;
mod tui;
mod ui;
mod utils;

use ticks::{AccessToken, TickTick};

#[tokio::main]
async fn main() {
    if let Some((client_id, client_secret)) = auth::get_client_id() {
        if let Some(access_token) = auth::get_access_token(client_id, client_secret).await {
            let _ = run(access_token).await;
        }
    }
}

async fn run(access_token: AccessToken) -> anyhow::Result<()> {
    let client = create_client(access_token)?;
    let mut app = app::App::new(client)?;
    app.run().await?;
    Ok(())
}

fn create_client(access_token: AccessToken) -> anyhow::Result<TickTick> {
    match TickTick::new(access_token) {
        Ok(c) => Ok(c),
        Err(e) => {
            auth::clear_token_cache();
            Err(anyhow::anyhow!("Failed to create TickTick client: {:?}", e))
        }
    }
}
