---
title: CLI reference
description: The acli command-line client, its configuration, authentication, input files, and full command list.
---

`acli` is a command-line client for the Attricat API, built for scripts, CI jobs, and agents. On success it prints the API's JSON response unchanged to standard output. Errors go to standard error as JSON. Every command has built-in help: `acli --help` and `acli <group> --help`.

`acli` is included in the container image:

```sh
docker run --rm -e CATALOG_SERVER -e CATALOG_TOKEN ghcr.io/attricat/attricat@sha256:… acli health
```

## Configuration

| Setting | Flag | Environment | Description |
| --- | --- | --- | --- |
| Server URL | `--server` | `CATALOG_SERVER`, then `CATALOG_API_URL` | API address. Defaults to `http://127.0.0.1:3000`. |
| Token | `--token`, `--token-stdin` | `CATALOG_TOKEN` | Personal API token. |
| Session file | `--session-file` | `CATALOG_SESSION_FILE` | Stores a browser session between commands. |
| Web URL | | `CATALOG_WEB_URL` | When set, saved-search commands also print a short Explorer link. |
| Skip `.env` | `--no-env` | | Do not read `.env` from the current directory. |

By default, `acli` reads `.env` from the current directory without overriding variables already set in the shell.

Credentials are only sent over HTTPS, or to a loopback address for local development.

## Authenticate

**With a token** (recommended for automation). Create a personal API token in the web app under **Profile → Personal API tokens**, then:

```sh
export CATALOG_TOKEN=cat_pat_…
acli blueprint list
```

Prefer `CATALOG_TOKEN` or `--token-stdin` over `--token`, which leaves the secret in your shell history and process list.

**With a password session.** Sign in once and keep the session in a file:

```sh
acli --session-file ~/.acli-session auth discover acme.example
printf '%s' "$PASSWORD" | acli --session-file ~/.acli-session auth login acme.example --email you@acme.example --password-stdin
acli --session-file ~/.acli-session blueprint list
```

The session file holds only the session and CSRF cookies, is created with mode `0600`, and is deleted on `auth logout`. Don't commit or share it. A token, when given, always takes precedence over a session file.

## Output and exit codes

Commands that download files, artifacts, or metrics require `--output <path>` and print `null` on success, so standard output is always JSON.

| Exit code | Meaning |
| --- | --- |
| `0` | Success. |
| `2` | Invalid input or configuration. |
| `3` | Could not reach the server. |
| `4` | The API returned an error. |
| `5` | The API returned invalid JSON. |

```json
{ "error": { "code": "invalid_blueprint_definition", "message": "blueprints must define at least one attribute", "status": 422 } }
```

## JSON arguments

Options that take structured data (`--permissions`, `--entries`, `--filters`, `--relationship-tree-facets`, `--relationships`, `--selected-target-ids`, `--values`, `--discard-attributes`, `--configuration`, `--body`, `--payload`, `--input`, `--state`) accept either inline JSON or a path to a JSON file.

Blueprint, workflow, and rule commands take `--file <path>` or `--stdin` and send the TOML unchanged.

## Value files

`entity create --values`, `entity update --values`, and `value append --file` read TOML:

```toml
[[values]]
kind = "scalar"
attribute_code = "title"
value = "Linen shirt"

[[values]]
kind = "scalar"
attribute_code = "order_cutoff"
value = { time = "09:30:00", time_zone = "Europe/Warsaw" }

[[values]]
kind = "relationship"
attribute_code = "categories"
target_entity_id = "00000000-0000-0000-0000-000000000004"
```

Each entry names the attribute with `attribute_code` or `attribute_id`. Scalars use TOML's native types: strings, decimals, integers, booleans, dates, and RFC 3339 datetimes. Every value needs a context: pass `--context-id` for the whole file or set `context_id` on an entry.

Relationship replacement and removal files (`value replace`, `value remove`, `entity update --relationships`):

```toml
[[relationships]]
attribute_code = "categories"
target_entity_ids = ["00000000-0000-0000-0000-000000000004"]
```

`replace` makes the list the complete set; `remove` unlinks only the listed targets.

Removing context overrides (`entity update --remove-values`):

```toml
[[remove_values]]
attribute_code = "subtitle"
```

Context files (`context create --file`):

```toml
code = "PL"
parent_id = "00000000-0000-4000-8000-000000000001"  # omit for a top-level context

[data]
language = "pl"
```

## Commands

### Health and authentication

```sh
acli health
acli auth discover <workspace-identifier>
acli auth login <workspace-identifier> --email <email> --password-stdin
acli auth session | renew | logout
acli auth password-reset --email <email>
acli auth password-reset-confirm --token-stdin --password-stdin
acli token list
acli token create --label <label> --permissions <json> [--expires-at <rfc3339>]
acli token revoke <token-id>
```

