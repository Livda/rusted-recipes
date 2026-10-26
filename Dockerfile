FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY templates templates
COPY migrations migrations
RUN cargo build --release

FROM debian:bookworm-slim
RUN useradd --system --create-home app
WORKDIR /app
COPY --from=build /app/target/release/rusted-recipes /usr/local/bin/rusted-recipes
COPY static static
USER app
ENV BIND_ADDR=0.0.0.0:8000
EXPOSE 8000
ENTRYPOINT ["rusted-recipes"]
CMD ["serve"]
