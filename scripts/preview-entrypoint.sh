#!/bin/sh
set -eu

SOCK=/tmp/tailscaled.sock

tailscaled \
    --tun=userspace-networking \
    --state=mem: \
    --socket="$SOCK" &

i=0
until tailscale --socket="$SOCK" status >/dev/null 2>&1; do
    i=$((i + 1))
    if [ $i -gt 50 ]; then
        echo "tailscaled did not become ready within 10s" >&2
        exit 1
    fi
    sleep 0.2
done

tailscale --socket="$SOCK" up \
    --auth-key="$TS_AUTHKEY" \
    --hostname="$FLY_APP_NAME" \
    --advertise-tags=tag:prodzilla-preview \
    --accept-dns=false

tailscale --socket="$SOCK" serve --bg --https=443 http://localhost:3000

exec /bin/prodzilla
