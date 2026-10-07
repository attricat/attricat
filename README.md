<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="apps/catalog-web/design/assets/logos/wordmark-dark.svg">
    <img alt="Attricat" src="apps/catalog-web/design/assets/logos/wordmark-light.svg" width="242" height="48">
  </picture>
</p>

Attricat is a catalog for structured records whose shape changes over time.

You describe a kind of record with a **blueprint**, which lists its fields and
their types. Records built from a blueprint are **entities**. When you change a
blueprint, Attricat saves it as a new revision. Each entity keeps the exact
revision it was created with.

The project has three parts:

| Part | Path | What it is |
| --- | --- | --- |
| API | `apps/api` | Rust (Axum) server and SQLx database migrations |
| CLI | `apps/catalog-cli` | Command-line client for the API, with JSON output |
| Web app | `apps/catalog-web` | React and Vite interface |

## Self-hosting

### Try the published image

To try Attricat without building it, use
[`deploy/compose.quickstart.yml`](deploy/compose.quickstart.yml). It runs
`ghcr.io/attricat/attricat:latest` with PostgreSQL, RustFS and Mailpit. It
reuses those service definitions from `apps/api/compose.yml`, so run it from a
checkout of this repository:

```sh
export ATTRICAT_OWNER_PASSWORD='pick-a-password'
docker compose -f deploy/compose.quickstart.yml up -d --wait
```

Open <http://localhost:3000/login> and sign in with workspace `default.local`,
email `owner@example.com` and the password you exported. Email the server
sends, such as password resets, shows up in Mailpit at <http://localhost:8025>.

To use different host ports, set `ATTRICAT_PORT` or `MAILPIT_UI_PORT`. To run a
specific build, set `ATTRICAT_IMAGE`, for example to
`ghcr.io/attricat/attricat:<commit-sha>`. Run
`docker compose -f deploy/compose.quickstart.yml pull` to update to the newest
image. `down` stops the stack and `down -v` also deletes its data. The image is
built for amd64 only, so Apple Silicon and other arm64 machines run it under
emulation.

This setup is for local evaluation. It binds to localhost, uses plain HTTP and
has fixed internal credentials. To deploy for real, see
[Deploy to production](#deploy-to-production).

### Deploy to production

Attricat ships as one container image with three commands:

- `migrate` applies database migrations
- `api` runs the API and serves the web app
- `file-worker` generates file variants and deletes stored files

Every push to `main` publishes `ghcr.io/attricat/attricat:<commit-sha>`. Pin
deployments to a digest, not `:latest`.

You provide PostgreSQL, private S3-compatible storage, SMTP and monitoring.
Start from [`deploy/compose.yml`](deploy/compose.yml). It binds the API to
`127.0.0.1:3000`, so put a TLS-terminating reverse proxy in front of it.
[Configuration](docs/configuration.md) lists every setting.

## Conversational agents (optional)

Users can ask an LLM to read and change the catalog.

To turn it on, set `LLM_API_KEY` in the API's environment, and `LLM_BASE_URL`
or `LLM_MODEL` if you need them. Restart the API, then open **Conversations**
in the web app. Without a key, agents are off and everything else works as
usual.

Before you turn it on:

- Conversation text and tool results go to the configured provider. The
  browser never sees the provider key.
- Read-only tools run on their own. Every change to the catalog, including
  changes from scheduled runs, waits for a person to approve it. Review the
  proposed arguments and change summary before you approve.
- An approved scheduled run writes with the permissions of the user who started
  it.
- Use a provider account with no more access than it needs.
- Give the `agents.run` permission only to people who are allowed to request
  catalog changes.

See [agent configuration](docs/configuration.md#agent-provider) and the
[agent API](docs/api.md#agents) for limits and details.

## Documentation

- [Documentation index](docs/index.md)
- [Writing blueprints](docs/blueprints.md)
- [Relationships walkthrough](examples/relationships/README.md)
- [CLI](docs/cli.md)
- [API reference](docs/api.md)
- [Configuration](docs/configuration.md)
- [File storage and retention](docs/configuration.md#file-storage-operations)
- [Accounts](docs/authentication.md)

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) explains how to run Attricat from source
and the rules for UI and database changes.

## License

The code is licensed under the [GNU Affero General Public License, version
3](LICENSE) (`AGPL-3.0-only`).

The design assets in `apps/catalog-web/design/` come from the separate
[Attricat design repository](https://github.com/attricat/design) and follow its
license. Third-party dependencies keep their own licenses.
