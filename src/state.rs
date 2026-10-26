use axum_extra::extract::cookie::Key;
use sqlx::MySqlPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    /// Clé de signature du cookie remember-me.
    pub key: Key,
    pub cookie_secure: bool,
}
