# syntax=docker/dockerfile:1
#
# Layered on top of the per-PR prod image. PR_NUMBER is supplied by the
# preview deploy workflow and selects the GHCR tag built by pr-image.yaml.
ARG PR_NUMBER
FROM ghcr.io/stevensdavid/prodzilla:pr-${PR_NUMBER}

USER root

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        curl \
        gnupg \
        ca-certificates \
    && curl -fsSL https://pkgs.tailscale.com/stable/debian/bookworm.noarmor.gpg \
        -o /usr/share/keyrings/tailscale-archive-keyring.gpg \
    && curl -fsSL https://pkgs.tailscale.com/stable/debian/bookworm.tailscale-keyring.list \
        -o /etc/apt/sources.list.d/tailscale.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends tailscale \
    && apt-get purge -y curl gnupg \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

COPY scripts/preview-entrypoint.sh /usr/local/bin/preview-entrypoint.sh
RUN chmod +x /usr/local/bin/preview-entrypoint.sh

USER appuser

ENTRYPOINT ["/usr/local/bin/preview-entrypoint.sh"]
