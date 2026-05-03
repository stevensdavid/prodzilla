# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.85

# Build frontend
FROM node:20-slim AS frontend-build
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN --mount=type=cache,target=/root/.npm npm ci
COPY frontend/ ./
RUN npm run build

# Rust build base with cargo-chef preinstalled
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Generate the dependency recipe. Only Cargo.toml/Cargo.lock content
# (via recipe.json) determines the cook cache key downstream.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Cook deps, then build the binary. Keep target/ in the layer (so the cooked
# deps live in a cacheable Docker layer) and use BuildKit cache mounts only
# for cargo's registry/git, which speeds up cold-cache fetches.
FROM chef AS build
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

COPY . .
COPY --from=frontend-build /app/frontend/dist ./frontend/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --locked --release --bin prodzilla \
    && cp ./target/release/prodzilla /bin/prodzilla

FROM debian:bookworm-slim AS final

RUN apt-get update && apt-get install -y libssl-dev ca-certificates
# Create a non-privileged user that the app will run under.
# See https://docs.docker.com/go/dockerfile-user-best-practices/
ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

# Copy the executable and frontend assets from the build stages.
COPY --from=build /bin/prodzilla /bin/
COPY --from=frontend-build /app/frontend/dist /frontend/dist

# Expose the port that the application listens on.
EXPOSE 3000

# What the container should run when it is started.
ENTRYPOINT ["/bin/prodzilla"]
