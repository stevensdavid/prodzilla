# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.78

FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-alpine AS build

WORKDIR /app

RUN apk add --no-cache musl-dev libressl-dev

COPY . .
RUN cargo build --locked --release --target-dir target && cp ./target/release/prodzilla /bin/prodzilla

FROM alpine:3.18 AS final

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

# Copy the executable from the "build" stage.
COPY --from=build /bin/prodzilla /bin/

# Expose the port that the application listens on.
EXPOSE 3000

# What the container should run when it is started.
ENTRYPOINT ["/bin/prodzilla"]
