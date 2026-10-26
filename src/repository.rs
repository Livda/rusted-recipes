use sqlx::{MySql, MySqlPool, QueryBuilder, Row, mysql::MySqlRow};

use crate::models::{Diet, NewRecipe, Recipe, Season, User};

#[derive(Clone, Debug)]
pub struct Filter {
    pub season: Season,
    pub diet: Diet,
    pub search: Option<String>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            season: Season::All,
            diet: Diet::All,
            search: None,
        }
    }
}

fn escape_like(shard: &str) -> String {
    let mut out = String::with_capacity(shard.len() + 2);
    out.push('%');
    for c in shard.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

/// Chaque mot de la recherche doit apparaître dans le nom ou dans les tags.
fn push_filter(qb: &mut QueryBuilder<MySql>, filter: &Filter) {
    qb.push(" WHERE 1 = 1");
    if filter.season != Season::All {
        qb.push(" AND season = ").push_bind(filter.season.as_str());
    }
    if filter.diet != Diet::All {
        qb.push(" AND diet = ").push_bind(filter.diet.as_str());
    }
    if let Some(search) = &filter.search {
        for shard in search.split_whitespace() {
            let pattern = escape_like(shard);
            qb.push(" AND (name LIKE ")
                .push_bind(pattern.clone())
                .push(" OR tags LIKE ")
                .push_bind(pattern)
                .push(")");
        }
    }
}

fn recipe_from_row(row: &MySqlRow) -> Result<Recipe, sqlx::Error> {
    let tags: String = row.try_get("tags")?;
    let diet: String = row.try_get("diet")?;
    let season: String = row.try_get("season")?;
    Ok(Recipe {
        id: row.try_get("id")?,
        // Valeurs inconnues en base : traitées comme « tout »
        diet: Diet::parse(&diet).unwrap_or(Diet::All),
        name: row.try_get("name")?,
        note: row.try_get("note")?,
        season: Season::parse(&season).unwrap_or(Season::All),
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        url: row.try_get("url")?,
    })
}

pub async fn count_recipes(pool: &MySqlPool, filter: &Filter) -> Result<u64, sqlx::Error> {
    let mut qb = QueryBuilder::new("SELECT COUNT(id) FROM app_recipe");
    push_filter(&mut qb, filter);
    let count: i64 = qb.build().fetch_one(pool).await?.try_get(0)?;
    Ok(count as u64)
}

pub async fn find_recipes(
    pool: &MySqlPool,
    filter: &Filter,
    limit: u64,
    offset: u64,
) -> Result<Vec<Recipe>, sqlx::Error> {
    let mut qb =
        QueryBuilder::new("SELECT id, diet, name, note, season, tags, url FROM app_recipe");
    push_filter(&mut qb, filter);
    qb.push(" ORDER BY name ASC, id ASC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    qb.build()
        .fetch_all(pool)
        .await?
        .iter()
        .map(recipe_from_row)
        .collect()
}

pub async fn find_recipe(pool: &MySqlPool, id: i32) -> Result<Option<Recipe>, sqlx::Error> {
    sqlx::query("SELECT id, diet, name, note, season, tags, url FROM app_recipe WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .as_ref()
        .map(recipe_from_row)
        .transpose()
}

pub async fn insert_recipe(pool: &MySqlPool, recipe: &NewRecipe) -> Result<i32, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO app_recipe (diet, name, note, season, tags, url) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(recipe.diet.as_str())
    .bind(&recipe.name)
    .bind(&recipe.note)
    .bind(recipe.season.as_str())
    .bind(tags_json(&recipe.tags))
    .bind(&recipe.url)
    .execute(pool)
    .await?;
    Ok(result.last_insert_id() as i32)
}

/// Renvoie `false` si la recette n'existe pas.
pub async fn update_recipe(
    pool: &MySqlPool,
    id: i32,
    recipe: &NewRecipe,
) -> Result<bool, sqlx::Error> {
    // `rows_affected` vaut 0 quand rien ne change : on vérifie l'existence séparément.
    sqlx::query(
        "UPDATE app_recipe SET diet = ?, name = ?, note = ?, season = ?, tags = ?, url = ? WHERE id = ?",
    )
    .bind(recipe.diet.as_str())
    .bind(&recipe.name)
    .bind(&recipe.note)
    .bind(recipe.season.as_str())
    .bind(tags_json(&recipe.tags))
    .bind(&recipe.url)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(find_recipe(pool, id).await?.is_some())
}

pub async fn delete_recipe(pool: &MySqlPool, id: i32) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM app_recipe WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

fn tags_json(tags: &[String]) -> String {
    serde_json::to_string(tags).expect("une liste de chaînes est toujours sérialisable")
}

fn user_from_row(row: &MySqlRow) -> Result<User, sqlx::Error> {
    let roles: String = row.try_get("roles")?;
    Ok(User {
        id: row.try_get("id")?,
        username: row.try_get("username")?,
        roles: serde_json::from_str(&roles).unwrap_or_default(),
        password: row.try_get("password")?,
    })
}

pub async fn find_user_by_username(
    pool: &MySqlPool,
    username: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query("SELECT id, username, roles, password FROM app_user WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?
        .as_ref()
        .map(user_from_row)
        .transpose()
}

pub async fn find_user(pool: &MySqlPool, id: i32) -> Result<Option<User>, sqlx::Error> {
    sqlx::query("SELECT id, username, roles, password FROM app_user WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .as_ref()
        .map(user_from_row)
        .transpose()
}

pub async fn insert_user(
    pool: &MySqlPool,
    username: &str,
    roles: &[String],
    password_hash: &str,
) -> Result<i32, sqlx::Error> {
    let result = sqlx::query("INSERT INTO app_user (username, roles, password) VALUES (?, ?, ?)")
        .bind(username)
        .bind(tags_json(roles))
        .bind(password_hash)
        .execute(pool)
        .await?;
    Ok(result.last_insert_id() as i32)
}

#[cfg(test)]
mod tests {
    use super::escape_like;

    #[test]
    fn like_wildcards_are_escaped() {
        assert_eq!(escape_like("a%b_c\\"), "%a\\%b\\_c\\\\%");
    }
}
