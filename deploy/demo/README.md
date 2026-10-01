# demo.attricat.com

The single-host evaluation deployment behind <https://demo.attricat.com>, reached
with `ssh attricat-demo`. PostgreSQL, RustFS and Mailpit live on this host; Mailpit
captures all outgoing mail. For production, use [`deploy/compose.yml`](../compose.yml)
and [`docs/operations.md`](../../docs/operations.md) instead.

## Fixed catalog and hourly reset

`reset/catalog.json` is the canonical demo fixture: **20 real KNIPEX hand tools**,
five categories and one manufacturer. It contains the literal product values,
blueprint definitions, table columns, relationships, navigation and publication
configuration. Product-family source URLs are recorded in the data. Resets do
not research products or download anything. Descriptions are original summaries;
there are no copied photographs, prices, inventory claims or implied manufacturer
affiliation. This is a fixture loader, not a solution pack or performance generator.

Every reset restores `owner@example.com` / `test`, workspace login identifier
`default.local`. **This is an administrator account, not restricted guest access.**
Do not reuse these credentials anywhere else. Guest restrictions, in-app notices
and a contact action from sales issue #1 remain separate work. Any public notice
must say **resets hourly**, not daily.

The loader refuses an existing catalog. It validates the JSON before writing,
loads through the HTTP API, and reads back every record, value, relationship,
blueprint definition, publication and navigation entry. Exact reproduction means
the same logical content; generated IDs, timestamps, password hashes and audit
history naturally differ. No fixtures are inserted directly into SQL.

## Host layout and prerequisites

Everything lives in `/opt/attricat-demo/`:

- `compose.yml`, `reset.sh`, `reset/`, `host/`, and `traefik/` from this directory;
- `.env` (mode 600), based on `.env.example`, with demo-only infrastructure secrets;
- `traefik/certs/origin.pem` and `origin.key`, a Cloudflare Origin CA certificate;
- `.reset-status.json`, the latest result (no credentials);
- `.reset-lock/` while a reset is running;
- `traefik/dynamic/maintenance.yml` while public access is closed.

Requires Bash, jq, Docker Engine, and Docker Compose v2.30+ (or v5). The host does
not need Node: the loader and maintenance responder use `node:24-alpine`.

Cloudflare SSL/TLS mode must be **Full (strict)**. Do not cache the demo's API,
HTML or maintenance responses at the edge. Traefik publishes IPv4 ports 80/443;
its file-provider routes use this deployment's Docker DNS, without Docker socket
access. `host/docker-user-firewall.sh` installed in `/usr/local/sbin/` and
`host/docker-user-firewall.service` in `/etc/systemd/system/` restrict new
connections to Cloudflare's ranges. UFW allows only SSH. Keep the ranges in that
script and `traefik/traefik.yml` in sync with <https://www.cloudflare.com/ips/>.

## Install/update the reset tooling

Copy the checked-in files without deleting `.env`, certificates, or runtime state:

```sh
rsync -av --exclude=.env --exclude=certs/ --exclude=compose.override.yml \
  --exclude=.reset-lock/ --exclude=.reset-status.json \
  --exclude=maintenance.yml --exclude=maintenance.tmp \
  deploy/demo/ attricat-demo:/opt/attricat-demo/
ssh attricat-demo
cd /opt/attricat-demo
chmod +x reset.sh
# Keep the existing POSTGRES_PASSWORD and S3_SECRET_ACCESS_KEY.
# Set ATTRICAT_OWNER_EMAIL=owner@example.com and ATTRICAT_OWNER_PASSWORD=test in .env.
chmod 600 .env
# Fetch prerequisites explicitly. reset.sh itself never pulls images.
docker compose --profile tools pull
```

The old `traefik/dynamic.yml` is replaced by `traefik/dynamic/tls.yml`; an old
copy on the server is no longer read. The first reset recreates the proxy with
its new mounts. Do not sync with `--delete`: that could remove the maintenance
marker while a failed reset is intentionally keeping the demo closed.

## Manual reset (destructive, no confirmation)

```sh
cd /opt/attricat-demo
./reset.sh
# Or from your laptop:
ssh attricat-demo 'cd /opt/attricat-demo && ./reset.sh'
```

`reset.sh /some/deployment` also works. There is no hostname/path allowlist. It
uses that directory's Compose configuration, including `compose.override.yml`,
and resolves the data volume names from Compose rather than hardcoding them.
Do not add public ports to the API or storage services: they would bypass the
maintenance route.

The workflow:

1. Acquire an atomic lock and preflight configuration, local images and fixture.
2. Retain the current API container's image ID (or the locally cached configured
   image for a first start). Reset is not an application upgrade.
3. Enable an all-path HTTP 503 maintenance response with `Retry-After` and
   `Cache-Control: no-store`; verify it through the origin proxy.
4. Stop/remove application, worker, migration, storage and mail containers.
5. Remove only the Compose-managed `postgres-data` and `rustfs-data` volumes.
6. Recreate storage and the bucket, run migrations, bootstrap the owner, and
   wait for API/worker readiness.
