# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.85

# Build frontend
FROM node:20-slim AS frontend-build
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# Build Rust binary
FROM rust:${RUST_VERSION}-slim-bookworm AS build

WORKDIR /app

RUN apt-get update -y && apt-get install -y libssl-dev pkg-config

COPY . .
COPY --from=frontend-build /app/frontend/dist ./frontend/dist
RUN cargo build --locked --release --target-dir target && cp ./target/release/prodzilla /bin/prodzilla

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
