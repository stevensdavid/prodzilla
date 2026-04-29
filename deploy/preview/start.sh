#!/bin/sh
# Entrypoint for preview-image containers running on Fly.io.
# Brings up tailscaled in userspace networking mode (no /dev/net/tun needed
# on a Fly machine), joins the tailnet with an ephemeral auth key, then
# exec's prodzilla. tailscale serve exposes the local API on the tailnet
# hostname over HTTPS so mobile clients reach it at
# https://$FLY_APP_NAME.<tailnet>.ts.net/.
set -eu

: "${TS_AUTHKEY:?TS_AUTHKEY must be set as a Fly secret}"
: "${FLY_APP_NAME:?FLY_APP_NAME is normally injected by Fly}"

CONFIG="${PRODZILLA_CONFIG:-/etc/prodzilla/prodzilla.yml}"
if [ ! -f "$CONFIG" ]; then
    echo "config $CONFIG not found, falling back to sample"
    CONFIG=/etc/prodzilla/sample.prodzilla.yml
fi

mkdir -p /var/run/tailscale /var/lib/tailscale

/usr/sbin/tailscaled \
    --state=/var/lib/tailscale/tailscaled.state \
    --socket=/var/run/tailscale/tailscaled.sock \
    --tun=userspace-networking \
    --socks5-server=localhost:1055 &

# Wait for the local control socket so `tailscale up` can talk to it.
i=0
until /usr/bin/tailscale --socket=/var/run/tailscale/tailscaled.sock status >/dev/null 2>&1; do
    i=$((i + 1))
    if [ "$i" -gt 60 ]; then
        echo "tailscaled did not become ready within 30s" >&2
        exit 1
    fi
    sleep 0.5
done

/usr/bin/tailscale --socket=/var/run/tailscale/tailscaled.sock up \
    --authkey="$TS_AUTHKEY" \
    --hostname="$FLY_APP_NAME" \
    --accept-dns=false

/usr/bin/tailscale --socket=/var/run/tailscale/tailscaled.sock serve \
    --bg --https=443 http://127.0.0.1:3000

exec /bin/prodzilla -f "$CONFIG"
