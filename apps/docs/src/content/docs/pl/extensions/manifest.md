---
title: Dokumentacja manifestu
description: Każde pole pliku manifest.json rozszerzenia, w tym uprawnienia, uprawnienia hosta, kontrybucje i deklaracje.
---

`manifest.json` to ścisły JSON. Nieznane pola w dowolnym miejscu sprawiają, że pakiet jest nieprawidłowy.

Identyfikatory używane w manifeście (`attricat.id` oraz identyfikatory artefaktów, zależności, reguł uprawnień, procedur obsługi i kontrybucji) mogą zawierać litery ASCII, cyfry, `.`, `_` i `-`. Zachowuj je bez zmian między wydaniami; odwołują się do nich układy i przyznane uprawnienia.

## Pola najwyższego poziomu

| Pole | Wymagane | Opis |
| --- | --- | --- |
| `manifest_version` | Tak | `1`. |
| `name` | Tak | Nazwa wyświetlana. |
| `version` | Tak | Wersja wydania, SemVer. |
| `description` | Tak | Jednowierszowy opis. |
| `icons` | Tak | Mapa rozmiarów na ścieżki ikon, z co najmniej jednym wpisem, np. `{ "48": "assets/icon-48.svg" }`. |
| `attricat.id` | Tak | Identyfikator rozszerzenia, np. `acme.inventory`. |
| `attricat.host_api` | Tak | Zakres SemVer obsługiwanych wersji API hosta. Musi obejmować bieżącą wersję API hosta, `1.1.0`, np. `>=1.0.0, <2.0.0` lub `^1.1`. |
| `artifacts` | Tak | Co najmniej jeden artefakt. |
| `permissions` | | Uprawnienia, które muszą zostać przyznane, zanim rozszerzenie będzie można włączyć. |
| `optional_permissions` | | Uprawnienia, które administrator może przyznać. |
| `host_permissions` | | Reguły ruchu wychodzącego, które muszą zostać przyznane. |
| `optional_host_permissions` | | Reguły ruchu wychodzącego, które administrator może przyznać. |
| `configuration` | | Schemat ustawień instalacji. |
| `scoped_configuration` | | Ustawienia przechowywane dla Schematu lub atrybutu. |
| `dependencies` | | Inne rozszerzenia, których to rozszerzenie potrzebuje. |
| `event_contracts` | | Zdarzenia publikowane lub konsumowane przez to rozszerzenie. |
| `server` | | Procedury obsługi zdarzeń, polecenia, operacje, webhooki, rodzaje połączeń i miejsca docelowe eksportu. |
| `ui` | | Kontrybucje klienckie. |
| `cell_renderers` | | Renderery komórek tabeli przeglądarki rekordów. |
| `attribute_types` | | Typy atrybutów dla Schematów. |

## Artefakty

```json
"artifacts": [
  { "id": "server", "kind": "server_wasm", "path": "dist/server.wasm" },
  { "id": "panel", "kind": "client_component", "path": "dist/panel.js" }
]
```

`kind` ma wartość `server_wasm` (komponent WebAssembly) lub `client_component` (moduł JavaScript). Ścieżki są względne wobec katalogu głównego archiwum. Z pakietu wypakowywane są tylko zadeklarowane artefakty.

## Uprawnienia

Wymień uprawnienia w `permissions` lub `optional_permissions`. Każde z nich zezwala na pewną klasę operacji; administrator musi je przyznać.

### Katalog i serwer

