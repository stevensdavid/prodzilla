# syntax=docker/dockerfile:1
#
# Preview-only image. Builds prodzilla the same way as the root Dockerfile,
# then layers in tailscaled so the machine can join a private tailnet at
# startup. Used exclusively by .github/workflows/pr-preview.yml; production
# images are still built from the root Dockerfile.

ARG RUST_VERSION=1.78

FROM rust:${RUST_VERSION}-slim-bookworm AS build
WORKDIR /app
RUN apt-get update -y && apt-get install -y libssl-dev pkg-config
COPY . .
RUN cargo build --locked --release --target-dir target \
    && cp ./target/release/prodzilla /bin/prodzilla

# Stage the config dir. The PR's prodzilla.yml is preferred; if missing,
# start.sh falls back to the sample.
RUN mkdir -p /out/etc/prodzilla \
    && cp deploy/preview/sample.prodzilla.yml /out/etc/prodzilla/sample.prodzilla.yml \
    && if [ -f prodzilla.yml ]; then cp prodzilla.yml /out/etc/prodzilla/prodzilla.yml; fi

FROM debian:bookworm-slim AS final

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        gnupg \
        libssl-dev \
    && curl -fsSL https://pkgs.tailscale.com/stable/debian/bookworm.noarmor.gpg \
        -o /usr/share/keyrings/tailscale-archive-keyring.gpg \
    && curl -fsSL https://pkgs.tailscale.com/stable/debian/bookworm.tailscale-keyring.list \
        -o /etc/apt/sources.list.d/tailscale.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends tailscale \
    && apt-get purge -y curl gnupg \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /bin/prodzilla /bin/prodzilla
COPY --from=build /out/etc/prodzilla /etc/prodzilla
COPY deploy/preview/start.sh /usr/local/bin/start.sh
RUN chmod +x /usr/local/bin/start.sh

EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/start.sh"]
