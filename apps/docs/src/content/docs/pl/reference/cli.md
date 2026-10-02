---
title: Dokumentacja CLI
description: Klient wiersza poleceń acli, jego konfiguracja, uwierzytelnianie, pliki wejściowe i pełna lista poleceń.
---

`acli` to klient wiersza poleceń dla API Attricat, przeznaczony dla skryptów, zadań CI i agentów. Po powodzeniu wypisuje odpowiedź JSON z API bez zmian na standardowe wyjście. Błędy trafiają na standardowe wyjście błędów jako JSON. Każde polecenie ma wbudowaną pomoc: `acli --help` i `acli <group> --help`.

`acli` jest dołączony do obrazu kontenera:

```sh
docker run --rm -e CATALOG_SERVER -e CATALOG_TOKEN ghcr.io/attricat/attricat@sha256:… acli health
```

## Konfiguracja

| Ustawienie | Flaga | Zmienna środowiskowa | Opis |
| --- | --- | --- | --- |
| Adres serwera | `--server` | `CATALOG_SERVER`, potem `CATALOG_API_URL` | Adres API. Domyślnie `http://127.0.0.1:3000`. |
| Token | `--token`, `--token-stdin` | `CATALOG_TOKEN` | Osobisty token API. |
| Plik sesji | `--session-file` | `CATALOG_SESSION_FILE` | Przechowuje sesję przeglądarki między poleceniami. |
| Adres aplikacji webowej | | `CATALOG_WEB_URL` | Gdy jest ustawiony, polecenia zapisanych wyszukiwań wypisują też krótki link do przeglądarki encji. |
| Pominięcie `.env` | `--no-env` | | Nie odczytuje `.env` z bieżącego katalogu. |

Domyślnie `acli` odczytuje `.env` z bieżącego katalogu, nie nadpisując zmiennych już ustawionych w powłoce.

Dane uwierzytelniające są wysyłane tylko przez HTTPS lub na adres loopback w lokalnym środowisku deweloperskim.

## Uwierzytelnianie

**Tokenem** (zalecane w automatyzacji). Utwórz osobisty token API w aplikacji webowej w **Profil → Osobiste tokeny API**, a następnie:

```sh
export CATALOG_TOKEN=cat_pat_…
acli blueprint list
```

Zamiast `--token` wybieraj `CATALOG_TOKEN` lub `--token-stdin`, ponieważ `--token` pozostawia sekret w historii powłoki i na liście procesów.

**Sesją z hasłem.** Zaloguj się raz i zachowaj sesję w pliku:

```sh
acli --session-file ~/.acli-session auth discover acme.example
printf '%s' "$PASSWORD" | acli --session-file ~/.acli-session auth login acme.example --email you@acme.example --password-stdin
acli --session-file ~/.acli-session blueprint list
```

Plik sesji zawiera tylko pliki cookie sesji i CSRF, jest tworzony z trybem `0600` i usuwany przy `auth logout`. Nie dodawaj go do repozytorium ani go nie udostępniaj. Podany token zawsze ma pierwszeństwo przed plikiem sesji.

## Wyjście i kody wyjścia

Polecenia, które pobierają pliki, artefakty lub metryki, wymagają `--output <path>` i po powodzeniu wypisują `null`, dzięki czemu standardowe wyjście zawsze zawiera JSON.

| Kod wyjścia | Znaczenie |
| --- | --- |
| `0` | Powodzenie. |
| `2` | Nieprawidłowe dane wejściowe lub konfiguracja. |
| `3` | Nie udało się połączyć z serwerem. |
| `4` | API zwróciło błąd. |
| `5` | API zwróciło nieprawidłowy JSON. |

```json
{ "error": { "code": "invalid_blueprint_definition", "message": "blueprints must define at least one attribute", "status": 422 } }
```

## Argumenty JSON

Opcje przyjmujące dane strukturalne (`--permissions`, `--entries`, `--filters`, `--relationship-tree-facets`, `--relationships`, `--selected-target-ids`, `--values`, `--discard-attributes`, `--configuration`, `--body`, `--payload`, `--input`, `--state`) akceptują JSON podany bezpośrednio albo ścieżkę do pliku JSON.

Polecenia schematów, przepływów pracy i reguł przyjmują `--file <path>` lub `--stdin` i wysyłają TOML bez zmian.

