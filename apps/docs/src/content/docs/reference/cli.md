---
title: CLI reference
description: The acli command-line client, its configuration, authentication, input files, and full command list.
---

`acli` is a command-line client for the Attricat API, built for scripts, CI jobs, and agents. On success it prints the API's JSON response unchanged to standard output. Errors go to standard error as JSON. Every command has built-in help: `acli --help` and `acli <group> --help`.

## Install

Each [release](https://github.com/attricat/attricat/releases) publishes `acli` binaries for Linux, macOS and Windows with the same version as the server. Install the latest release on Linux or macOS:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/attricat/attricat/releases/latest/download/acli-installer.sh | sh
```

On Windows, in PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/attricat/attricat/releases/latest/download/acli-installer.ps1 | iex"
```

The installer puts `acli` in `~/.local/bin` (or `$XDG_BIN_HOME`). To install a specific version, replace `latest/download` with `download/v0.3.0`, or download an archive and its `.sha256` checksum from the release. Use the `acli` version that matches your server; `acli --version` prints it.

`acli` is also included in the container image:

```sh
docker run --rm -e ATTRICAT_API_URL -e ATTRICAT_TOKEN ghcr.io/attricat/attricat@sha256:… acli health
```

## Configuration

| Setting | Flag | Environment | Description |
| --- | --- | --- | --- |
| Server URL | `--server` | `ATTRICAT_API_URL` | API base URL, including `/api`. Defaults to `http://127.0.0.1:3000/api`. |
| Token | `--token`, `--token-stdin` | `ATTRICAT_TOKEN` | Personal API token. |
| Session file | `--session-file` | `ATTRICAT_SESSION_FILE` | Stores a browser session between commands. |
| Web URL | | `ATTRICAT_WEB_URL` | The web app's origin, such as `https://attricat.example.com`, with no path. When set, saved-search commands also print a short Explorer link. |
| Skip `.env` | `--no-env` | | Do not read `.env` from the current directory. |

By default, `acli` reads `.env` from the current directory without overriding variables already set in the shell.

Credentials are only sent over HTTPS, or to a loopback address for local development.

## Authenticate

**With a token** (recommended for automation). Create a personal API token in the web app under **Profile → Personal API tokens**, then:

```sh
export ATTRICAT_TOKEN=cat_pat_…
acli blueprint list
```

Prefer `ATTRICAT_TOKEN` or `--token-stdin` over `--token`, which leaves the secret in your shell history and process list.

`--token-stdin` reads all of standard input, so it cannot be combined with a command flag that also reads it, such as `--password-stdin`, `--value-stdin`, `--client-secret-stdin` or `--stdin`; `acli` rejects the combination with `invalid_input`. Use `ATTRICAT_TOKEN` for those commands.

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

When the API returns machine-readable `details` (for example check violations or a unique-key conflict), the error object includes them unchanged.

## JSON arguments

Options that take structured data (`--permissions`, `--entries`, `--filters`, `--relationship-tree-facets`, `--relationship-filters`, `--relationships`, `--files`, `--selected-target-ids`, `--include-ids`, `--values`, `--discard-attributes`, `--configuration`, `--body`, `--payload`, `--input`, `--state`, `--operations`) accept either inline JSON or a path to a JSON file.

Blueprint, workflow, and rule commands take `--file <path>` or `--stdin` and send the YAML unchanged.

## Value files

`record create --values`, `record update --values`, and `value append --file` read YAML:

```yaml
values:
  - kind: scalar
    attribute_code: title
    value: Linen shirt

  - kind: scalar
    attribute_code: order_cutoff
    value: {time: '09:30:00', time_zone: Europe/Warsaw}

  - kind: relationship
    attribute_code: categories
    target_record_id: '00000000-0000-0000-0000-000000000004'
```

Each entry names the attribute with `attribute_code` or `attribute_id`. Scalars use YAML's native types: strings, decimals, integers, booleans (`true` or `false`), dates, and RFC 3339 datetimes. Quote a string that YAML would otherwise read as another type, such as a UUID that starts with digits or `'09:30:00'`. Every value needs a context: pass `--context-id` for the whole file or set `context_id` on an entry.

Relationship replacement and removal files (`value replace`, `value remove`, `record update --relationships`):

```yaml
relationships:
  - attribute_code: categories
    target_record_ids: ['00000000-0000-0000-0000-000000000004']
```

`replace` makes the list the complete set; `remove` unlinks only the listed targets.

Removing context overrides (`record update --remove-values`):

```yaml
remove_values:
  - attribute_code: subtitle
```

Context files (`context create --file`):

```yaml
code: PL
parent_id: '00000000-0000-4000-8000-000000000001'  # omit for a top-level context

data:
  language: pl
```

## Commands

### Health and authentication

```sh
acli health [live | ready | system]
acli auth discover <workspace-identifier>
acli auth login <workspace-identifier> --email <email> --password-stdin
acli auth session | renew | logout
acli auth password-reset --email <email>
acli auth password-reset-confirm --reset-token-stdin --password-stdin
acli auth preferences --time-zone <iana-zone> | --clear-time-zone
acli auth display-name <name>
acli auth avatar set --file avatar.png
acli auth avatar remove
acli auth access --permission <code> --kind record|blueprint|context --id <id> [--id <id> ...]
acli auth sample-logins
acli token list
acli token create --label <label> --permissions <json> [--expires-at <rfc3339>]
acli token create --generator [--expires-at <rfc3339>]
acli token revoke <token-id>
```

### Blueprints

```sh
acli blueprint list [--include-drafts]
acli blueprint catalogue
acli blueprint create --file product.yaml
acli blueprint validate [--blueprint-id <id>] --file product.yaml
acli blueprint create-version <blueprint-id> --file product-v2.yaml
acli blueprint list-versions <blueprint-id>
acli blueprint publish <blueprint-id> <version>
acli blueprint get <blueprint-id>
acli blueprint get-version <blueprint-id> <version>
acli blueprint resolve <code> [--version <version>] [--include-drafts]
acli blueprint migration-impact <blueprint-id> <version>
acli blueprint safe-migration-batch <blueprint-id> <version> [--archive-removed-values]
acli blueprint migration-batches <blueprint-id>
acli blueprint sequences <blueprint-id>
acli blueprint advance-sequence <blueprint-id> <attribute-code> <next-value> [--period <period>]
acli blueprint publish-records <blueprint-id> <version> --context-id <channel>
acli blueprint publish-records-all <blueprint-id> <version>

acli reusable-attribute list [--include-drafts]
acli reusable-attribute filter-fields --blueprint <code>
acli reusable-attribute create --file gtin.yaml
acli reusable-attribute validate [--definition-id <definition-id>] --file gtin.yaml
acli reusable-attribute create-version <definition-id> --file gtin-v2.yaml
acli reusable-attribute publish <revision-id>
acli reusable-attribute apply <record-id> --revision-id <revision-id>
acli reusable-attribute group list
acli reusable-attribute group create --code <code> --name <name> [--position <n>] --revision-id <revision-id> [--revision-id <revision-id> ...]
acli reusable-attribute group apply <record-id> <group-id>
```

`safe-migration-batch` refuses a version that removes attributes still holding values; when `migration-impact` reports `requires_removal_disposition`, add `--archive-removed-values` to archive those values. `reusable-attribute filter-fields` lists the reusable attributes the Explorer offers as filters on a blueprint's records.

### Contexts

```sh
acli context list
acli context get <code>
acli context create --code <code> [--parent-id <id>] [--data <json>]
acli context create --file context.yaml
acli context update <context-id> --parent-id <id> --data <json>
acli context delete <context-id>
```

### Records and values

```sh
acli record create --blueprint <code> [--values values.yaml] [--files <json-or-file>] [--context-id <id>] [--system-tags <json>] [--system-metadata <json>]
acli record get | form | delete | duplicate | export-status <record-id>
acli record labels --record-id <id> [--record-id <id> ...]
acli record update <record-id> [--values f.yaml] [--relationships f.yaml] [--remove-values f.yaml] [--context-id <id>] [--system-tags <json>] [--system-metadata <json>] [--expected-updated-at <t>]
acli record batch --operations <json-or-file>
acli record preview <record-id> [--relationship-depth <n>] [--relationship-limit <n>]
acli record resolved-preview <record-id> --context-id <id>
acli record search --blueprint <code> [--query <text>] [--filters <json>] [--version <n>] [--sort-field <f> [--sort-direction asc|desc] [--sort-context-code <code>]] [--size <n>] [--cursor <c>] [--include-total] [--outdated] [--system-tags <json>] [--relationship-tree-facets <json>] [--relationship-filters <json>] [--export-extension-id <id> [--export-connection <name>] --export-status <state>] [--data-quality open|acknowledged|clean] [--context-code <code>] [--omit-blueprint] [--omit-previews]
acli record system-tags --blueprint <code> [--version <n>] [--prefix <text>] [--limit <n>]
acli record facet-children --blueprint <code> --context-id <id> [--version <n>] [--query <text>] [--source-relationship-field <f>] [--hierarchy-field <f>] [--parent-id <id>] [--cursor <c>] [--selected-target-ids <json>] [--include-ids <json>]
acli record list --blueprint <code> --related-from <record-id> --relationship <attribute> [--limit <n>] [--cursor <c>]
acli record incoming-relationships <record-id> --relationships <json> [--size <n>] [--cursor <c>]
acli record hierarchy <record-id> --context-id <id> --field <relationship>
acli record changes | value-history <record-id> [--limit <n>] [--offset <n>]
acli record restore-value <record-id> <history-id> [--expected-updated-at <t>]
acli record migrate <record-id> [--values <json-or-file>] [--relationships <json-or-file>] [--discard-attributes <json-or-file>] [--expected-updated-at <t>]
acli record transitions <record-id> [--context-id <id>]
acli record transition <record-id> --attribute <status> --to <option> [--context-id <id>] [--expected-updated-at <t>]
acli record unlock <record-id> --attribute <status> --to <option> --expected-updated-at <t> [--context-id <id>]
acli record approvals | retention-holds <record-id> [--before-time <t> --before-id <id>]
acli record comment list <record-id> [--before-time <t> --before-id <id>]
acli record comment add <record-id> --body <text>
acli record comment count <record-id>
acli record comment edit <record-id> <comment-id> --body <text> --revision <n>
acli record rule list | finding-count <record-id>
acli record rule findings | runs <record-id> [--limit <n>] [--offset <n>]
acli record migrate-bulk --blueprint <code> --from-version <n> [--size <n>] [--dry-run]
acli record publication list <record-id>
acli record publication publish | unpublish <record-id> --context-id <channel>
acli record publication publish-all | readiness <record-id>
acli record publication status --record-id <id> [--record-id <id> ...]

acli value current <record-id>
acli value append <record-id> --file values.yaml --context-id <id> [--expected-updated-at <t>]
acli value replace-relationships | remove-relationships <record-id> --file relationships.yaml --context-id <id>
```

`record batch` takes a JSON array of create, update, and delete operations, inline or in a file, and saves them all or none. See [Batch changes](/reference/api/#batch-changes) for the operation format.

With `--expected-updated-at`, a write fails instead of overwriting a record that changed after that time. `record transition` sets a status like the web app does, so workflow and approval rules apply; `record unlock` needs the `records.unlock` permission. Comment, approval, and retention-hold lists are newest first: pass the last item's `created_at` and `id` as `--before-time` and `--before-id` for the next page.

`record rule` shows a record's rules, findings, and rule runs to anyone who can read the record; the `rule` commands need `rules.read`, and so does `record search --data-quality`. Sorting by publication status needs `--sort-context-code` to name the channel. `record system-tags` lists the system tags on a blueprint's records, most used first.

### Files

```sh
acli file upload <record-id> <attribute-code> --file a.png [--file b.png] [--context-id <id>]
acli file stage-upload <blueprint-id> <attribute-code> --file a.png [--file b.png] [--context-id <id>]
acli file set-references <record-id> <attribute-code> [--file-id <id> ...] [--expected-file-id <id> ...] [--context-id <id>]
acli file get <file-id>
acli file retention-hold list <file-id>
acli file retention-hold create <file-id> --days <n> --reason <text>
acli file retention-hold release <file-id> <hold-id>
acli file download-original <file-id> --output original.bin [--range 'bytes=0-1023']
acli file download-variant <file-id> thumbnail|display --output preview.webp [--range 'bytes=0-1023']
```

`stage-upload` uploads files before the record exists; pass the returned file IDs to `record create --files`, for example `[{"attribute_code":"manual","file_ids":["<file-id>"]}]`. `set-references` replaces the files an attribute references, in order, and fails if `--expected-file-id` does not list exactly the current ones. Retention holds need the `files.hold` permission.

### Saved searches

```sh
acli saved-view list [--query <text>]
acli saved-view get <id>
acli saved-view create --name <name> --state <json> [--visibility private|workspace] [--description <text>]
acli saved-view update <id> [--name <name>] [--state <json>] [--visibility private|workspace] [--description <text>]
acli saved-view delete <id>
acli saved-view link --state <json>
acli saved-view get-link <id>
```

The state needs at least a blueprint code, for example `{"blueprint":"product","query":"linen"}`. `update` keeps every field you omit.

### Notifications

These commands act on the signed-in user's own [inbox](/guides/inbox/).

```sh
acli notification list [--unread] [--before-time <created_at> --before-id <id>]
acli notification count
acli notification get <id>
acli notification read <id>
acli notification unread <id>
acli notification read-all [--up-to <rfc3339>]
acli notification delete <id>
```

`delete` is permanent. `read-all --up-to` marks only notifications created by that time, so ones that arrived after you listed them stay unread. `list` returns 30 notifications per page; pass the `created_at` and `id` of the last one to read the next page.

### Translations

```sh
acli lexicon list [--language <tag>]
acli lexicon set --key <text> --language <tag> --text <text> [--context <text>] [--plural-category <category>]
acli lexicon delete --key <text> --language <tag> [--context <text>] [--plural-category <category>]
acli lexicon export --language <tag>
acli lexicon import --file <lexicon.json|lexicon.yaml> [--replace]
acli lexicon report [--language <tag>]...
```

`--plural-category` is `zero`, `one`, `two`, `few`, `many`, or `other` (the default) and must be used by the language. `export` prints a file that `import` accepts; `--replace` deletes the language's entries that the file does not contain. Repeat `--language` to report several languages; by default the report covers English and every language with entries. Changes require `blueprints.write`. See [Translate labels](/builders/translations/).

### Rules and workflows

```sh
acli rule list [--blueprint-id <id>]
acli rule get <rule-id>
acli rule revisions <rule-id>
acli rule revision <rule-id> <version>
acli rule validate | create --blueprint-id <id> [--context-id <id>] --file rule.yaml
acli rule update <rule-id> [--accept-existing-violations] --file rule.yaml
acli rule enable <rule-id> [--accept-existing-violations]
acli rule disable <rule-id>
acli rule run-now <rule-id> --idempotency-key <key> [--record-id <id>] [--dry-run [--definition-file rule.yaml | --definition-stdin]]
acli rule run-list [--rule-id <id>] [--record-id <id>] [--scope all-records|record] [--limit <n>] [--offset <n>]
acli rule run-get <run-id>
acli rule run-replay <run-id>
acli rule findings [--record-id <id>] [--rule-id <id>] [--blueprint-id <id>] [--context-id <id>] [--severity info|warning|error|critical] [--state open|acknowledged|resolved] [--unresolved] [--limit <n>] [--offset <n>]
acli rule finding-count [--record-id <id>] [--rule-id <id>] [--blueprint-id <id>] [--context-id <id>] [--severity <severity>] [--state <state> | --include-resolved]
acli rule recheck --record-id <id> --idempotency-key <key>
acli rule acknowledge <finding-id>
acli rule acknowledge-many --finding-id <id> [--finding-id <id> ...]
acli rule acknowledge-many --matching [--record-id <id>] [--rule-id <id>] [--blueprint-id <id>] [--context-id <id>] [--severity <severity>]

acli workflow validate | create --file workflow.yaml
acli workflow list | get <workflow-id> | list-versions <workflow-id>
acli workflow get-version <workflow-id> <version>
acli workflow create-version <workflow-id> --file workflow.yaml
acli workflow publish | enable <workflow-id> <version>
acli workflow disable <workflow-id>
acli workflow run-now <workflow-id> --record-id <id> --idempotency-key <key>
acli workflow run-list [--limit <n>] [--offset <n>]
acli workflow run-targets <run-id>
acli workflow run-replay <run-id>
```

### Agent

```sh
acli agent conversation list [--record-id <id> [--context-id <id>] | --all] [--cursor <cursor>]
acli agent conversation search [--query <text>] [--record-id <id> [--context-id <id>] | --all] [--cursor <cursor>]
acli agent conversation create [--title <title>] [--record-id <id> [--context-id <id>]]
acli agent conversation get | delete <conversation-id>
acli agent conversation rename <conversation-id> --title <title>
acli agent message list <conversation-id>
acli agent message send <conversation-id> --content <text> [--attachment-id <id> ...] [--follow]
acli agent upload <conversation-id> --file notes.pdf [--file photo.jpg]
acli agent run list <conversation-id>
acli agent run events <run-id> [--last-event-id <id>]
acli agent approval list [--conversation-id <id>]
acli agent approval approve | reject <tool-call-id>
acli agent smart-fill --record-id <id> --content <text> [--default-context] [--context-id <id>] [--conversation-id <id>] [--draft-values <json>] [--attachment-id <id> ...]
```

`run events` and `message send --follow` print one JSON line per run event until the run ends, and resume where they stopped if the server closes the stream. Approve or reject each change the agent proposes with `approval approve` or `approval reject`. See [Agents and approvals](/guides/agents/).

### Workspace

```sh
acli workspace member list
acli workspace member set-state <member-id> --state active|inactive
acli workspace member grant <member-id> --role-id <id> --scope-type workspace|blueprint-family|record|context-subtree --scope-target-id <id>
acli workspace member revoke-grant <member-id> <grant-id>
acli workspace member transfer-ownership <member-id>
acli workspace role list | permission list | assignable-role list
acli workspace role create --code <code> --permissions <json>
acli workspace role update <role-id> --code <code> --permissions <json>
acli workspace role duplicate <role-id> --code <code>
acli workspace role retire <role-id> [--replacement-role-id <id>]
acli workspace invitation list
acli workspace invitation create --email <email> --role-id <id> --scope-type workspace|blueprint-family|record|context-subtree --scope-target-id <id> --expires-at <rfc3339>
acli workspace invitation revoke <invitation-id>
acli workspace invitation accept --secret-stdin
acli workspace user create --email <email> [--display-name <name>] [--invite-role-id <id> --scope-type workspace|blueprint-family|record|context-subtree --scope-target-id <id> --expires-at <rfc3339>]
acli workspace user set-password --onboarding-secret-stdin --invitation-secret-stdin --password-stdin
acli workspace navigation get | sidebar
acli workspace navigation set --entries <json>
acli workspace agent get
acli workspace agent set [--bulk-edits per-proposal|per-record] [--bulk-edit-max-records <1-5000>]
acli workspace tag-color list
acli workspace tag-color set <tag> gray|blue|teal|green|yellow|orange|red|purple|pink
acli workspace tag-color reset <tag>
acli workspace publication-channel list
acli workspace publication-channel set <context-id> --enabled true|false [--required-rule-code <code> ... | --clear-required-rule-codes] [--require-valid-record true|false]
acli workspace grant-target list workspace|blueprint-family|record|context-subtree
acli workspace token-permission list
acli directory list
acli directory user <user-id>
acli team list
acli team create --code <code> --name <name> [--member <user-id> …]
acli team update <team-id> [--name <name>] [--member <user-id> … | --clear-members]
acli team delete <team-id>
acli audit list [--limit <n>] [--offset <n>] [--occurred-after <t>] [--occurred-before <t>] [--actor-user-id <id>] [--action-category <c>] [--target-type <t>] [--executor-type human|agent] [--agent-run-id <id>] [--agent-tool-call-id <id>]
```

### Extensions

```sh
acli extension registry list | discover
acli extension registry add --source <owner/repository>
acli extension registry remove <registry-id>
acli extension registry get <owner> <repository>
acli extension list
acli extension runtime [--blueprint-id <id> --blueprint-version <n>]
acli extension get | enable | disable | remove <extension-id>
acli extension install --owner <o> --repository <r> --release-id <github-release-id>
acli extension sideload --file extension.tar.zst
acli extension new <directory> --id <extension-id> [--name <name>] [--sdk <path>]
acli extension dev <extension-id> [--bundle dist/ui.js] [--build <command>] [--watch <path>]... [--once]
acli extension upgrade <extension-id> --owner <o> --repository <r> --release-id <id>
acli extension configure <extension-id> --configuration <json>
acli extension grant <extension-id> --grant-kind capability|host_permission|event_publish|event_subscribe --grant-id <id>
acli extension revoke <extension-id> --grant-kind <grant-kind> --grant-id <grant-id>
acli extension quarantine <extension-id> --diagnostic-code <code>
acli extension workspace-mode --enabled true|false
acli extension operation start <extension-id> --operation-id <id> --input <json> --idempotency-key <key> [--input-file-id <id>] [--destination <json>]
acli extension operation list
acli extension operation get | artifacts | deliveries | cancel | replay <run-id>
acli extension operation download <run-id> <artifact-id> --output <path>
acli extension run list [--extension-id <id>]
acli extension run get | cancel <run-id> [--own]
acli extension run download <run-id> <artifact-id> --output <path> [--own]
acli extension annotations namespace|adopt <extension-id>
acli extension annotations repair <extension-id> <record-id> --patch <json>
acli extension schedule list
acli extension schedule create <extension-id> --operation-id <id> --input <json> [--input-file-id <id>] [--destination <json>] --interval-seconds <60-2592000>
acli extension schedule update <schedule-id> --enabled true|false --interval-seconds <n>
acli extension connector-job list <blueprint-id>
acli extension connector-job run <job-id> --idempotency-key <key>
acli extension connector-job resync <job-id> --idempotency-key <key> [--record-id <id> ... | --status pending|retrying|uncertain|rejected|synced|withdrawn]
acli extension run start <extension-id> <contribution-id> --release-id <id> --operation-id <id> --input <json> --idempotency-key <key> --blueprint-id <id> --blueprint-version <n> [--context-id <id>] --record-id <id> [--record-id <id> ...]
acli extension connection list <extension-id>
acli extension connection create <extension-id> --connection <name> --kind <kind> --display-name <name> --grant-type refresh-token|client-credentials|device-code|authorization-code --token-url <url> --client-id <id> [--device-authorization-url <url>] [--authorization-url <url>] [--scope <scope>] [--parameters <json>] [--client-secret-stdin] [--refresh-token-stdin]
acli extension connection update <extension-id> <connection> [--display-name <name>] [--token-url <url>] [--device-authorization-url <url>] [--authorization-url <url>] [--scope <scope>] [--parameters <json>] [--client-secret-stdin] [--refresh-token-stdin]
acli extension connection delete | device-authorization | poll | authorize | test <extension-id> <connection>
acli extension connection exchange-code <extension-id> <connection> --code <code> --redirect-uri <url> [--code-verifier <verifier>]
acli extension export summary
acli extension export status <extension-id> [--connection <name>] [--status <state>] [--after <id>] [--limit <n>]
acli extension export retry <extension-id> [--connection <name>] [--record-id <id> ... | --status <state>] [--run --idempotency-key <key>]
acli extension export preview <extension-id> <export-id> --record-id <id> [--connection <name>]
acli extension budget list <extension-id>
acli extension budget set <extension-id> <host-permission-id> --requests <n> --per-seconds <n>
acli extension budget reset <extension-id> <host-permission-id>
acli extension secret list
acli extension secret set <name> --value-stdin
acli extension secret delete <name>
acli extension layout get
acli extension layout set --layout <json-or-file>
```

`extension new` writes a native UI extension into a new directory and needs no server; with `--sdk`, its packages link to a local SDK checkout. `extension dev` needs a development server with developer tools on: it uploads the built bundle, then rebuilds with `--build` and uploads again whenever a watched path changes, replacing the installed release's Worker without new grants. See [Native UI](/extensions/native/).

Connection secrets and workspace secret values are read from standard input so they never appear in process arguments; with both `--client-secret-stdin` and `--refresh-token-stdin`, put the client secret on the first line. `connection authorize` needs a browser session (`--session-file`) and returns the provider's authorization URL, but the provider's callback completes only in the session that started it, which your browser does not carry; authorize from the web app instead, or use `connection exchange-code` with a code you obtained yourself.

### Solution packs

```sh
acli solution-pack inspect --file pack.tar.zst
acli solution-pack plan create --file pack.tar.zst --prefix <prefix> --blueprint-publication draft|publish [--include-sample-data] [--map key=code ... --map-asset key=uuid ... --map-context key=code ...]
acli solution-pack plan get <plan-id>
acli solution-pack apply <plan-id>
acli solution-pack applications list | get <id> | abandon <id>
acli solution-pack checks list | rerun <application-id>
acli solution-pack checks get <application-id> <run-id>
acli presentation-asset list | get <id>
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
