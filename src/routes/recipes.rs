use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    auth::{self, MaybeUser, RequireUser},
    error::{AppError, AppResult},
    models::{Diet, NewRecipe, Recipe, Season},
    pagination::Pagination,
    repository::{self, Filter},
    routes,
    state::AppState,
    views::{
        DeleteTemplate, PageContext, PaginationView, RecipeFormTemplate, RecipeIndexTemplate,
        SearchBar, diet_options, render, season_options,
    },
};

const PER_PAGE: u64 = 24;
const MAX_LEN: usize = 255;

#[derive(Deserialize)]
pub struct IndexQuery {
    diet: Option<String>,
    season: Option<String>,
    search: Option<String>,
    page: Option<String>,
}

pub async fn index(
    State(state): State<AppState>,
    session: Session,
    MaybeUser(user): MaybeUser,
    Query(query): Query<IndexQuery>,
) -> AppResult<Html<String>> {
    let filter = Filter {
        diet: query
            .diet
            .as_deref()
            .and_then(Diet::parse)
            .unwrap_or(Diet::All),
        season: query
            .season
            .as_deref()
            .and_then(Season::parse)
            .unwrap_or(Season::All),
        search: query
            .search
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty()),
    };
    let requested = query
        .page
        .as_deref()
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(1);

    let total = repository::count_recipes(&state.pool, &filter).await?;
    let pagination = Pagination::new(total, PER_PAGE, requested);
    let recipes = repository::find_recipes(
        &state.pool,
        &filter,
        PER_PAGE,
        u64::from(pagination.current - 1) * PER_PAGE,
    )
    .await?;

    let search = filter.search.as_deref();
    render(RecipeIndexTemplate {
        ctx: PageContext::build(&session, user.as_ref(), "recipes").await?,
        search_bar: SearchBar::new(filter.season, filter.diet, search),
        recipes,
        pagination: PaginationView::new(&pagination, |page| {
            routes::recipes_index(filter.diet, filter.season, search, page)
        }),
    })
}

// ------------------------------------------------------------ formulaire

#[derive(Default)]
struct FormValues {
    id: Option<i32>,
    name: String,
    diet: String,
    season: String,
    url: String,
    note: String,
    tags: String,
}

impl FormValues {
    fn from_recipe(recipe: &Recipe) -> Self {
        Self {
            id: Some(recipe.id),
            name: recipe.name.clone(),
            diet: recipe.diet.as_str().to_owned(),
            season: recipe.season.as_str().to_owned(),
            url: recipe.url.clone(),
            note: recipe.note.clone().unwrap_or_default(),
            tags: recipe.tags.join(", "),
        }
    }
}

async fn render_form(
    session: &Session,
    user: &crate::models::User,
    values: FormValues,
    errors: Vec<&'static str>,
) -> AppResult<Html<String>> {
    render(RecipeFormTemplate {
        ctx: PageContext::build(session, Some(user), "recipe-form").await?,
        id: values.id,
        name: values.name,
        diets: diet_options(Diet::parse(&values.diet).unwrap_or(Diet::All)),
        seasons: season_options(Season::parse(&values.season).unwrap_or(Season::All)),
        url: values.url,
        note: values.note,
        tags: values.tags,
        errors,
    })
}

pub async fn new(session: Session, RequireUser(user): RequireUser) -> AppResult<Html<String>> {
    render_form(&session, &user, FormValues::default(), vec![]).await
}

pub async fn edit(
    State(state): State<AppState>,
    session: Session,
    RequireUser(user): RequireUser,
    Path(id): Path<i32>,
) -> AppResult<Html<String>> {
    let recipe = repository::find_recipe(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    render_form(&session, &user, FormValues::from_recipe(&recipe), vec![]).await
}

#[derive(Deserialize)]
pub struct SaveForm {
    #[serde(default)]
    _csrf_token: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    diet: String,
    #[serde(default)]
    season: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    tags: String,
}

fn split_tags(raw: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for tag in raw.split(',').map(|t| t.trim().trim_start_matches('#')) {
        if !tag.is_empty() && !tags.iter().any(|t| t == tag) {
            tags.push(tag.to_owned());
        }
    }
    tags
}

/// Valide le formulaire ; les erreurs sont affichées à côté du formulaire.
fn validate(form: &SaveForm) -> Result<NewRecipe, Vec<&'static str>> {
    let mut errors = Vec::new();

    let name = form.name.trim();
    if name.is_empty() {
        errors.push("Le nom est obligatoire.");
    } else if name.chars().count() > MAX_LEN {
        errors.push("Le nom est trop long (255 caractères maximum).");
    }

    let url = form.url.trim();
    if url.is_empty() {
        errors.push("Le lien est obligatoire.");
    } else if !(url.starts_with("http://") || url.starts_with("https://")) {
        // Le lien est rendu dans un `href` : on refuse `javascript:` et consorts.
        errors.push("Le lien doit commencer par http:// ou https://.");
    }

    let note = form.note.trim();
    if note.chars().count() > MAX_LEN {
        errors.push("La note est trop longue (255 caractères maximum).");
    }

    let diet = Diet::parse(form.diet.trim());
    if diet.is_none() {
        errors.push("Le régime est invalide.");
    }
    let season = Season::parse(form.season.trim());
    if season.is_none() {
        errors.push("La saison est invalide.");
    }

    match (diet, season) {
        (Some(diet), Some(season)) if errors.is_empty() => Ok(NewRecipe {
            diet,
            name: name.to_owned(),
            note: (!note.is_empty()).then(|| note.to_owned()),
            season,
            tags: split_tags(&form.tags),
            url: url.to_owned(),
        }),
        _ => Err(errors),
    }
}

