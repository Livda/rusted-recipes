//! Construction des URLs et assemblage du routeur.

use axum::{
    Router,
    routing::{get, post},
};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer, cookie::time::Duration};

use crate::{
    models::{Diet, Season},
    state::AppState,
};

mod auth;
mod home;
mod recipes;

pub const HOME: &str = "/";
pub const LOGIN: &str = "/login";
pub const LOGOUT: &str = "/logout";
pub const RECIPES: &str = "/recipes/";
pub const RECIPE_NEW: &str = "/recipes/new";
pub const RECIPE_SAVE: &str = "/recipes/save";

pub fn recipe_edit(id: i32) -> String {
    format!("/recipes/{id}/edit")
}

pub fn recipe_delete(id: i32) -> String {
    format!("/recipes/{id}/delete")
}

pub fn recipes_index(diet: Diet, season: Season, search: Option<&str>, page: u32) -> String {
    let mut query: Vec<(&str, String)> = vec![
        ("diet", diet.as_str().to_owned()),
        ("season", season.as_str().to_owned()),
    ];
    if let Some(search) = search.filter(|s| !s.is_empty()) {
        query.push(("search", search.to_owned()));
    }
    if page > 1 {
        query.push(("page", page.to_string()));
    }
    let encoded = serde_urlencoded::to_string(query).expect("clés et valeurs sont des chaînes");
    format!("{RECIPES}?{encoded}")
}

pub fn router(state: AppState) -> Router {
    let sessions = SessionManagerLayer::new(MemoryStore::default())
        .with_name("recipes_session")
        .with_secure(state.cookie_secure)
        .with_expiry(Expiry::OnInactivity(Duration::hours(2)));

    Router::new()
        .route(HOME, get(home::index))
        .route(LOGIN, get(auth::login_form).post(auth::login))
        .route(LOGOUT, post(auth::logout))
        .route("/recipes", get(recipes::index))
        .route(RECIPES, get(recipes::index))
        .route(RECIPE_NEW, get(recipes::new))
        .route(RECIPE_SAVE, post(recipes::save))
        .route("/recipes/{id}/edit", get(recipes::edit))
        .route(
            "/recipes/{id}/delete",
            get(recipes::delete_confirm).post(recipes::delete),
        )
        .nest_service("/static", ServeDir::new("static"))
        .layer(sessions)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_url_is_encoded() {
        assert_eq!(
            recipes_index(Diet::All, Season::Winter, Some("tarte & café"), 3),
            "/recipes/?diet=all&season=winter&search=tarte+%26+caf%C3%A9&page=3"
        );
        assert_eq!(
            recipes_index(Diet::Vegan, Season::All, None, 1),
            "/recipes/?diet=vegan&season=all"
        );
    }
}
