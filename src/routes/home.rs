use axum::response::Html;
use tower_sessions::Session;

use crate::{
    auth::MaybeUser,
    error::AppResult,
    models::{Diet, Season},
    views::{HomeTemplate, PageContext, SearchBar, render},
};

pub async fn index(session: Session, MaybeUser(user): MaybeUser) -> AppResult<Html<String>> {
    render(HomeTemplate {
        ctx: PageContext::build(&session, user.as_ref(), "home").await?,
        search_bar: SearchBar::new(Season::All, Diet::All, None),
    })
}
