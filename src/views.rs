use askama::Template;
use axum::response::Html;
use tower_sessions::Session;

use crate::{
    auth::{self, Flash},
    error::AppResult,
    models::{Diet, Recipe, Season, User},
    pagination::{PageItem, Pagination},
    routes,
};

/// Contexte commun à toutes les pages (remplace `app.*` de Twig).
pub struct PageContext {
    pub username: Option<String>,
    pub flashes: Vec<Flash>,
    pub active: &'static str,
    pub csrf: String,
}

impl PageContext {
    pub async fn build(
        session: &Session,
        user: Option<&User>,
        active: &'static str,
    ) -> AppResult<Self> {
        Ok(Self {
            username: user.map(|u| u.username.clone()),
            flashes: auth::take_flashes(session).await?,
            active,
            csrf: auth::csrf_token(session).await?,
        })
    }

    pub fn logged_in(&self) -> bool {
        self.username.is_some()
    }

    pub fn home_url(&self) -> &'static str {
        routes::HOME
    }

    pub fn login_url(&self) -> &'static str {
        routes::LOGIN
    }

    pub fn logout_url(&self) -> &'static str {
        routes::LOGOUT
    }

    pub fn recipes_url(&self) -> &'static str {
        routes::RECIPES
    }

    pub fn recipe_new_url(&self) -> &'static str {
        routes::RECIPE_NEW
    }

    pub fn recipe_save_url(&self) -> &'static str {
        routes::RECIPE_SAVE
    }
}

pub struct SelectOption {
    pub value: &'static str,
    pub label: &'static str,
    pub selected: bool,
}

pub fn season_options(selected: Season) -> Vec<SelectOption> {
    Season::ALL
        .into_iter()
        .map(|s| SelectOption {
            value: s.as_str(),
            label: s.label(),
            selected: s == selected,
        })
        .collect()
}

pub fn diet_options(selected: Diet) -> Vec<SelectOption> {
    Diet::ALL
        .into_iter()
        .map(|d| SelectOption {
            value: d.as_str(),
            label: d.label(),
            selected: d == selected,
        })
        .collect()
}

pub struct SearchBar {
    pub seasons: Vec<SelectOption>,
    pub diets: Vec<SelectOption>,
    pub search: String,
}

impl SearchBar {
    pub fn new(season: Season, diet: Diet, search: Option<&str>) -> Self {
        Self {
            seasons: season_options(season),
            diets: diet_options(diet),
            search: search.unwrap_or_default().to_owned(),
        }
    }
}

pub struct PageLink {
    pub label: String,
    pub href: Option<String>,
    pub current: bool,
}

pub struct PaginationView {
    pub previous: Option<String>,
    pub next: Option<String>,
    pub links: Vec<PageLink>,
}

impl PaginationView {
    pub fn new(pagination: &Pagination, href: impl Fn(u32) -> String) -> Option<Self> {
        if pagination.page_count < 2 {
            return None;
        }
        let links = pagination
            .items()
            .into_iter()
            .map(|item| match item {
                PageItem::Page(n) if n == pagination.current => PageLink {
                    label: n.to_string(),
                    href: None,
                    current: true,
                },
                PageItem::Page(n) => PageLink {
                    label: n.to_string(),
                    href: Some(href(n)),
                    current: false,
                },
                PageItem::Gap => PageLink {
                    label: "…".to_owned(),
                    href: None,
                    current: false,
                },
            })
            .collect();
        Some(Self {
            previous: pagination.previous.map(&href),
            next: pagination.next.map(&href),
            links,
        })
    }
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub ctx: PageContext,
    pub search_bar: SearchBar,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub ctx: PageContext,
    pub last_username: String,
    pub error: Option<&'static str>,
}

#[derive(Template)]
#[template(path = "recipe/index.html")]
pub struct RecipeIndexTemplate {
    pub ctx: PageContext,
    pub search_bar: SearchBar,
    pub recipes: Vec<Recipe>,
    pub pagination: Option<PaginationView>,
}

#[derive(Template)]
#[template(path = "recipe/form.html")]
pub struct RecipeFormTemplate {
    pub ctx: PageContext,
    pub id: Option<i32>,
    pub name: String,
    pub diets: Vec<SelectOption>,
    pub seasons: Vec<SelectOption>,
    pub url: String,
    pub note: String,
    pub tags: String,
    pub errors: Vec<&'static str>,
}

#[derive(Template)]
#[template(path = "recipe/delete.html")]
pub struct DeleteTemplate {
    pub ctx: PageContext,
    pub recipe: Recipe,
}

/// Réponse HTML d'un template (remplace `askama_axum`).
pub fn render<T: Template>(template: T) -> AppResult<Html<String>> {
    Ok(Html(template.render()?))
}

// Les URLs sont construites côté Rust ; ces alias évitent d'importer `routes` partout.
impl RecipeIndexTemplate {
    pub fn edit_url(&self, recipe: &Recipe) -> String {
        routes::recipe_edit(recipe.id)
    }

    pub fn delete_url(&self, recipe: &Recipe) -> String {
        routes::recipe_delete(recipe.id)
    }
}

impl DeleteTemplate {
    pub fn action(&self) -> String {
        routes::recipe_delete(self.recipe.id)
    }
}
