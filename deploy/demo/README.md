# demo.attricat.com

The single-host deployment behind <https://demo.attricat.com>. It is an
evaluation demo, not a production template: it tracks `:latest`, keeps
PostgreSQL, RustFS, and Mailpit on the same host, and catches all mail in
Mailpit. For production, use [`deploy/compose.yml`](../compose.yml) and
[`docs/operations.md`](../../docs/operations.md).

## Layout on the host

Everything lives in `/opt/attricat-demo/`:

- `compose.yml` and `traefik/*.yml` from this directory;
- `.env` (mode 600) created from `.env.example` with generated secrets;
- `traefik/certs/origin.pem` and `origin.key`, a Cloudflare Origin CA
  certificate for `*.attricat.com`. Cloudflare's SSL/TLS mode must be
  **Full (strict)**.

Traefik publishes 80 and 443 on IPv4 only and routes only the API container.
`host/docker-user-firewall.sh` installed as `/usr/local/sbin/` with
`host/docker-user-firewall.service` in `/etc/systemd/system/` limits new
connections to those ports to Cloudflare's ranges. UFW allows only SSH. Keep the
Cloudflare ranges in the firewall script and `traefik/traefik.yml` in sync with
<https://www.cloudflare.com/ips/>.

## Operating it

```sh
cd /opt/attricat-demo
docker compose pull && docker compose up -d   # update; migrate runs first
docker compose ps
ssh -L 8025:127.0.0.1:8025 <host>             # Mailpit UI at localhost:8025
```

The host is backed up as a whole VPS; stop the stack before a manual snapshot
for a clean PostgreSQL state.