7. Load and fully verify the catalog over the private Docker network. The loader
   logs out its temporary session; no PAT or persistent session file is needed.
8. Reopen the proxy and verify readiness through it.

Traefik is restarted when changing modes so this also works on bind mounts that
lack filesystem notifications. Expect a brief connection interruption at those
transitions. In-flight old requests finish or are terminated before the database
is removed; no public writes reach the replacement while it is being seeded.

Application data, uploaded objects, users, credentials, sessions, tokens,
configuration and captured mail are discarded. `.env`, infrastructure secrets,
TLS certificates, tooling and host backups are preserved. External Compose
volumes are not supported by this reset workflow.

## Schedule: every HH:00 UTC

After a successful manual reset:

```sh
sudo install -m 644 host/attricat-demo-reset.{service,timer} /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now attricat-demo-reset.timer
systemctl list-timers attricat-demo-reset.timer
```

The timer uses `OnCalendar=*-*-* *:00:00 UTC`, no randomized delay, and one-second
accuracy. `Persistent=true` runs one catch-up reset if an hourly run was missed
while the VPS was off. It does not queue every missed hour. The service times out
after 20 minutes; overlapping manual/timer invocations are refused by the lock.

Use `systemctl start attricat-demo-reset.service` for an operator reset recorded
in the same journal. Disable the timer during deployment changes:

```sh
sudo systemctl stop attricat-demo-reset.timer
# Deploy/update and perform a verified manual reset.
sudo systemctl start attricat-demo-reset.timer
```

Reset deliberately keeps the running application image even if `:latest` was
pulled. For an application upgrade, stop the timer, explicitly update/recreate
the app with `docker compose pull && docker compose up -d`, then reset and verify
before restarting the timer. This is an evaluation environment, not a zero-downtime
upgrade procedure.

## Failures, notification and retention

```sh
journalctl -u attricat-demo-reset.service --since today
jq . .reset-status.json
docker compose ps -a
```

Failures after maintenance begins leave the maintenance marker in place and stop
the API and worker. Fix the cause and run the same reset again; do not manually
reopen a partially populated catalog. Preflight failure does not stop an otherwise
healthy demo. There is no automatic restore of visitor data.

Configure an executable operator notification hook in `/etc/attricat-demo-reset.env`:

```sh
DEMO_RESET_FAILURE_HOOK=/usr/local/sbin/notify-attricat-demo-reset
```

The hook receives the deployment directory and failed phase as two arguments.
Use your existing alert transport and store its credentials outside this repo.
It should return promptly and nonzero if delivery fails. **There is no external
alert destination configured by default.** Monitor the systemd unit too: a host
outage or SIGKILL cannot run the script's hook. The integration test exercises
hook invocation, not delivery to a particular operator.

A hard kill can leave `.reset-lock/` behind. First stop the timer and confirm no
reset process or one-off loader container is still running (`docker compose ps
-a`, `ps aux`). Remove the stale lock only after that check, then rerun reset and
restart the timer. Never remove the maintenance marker as a recovery shortcut.

The host is backed up as a whole VPS. An hourly reset **does not erase existing
backups or host logs**. Define VPS snapshot retention with the hosting provider;
stop the stack before manual snapshots. Keep reset/service logs bounded with the
host's journald retention policy, and configure Docker log rotation (for example,
the `local` log driver with `max-size=10m`, `max-file=3`). Those are host-wide
operational settings, not silently changed by this script. Reset logs contain
phase, image ID, fixture hash, counts and errors; avoid logging visitor content
or authentication headers in alerts. Review API/proxy log and snapshot retention
before advertising any stronger data-deletion promise.

## Change the fixture / test locally

Edit only the checked-in JSON to change the catalog. Keep stable record keys,
valid relationship references, ordered blueprint dependencies, and at most 100
products. All records have explicit values; there are no random or clock-based
defaults. Update source URLs when changing factual product specifications. This
initial fixture has no binary assets.

```sh
node deploy/demo/reset/load.mjs validate
node --test deploy/demo/reset/load.test.mjs
# Requires Node 24+, Docker, jq, Bash, openssl and curl.
node deploy/demo/reset/test-reset.mjs
```

The integration test creates a uniquely named local deployment under
`~/.cache/attricat-reset-e2e-*`, using the locally cached
`ghcr.io/attricat/attricat:latest` (override with `TEST_ATTRICAT_IMAGE`). Missing
prerequisite images are pulled. It uses loopback-only TLS on an allocated port,
self-signed test certificates, and amd64 emulation on ARM hosts. It never connects
to the VPS. It tests complete restoration, mutations/deletions/extra records,
old session/PAT invalidation, object/mail removal, public write blocking,
overlap refusal, load failure, notification and recovery. Containers/volumes are
removed afterward; failed-run files remain for diagnosis.

To verify an already populated deployment without resetting:

```sh
cd /opt/attricat-demo
docker compose run --rm --no-deps loader verify
```

Loading without a reset (`loader load`) requires an empty workspace and the owner
credentials configured in `.env`. There is intentionally no merge/resume mode for
this tiny fixture: after partial failure, reset and load it afresh.
