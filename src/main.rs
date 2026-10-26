use std::{env, io};

use axum_extra::extract::cookie::Key;
use clap::{CommandFactory, Parser, Subcommand};
use rusted_recipes::{password, repository, routes, state::AppState};
use sqlx::mysql::MySqlPoolOptions;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version, about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Démarre le serveur HTTP
    Serve,
    /// Crée un utilisateur (mot de passe lu sur l'entrée standard)
    CreateUser {
        username: String,
        /// Donne le rôle ROLE_ADMIN à l'utilisateur
        #[arg(long)]
        admin: bool,
    },
    /// Liste les commandes disponibles
    List,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    if matches!(cli.command, Command::List) {
        Cli::command().print_help()?;
        return Ok(());
    }

    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rusted_recipes=info,tower_http=info".into()),
        )
        .init();

    let database_url = env::var("DATABASE_URL").map_err(|_| "DATABASE_URL est requis")?;
    match cli.command {
        Command::CreateUser { username, admin } => {
            create_user(&database_url, &username, admin).await
        }
        Command::Serve => serve(&database_url).await,
        Command::List => unreachable!(),
    }
}

async fn serve(database_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let secret = env::var("APP_SECRET").map_err(|_| "APP_SECRET est requis")?;
    if secret.len() < 32 {
        return Err("APP_SECRET doit faire au moins 32 caractères".into());
    }
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8000".into());

    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    if env::var_os("RUN_MIGRATIONS").is_some() {
        sqlx::migrate!().run(&pool).await?;
    }

    let state = AppState {
        pool,
        key: Key::derive_from(secret.as_bytes()),
        cookie_secure: env::var_os("COOKIE_SECURE").is_some(),
    };

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("écoute sur http://{bind_addr}");
    axum::serve(listener, routes::router(state)).await?;
    Ok(())
}

/// Le mot de passe est lu sur l'entrée standard.
async fn create_user(
    database_url: &str,
    username: &str,
    admin: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    let password = password.trim_end_matches(['\r', '\n']);
    if password.is_empty() {
        return Err("mot de passe vide (à fournir sur l'entrée standard)".into());
    }

    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    let roles = if admin {
        vec!["ROLE_ADMIN".to_string()]
    } else {
        Vec::new()
    };
    let id = repository::insert_user(&pool, username, &roles, &password::hash(password)?).await?;
    println!("utilisateur {username} créé (id {id})");
    Ok(())
}
