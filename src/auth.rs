use std::time::{SystemTime, UNIX_EPOCH};

use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::{
    error::{AppError, AppResult},
    models::User,
    repository,
    state::AppState,
};

const USER_KEY: &str = "user_id";
const CSRF_KEY: &str = "csrf";
const FLASH_KEY: &str = "flashes";
pub const REMEMBER_COOKIE: &str = "RECIPE-REMEMBER-ME";
const REMEMBER_LIFETIME_SECS: u64 = 604_800; // 1 semaine

fn internal(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("session : {e}"))
}

// ---------------------------------------------------------------- CSRF

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Jeton CSRF de la session, créé à la demande.
pub async fn csrf_token(session: &Session) -> AppResult<String> {
    if let Some(token) = session.get::<String>(CSRF_KEY).await.map_err(internal)? {
        return Ok(token);
    }
    let token = random_token();
    session.insert(CSRF_KEY, &token).await.map_err(internal)?;
    Ok(token)
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub async fn verify_csrf(session: &Session, submitted: &str) -> AppResult<()> {
    let expected = session.get::<String>(CSRF_KEY).await.map_err(internal)?;
    match expected {
        Some(expected) if constant_time_eq(&expected, submitted) => Ok(()),
        _ => Err(AppError::BadRequest("Jeton CSRF invalide")),
    }
}

// --------------------------------------------------------------- flashes

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flash {
    /// `success` ou `danger`, utilisé comme `data-type` en CSS.
    pub kind: String,
    pub message: String,
}

pub async fn add_flash(session: &Session, kind: &str, message: String) -> AppResult<()> {
    let mut flashes: Vec<Flash> = session
        .get(FLASH_KEY)
        .await
        .map_err(internal)?
        .unwrap_or_default();
    flashes.push(Flash {
        kind: kind.to_owned(),
        message,
    });
    session.insert(FLASH_KEY, flashes).await.map_err(internal)
}

pub async fn take_flashes(session: &Session) -> AppResult<Vec<Flash>> {
    Ok(session
        .remove::<Vec<Flash>>(FLASH_KEY)
        .await
        .map_err(internal)?
        .unwrap_or_default())
}

// ------------------------------------------------------------- connexion

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ouvre la session : l'identifiant est renouvelé (anti fixation de session).
pub async fn log_in(session: &Session, user: &User) -> AppResult<()> {
    session.cycle_id().await.map_err(internal)?;
    session.insert(USER_KEY, user.id).await.map_err(internal)
}

pub async fn log_out(session: &Session) -> AppResult<()> {
    session.flush().await.map_err(internal)
}

pub fn remember_cookie(user: &User, secure: bool) -> Cookie<'static> {
    base_cookie(
        format!("{}:{}", user.id, now() + REMEMBER_LIFETIME_SECS),
        secure,
    )
    .max_age(time::Duration::seconds(REMEMBER_LIFETIME_SECS as i64))
    .build()
}

pub fn forget_cookie(secure: bool) -> Cookie<'static> {
    base_cookie(String::new(), secure)
        .max_age(time::Duration::ZERO)
        .build()
}

fn base_cookie(value: String, secure: bool) -> cookie::CookieBuilder<'static> {
    Cookie::build((REMEMBER_COOKIE, value))
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
}

/// `id:expiration` → id si le cookie n'est pas expiré.
fn parse_remember(value: &str, now: u64) -> Option<i32> {
    let (id, expires) = value.split_once(':')?;
    (expires.parse::<u64>().ok()? > now).then(|| id.parse().ok())?
}

// ------------------------------------------------------------ extracteurs

/// Utilisateur courant, s'il y en a un (session, sinon cookie remember-me).
pub struct MaybeUser(pub Option<User>);

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|(_, msg)| AppError::Internal(msg.to_owned()))?;

        if let Some(id) = session.get::<i32>(USER_KEY).await.map_err(internal)? {
            if let Some(user) = repository::find_user(&state.pool, id).await? {
                return Ok(MaybeUser(Some(user)));
            }
            // Utilisateur supprimé depuis : la session n'est plus valable.
            session.flush().await.map_err(internal)?;
        }

        let jar = SignedCookieJar::from_headers(&parts.headers, state.key.clone());
        let remembered = jar
            .get(REMEMBER_COOKIE)
            .and_then(|c| parse_remember(c.value(), now()));
        if let Some(id) = remembered
            && let Some(user) = repository::find_user(&state.pool, id).await?
        {
            log_in(&session, &user).await?;
            return Ok(MaybeUser(Some(user)));
        }

        Ok(MaybeUser(None))
    }
}

/// Garde `ROLE_USER` : redirige vers `/login` sinon.
pub struct RequireUser(pub User);

impl FromRequestParts<AppState> for RequireUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        match MaybeUser::from_request_parts(parts, state).await?.0 {
            Some(user) if user.has_role("ROLE_USER") => Ok(RequireUser(user)),
            _ => Err(AppError::LoginRequired),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_value_parsing() {
        assert_eq!(parse_remember("7:200", 100), Some(7));
        assert_eq!(parse_remember("7:100", 100), None);
        assert_eq!(parse_remember("x:200", 100), None);
        assert_eq!(parse_remember("7", 100), None);
        assert_eq!(parse_remember("", 100), None);
    }

    #[test]
    fn csrf_comparison() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "ab"));
    }

    #[test]
    fn tokens_are_random_hex() {
        let (a, b) = (random_token(), random_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}
