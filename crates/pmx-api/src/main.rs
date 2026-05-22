use std::{env, net::SocketAddr};

use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind_addr = env::var("PMX_API_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse::<SocketAddr>()?;
    let storage = env::var("PMX_API_STORAGE").unwrap_or_else(|_| {
        if env::var("PMX_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .is_ok()
        {
            "postgres".to_owned()
        } else {
            "in_memory_scaffold".to_owned()
        }
    });
    let profile = env::var("PMX_API_PROFILE").unwrap_or_else(|_| "local".to_owned());

    let app = match storage.as_str() {
        "postgres" => {
            let database_url =
                env::var("PMX_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"))?;
            let apply_schema = env_flag("PMX_API_APPLY_SCHEMA");
            pmx_api::try_postgres_app(database_url, apply_schema).await?
        }
        "in_memory_scaffold" if profile != "production" => pmx_api::app(),
        "in_memory_scaffold" => {
            return Err(
                "PMX_API_STORAGE=in_memory_scaffold is forbidden for production profile".into(),
            );
        }
        other => {
            return Err(format!(
                "unsupported PMX_API_STORAGE={other}; expected postgres or in_memory_scaffold"
            )
            .into());
        }
    };

    let listener = TcpListener::bind(bind_addr).await?;
    eprintln!(
        "pmx-api listening addr={bind_addr} storage={storage} profile={profile} live_submit=false live_cancel=false"
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