pub async fn save(
    State(state): State<AppState>,
    session: Session,
    RequireUser(user): RequireUser,
    Form(form): Form<SaveForm>,
) -> AppResult<Response> {
    auth::verify_csrf(&session, &form._csrf_token).await?;

    let id = match form.id.trim() {
        "" => None,
        raw => Some(raw.parse::<i32>().map_err(|_| AppError::NotFound)?),
    };

    let recipe = match validate(&form) {
        Ok(recipe) => recipe,
        Err(errors) => {
            let values = FormValues {
                id,
                name: form.name,
                diet: form.diet,
                season: form.season,
                url: form.url,
                note: form.note,
                tags: form.tags,
            };
            let html = render_form(&session, &user, values, errors).await?;
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, html).into_response());
        }
    };

    let message = match id {
        Some(id) => {
            if !repository::update_recipe(&state.pool, id, &recipe).await? {
                return Err(AppError::NotFound);
            }
            format!("La recette {} a été modifiée avec succès.", recipe.name)
        }
        None => {
            repository::insert_recipe(&state.pool, &recipe).await?;
            format!("La recette {} a été créée avec succès.", recipe.name)
        }
    };
    auth::add_flash(&session, "success", message).await?;
    Ok(Redirect::to(routes::RECIPES).into_response())
}

// ------------------------------------------------------------ suppression

pub async fn delete_confirm(
    State(state): State<AppState>,
    session: Session,
    RequireUser(user): RequireUser,
    Path(id): Path<i32>,
) -> AppResult<Html<String>> {
    let recipe = repository::find_recipe(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    render(DeleteTemplate {
        ctx: PageContext::build(&session, Some(&user), "recipes").await?,
        recipe,
    })
}

#[derive(Deserialize)]
pub struct CsrfForm {
    #[serde(default)]
    _csrf_token: String,
}

pub async fn delete(
    State(state): State<AppState>,
    session: Session,
    RequireUser(_user): RequireUser,
    Path(id): Path<i32>,
    Form(form): Form<CsrfForm>,
) -> AppResult<Redirect> {
    auth::verify_csrf(&session, &form._csrf_token).await?;
    let recipe = repository::find_recipe(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    repository::delete_recipe(&state.pool, id).await?;
    auth::add_flash(
        &session,
        "success",
        format!("La recette {} a bien été supprimée.", recipe.name),
    )
    .await?;
    Ok(Redirect::to(routes::RECIPES))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> SaveForm {
        SaveForm {
            _csrf_token: String::new(),
            id: String::new(),
            name: "  Tarte  ".into(),
            diet: "veggie".into(),
            season: "summer".into(),
            url: "https://example.org/tarte".into(),
            note: "  ".into(),
            tags: "four, #fruit, four,".into(),
        }
    }

    #[test]
    fn valid_form_is_normalised() {
        let recipe = validate(&form()).unwrap();
        assert_eq!(recipe.name, "Tarte");
        assert_eq!(recipe.note, None);
        assert_eq!(recipe.tags, vec!["four", "fruit"]);
        assert_eq!((recipe.diet, recipe.season), (Diet::Veggie, Season::Summer));
    }

    #[test]
    fn invalid_fields_are_reported() {
        let mut f = form();
        f.name = " ".into();
        f.url = "javascript:alert(1)".into();
        f.diet = "carnivore".into();
        f.season = String::new();
        assert_eq!(validate(&f).unwrap_err().len(), 4);
    }

    #[test]
    fn tags_are_trimmed_deduplicated_and_stripped_of_hash() {
        assert_eq!(split_tags(" #a, b ,a,, ##c ,#"), vec!["a", "b", "c"]);
        assert!(split_tags("").is_empty());
        assert!(split_tags(" , ,").is_empty());
    }

    #[test]
    fn length_limits_count_characters() {
        let mut f = form();
        f.name = "é".repeat(MAX_LEN);
        assert!(validate(&f).is_ok());
        f.name = "é".repeat(MAX_LEN + 1);
        assert_eq!(validate(&f).unwrap_err().len(), 1);

        let mut f = form();
        f.note = "x".repeat(MAX_LEN + 1);
        assert_eq!(validate(&f).unwrap_err().len(), 1);
    }

    #[test]
    fn only_http_links_are_accepted() {
        for ok in ["http://a.b", "https://a.b/c?d=e", "  https://a.b  "] {
            let mut f = form();
            f.url = ok.into();
            assert!(validate(&f).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "ftp://a.b",
            "javascript:alert(1)",
            "//a.b",
            "HTTP://a.b",
        ] {
            let mut f = form();
            f.url = bad.into();
            assert!(validate(&f).is_err(), "{bad}");
        }
    }
}
