# PR preview environments

Per-PR previews of prodzilla deployed to Fly.io and reachable only on a
private Tailscale tailnet, so the unauthenticated API is never exposed to
the public internet.

The workflow at `.github/workflows/pr-preview.yml` builds the image in
`deploy/preview/Dockerfile`, deploys it as a per-PR Fly app
(`prodzilla-pr-<N>`), and tears it down when the PR is closed.

## Cost model

A 256 MB `shared-cpu-1x` machine runs ~$2/mo while up. Because traffic
arrives over WireGuard rather than fly-proxy, Fly's auto-stop feature
doesn't apply — the machine runs continuously while the PR is open. The
destroy job on PR close bounds total spend: open PR → meter ticks; closed
PR → machine destroyed and billing stops. Two simultaneously-open PRs
≈ $4/mo, well inside the stated $20/mo budget.

## One-time setup

### 1. Fly.io

```sh
flyctl auth signup    # or: flyctl auth login
flyctl orgs list      # note the slug you want previews to live under
```

Create a deploy token scoped to the preview-app naming pattern:

```sh
flyctl tokens create deploy --name pr-preview --expiry 8760h
```

(You can additionally restrict the token to apps matching `prodzilla-pr-*`
via the Fly web UI under Tokens → Deploy tokens.)

### 2. Tailscale

In <https://login.tailscale.com/admin/settings/keys>, generate an auth key
with these properties:

- **Reusable**: yes
- **Ephemeral**: yes (so dead preview machines drop off the tailnet)
- **Pre-approved**: yes
- **Tags**: `tag:pr-preview`

Then update your tailnet ACLs (`Access Controls`) so:

- `tag:pr-preview` is owned by your user account.
- Your devices can reach `tag:pr-preview` on port 443.

A minimal ACL fragment:

```jsonc
{
  "tagOwners": {
    "tag:pr-preview": ["autogroup:owner"]
  },
  "acls": [
    {
      "action": "accept",
      "src": ["autogroup:owner"],
      "dst": ["tag:pr-preview:443"]
    }
  ]
}
```

In **DNS**, enable **MagicDNS** and **HTTPS Certificates**. Tailscale
Serve provisions per-device HTTPS certs from these settings; without them,
`tailscale serve --https=443` in the start-up script will fail.

Note your tailnet name (e.g. `tail1234.ts.net` → `tail1234`); you'll set
it as a repo variable below.

### 3. GitHub repo configuration

In **Settings → Environments**, create an environment named `pr-preview`.
Optionally add yourself as a **required reviewer** so every deploy pauses
for a one-click approval before secrets release. Remove the reviewer
later if the friction becomes annoying — the same-repo head filter in the
workflow already blocks fork PRs.

In **Settings → Secrets and variables → Actions**, add:

| Type     | Name            | Value                                            |
| -------- | --------------- | ------------------------------------------------ |
| Secret   | `FLY_API_TOKEN` | the deploy token from step 1                     |
| Secret   | `TS_AUTHKEY`    | the Tailscale auth key from step 2               |
| Variable | `FLY_ORG`       | the Fly org slug (e.g. `personal`)               |
| Variable | `FLY_REGION`    | a Fly region code (e.g. `arn`, `iad`, `fra`)     |
| Variable | `TAILNET_NAME`  | the prefix of your tailnet (e.g. `tail1234`)     |

### 4. Mobile

Install the Tailscale app on your phone (iOS / Android) and sign in to
the same tailnet. Once a preview is deployed, open
`https://prodzilla-pr-<N>.<TAILNET_NAME>.ts.net/` in mobile Safari /
Chrome — Tailscale terminates HTTPS with a per-device cert.

## How a PR flows through the preview

1. PR opened → `deploy` job runs, waits at the environment gate (if a
   reviewer is configured), then builds `deploy/preview/Dockerfile`,
   creates Fly app `prodzilla-pr-<N>`, sets `TS_AUTHKEY` as a Fly secret,
   and deploys.
2. Bot comments on the PR with the preview URL.
3. New push → `synchronize` event → workflow redeploys the same Fly app
   (concurrency cancels any in-flight deploy for the PR).
4. PR closed/merged → `destroy` job runs and removes the Fly app. The
   ephemeral Tailscale device deregisters automatically.

## Troubleshooting

- `flyctl logs --app prodzilla-pr-<N>` shows tailscaled and prodzilla
  output side-by-side.
- If `tailscale up` fails with "auth key expired", regenerate the auth
  key (step 2) and update the GitHub secret.
- If the machine never wakes on a request, check that the tailnet ACL
  permits port 443 from your devices, and that `tailscale serve` ran
  successfully in the start-up logs.
- Cost can be inspected with `flyctl billing show`. A running 256 MB
  shared-cpu-1x machine is ~$2/mo; close stale PRs to stop the meter.

## Limitations

- **Fork PRs are not deployed.** GitHub withholds secrets from fork-PR
  runs of `pull_request` workflows, and the workflow filters them out
  explicitly. To preview a fork's changes, push the branch into this
  repo first (e.g. `git push origin <fork-author>/<branch>:review/<topic>`).
- **State is in-memory.** Restarting (or auto-stopping then resuming) a
  preview clears probe history. This matches prodzilla's design — see
  `src/app_state.rs`.
- **No persistent storage.** If a future feature needs a Fly volume,
  add `[mounts]` to `fly.toml.template` and provision a volume in the
  workflow.
