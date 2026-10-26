use std::collections::BTreeMap;

use axum::{
    Router,
    body::Body,
    http::{Request, Response, StatusCode, header},
};
use axum_extra::extract::cookie::Key;
use http_body_util::BodyExt;
use rusted_recipes::{routes, state::AppState};
use sqlx::MySqlPool;
use tower::ServiceExt;

struct Client {
    app: Router,
    cookies: BTreeMap<String, String>,
}

struct Reply {
    status: StatusCode,
    location: Option<String>,
    body: String,
}

impl Client {
    fn new(app: Router) -> Self {
        Self {
            app,
            cookies: BTreeMap::new(),
        }
    }

    /// Même application (même magasin de sessions), mais sans aucun cookie.
    fn fresh(&self) -> Self {
        Self::new(self.app.clone())
    }

    async fn send(&mut self, mut req: Request<Body>) -> Reply {
        if !self.cookies.is_empty() {
            let cookie = self
                .cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("; ");
            req.headers_mut()
                .insert(header::COOKIE, cookie.parse().unwrap());
        }
        let res: Response<Body> = self.app.clone().oneshot(req).await.unwrap();
        for value in res.headers().get_all(header::SET_COOKIE) {
            let raw = value.to_str().unwrap();
            let (pair, attrs) = raw.split_once(';').unwrap_or((raw, ""));
            let (name, val) = pair.split_once('=').unwrap();
            if val.is_empty() || attrs.contains("Max-Age=0") {
                self.cookies.remove(name);
            } else {
                self.cookies.insert(name.to_owned(), val.to_owned());
            }
        }
        let status = res.status();
        let location = res
            .headers()
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            location,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    async fn get(&mut self, path: &str) -> Reply {
        self.send(Request::get(path).body(Body::empty()).unwrap())
            .await
    }

    async fn post(&mut self, path: &str, form: &[(&str, &str)]) -> Reply {
        let body = serde_urlencoded::to_string(form).unwrap();
        self.send(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
    }

    /// Jeton CSRF présent dans la page `path`.
    async fn csrf(&mut self, path: &str) -> String {
        let page = self.get(path).await;
        let marker = "name=\"_csrf_token\" value=\"";
        let start = page.body.find(marker).expect("jeton CSRF absent") + marker.len();
        page.body[start..].split('"').next().unwrap().to_owned()
    }

    async fn login(&mut self, username: &str, password: &str, remember: bool) -> Reply {
        let token = self.csrf("/login").await;
        let mut form = vec![
            ("_username", username),
            ("_password", password),
            ("_csrf_token", token.as_str()),
        ];
        if remember {
            form.push(("_remember_me", "on"));
        }
        self.post("/login", &form).await
    }
}

async fn setup(pool: MySqlPool) -> Client {
    for (username, roles, password) in [
        ("admin", r#"["ROLE_ADMIN"]"#, "admin"),
        ("user", r#"["ROLE_USER"]"#, "user"),
    ] {
        sqlx::query("INSERT INTO app_user (username, roles, password) VALUES (?, ?, ?)")
            .bind(username)
            .bind(roles)
            .bind(bcrypt::hash(password, 4).unwrap())
            .execute(&pool)
            .await
            .unwrap();
    }
    let state = AppState {
        pool,
        key: Key::derive_from(b"test-secret-test-secret-test-secret-0123"),
        cookie_secure: false,
    };
    Client::new(routes::router(state))
}

macro_rules! db_test {
    ($name:ident, |$client:ident| $body:block) => {
        #[sqlx::test(
            migrations = "./migrations",
            fixtures(path = "../fixtures", scripts("recipes"))
        )]
        async fn $name(pool: MySqlPool) {
            let mut $client = setup(pool).await;
            $body
        }
    };
}

db_test!(index_is_paginated, |c| {
    let page1 = c.get("/recipes/").await;
    assert_eq!(page1.status, StatusCode::OK);
    assert_eq!(page1.body.matches("class=\"recipe-card\"").count(), 24);
    assert!(page1.body.contains("rel=\"next\""));

    // 103 recettes → 5 pages ; une page hors limites est ramenée à la dernière
    let last = c.get("/recipes/?page=999").await;
    assert_eq!(last.body.matches("class=\"recipe-card\"").count(), 7);
    assert!(last.body.contains("aria-current=\"page\">5<"));

    let garbage = c.get("/recipes/?page=abc&diet=nope&season=").await;
    assert_eq!(garbage.status, StatusCode::OK);
});

db_test!(index_filters, |c| {
    let veggie = c.get("/recipes/?diet=veggie").await;
    assert!(veggie.body.contains("Quiche aux poireaux"));
    assert!(!veggie.body.contains("Moussaka"));

    let winter = c.get("/recipes/?season=winter").await;
    assert!(winter.body.contains("Quiche aux poireaux"));
    assert_eq!(winter.body.matches("class=\"recipe-card\"").count(), 1);

    let by_tag = c.get("/recipes/?search=aubergine").await;
    assert!(by_tag.body.contains("Moussaka"));
    assert_eq!(by_tag.body.matches("class=\"recipe-card\"").count(), 1);

    let by_name = c.get("/recipes/?search=chili").await;
    assert!(by_name.body.contains("Chili con Carne"));

    // tous les mots doivent correspondre
    let both = c.get("/recipes/?search=moussaka+four").await;
    assert_eq!(both.body.matches("class=\"recipe-card\"").count(), 1);
    let none = c.get("/recipes/?search=moussaka+zzz").await;
    assert_eq!(none.body.matches("class=\"recipe-card\"").count(), 0);

    // les jokers LIKE sont neutralisés
    let wildcard = c.get("/recipes/?search=%25").await;
    assert_eq!(wildcard.body.matches("class=\"recipe-card\"").count(), 0);
});

db_test!(protected_routes_redirect_to_login, |c| {
    for path in ["/recipes/new", "/recipes/1/edit", "/recipes/1/delete"] {
        let res = c.get(path).await;
        assert_eq!(res.status, StatusCode::SEE_OTHER, "{path}");
        assert_eq!(res.location.as_deref(), Some("/login"));
    }
    let res = c.post("/recipes/save", &[("name", "x")]).await;
    assert_eq!(res.location.as_deref(), Some("/login"));
});

db_test!(login_and_logout, |c| {
    let bad = c.login("admin", "wrong", false).await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert!(bad.body.contains("Identifiants invalides."));
    assert_eq!(c.get("/recipes/new").await.status, StatusCode::SEE_OTHER);

    let unknown = c.login("ghost", "ghost", false).await;
    assert_eq!(unknown.status, StatusCode::UNAUTHORIZED);

    // sans jeton CSRF : refusé
    let no_csrf = c
        .post("/login", &[("_username", "admin"), ("_password", "admin")])
        .await;
    assert_eq!(no_csrf.status, StatusCode::BAD_REQUEST);

    let ok = c.login("admin", "admin", false).await;
    assert_eq!(ok.status, StatusCode::SEE_OTHER);
    assert_eq!(ok.location.as_deref(), Some("/recipes/"));
    assert_eq!(c.get("/recipes/new").await.status, StatusCode::OK);

    let token = c.csrf("/recipes/").await;
    let out = c.post("/logout", &[("_csrf_token", &token)]).await;
    assert_eq!(out.location.as_deref(), Some("/"));
    assert_eq!(c.get("/recipes/new").await.status, StatusCode::SEE_OTHER);
});

db_test!(remember_me_restores_login, |c| {
    c.login("user", "user", true).await;
    let remember = c
        .cookies
        .get("RECIPE-REMEMBER-ME")
        .cloned()
        .expect("cookie remember-me");

    // nouveau navigateur : seul le cookie remember-me est présent
    let mut other = c.fresh();
    other
        .cookies
        .insert("RECIPE-REMEMBER-ME".into(), remember.clone());
    assert_eq!(other.get("/recipes/new").await.status, StatusCode::OK);

    // un cookie falsifié est ignoré
    let mut forged = c.fresh();
    forged
        .cookies
        .insert("RECIPE-REMEMBER-ME".into(), "1:99999999999".into());
    assert_eq!(
        forged.get("/recipes/new").await.status,
        StatusCode::SEE_OTHER
    );

    // sans la case cochée, pas de cookie
    let mut c2 = c.fresh();
    c2.login("user", "user", false).await;
    assert!(!c2.cookies.contains_key("RECIPE-REMEMBER-ME"));
});

db_test!(create_edit_and_validate, |c| {
    c.login("user", "user", false).await;
    let token = c.csrf("/recipes/new").await;

    let valid = [
        ("_csrf_token", token.as_str()),
        ("name", "Tarte aux pommes"),
        ("diet", "veggie"),
        ("season", "autumn"),
        ("url", "https://example.org/tarte"),
        ("note", "Beaucoup de beurre"),
        ("tags", "four, dessert"),
    ];

    // jeton invalide : refusé (bug Symfony corrigé : le jeton valide passe, l'invalide échoue)
    let mut forged = valid.to_vec();
    forged[0].1 = "nope";
    assert_eq!(
        c.post("/recipes/save", &forged).await.status,
        StatusCode::BAD_REQUEST
    );

    let created = c.post("/recipes/save", &valid).await;
    assert_eq!(created.location.as_deref(), Some("/recipes/"));
    let list = c.get("/recipes/?search=tarte").await;
    assert!(list.body.contains("a été créée avec succès"));
    assert!(list.body.contains("#dessert"));
    assert!(list.body.contains("Beaucoup de beurre"));

    // édition
    let id: i32 = {
        let marker = "/recipes/";
        let start = list.body.find("/edit").unwrap();
        let before = &list.body[..start];
        before[before.rfind(marker).unwrap() + marker.len()..]
            .parse()
            .unwrap()
    };
    let form = c.get(&format!("/recipes/{id}/edit")).await;
    assert!(form.body.contains("value=\"Tarte aux pommes\""));
    let id_s = id.to_string();
    let mut edit = valid.to_vec();
    edit.push(("id", &id_s));
    edit[1].1 = "Tarte aux poires";
    c.post("/recipes/save", &edit).await;
    let list = c.get("/recipes/?search=tarte").await;
    assert!(list.body.contains("Tarte aux poires"));
    assert!(!list.body.contains("Tarte aux pommes"));

    // validation : formulaire ré-affiché avec les erreurs
    let mut invalid = valid.to_vec();
    invalid[1].1 = " ";
    invalid[4].1 = "javascript:alert(1)";
    let res = c.post("/recipes/save", &invalid).await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(res.body.contains("Le nom est obligatoire."));
    assert!(res.body.contains("http://"));

    // édition d'une recette inexistante
    let mut ghost = valid.to_vec();
    ghost.push(("id", "999999"));
    assert_eq!(
        c.post("/recipes/save", &ghost).await.status,
        StatusCode::NOT_FOUND
    );
});

db_test!(delete_requires_post_and_csrf, |c| {
    c.login("user", "user", false).await;
    let confirm = c.get("/recipes/1/delete").await;
    assert_eq!(confirm.status, StatusCode::OK);
    assert!(confirm.body.contains("method=\"post\""));
    // la page de confirmation ne supprime rien
    assert!(
        c.get("/recipes/?search=quiche")
            .await
            .body
            .contains("Quiche aux poireaux")
    );

    let token = c.csrf("/recipes/1/delete").await;
    assert_eq!(
        c.post("/recipes/1/delete", &[("_csrf_token", "bad")])
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    let done = c
        .post("/recipes/1/delete", &[("_csrf_token", &token)])
        .await;
    assert_eq!(done.location.as_deref(), Some("/recipes/"));

    let list = c.get("/recipes/?search=quiche").await;
    assert!(list.body.contains("a bien été supprimée"));
    assert!(!list.body.contains("class=\"recipe-card\""));
    assert_eq!(
        c.get("/recipes/1/delete").await.status,
        StatusCode::NOT_FOUND
    );
});

db_test!(home_and_static, |c| {
    let home = c.get("/").await;
    assert_eq!(home.status, StatusCode::OK);
    assert!(home.body.contains("Recipes eBook"));
    let css = c.get("/static/app.css").await;
    assert_eq!(css.status, StatusCode::OK);
});

db_test!(output_is_escaped, |c| {
    c.login("user", "user", false).await;
    let token = c.csrf("/recipes/new").await;
    let form = [
        ("_csrf_token", token.as_str()),
        ("name", "<script>alert(1)</script>"),
        ("diet", "all"),
        ("season", "all"),
        ("url", "https://example.org/?a=1&b=\"2\""),
        ("note", "<b>gras</b>"),
        ("tags", "<i>"),
    ];
    c.post("/recipes/save", &form).await;
    let list = c.get("/recipes/?search=script").await;
    assert!(list.body.contains("&#60;script&#62;"));
    assert!(!list.body.contains("<script>alert(1)"));
    assert!(!list.body.contains("<b>gras</b>"));
});

db_test!(flash_is_shown_once, |c| {
    c.login("user", "user", false).await;
    let token = c.csrf("/recipes/1/delete").await;
    c.post("/recipes/1/delete", &[("_csrf_token", &token)])
        .await;
    assert!(
        c.get("/recipes/")
            .await
            .body
            .contains("a bien été supprimée")
    );
    assert!(
        !c.get("/recipes/")
            .await
            .body
            .contains("a bien été supprimée")
    );
});

db_test!(missing_resources, |c| {
    assert_eq!(c.get("/nope").await.status, StatusCode::NOT_FOUND);

    c.login("user", "user", false).await;
    assert_eq!(
        c.get("/recipes/999999/edit").await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.get("/recipes/999999/delete").await.status,
        StatusCode::NOT_FOUND
    );
    let token = c.csrf("/recipes/new").await;
    assert_eq!(
        c.post("/recipes/999999/delete", &[("_csrf_token", &token)])
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    // identifiant non numérique : jamais un succès
    assert!(c.get("/recipes/abc/edit").await.status.is_client_error());
    let mut bad_id = vec![
        ("_csrf_token", token.as_str()),
        ("name", "x"),
        ("id", "abc"),
    ];
    bad_id.extend([("diet", "all"), ("season", "all"), ("url", "http://x")]);
    assert_eq!(
        c.post("/recipes/save", &bad_id).await.status,
        StatusCode::NOT_FOUND
    );
});

db_test!(login_page_and_trailing_slash, |c| {
    let page = c.get("/login").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("_csrf_token"));
    assert_eq!(c.get("/recipes").await.status, StatusCode::OK);
    // logout sans jeton valide : refusé
    assert_eq!(
        c.post("/logout", &[("_csrf_token", "bad")]).await.status,
        StatusCode::BAD_REQUEST
    );
});
