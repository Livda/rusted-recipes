use axum::{
    Form,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    auth::{self, MaybeUser},
    error::AppResult,
    password, repository,
    state::AppState,
    views::{LoginTemplate, PageContext, render},
};

pub async fn login_form(session: Session, MaybeUser(user): MaybeUser) -> AppResult<Response> {
    if user.is_some() {
        return Ok(Redirect::to(super::RECIPES).into_response());
    }
    let page = LoginTemplate {
        ctx: PageContext::build(&session, None, "login").await?,
        last_username: String::new(),
        error: None,
    };
    Ok(render(page)?.into_response())
}

#[derive(Deserialize)]
pub struct LoginForm {
    #[serde(default)]
    _username: String,
    #[serde(default)]
    _password: String,
    #[serde(default)]
    _csrf_token: String,
    _remember_me: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> AppResult<Response> {
    auth::verify_csrf(&session, &form._csrf_token).await?;

    let user = repository::find_user_by_username(&state.pool, form._username.trim()).await?;
    // Même message pour un compte inconnu et un mauvais mot de passe.
    let user = user.filter(|u| password::verify(&form._password, &u.password));

    let Some(user) = user else {
        let page = LoginTemplate {
            ctx: PageContext::build(&session, None, "login").await?,
            last_username: form._username,
            error: Some("Identifiants invalides."),
        };
        let html: Html<String> = render(page)?;
        return Ok((StatusCode::UNAUTHORIZED, html).into_response());
    };

    auth::log_in(&session, &user).await?;

    let jar = SignedCookieJar::from_headers(&headers, state.key.clone());
    let jar = if form._remember_me.is_some() {
        jar.add(auth::remember_cookie(&user, state.cookie_secure))
    } else {
        jar.add(auth::forget_cookie(state.cookie_secure))
    };
    Ok((jar, Redirect::to(super::RECIPES)).into_response())
}

#[derive(Deserialize)]
pub struct CsrfForm {
    #[serde(default)]
    _csrf_token: String,
}

pub async fn logout(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<CsrfForm>,
) -> AppResult<Response> {
    auth::verify_csrf(&session, &form._csrf_token).await?;
    auth::log_out(&session).await?;
    let jar = SignedCookieJar::from_headers(&headers, state.key.clone())
        .add(auth::forget_cookie(state.cookie_secure));
    Ok((jar, Redirect::to(super::HOME)).into_response())
}
