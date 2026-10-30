---
title: Dokumentacja CLI
description: Klient wiersza poleceń acli, jego konfiguracja, uwierzytelnianie, pliki wejściowe i pełna lista poleceń.
---

`acli` to klient wiersza poleceń dla API Attricat, przeznaczony dla skryptów, zadań CI i agentów. Po powodzeniu wypisuje odpowiedź JSON z API bez zmian na standardowe wyjście. Błędy trafiają na standardowe wyjście błędów jako JSON. Każde polecenie ma wbudowaną pomoc: `acli --help` i `acli <group> --help`.

## Instalacja

Każde [wydanie](https://github.com/attricat/attricat/releases) publikuje pliki binarne `acli` dla systemów Linux, macOS i Windows w tej samej wersji co serwer. Zainstaluj najnowsze wydanie w systemie Linux lub macOS:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/attricat/attricat/releases/latest/download/acli-installer.sh | sh
```

W systemie Windows, w PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/attricat/attricat/releases/latest/download/acli-installer.ps1 | iex"
```

Instalator umieszcza `acli` w `~/.local/bin` (lub `$XDG_BIN_HOME`). Aby zainstalować konkretną wersję, zamień `latest/download` na `download/v0.3.0` albo pobierz z wydania archiwum i jego sumę kontrolną `.sha256`. Używaj wersji `acli` zgodnej z wersją serwera; wypisuje ją `acli --version`.

`acli` jest też dołączony do obrazu kontenera:

```sh
docker run --rm -e ATTRICAT_API_URL -e ATTRICAT_TOKEN ghcr.io/attricat/attricat@sha256:… acli health
```

## Konfiguracja

| Ustawienie | Flaga | Zmienna środowiskowa | Opis |
| --- | --- | --- | --- |
| Adres serwera | `--server` | `ATTRICAT_API_URL` | Bazowy adres URL API razem z `/api`. Domyślnie `http://127.0.0.1:3000/api`. |
| Token | `--token`, `--token-stdin` | `ATTRICAT_TOKEN` | Osobisty token API. |
| Plik sesji | `--session-file` | `ATTRICAT_SESSION_FILE` | Przechowuje sesję przeglądarki między poleceniami. |
| Adres aplikacji webowej | | `ATTRICAT_WEB_URL` | Adres główny (origin) aplikacji webowej, np. `https://attricat.example.com`, bez ścieżki. Gdy jest ustawiony, polecenia zapisanych wyszukiwań wypisują też krótki link do przeglądarki rekordów. |
| Pominięcie `.env` | `--no-env` | | Nie odczytuje `.env` z bieżącego katalogu. |

Domyślnie `acli` odczytuje `.env` z bieżącego katalogu, nie nadpisując zmiennych już ustawionych w powłoce.

Dane uwierzytelniające są wysyłane tylko przez HTTPS lub na adres loopback w lokalnym środowisku deweloperskim.

## Uwierzytelnianie

**Tokenem** (zalecane w automatyzacji). Utwórz osobisty token API w aplikacji webowej w **Profil → Osobiste tokeny API**, a następnie:

```sh
export ATTRICAT_TOKEN=cat_pat_…
acli blueprint list
```

Zamiast `--token` wybieraj `ATTRICAT_TOKEN` lub `--token-stdin`, ponieważ `--token` pozostawia sekret w historii powłoki i na liście procesów.

`--token-stdin` czyta całe standardowe wejście, więc nie można go łączyć z flagą polecenia, która również je czyta, na przykład `--password-stdin`, `--value-stdin`, `--client-secret-stdin` czy `--stdin`; `acli` odrzuca takie połączenie z kodem `invalid_input`. W tych poleceniach użyj `ATTRICAT_TOKEN`.

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

Gdy API zwraca szczegóły w formie czytelnej dla maszyn (na przykład naruszenia kontroli lub konflikt klucza unikalnego), obiekt błędu zawiera je bez zmian w polu `details`.

## Argumenty JSON

Opcje przyjmujące dane strukturalne (`--permissions`, `--entries`, `--filters`, `--relationship-tree-facets`, `--relationship-filters`, `--relationships`, `--files`, `--selected-target-ids`, `--include-ids`, `--values`, `--discard-attributes`, `--configuration`, `--body`, `--payload`, `--input`, `--state`, `--operations`) akceptują JSON podany bezpośrednio albo ścieżkę do pliku JSON.

Polecenia schematów, przepływów pracy i reguł przyjmują `--file <path>` lub `--stdin` i wysyłają YAML bez zmian.

## Pliki wartości

`record create --values`, `record update --values` i `value append --file` odczytują YAML:

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

Każdy wpis wskazuje atrybut przez `attribute_code` lub `attribute_id`. Wartości skalarne używają natywnych typów YAML: ciągów znaków, liczb dziesiętnych, liczb całkowitych, wartości logicznych (`true` lub `false`), dat i dat z godziną w formacie RFC 3339. Ujmij w cudzysłów ciąg znaków, który YAML odczytałby jako inny typ, np. UUID zaczynający się od cyfr albo `'09:30:00'`. Każda wartość potrzebuje kontekstu: przekaż `--context-id` dla całego pliku lub ustaw `context_id` we wpisie.

Pliki zastępowania i usuwania relacji (`value replace`, `value remove`, `record update --relationships`):

```yaml
relationships:
  - attribute_code: categories
    target_record_ids: ['00000000-0000-0000-0000-000000000004']
```

`replace` ustawia listę jako pełny zbiór; `remove` odłącza tylko wymienione cele.

Usuwanie nadpisań w kontekstach (`record update --remove-values`):

```yaml
remove_values:
  - attribute_code: subtitle
```

Pliki kontekstów (`context create --file`):

```yaml
code: PL
parent_id: '00000000-0000-4000-8000-000000000001'  # omit for a top-level context

data:
  language: pl
```

## Polecenia

### Stan i uwierzytelnianie

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
acli token revoke <token-id>
```

### Schematy

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

`safe-migration-batch` odrzuca wersję, która usuwa atrybuty nadal mające wartości; gdy `migration-impact` zwraca `requires_removal_disposition`, dodaj `--archive-removed-values`, aby zarchiwizować te wartości. `reusable-attribute filter-fields` wypisuje atrybuty wielokrotnego użytku, które Przeglądarka oferuje jako filtry dla rekordów schematu.

### Konteksty

```sh
acli context list
acli context get <code>
acli context create --code <code> [--parent-id <id>] [--data <json>]
acli context create --file context.yaml
acli context update <context-id> --parent-id <id> --data <json>
acli context delete <context-id>
```

### Rekordy i wartości

```sh
acli record create --blueprint <code> [--values values.yaml] [--files <json-or-file>] [--context-id <id>] [--system-tags <json>] [--system-metadata <json>]
acli record get | form | delete | duplicate | export-status <record-id>
acli record labels --record-id <id> [--record-id <id> ...]
acli record update <record-id> [--values f.yaml] [--relationships f.yaml] [--remove-values f.yaml] [--context-id <id>] [--expected-updated-at <t>]
acli record batch --operations <json-or-file>
acli record preview <record-id> [--relationship-depth <n>] [--relationship-limit <n>]
acli record resolved-preview <record-id> --context-id <id>
acli record search --blueprint <code> [--query <text>] [--filters <json>] [--version <n>] [--sort-field <f> [--sort-direction asc|desc] [--sort-context-code <code>]] [--size <n>] [--cursor <c>] [--include-total] [--outdated] [--system-tags <json>] [--relationship-tree-facets <json>] [--relationship-filters <json>] [--export-extension-id <id> [--export-connection <name>] --export-status <state>] [--data-quality open|acknowledged|clean] [--context-code <code>] [--omit-blueprint] [--omit-previews]
acli record system-tags --blueprint <code> [--version <n>] [--prefix <text>] [--limit <n>]
acli record facet-children --blueprint <code> --source-relationship-field <f> --context-id <id> [--hierarchy-field <f>] [--parent-id <id>]
acli record list --blueprint <code> --related-from <record-id> --relationship <attribute>
acli record incoming-relationships <record-id> --relationships <json>
acli record hierarchy <record-id> --context-id <id> --field <relationship>
acli record changes | value-history <record-id> [--limit <n>] [--offset <n>]
acli record restore-value <record-id> <history-id> [--expected-updated-at <t>]
acli record migrate <record-id> [--expected-updated-at <t>]
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
acli record migrate-bulk --blueprint <code> --from-version <n> [--dry-run]
acli record publication list <record-id>
acli record publication publish | unpublish <record-id> --context-id <channel>
acli record publication publish-all | readiness <record-id>
acli record publication status --record-id <id> [--record-id <id> ...]

acli value current <record-id>
acli value append <record-id> --file values.yaml --context-id <id> [--expected-updated-at <t>]
acli value replace-relationships | remove-relationships <record-id> --file relationships.yaml --context-id <id>
```

`record batch` przyjmuje tablicę JSON z operacjami tworzenia, aktualizacji i usuwania, podaną bezpośrednio lub w pliku, i zapisuje wszystkie albo żadną. Format operacji opisuje sekcja [Zmiany wsadowe](/pl/reference/api/#zmiany-wsadowe).

Z `--expected-updated-at` zapis kończy się błędem zamiast nadpisać rekord zmieniony po tej chwili. `record transition` zmienia status tak jak aplikacja webowa, więc obowiązują reguły przepływów i zatwierdzeń; `record unlock` wymaga uprawnienia `records.unlock`. Listy komentarzy, zatwierdzeń i blokad retencji są posortowane od najnowszych: aby pobrać kolejną stronę, podaj `created_at` i `id` ostatniego elementu w `--before-time` i `--before-id`.

`record rule` pokazuje reguły rekordu, ich ustalenia i wykonania każdemu, kto może odczytać rekord; polecenia `rule` wymagają uprawnienia `rules.read`, podobnie jak `record search --data-quality`. Sortowanie według statusu publikacji wymaga podania kanału w `--sort-context-code`. `record system-tags` wypisuje tagi systemowe rekordów schematu, od najczęściej używanych.

### Pliki

```sh
acli file upload <record-id> <attribute-code> --file a.png [--file b.png] [--context-id <id>]
acli file stage-upload <blueprint-id> <attribute-code> --file a.png [--file b.png] [--context-id <id>]
acli file set-references <record-id> <attribute-code> [--file-id <id> ...] [--expected-file-id <id> ...] [--context-id <id>]
acli file get <file-id>
acli file retention-hold list <file-id>
acli file retention-hold create <file-id> --days <n> --reason <text>
acli file retention-hold release <file-id> <hold-id>
acli file download-original <file-id> --output original.bin [--range 'bytes=0-1023']
acli file download-variant <file-id> thumbnail|display --output preview.webp
```

`stage-upload` przesyła pliki, zanim rekord powstanie; zwrócone identyfikatory plików przekaż do `record create --files`, na przykład `[{"attribute_code":"manual","file_ids":["<file-id>"]}]`. `set-references` zastępuje pliki, do których odwołuje się atrybut, w podanej kolejności, i kończy się błędem, jeśli `--expected-file-id` nie wymienia dokładnie obecnych. Blokady retencji wymagają uprawnienia `files.hold`.

### Zapisane wyszukiwania

```sh
acli saved-view list [--query <text>]
acli saved-view get <id>
acli saved-view create --name <name> --state <json> [--visibility private|workspace] [--description <text>]
acli saved-view update <id> [--name <name>] [--state <json>] [--visibility private|workspace] [--description <text>]
acli saved-view delete <id>
acli saved-view link --state <json>
acli saved-view get-link <id>
```

Stan musi zawierać co najmniej kod schematu, np. `{"blueprint":"product","query":"linen"}`. `update` zachowuje pominięte pola.

### Powiadomienia

Polecenia działają na [skrzynce powiadomień](/pl/guides/inbox/) zalogowanego użytkownika.

```sh
acli notification list [--unread] [--before-time <created_at> --before-id <id>]
acli notification count
acli notification get <id>
acli notification read <id>
acli notification unread <id>
acli notification read-all [--up-to <rfc3339>]
acli notification delete <id>
```

`delete` usuwa trwale. `read-all --up-to` oznacza tylko powiadomienia utworzone do tej chwili, więc te, które przyszły po wyświetleniu listy, pozostają nieprzeczytane. `list` zwraca po 30 powiadomień na stronę; aby pobrać kolejną, podaj `created_at` i `id` ostatniego z nich.

### Tłumaczenia

```sh
acli lexicon list [--language <znacznik>]
acli lexicon set --key <tekst> --language <znacznik> --text <tekst> [--context <tekst>] [--plural-category <kategoria>]
acli lexicon delete --key <tekst> --language <znacznik> [--context <tekst>] [--plural-category <kategoria>]
acli lexicon export --language <znacznik>
acli lexicon import --file <lexicon.json|lexicon.yaml> [--replace]
acli lexicon report [--language <znacznik>]...
```

`--plural-category` to `zero`, `one`, `two`, `few`, `many` lub `other` (domyślnie) i musi być używana przez dany język. `export` wypisuje plik, który przyjmuje `import`; `--replace` usuwa wpisy języka, których nie ma w pliku. Powtórz `--language`, aby objąć raportem kilka języków; domyślnie raport obejmuje angielski i każdy język, który ma wpisy. Zmiany wymagają uprawnienia `blueprints.write`. Zobacz [Tłumaczenie etykiet](/pl/builders/translations/).

### Reguły i przepływy pracy

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

`run events` i `message send --follow` wypisują jeden wiersz JSON na każde zdarzenie wykonania, aż wykonanie się zakończy, i wznawiają odczyt od miejsca przerwania, jeśli serwer zamknie strumień. Każdą zmianę zaproponowaną przez agenta zatwierdź lub odrzuć poleceniem `approval approve` albo `approval reject`. Zobacz [Agenci i zatwierdzenia](/pl/guides/agents/).

### Obszar roboczy

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
acli workspace user create --email <email> [--display-name <name>]
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
acli audit list [--occurred-after <t>] [--occurred-before <t>] [--actor-user-id <id>] [--action-category <c>] [--target-type <t>] [--executor-type human|agent]
```

### Rozszerzenia

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
acli extension run download <run-id> <artifact-id> --output <path>
acli extension annotations namespace|adopt <extension-id>
acli extension annotations repair <extension-id> <record-id> --patch <json>
acli extension schedule list
acli extension schedule create <extension-id> --operation-id <id> --input <json> --interval-seconds <60-2592000>
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

Sekrety połączeń i wartości sekretów obszaru roboczego są czytane ze standardowego wejścia, więc nigdy nie trafiają do argumentów procesu; z `--client-secret-stdin` i `--refresh-token-stdin` jednocześnie podaj sekret klienta w pierwszym wierszu. `connection authorize` wymaga sesji przeglądarki (`--session-file`) i zwraca adres autoryzacji u dostawcy, ale wywołanie zwrotne dostawcy kończy autoryzację tylko w sesji, która ją rozpoczęła, a Twoja przeglądarka jej nie ma; autoryzuj w aplikacji internetowej albo użyj `connection exchange-code` z kodem uzyskanym samodzielnie.

### Pakiety rozwiązań

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

### Operacje

```sh
acli data-health summary | blueprints [--stale-after-days <n>]
acli data-health freshness | completeness | contexts | relationships | storage | background-processing
acli data-health refresh
acli metrics get --output metrics.prom
acli event dead-letters
acli event replay <consumer-id> <event-id>
```
