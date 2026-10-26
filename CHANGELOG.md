# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),
and this project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-10-26

### Added

- Serveur web axum avec rendu de templates askama et fichiers statiques.
- Liste, recherche et pagination des recettes ; création et édition de recettes.
- Authentification par formulaire (sessions signées, mots de passe hachés avec argon2/bcrypt).
- Commande `create-user [--admin]` pour créer un utilisateur depuis l'entrée standard.
- Migration initiale (`migrations/0001_init.sql`), activable avec `RUN_MIGRATIONS`.
- `Dockerfile`, `compose.yaml` de développement et intégration continue GitHub Actions.
- `README.md` avec un exemple de `compose.yaml` de production.

[Unreleased]: https://github.com/Livda/rusted-recipes/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/Livda/rusted-recipes/releases/tag/v1.0.0
