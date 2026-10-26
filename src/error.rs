use axum::{
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};

#[derive(Debug)]
pub enum AppError {
    NotFound,
    /// Jeton CSRF absent ou invalide.
    BadRequest(&'static str),
    /// Utilisateur non authentifié : renvoyé vers la page de connexion.
    LoginRequired,
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Internal(format!("base de données : {e}"))
    }
}

impl From<askama::Error> for AppError {
    fn from(e: askama::Error) -> Self {
        AppError::Internal(format!("template : {e}"))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Page introuvable").into_response(),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            AppError::LoginRequired => Redirect::to("/login").into_response(),
            AppError::Internal(msg) => {
                tracing::error!("{msg}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Erreur interne").into_response()
            }
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
