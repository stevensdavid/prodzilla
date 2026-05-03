#!/bin/sh
set -eu

SOCK=/tmp/tailscaled.sock

tailscaled \
    --tun=userspace-networking \
    --state=mem: \
    --socket="$SOCK" &

i=0
until [ -S "$SOCK" ]; do
    i=$((i + 1))
    if [ $i -gt 50 ]; then
        echo "tailscaled socket did not appear within 10s" >&2
        exit 1
    fi
    sleep 0.2
done

echo "TS_AUTHKEY len=${#TS_AUTHKEY} prefix=$(printf '%.15s' "$TS_AUTHKEY")" >&2

tailscale --socket="$SOCK" up \
    --auth-key="$TS_AUTHKEY" \
    --hostname="$FLY_APP_NAME" \
    --advertise-tags=tag:prodzilla-preview \
    --accept-dns=false

tailscale --socket="$SOCK" serve --bg --https=443 http://localhost:3000

exec /bin/prodzilla
