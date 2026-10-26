# rusted-recipes

Application web de recettes de cuisine écrite en Rust (axum, askama, sqlx/MariaDB). Elle partage son schéma de base de données avec l'ancienne version Symfony.

## Prérequis

- Rust (édition 2024)
- Docker et Docker Compose (MariaDB 11 pour le dev et les tests)

## Développement

```bash
cp .env.example .env
make up      # lance MariaDB (port 3006) puis le serveur sur http://127.0.0.1:8000
make seed    # charge les recettes de démo (fixtures/dev_recipes.sql)
make tests   # lance les tests contre la base Docker
make fmt clippy
```

Créer un utilisateur (le mot de passe est lu sur l'entrée standard) :

```bash
echo 'mon-mot-de-passe' | cargo run -- create-user alice --admin
```

## Configuration

| Variable         | Obligatoire | Description                                                                  |
| ---------------- | ----------- | ---------------------------------------------------------------------------- |
| `DATABASE_URL`   | oui         | URL MySQL/MariaDB, ex. `mysql://user:pass@host:3306/db`                      |
| `APP_SECRET`     | oui (serve) | Clé de signature des cookies, 32 caractères minimum                          |
| `BIND_ADDR`      | non         | Adresse d'écoute (défaut `127.0.0.1:8000`, `0.0.0.0:8000` dans l'image)      |
| `COOKIE_SECURE`  | non         | Si défini, cookies marqués `Secure` : à activer derrière HTTPS               |
| `RUN_MIGRATIONS` | non         | Si défini, applique `migrations/` au démarrage (voir la note ci-dessous)     |
| `RUST_LOG`       | non         | Filtre de logs (défaut `rusted_recipes=info,tower_http=info`)                |

> Tant que Symfony gère le schéma, ne pas définir `RUN_MIGRATIONS`.

## Image Docker

```bash
make image IMAGE=ghcr.io/<org>/rusted-recipes TAG=1.0.0
docker push ghcr.io/<org>/rusted-recipes:1.0.0
```

## Production

Exemple de `compose.yaml` : image publiée dans un registre, base non exposée, secrets fournis par un fichier `.env` (non versionné) ou l'environnement.

```yaml
services:
  database:
    image: mariadb:11
    restart: unless-stopped
    environment:
      MARIADB_ROOT_PASSWORD: ${MARIADB_ROOT_PASSWORD:?}
      MARIADB_DATABASE: ${MARIADB_DATABASE:-recipes}
      MARIADB_USER: ${MARIADB_USER:-recipes}
      MARIADB_PASSWORD: ${MARIADB_PASSWORD:?}
    volumes:
      - database:/var/lib/mysql
    healthcheck:
      test: ["CMD", "healthcheck.sh", "--connect", "--innodb_initialized"]
      interval: 5s
      timeout: 5s
      retries: 30

  app:
    image: ghcr.io/<org>/rusted-recipes:${APP_TAG:-latest}
    restart: unless-stopped
    depends_on:
      database:
        condition: service_healthy
    environment:
      DATABASE_URL: mysql://${MARIADB_USER:-recipes}:${MARIADB_PASSWORD:?}@database:3306/${MARIADB_DATABASE:-recipes}
      APP_SECRET: ${APP_SECRET:?}
      COOKIE_SECURE: "1"
      RUST_LOG: rusted_recipes=info
    ports:
      # Exposé uniquement en local : placer un reverse proxy HTTPS devant (Caddy, nginx, Traefik…)
      - "127.0.0.1:8000:8000"

volumes:
  database:
```

`.env` associé :

```env
MARIADB_ROOT_PASSWORD=<mot de passe fort>
MARIADB_PASSWORD=<mot de passe fort>
APP_SECRET=<au moins 32 caractères aléatoires, ex. openssl rand -hex 32>
APP_TAG=1.0.0
```

Points d'attention :

- Le mot de passe de la base ne doit pas contenir de caractères spéciaux d'URL (`@`, `/`, `:`…) car il est inséré dans `DATABASE_URL` ; privilégier `openssl rand -hex 24`.
- `COOKIE_SECURE` suppose un accès en HTTPS via le reverse proxy.
- Pour créer un utilisateur en production :
  `echo 'mot-de-passe' | docker compose run --rm -T app create-user alice --admin`
- Sauvegarder le volume `database` (ex. `mariadb-dump`).