### Blueprints

```sh
acli blueprint list [--include-drafts]
acli blueprint catalogue
acli blueprint create --file product.toml
acli blueprint revision <blueprint-id> --file product-v2.toml
acli blueprint revision-list <blueprint-id>
acli blueprint publish <blueprint-id> <version>
acli blueprint get <blueprint-id>
acli blueprint get-version <blueprint-id> <version>
acli blueprint resolve <code> [--version <version>] [--include-drafts]
acli blueprint safe-migration-batch <blueprint-id> <version>
acli blueprint publish-entities <blueprint-id> <version> --context-id <channel>
acli blueprint publish-entities-all <blueprint-id> <version>
```

### Contexts

```sh
acli context list
acli context get <code>
acli context create --code <code> [--parent-id <id>] [--data <json>]
acli context create --file context.toml
acli context update <context-id> --parent-id <id> --data <json>
acli context delete <context-id>
```

### Entities and values

```sh
acli entity create --blueprint <code> --values values.toml [--context-id <id>] [--system-tags <json>] [--system-metadata <json>]
acli entity get | form | delete <entity-id>
acli entity update <entity-id> [--values f.toml] [--relationships f.toml] [--remove-values f.toml] [--context-id <id>]
acli entity preview <entity-id> [--relationship-depth <n>] [--relationship-limit <n>]
acli entity resolved-preview <entity-id> --context-id <id>
acli entity search --blueprint <code> [--query <text>] [--filters <json>] [--version <n>] [--sort-field <f> --sort-direction asc|desc] [--size <n>] [--cursor <c>] [--include-total] [--outdated] [--system-tags <json>] [--relationship-tree-facets <json>]
acli entity facet-children --blueprint <code> --source-relationship-field <f> --context-id <id> [--hierarchy-field <f>] [--parent-id <id>]
acli entity list --blueprint <code> --related-from <entity-id> --relationship <attribute>
acli entity incoming-relationships <entity-id> --relationships <json>
acli entity hierarchy <entity-id> --context-id <id> --field <relationship>
acli entity changes | value-history <entity-id>
acli entity restore-value <entity-id> <history-id>
acli entity migrate <entity-id>
acli entity migrate-bulk --blueprint <code> --from-version <n> [--dry-run]
acli entity publication list <entity-id>
acli entity publication publish | unpublish <entity-id> --context-id <channel>
acli entity publication publish-all <entity-id>

acli value current <entity-id>
acli value append <entity-id> --file values.toml --context-id <id>
acli value replace | remove <entity-id> --file relationships.toml --context-id <id>
```

### Files

```sh
acli file upload <entity-id> <attribute-code> --file a.png [--file b.png] [--context-id <id>]
acli file metadata <file-id>
acli file download-original <file-id> --output original.bin [--range 'bytes=0-1023']
acli file download-variant <file-id> thumbnail|display --output preview.webp
```

### Saved searches

```sh
acli saved-view list [--query <text>]
acli saved-view get <id>
acli saved-view create --name <name> --state <json> [--visibility private|workspace] [--description <text>]
acli saved-view update <id> --name <name> --state <json>
acli saved-view delete <id>
acli saved-view link --state <json>
```

The state needs at least a blueprint code, for example `{"blueprint":"product","query":"linen"}`.

### Translations

```sh
acli lexicon list [--language <tag>]
acli lexicon set --key <text> --language <tag> --text <text> [--context <text>] [--plural-category <category>]
acli lexicon delete --key <text> --language <tag> [--context <text>] [--plural-category <category>]
acli lexicon export --language <tag>
acli lexicon import --file <lexicon.json|lexicon.toml> [--replace]
acli lexicon report [--language <tag>]...
```

`--plural-category` is `zero`, `one`, `two`, `few`, `many`, or `other` (the default) and must be used by the language. `export` prints a file that `import` accepts; `--replace` deletes the language's entries that the file does not contain. Repeat `--language` to report several languages; by default the report covers English and every language with entries. Changes require `blueprints.write`. See [Translate labels](/builders/translations/).

### Rules and workflows