## Pliki wartości

`entity create --values`, `entity update --values` i `value append --file` odczytują TOML:

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

Każdy wpis wskazuje atrybut przez `attribute_code` lub `attribute_id`. Wartości skalarne używają natywnych typów TOML: ciągów znaków, liczb dziesiętnych, liczb całkowitych, wartości logicznych, dat i dat z godziną w formacie RFC 3339. Każda wartość potrzebuje kontekstu: przekaż `--context-id` dla całego pliku lub ustaw `context_id` we wpisie.

Pliki zastępowania i usuwania relacji (`value replace`, `value remove`, `entity update --relationships`):

```toml
[[relationships]]
attribute_code = "categories"
target_entity_ids = ["00000000-0000-0000-0000-000000000004"]
```

`replace` ustawia listę jako pełny zbiór; `remove` odłącza tylko wymienione cele.

Usuwanie nadpisań w kontekstach (`entity update --remove-values`):

```toml
[[remove_values]]
attribute_code = "subtitle"
```

Pliki kontekstów (`context create --file`):

```toml
code = "PL"
parent_id = "00000000-0000-4000-8000-000000000001"  # omit for a top-level context

[data]
language = "pl"
```

## Polecenia

### Stan i uwierzytelnianie

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

### Schematy

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

### Konteksty

```sh
acli context list
acli context get <code>
acli context create --code <code> [--parent-id <id>] [--data <json>]
acli context create --file context.toml
acli context update <context-id> --parent-id <id> --data <json>
acli context delete <context-id>
```

### Encje i wartości

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

### Pliki

```sh
acli file upload <entity-id> <attribute-code> --file a.png [--file b.png] [--context-id <id>]
acli file metadata <file-id>
acli file download-original <file-id> --output original.bin [--range 'bytes=0-1023']
acli file download-variant <file-id> thumbnail|display --output preview.webp
```

### Zapisane wyszukiwania

```sh
acli saved-view list [--query <text>]
acli saved-view get <id>
acli saved-view create --name <name> --state <json> [--visibility private|workspace] [--description <text>]
acli saved-view update <id> --name <name> --state <json>
acli saved-view delete <id>
acli saved-view link --state <json>
```

Stan musi zawierać co najmniej kod schematu, np. `{"blueprint":"product","query":"linen"}`.

### Tłumaczenia

```sh
acli lexicon list [--language <znacznik>]
acli lexicon set --key <tekst> --language <znacznik> --text <tekst> [--context <tekst>] [--plural-category <kategoria>]
acli lexicon delete --key <tekst> --language <znacznik> [--context <tekst>] [--plural-category <kategoria>]
acli lexicon export --language <znacznik>
acli lexicon import --file <lexicon.json|lexicon.toml> [--replace]
acli lexicon report [--language <znacznik>]...
```

`--plural-category` to `zero`, `one`, `two`, `few`, `many` lub `other` (domyślnie) i musi być używana przez dany język. `export` wypisuje plik, który przyjmuje `import`; `--replace` usuwa wpisy języka, których nie ma w pliku. Powtórz `--language`, aby objąć raportem kilka języków; domyślnie raport obejmuje angielski i każdy język, który ma wpisy. Zmiany wymagają uprawnienia `blueprints.write`. Zobacz [Tłumaczenie etykiet](/pl/builders/translations/).

### Reguły i przepływy pracy

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
acli workflow run-replay <run-id>
```

### Obszar roboczy

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
acli audit list [--occurred-after <t>] [--occurred-before <t>] [--actor-user-id <id>] [--action-category <c>] [--target-type <t>] [--executor-type human|agent]
```

### Rozszerzenia

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
acli extension-schedule list
acli extension-schedule create <extension-id> --operation-id <id> --input <json> --interval-seconds <60-2592000>
acli extension-schedule update <schedule-id> --enabled true|false --interval-seconds <n>
acli connector-job list <blueprint-id>
acli connector-job run <job-id> --idempotency-key <key>
```

### Pakiety rozwiązań

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

### Operacje

```sh
acli data-health summary | blueprints [--stale-after-days <n>]
acli data-health freshness | completeness | contexts | relationships | storage | background-processing
acli data-health refresh
acli metrics get --output metrics.prom
acli event dead-letters
acli event replay <consumer-id> <event-id>
```
