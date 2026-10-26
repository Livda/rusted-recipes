SHELL := /bin/bash
TEST_DATABASE_URL ?= mysql://root:symfony@127.0.0.1:3006/symfony

fmt:
	cargo fmt
.PHONY: fmt

clippy:
	cargo clippy --all-targets -- -D warnings
.PHONY: clippy

up:
	docker compose up -d --wait database
	RUN_MIGRATIONS=1 cargo run -- serve
.PHONY: up

tests:
	docker compose up -d --wait database
	DATABASE_URL=$(TEST_DATABASE_URL) cargo test
.PHONY: tests

seed:
	docker compose up -d --wait database
	docker compose exec -T database sh -c 'exec $$(command -v mariadb || command -v mysql) -uroot -psymfony symfony' < fixtures/dev_recipes.sql
.PHONY: seed

IMAGE ?= rusted-recipes
TAG ?= latest

image:
	docker build -t $(IMAGE):$(TAG) .
.PHONY: image