```sh
acli rule list [--blueprint-id <id>]
acli rule validate | create --blueprint-id <id> --blueprint-version <n> [--context-id <id>] --file rule.toml
acli rule revision <rule-id> --blueprint-id <id> --blueprint-version <n> --file rule.toml
acli rule publish | enable <rule-id> <version>
acli rule disable <rule-id>
acli rule run-now <rule-id> --idempotency-key <key> [--entity-id <id>] [--dry-run]
acli rule run-list
acli rule run-replay <run-id>
acli rule findings [--entity-id <id>]
acli rule acknowledge <finding-id>

acli workflow validate | create --file workflow.toml
acli workflow list | get <workflow-id> | revision-list <workflow-id>
acli workflow revision <workflow-id> --file workflow.toml
acli workflow publish | enable <workflow-id> <version>
acli workflow disable <workflow-id>
acli workflow run-now <workflow-id> --entity-id <id> --idempotency-key <key>
acli workflow run-list
acli workflow run-targets <run-id>
acli workflow run-replay <run-id>
```

### Workspace

```sh
acli workspace member list
acli workspace member set-state <member-id> --state active|inactive
acli workspace member grant <member-id> --role-id <id> --scope-type <scope> --scope-target-id <id>
acli workspace member revoke-grant <member-id> <grant-id>
acli workspace member transfer-ownership <member-id>
acli workspace role list | permission list | assignable-role list
acli workspace role create --code <code> --permissions <json>
acli workspace role update <role-id> --code <code> --permissions <json>
acli workspace role duplicate <role-id> --code <code>
acli workspace role retire <role-id> [--replacement-role-id <id>]
acli workspace invitation list
acli workspace invitation create --email <email> --role-id <id> --scope-type <scope> --scope-target-id <id> --expires-at <rfc3339>
acli workspace invitation revoke <invitation-id>
acli workspace invitation accept --secret-stdin
acli workspace user create --email <email> [--display-name <name>]
acli workspace navigation get | sidebar
acli workspace navigation set --entries <json>
acli workspace publication-channel list
acli workspace publication-channel set <context-id> --enabled true|false
acli workspace grant-target list <scope-type>
acli workspace token-permission list
acli directory
acli team list
acli team create --code <code> --name <name> [--member <user-id> …]
acli team update <team-id> [--name <name>] [--member <user-id> … | --clear-members]
acli team delete <team-id>
acli audit list [--occurred-after <t>] [--occurred-before <t>] [--actor-user-id <id>] [--action-category <c>] [--target-type <t>] [--executor-type human|agent]
```

### Extensions

```sh
acli extension-registry list | discover
acli extension-registry add --source <owner/repository>
acli extension-registry remove <registry-id>
acli extension-registry details <owner> <repository>
acli extension list | runtime
acli extension detail | enable | disable | remove <extension-id>
acli extension install --owner <o> --repository <r> --release-id <github-release-id>
acli extension sideload --file extension.tar.zst
acli extension upgrade <extension-id> --owner <o> --repository <r> --release-id <id>
acli extension configure <extension-id> --configuration <json>
acli extension grant <extension-id> --grant-kind capability|host_permission|event_publish|event_subscribe --grant-id <id>
acli extension revoke <extension-id> <grant-kind> <grant-id>
acli extension quarantine <extension-id> --diagnostic-code <code>
acli extension workspace-mode --enabled true|false
acli extension-operation start <extension-id> --operation-id <id> --input <json> --idempotency-key <key> [--input-file-id <id>]
acli extension-operation list
acli extension-operation show | artifacts | deliveries | cancel | replay <run-id>
acli extension-operation download <run-id> <artifact-id> --output <path>
acli extension-run list [--extension-id <id>]
acli extension-run show | cancel <run-id>
acli extension-run download <run-id> <artifact-id> --output <path>
acli extension annotation-namespace <extension-id> [--adopt]
acli extension repair-annotations <extension-id> <entity-id> --patch <json>
acli extension-schedule list
acli extension-schedule create <extension-id> --operation-id <id> --input <json> --interval-seconds <60-2592000>
acli extension-schedule update <schedule-id> --enabled true|false --interval-seconds <n>
acli connector-job list <blueprint-id>
acli connector-job run <job-id> --idempotency-key <key>
```

### Solution packs

```sh
acli solution-pack inspect --file pack.tar.zst
acli solution-pack plan --file pack.tar.zst --prefix <prefix> --blueprint-publication draft|publish [--include-sample-data] [--map key=code] [--map-asset key=uuid] [--from-application <id>]
acli solution-pack plan show <plan-id>
acli solution-pack apply <plan-id>
acli solution-pack applications list | show <id> | abandon <id>
acli solution-pack checks list | rerun <application-id>
acli solution-pack checks show <application-id> <run-id>
acli presentation-asset list | show <id>
acli presentation-asset download <id> --output <path>
```

### Operations

```sh
acli data-health summary | blueprints [--stale-after-days <n>]
acli data-health freshness | completeness | contexts | relationships | storage | background-processing
acli data-health refresh
acli metrics get --output metrics.prom
acli event dead-letters
acli event replay <consumer-id> <event-id>
```
