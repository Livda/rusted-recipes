use rusted_recipes::{
    models::{Diet, NewRecipe, Season},
    repository::{self, Filter},
};
use sqlx::MySqlPool;

fn recipe(name: &str, diet: Diet, season: Season, tags: &[&str]) -> NewRecipe {
    NewRecipe {
        diet,
        name: name.to_owned(),
        note: None,
        season,
        tags: tags.iter().map(|t| t.to_string()).collect(),
        url: "https://example.org".to_owned(),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn recipe_crud_round_trip(pool: MySqlPool) {
    let mut new = recipe("Tarte", Diet::Veggie, Season::Autumn, &["four", "dessert"]);
    new.note = Some("beurre".into());
    let id = repository::insert_recipe(&pool, &new).await.unwrap();

    let found = repository::find_recipe(&pool, id).await.unwrap().unwrap();
    assert_eq!(found.name, "Tarte");
    assert_eq!(found.note.as_deref(), Some("beurre"));
    assert_eq!(found.tags, vec!["four", "dessert"]);
    assert_eq!((found.diet, found.season), (Diet::Veggie, Season::Autumn));

    new.name = "Tarte bis".into();
    assert!(repository::update_recipe(&pool, id, &new).await.unwrap());
    // mise à jour sans changement : la recette existe toujours
    assert!(repository::update_recipe(&pool, id, &new).await.unwrap());
    assert!(
        !repository::update_recipe(&pool, id + 1000, &new)
            .await
            .unwrap()
    );
    let found = repository::find_recipe(&pool, id).await.unwrap().unwrap();
    assert_eq!(found.name, "Tarte bis");

    assert!(repository::delete_recipe(&pool, id).await.unwrap());
    assert!(!repository::delete_recipe(&pool, id).await.unwrap());
    assert!(repository::find_recipe(&pool, id).await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn filters_pagination_and_ordering(pool: MySqlPool) {
    for (name, diet, season, tags) in [
        ("Banane", Diet::Vegan, Season::Summer, vec!["fruit"]),
        (
            "Abricot",
            Diet::Veggie,
            Season::Summer,
            vec!["fruit", "100%_pur"],
        ),
        ("Courge", Diet::Vegan, Season::Autumn, vec!["soupe"]),
    ] {
        repository::insert_recipe(&pool, &recipe(name, diet, season, &tags))
            .await
            .unwrap();
    }
    let all = Filter::default();
    assert_eq!(repository::count_recipes(&pool, &all).await.unwrap(), 3);

    let names = |rs: Vec<rusted_recipes::models::Recipe>| {
        rs.into_iter().map(|r| r.name).collect::<Vec<_>>()
    };
    let page = repository::find_recipes(&pool, &all, 2, 0).await.unwrap();
    assert_eq!(names(page), ["Abricot", "Banane"]);
    let page = repository::find_recipes(&pool, &all, 2, 2).await.unwrap();
    assert_eq!(names(page), ["Courge"]);

    let vegan_summer = Filter {
        diet: Diet::Vegan,
        season: Season::Summer,
        search: None,
    };
    let found = repository::find_recipes(&pool, &vegan_summer, 10, 0)
        .await
        .unwrap();
    assert_eq!(names(found), ["Banane"]);

    // recherche : nom ou tag, insensible à la casse, jokers littéraux
    for (search, expected) in [
        ("FRUIT", 2),
        ("soup", 1),
        ("100%", 1),
        ("%", 1),
        ("_", 1),
        ("zzz", 0),
    ] {
        let f = Filter {
            search: Some(search.into()),
            ..Filter::default()
        };
        assert_eq!(
            repository::count_recipes(&pool, &f).await.unwrap(),
            expected,
            "{search}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn users(pool: MySqlPool) {
    let roles = vec!["ROLE_ADMIN".to_owned()];
    let id = repository::insert_user(&pool, "alice", &roles, "hash")
        .await
        .unwrap();

    let by_name = repository::find_user_by_username(&pool, "alice")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (by_name.id, by_name.roles.clone(), by_name.password.as_str()),
        (id, roles, "hash")
    );
    assert!(by_name.has_role("ROLE_ADMIN") && by_name.has_role("ROLE_USER"));
    assert_eq!(
        repository::find_user(&pool, id)
            .await
            .unwrap()
            .unwrap()
            .username,
        "alice"
    );

    assert!(
        repository::find_user_by_username(&pool, "bob")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        repository::find_user(&pool, id + 1)
            .await
            .unwrap()
            .is_none()
    );
    // nom d'utilisateur unique
    assert!(
        repository::insert_user(&pool, "alice", &[], "x")
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn unknown_db_values_fall_back_to_all(pool: MySqlPool) {
    sqlx::query("INSERT INTO app_recipe (diet, name, season, tags, url) VALUES ('???', 'X', '???', 'pas du json', 'http://x')")
        .execute(&pool)
        .await
        .unwrap();
    let r = &repository::find_recipes(&pool, &Filter::default(), 10, 0)
        .await
        .unwrap()[0];
    assert_eq!((r.diet, r.season), (Diet::All, Season::All));
    assert!(r.tags.is_empty());
}