| Uprawnienie | Zezwala na |
| --- | --- |
| `attricat.read` | Odczyt rekordów, wartości i Schematów. |
| `attricat.write` | Zapis wartości i wykonywanie poleceń create, update, relationship i upsert. |
| `events.subscribe` | Odbieranie zdarzeń katalogu i konsumowanych zdarzeń rozszerzeń. |
| `events.emit` | Publikowanie zadeklarowanych zdarzeń tego rozszerzenia. |
| `storage.extension` | Odczyt i zapis własnego magazynu klucz-wartość rozszerzenia. |
| `configuration.read` | Odczyt konfiguracji instalacji. |
| `configuration.write` | Odczyt i zapis konfiguracji zakresowej. |
| `secrets.read` | Odczyt nazwanych sekretów obszaru roboczego w trakcie działania. |
| `logging.write` | Zapisywanie komunikatów dziennika. |
| `artifacts.read`, `artifacts.write` | Odczyt danych wejściowych operacji i zapis danych wyjściowych operacji. |
| `attricat.annotations.write` | Zapis własnej przestrzeni nazw tagów i metadanych rekordów. Zobacz [Adnotacje rekordów](/pl/extensions/operations/#adnotacje-rekordów). |
| `network.request` | Wykonywanie wychodzących żądań HTTPS pasujących do przyznanego uprawnienia hosta. |
| `exports.mappings` | Odczyt i zapis [mapowań eksportu](/pl/extensions/exports/#mapowania-eksportu) rozszerzenia. Wymagane do zadeklarowania `server.exports`. |
| `connections.use` | Odczyt [połączeń](/pl/extensions/exports/#połączenia) rozszerzenia i wysyłanie żądań z ich tokenami. Wymagane do zadeklarowania `server.connections`. |
| `attricat.files.read` | Kopiowanie pliku, do którego odwołuje się rekord, do danych wejściowych operacji przez `transfer.open-record-file`. |
| `attricat.files.public_links` | Wydawanie [publicznych linków](/pl/extensions/exports/#publiczne-linki-do-plików) do plików, do których odwołują się rekordy. |
| `webhooks.receive` | Deklarowanie przychodzących webhooków (jeszcze nie są dostarczane). |

### Interakcja z klientem

| Uprawnienie | Zezwala na |
| --- | --- |
| `client.commands` | Wywoływanie poleceń serwerowych rozszerzenia z jego interfejsu. |
| `client.navigation` | Przeniesienie użytkownika do rekordu. |
| `client.notification` | Wyświetlenie powiadomienia. |
| `client.refresh` | Odświeżenie bieżącego rekordu po zmianie. |
| `client.events` | Odbieranie w ramce zdarzeń zmiany kontekstu. |
| `client.theme.read` | Akceptowane dla zgodności. Motyw jest zawsze dostępny. |
| `client.operations.start` | Uruchamianie operacji interaktywnych rozszerzenia dla zaznaczenia ramki. |
| `client.operations.read` | Wyświetlanie, odczyt i pobieranie wyników uruchomień tego rozszerzenia należących do użytkownika. |
| `client.operations.cancel` | Anulowanie uruchomień tego rozszerzenia należących do użytkownika. |

Manifest przyjmuje też `client.confirmation`, `client.download`, `client.external_navigation`, `client.files.read`, `client.files.upload`, `client.search`, `client.live_updates`, `client.clipboard` i `client.locale.read`. Są one zarezerwowane dla API ramki, które jeszcze nie istnieją: ich przyznanie nic ramce nie daje, więc nie proś o nie.

### Umiejscowienie w kliencie

Każde uprawnienie umiejscowienia zezwala na kontrybucję w jednym miejscu osadzenia. Zobacz [Kontrybucje klienckie](/pl/extensions/client/#miejsca-osadzenia).

`client.blueprint_configuration`, `client.record_decoration`, `client.record_action`, `client.record_header_action`, `client.record_attribute_panel`, `client.explorer_row_action`, `client.explorer_table_cell`, `client.explorer_action`, `client.explorer_bulk_action`, `client.blueprint_detail_panel`, `client.blueprint_panel`, `client.blueprint_publish_check`, `client.file_panel`, `client.audit_event_panel`, `client.data_health_card`, `client.action_dialog`, `client.export_mapping`.

## Uprawnienia hosta

Uprawnienie hosta to reguła dla żądań wychodzących. Samo `network.request` na nic nie pozwala; każde żądanie musi pasować do przyznanej reguły.

```json
"host_permissions": [{
  "id": "inventory-api",
  "matches": ["https://api.inventory.example/v2/*"],
  "methods": ["GET", "POST"],
  "max_request_bytes": 65536,
  "max_response_bytes": 1048576,
  "timeout_ms": 10000
}]
```

| Pole | Domyślnie | Opis |
| --- | --- | --- |
| `id` | Wymagane | Stały identyfikator reguły, używany przy przyznawaniu uprawnień i wykonywaniu żądań. |
| `matches` | Wymagane | Wzorce URL. |
| `methods` | Wymagane | Dozwolone metody HTTP. |
| `max_request_bytes` | 65536 | Limit treści żądania. |
| `max_response_bytes` | 1048576 | Limit treści odpowiedzi (maksymalnie 1 MiB). |
| `timeout_ms` | 10000 | Limit czasu żądania. |
| `max_transfer_bytes` | 0 | Włącza strumieniowe przesyłanie plików dla operacji, do 1 GiB. `0` je wyłącza. |
| `idempotent_delivery` | `false` | Zezwala na `POST` przy dostarczaniu danych wyjściowych. Ustaw tylko wtedy, gdy miejsce docelowe respektuje nagłówek `Idempotency-Key`. |
| `allow_query` | `false` | Pozwala `network.request.v2` i przesyłaniu v2 wysyłać ciągi zapytania, np. podpisane adresy URL, do miejsc docelowych tej reguły. Wartości zapytania nigdy nie trafiają do dzienników. |
| `response_headers` | `[]` | Do 16 nazw nagłówków odpowiedzi, pisanych małymi literami, które `network.request.v2` zwraca oprócz `content-type`, `etag` i `retry-after`, np. `location` albo identyfikator żądania. `set-cookie` i nagłówki uwierzytelniania są niedozwolone. |
| `rate_limit` | Wspólny limit | [Limit żądań](#limity-żądań) reguły. |

Wzorzec składa się z `http` lub `https`, dokładnej nazwy hosta albo wiodącego symbolu wieloznacznego `*.`, opcjonalnego portu i prefiksu ścieżki zakończonego `/*`. Zapytania, fragmenty, dane uwierzytelniające, `localhost` oraz adresy prywatne, pętli zwrotnej i lokalne dla łącza są odrzucane. W czasie żądania dozwolony jest tylko HTTPS, przekierowania nie są śledzone, a każda odpowiedź DNS musi być adresem publicznym. Ciąg zapytania jest dozwolony w czasie żądania tylko w regułach z `allow_query`.

### Limity żądań

```json
"rate_limit": { "requests": 5, "per_seconds": 1, "burst": 10 }
```

Limit działa jak wiadro tokenów (token bucket): co `per_seconds` sekund (od 1 do 86 400) dochodzi `requests` tokenów, a naraz można wykorzystać do `burst` (domyślnie `requests`). `requests` i `burst` mieszczą się w przedziale od 1 do 100 000. Każde żądanie pasujące do reguły, także przy przesyłaniu plików, zużywa jeden token. Zadeklaruj limit, który publikuje miejsce docelowe, aby rozszerzenie zwalniało, zamiast być dławione.

Reguły bez `rate_limit` dzielą 60 żądań na minutę na wydanie. Limity są liczone dla obszaru roboczego, rozszerzenia i reguły, osobno w każdym procesie API albo wspólnie dla replik przy `CACHE_BACKEND=redis`. Administrator może [obniżyć limit](/pl/builders/extensions/#limity-żądań), ale nigdy go nie podniesie; zmiana obowiązuje w ciągu 10 sekund.

## Konfiguracja

```json
"configuration": {
  "version": 1,
  "schema": {
    "type": "object",
    "properties": { "warehouse": { "type": "string" } },
    "required": ["warehouse"],
    "additionalProperties": false
  }
}
```

Konfiguracja administratora jest walidowana względem `schema`, zanim rozszerzenie będzie można włączyć. Nigdy nie umieszczaj sekretów w konfiguracji; używaj [sekretów](/pl/builders/extensions/#sekrety).

`scoped_configuration` ma ten sam kształt oraz dodatkowo `scopes`, listę zawierającą `blueprint` i/lub `attribute`. Przechowuje wartości dla wersji Schematu lub atrybutu i wymaga `configuration.write`.

## Zależności

```json
"dependencies": [{ "id": "acme.core", "version": ">=1.0.0, <2.0.0" }]
```

Rozszerzenie można włączyć tylko wtedy, gdy każda zależność jest zainstalowana, włączona i mieści się w zakresie.

## Kontrakty zdarzeń

```json
"event_contracts": {
  "exports": [{
    "id": "inventory.changed",
    "version": "1.0.0",
    "event_type": "plugin.acme.inventory.inventory_changed.v1",
    "schema": { "type": "object", "required": ["sku"] },
    "max_payload_bytes": 4096
  }],
  "consumes": [{
    "provider": "acme.pricing",
    "contract": "price.changed",
    "version": "^1.0.0"
  }]
}
```

Eksportowane typy zdarzeń muszą zaczynać się od `plugin.<extension-id>.` i kończyć na `.vN`. Ładunki są ograniczone do od 1 do 64 KiB. Zobacz [Środowisko wykonawcze serwera](/pl/extensions/server/#publikuj-zdarzenia-dla-innych-rozszerzeń).

## Serwer

```json
"server": {
  "event_handlers": [
    { "id": "on-update", "event_types": ["record.updated.v1"], "handler": "handle-event" }
  ],
  "commands": [
    { "id": "recalculate", "handler": "recalculate",
      "request_schema": { "type": "object" }, "response_schema": { "type": "object" } }
  ],
  "operations": [
    { "id": "export", "handler": "export", "request_schema": { "type": "object" } }
  ]
}
```

| Sekcja | Pola | Wymaga |
| --- | --- | --- |
| `event_handlers` | `id`, `event_types` (dokładne wersjonowane typy), `handler`, opcjonalnie `debounce_ms` (od 1 do 3 600 000) i `max_wait_ms` (co najmniej `debounce_ms`, najwyżej 86 400 000; domyślnie dziesięciokrotność `debounce_ms`). Zobacz [Łączenie serii zdarzeń](/pl/extensions/server/#łączenie-serii-zdarzeń). | `events.subscribe` i artefaktu `server_wasm` |
| `commands` | `id`, `handler`, `request_schema`, `response_schema`, `max_request_bytes`, `max_response_bytes` (domyślnie 64 KiB) | `client.commands` |
| `operations` | `id`, `handler`, `request_schema`, `max_request_bytes`, `max_checkpoint_bytes` (domyślnie i maksymalnie 64 KiB), opcjonalnie `interactive: {"version": 1, "max_selection": 1–50}` | `interactive` wymaga `client.operations.start` |
| `webhooks` | `id`, `event_type`, `handler`, `methods` (`["POST"]`), `authentication`, `max_body_bytes` | `webhooks.receive`. Zadeklarowane, ale jeszcze niedostarczane. |
| `connections` | Zobacz [Połączenia](#połączenia). | `connections.use` |
| `exports` | Zobacz [Miejsca docelowe eksportu](#miejsca-docelowe-eksportu). | `exports.mappings` |

`handler` to nazwa, którą widzi Twój komponent: polecenia otrzymują ją jako `request.handler`, a operacje jako `operation-id`. Procedury obsługi zdarzeń jej nie otrzymują; zobacz [Która procedura obsługi została wywołana](/pl/extensions/server/#która-procedura-obsługi-została-wywołana).

### Połączenia

Rodzaj połączenia opisuje typ konta w miejscu docelowym. Administratorzy tworzą nazwane połączenia każdego rodzaju, np. po jednym na sklep lub region marketplace'u.

```json
"connections": [{
  "id": "amazon",
  "title": "Amazon seller account",
  "grant_types": ["refresh_token"],
  "host_permission": "amazon-lwa",
  "api_host_permissions": ["amazon-sp"],
  "token_header": { "name": "x-amz-access-token" },
  "client_authentication": "body",
  "parameters_schema": { "type": "object", "required": ["seller_id"] }
}]
```

| Pole | Domyślnie | Opis |
| --- | --- | --- |
| `id` | Wymagane | Stały identyfikator rodzaju. |
| `title` | Wymagane | Nazwa wyświetlana, do 200 bajtów. |
| `grant_types` | Wymagane | Przepływy OAuth, które administrator może wybrać: `client_credentials`, `refresh_token`, `device_code`, `authorization_code`. Bez powtórzeń. |
| `host_permission` | Wymagane | Zadeklarowane uprawnienie hosta zezwalające na `POST`. Muszą do niego pasować punkty końcowe tokenu i autoryzacji urządzenia. |
| `api_host_permissions` | Wymagane | Zadeklarowane uprawnienia hosta, których żądania mogą zawierać token dostępu połączenia. Do innych żądań token nigdy nie jest dodawany. |
| `token_header` | `Authorization: Bearer` | `{ "name", "prefix"? }`: nagłówek z tokenem dostępu, z opcjonalnym prefiksem. |
| `client_authentication` | `body` | Sposób uwierzytelnienia klienta w punkcie końcowym tokenu: `body` (`client_id` i `client_secret` jako pola formularza) lub `basic` (HTTP Basic). |
| `parameters_schema` | | Obiekt JSON Schema dla niesekretnych parametrów połączenia. |

### Miejsca docelowe eksportu

Miejsce docelowe eksportu nadaje ekranom stanu eksportu, podglądu, ponawiania i ponownej synchronizacji nazwę i punkty wejścia:

```json
"exports": [{
  "id": "amazon",
  "title": "Amazon",
  "connection": "amazon",
  "preview_command": "preview",
  "resync_operation": "export"
}]
```

| Pole | Opis |
| --- | --- |
| `id` | Stały identyfikator miejsca docelowego. |
| `title` | Nazwa wyświetlana, do 200 bajtów. |
| `connection` | Opcjonalne. Rodzaj połączenia zadeklarowany w `connections`. |
| `preview_command` | Opcjonalne. Zadeklarowane polecenie serwerowe, które zwraca ładunek dla jednego rekordu bez wysyłania go. Otrzymuje `{export_id, record_id, connection}` i zgodnie z konwencją zwraca `{payload, issues}`. |
| `resync_operation` | Opcjonalne. Zadeklarowana operacja wykonywana przy pełnej ponownej synchronizacji. |
| `reconcile_operation` | Opcjonalne. Zadeklarowana operacja porównująca miejsce docelowe z Attricat. |

## Kontrybucje interfejsu

```json
"ui": [
  { "id": "workbench", "version": 1, "kind": "route", "artifact": "app", "title": "Formula workbench" },
  { "id": "workbench-nav", "version": 1, "kind": "navigation", "route": "workbench", "title": "Formula workbench" },
  { "id": "stock", "version": 1, "kind": "embedded", "artifact": "panel", "outlet": "record_preview_panel" },
  { "id": "recalc", "version": 1, "kind": "action", "artifact": "row", "outlet": "explorer_row_action" }
]
```

| `kind` | Pola | Opis |
| --- | --- | --- |
| `route` | `artifact`, `title` | Pełna strona pod adresem `/extensions/<extension-id>/<contribution-id>`. |
| `navigation` | `route`, `title` | Link na pasku bocznym do jednej ze stron tego rozszerzenia. |
| `embedded` | `artifact`, `outlet` | Ramka w miejscu `navigation`, `record_preview_panel`, `blueprint_attribute_configuration`, `record_attribute_decoration`, `record_action` lub `explorer_table_cell`. |
| `action` | `artifact`, `outlet` | Akcja rozmieszczana przez host w miejscu `explorer_row_action`, `explorer_action`, `explorer_bulk_action` lub `record_header_action`. |
| `panel` | `artifact`, `outlet` | Panel rozmieszczany przez host, tylko do odczytu z wyjątkiem `export_mapping`, w miejscu `blueprint_detail_panel`, `blueprint_panel`, `blueprint_publish_check`, `record_attribute_panel`, `file_panel`, `audit_event_panel`, `data_health_card` lub `export_mapping`. |
| `dialog` | `artifact`, `outlet`, `title` | Zarządzane przez host okno `action_dialog` otwierane przez akcje zaznaczenia tego rozszerzenia. |

Każde rozszerzenie może użyć każdego miejsca osadzenia jeden raz. `record_action`, `explorer_row_action` i `explorer_bulk_action` przyjmują `version` 1 lub 2; wersja 2 otrzymuje [kontekst zaznaczenia](/pl/extensions/client/#kontekst-zaznaczenia).

## Renderery komórek

```json
"cell_renderers": [
  { "id": "example.currency", "version": 1, "value_types": ["number", "integer"], "allowed_props": ["currency"] }
]
```

Do renderera komórek można odwołać się z widoku tabeli Schematu. Wymaga on `client.explorer_table_cell` i pasującej kontrybucji interfejsu `explorer_table_cell`.

## Typy atrybutów

```json
"attribute_types": [{
  "id": "money",
  "version": "1.0.0",
  "primitive": "number",
  "value_schema": { "type": "number", "minimum": 0 },
  "configuration_schema": {
    "type": "object",
    "properties": { "currency": { "type": "string", "pattern": "^[A-Z]{3}$" } },
    "required": ["currency"]
  }
}]
```

`primitive` ma wartość `string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time` lub `json`. Schematy odwołują się do typu jako `extension_type = "acme.commerce:money@^1"`. Walidacja używa wyłącznie zadeklarowanych schematów; żaden kod rozszerzenia nie jest uruchamiany w celu walidacji wartości. Zobacz [Typy atrybutów z rozszerzeń](/pl/reference/blueprint/#typy-atrybutów-z-rozszerzeń).
