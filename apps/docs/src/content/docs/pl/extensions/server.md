---
title: Środowisko wykonawcze serwera
description: Uruchamiaj kod rozszerzenia na serwerze jako komponent WebAssembly, reaguj na zdarzenia, odczytuj i zapisuj katalog, wywołuj usługi zewnętrzne i publikuj zdarzenia.
---

Serwerowy kod rozszerzenia to **komponent** WebAssembly zadeklarowany jako artefakt `server_wasm`. Działa w piaskownicy bez kontekstu WASI: bez systemu plików, zmiennych środowiskowych, gniazd i wstępnie otwartych plików, a jedynym zegarem jest interfejs `clock` hosta. Wszystko, co robi, przechodzi przez wywołania hosta, a host przy każdym wywołaniu sprawdza uprawnienia przyznane rozszerzeniu.

## Wersja API hosta

Interfejs hosta to pakiet WIT `attricat:host@1.1.0` w pliku `crates/extension-runtime/wit-host/attricat-extension.wit` w repozytorium Attricat. Ustaw `attricat.host_api` w manifeście na zakres, który go obejmuje, np. `">=1.0.0, <2.0.0"` lub `"^1.1"`. Każde wydanie z takim zakresem może używać wszystkich funkcji: procedur obsługi zdarzeń, poleceń, typowanego odczytu i zapisu, konfiguracji zakresowej, wszystkich rodzajów [operacji](/pl/extensions/operations/) i wszystkich miejsc osadzenia po stronie klienta. Jeśli komponent korzysta z czegokolwiek, co dodano w 1.1, zadeklaruj `"^1.1"`, aby host obsługujący tylko 1.0 odrzucił pakiet, zamiast zawieść w czasie działania.

Zbuduj komponent dla jednego z tych światów:

| Świat | Eksportuje |
| --- | --- |
| `attricat-extension` | `handler` i `operations` |
| `handler-extension` | `handler` (zdarzenia i polecenia) |
| `operation-extension` | `operations` |
| `outcome-extension` (1.1) | `event-handler`, `command-handler` i `deferrable-operations` |

`outcome-extension` eksportuje interfejsy z wersji 1.1, które zgłaszają [typowane wyniki](#typowane-wyniki) zamiast zwykłego błędu. Gdy komponent eksportuje interfejs z 1.1, host wywołuje go zamiast jego poprzednika z 1.0. Komponent, który potrzebuje tylko części z tych trzech eksportów, zwraca z pozostałych błąd `fault`.

Wszystkie importy są zawsze dostępne, ale część działa tylko we właściwym miejscu. Interfejsy operacji (`artifacts`, `attricat-data`, `attricat`, `transfer`, `selection`) zwracają błąd poza przebiegiem operacji. W przebiegu błąd zwracają typowane funkcje `read` i `write` oraz wywołania `attricat.read.v1` i `attricat.command.v1`; używaj wtedy interfejsów katalogu przypisanych do przebiegu.

Kolejne wersje 1.x tylko rozszerzają 1.0. Komponent zbudowany dla wcześniejszej wersji 1.x działa na nowszych hostach bez przebudowy. Jak zbudować komponent, opisuje sekcja [Zbuduj komponent serwerowy](/pl/extensions/build/#zbuduj-komponent-serwerowy).

## Wywoływanie hosta

Interfejs `api` ma funkcje typowane (`read`, `write`, `scoped-configuration-get`, `scoped-configuration-set`, `log`) i jedną funkcję ogólną, `call(operation, request)`. `call` przyjmuje nazwę operacji i żądanie jako tekst JSON, a zwraca odpowiedź jako tekst JSON albo komunikat błędu:

```rust
let response = api::call("storage.get.v1", r#"{"key":"last-sync"}"#)?;
```

| Operacja | Wymaga | Opis |
| --- | --- | --- |
| `configuration.get.v1` | `configuration.read` | [Konfiguracja](#konfiguracja) |
| `storage.get.v1`, `storage.set.v1`, `storage.delete.v1`, `storage.list.v1` | `storage.extension` | [Magazyn](#magazyn) |
| `secrets.get.v1` | `secrets.read` | [Sekrety](#sekrety) |
| `attricat.read.v1` | `attricat.read` | [Odczytuj i zapisuj katalog](#odczytuj-i-zapisuj-katalog) |
| `attricat.command.v1` | `attricat.write` albo `attricat.annotations.write` dla partii zawierających tylko adnotacje | [Odczytuj i zapisuj katalog](#odczytuj-i-zapisuj-katalog) |
| `events.emit.v1` | `events.emit` | [Publikuj zdarzenia](#publikuj-zdarzenia-dla-innych-rozszerzeń) |
| `network.request.v1`, `network.request.v2` | `network.request` | [Wywołuj usługi zewnętrzne](#wywołuj-usługi-zewnętrzne) |
| `connections.list.v1`, `connections.get.v1` | `connections.use` | [Konektory eksportu](/pl/extensions/exports/#połączenia) |
| `mappings.*.v1` | `exports.mappings` | [Konektory eksportu](/pl/extensions/exports/#mapowania-eksportu) |
| `files.link.v1` | `attricat.files.public_links` | [Konektory eksportu](/pl/extensions/exports/#publiczne-linki-do-plików) |
| `connector.scope.v1` | | [Konektory eksportu](/pl/extensions/exports/#zakres-przebiegu) |

Host sprawdza uprawnienie przy każdym wywołaniu, więc uprawnienie cofnięte w trakcie działania Twojego kodu obowiązuje od jego następnego wywołania. Odrzucone wywołanie zwraca błąd, np. `missing capability 'storage.extension'`, a nieznana nazwa zwraca `unknown host operation`. Żądania i odpowiedzi są ograniczone do 64 KiB JSON.

### Która procedura obsługi została wywołana

Manifest podaje `handler` dla każdej procedury obsługi zdarzeń, polecenia i operacji, ale komponent ma jeden eksport dla każdego rodzaju wywołania:

- **Polecenia** otrzymują `handler` polecenia jako `request.handler`. Rozdzielaj wywołania na jego podstawie.
- **Operacje** otrzymują `handler` operacji jako `request.operation-id`.
- **Procedury obsługi zdarzeń** otrzymują tylko zdarzenie. Rozdzielaj wywołania na podstawie `event.event-type`. Jeśli dwie procedury obsługi subskrybują ten sam typ, komponent zostanie wywołany dla każdej z nich z tym samym zdarzeniem.

## Obsługuj zdarzenia katalogu

Zadeklaruj procedurę obsługi i zasubskrybuj dokładne typy zdarzeń:

```json
"permissions": ["events.subscribe", "attricat.read", "attricat.write"],
"server": {
  "event_handlers": [{
    "id": "recalculate",
    "event_types": ["record.updated.v1"],
    "handler": "handle-event"
  }]
}
```

Eksport `handle-event` komponentu otrzymuje zdarzenie: jego identyfikator, typ, rodzaj i identyfikator agregatu, identyfikatory korelacji i przyczyny oraz ładunek JSON. Zdarzenia rekordów zawierają identyfikator rekordu, jego Schemat i wersję oraz listę faktów opisujących każdy zmieniony atrybut. Zobacz [dokumentację zdarzeń](/pl/reference/events/).

Zasady dostarczania:

- **Co najmniej raz.** To samo zdarzenie może dotrzeć dwukrotnie, także po zakończeniu Twojej procedury obsługi, ale zanim Attricat to zapisał. Używaj identyfikatora zdarzenia jako klucza idempotencji.
- **Brak gwarancji kolejności.** Nie wnioskuj o przyczynowości na podstawie kolejności nadejścia.
- **Bieżący stan odczytujesz sam.** Zdarzenie mówi, co się zmieniło; jeśli potrzebujesz pełnego stanu rekordu, odczytaj go.
- **Błędy powodują kwarantannę rozszerzenia.** W eksporcie `handler` z wersji 1.0 pułapka (trap), wyczerpanie paliwa lub pamięci, przekroczenie limitu czasu albo zwrócony błąd poddają instalację kwarantannie. Dostarczenie jest ponawiane i po skonfigurowanej liczbie prób staje się martwą wiadomością (dead letter). `event-handler` z wersji 1.1 może zgłosić niepowodzenie bez kwarantanny; zobacz [Typowane wyniki](#typowane-wyniki).

Przed każdym dostarczeniem Attricat ponownie sprawdza, czy instalacja jest włączona, nadal korzysta z tego samego wydania i wciąż ma potrzebne uprawnienia.

### Łączenie serii zdarzeń

Seria edycji jednego rekordu może wygenerować wiele zdarzeń. Dodaj do procedury obsługi `debounce_ms`, aby otrzymać tylko najnowsze:

```json
{ "id": "push", "event_types": ["record.updated.v1"], "handler": "handle-event", "debounce_ms": 30000 }
```

Zdarzenie czeka, aż jego rekord przez `debounce_ms` (do godziny) nie będzie się zmieniał. Zdarzenia zastąpione nowszym zdarzeniem tego samego rekordu są pomijane. Zdarzenie `export.retry_requested.v1` dotyczy jednego połączenia, więc zastępuje tylko wcześniejsze żądanie ponowienia dla tego samego połączenia. `max_wait_ms` (domyślnie dziesięciokrotność `debounce_ms`, najwyżej 24 godziny) ogranicza, jak długo zdarzenie może czekać, gdy wciąż nadchodzą nowe.

### Typowane wyniki

`event-handler` z wersji 1.1 eksportuje `handle-event(event, delivery)`, które zwraca `result<_, failure>`. `delivery` zawiera `sequence` zdarzenia, `record-revision` rekordu dla zdarzeń zmieniających treść rekordu numer próby `attempt`, zaczynający się od 1, oraz `source-kind` zdarzenia (`api`, `worker`, `plugin` lub `system`) i `source-name`, np. `extension:acme.inventory` lub `workflow:<id>`.

`failure` z interfejsu `outcomes` przyjmuje jedną z postaci:

| Przypadek | Co się dzieje |
| --- | --- |
| `rejected { code, message, record-id?, connection? }` | Niepowodzenie jest zapisywane i nigdy nie jest ponawiane; dostarczenie się kończy. Z `record-id` [stan eksportu](/pl/extensions/exports/#mapowania-eksportu) rekordu zmienia się na `rejected` z tym kodem i komunikatem. |
| `retry { message, after-ms? }` | Ponawiana jest tylko ta procedura obsługi, po `after-ms` albo z rosnącym odstępem od 5 sekund do godziny, najwyżej 10 razy. Potem dostarczenie jest zapisywane jako `exhausted`, a rekordy, które procedura nadal dzierżawi, przechodzą w stan `retrying` z błędem `retries_exhausted`. |
| `fault(message)` | Instalacja zostaje poddana kwarantannie, a rekordy dzierżawione przez procedurę przechodzą w stan `retrying` z błędem `handler_fault`. |

Pułapki, wyczerpanie paliwa lub pamięci i przekroczenie limitu czasu są traktowane jak `fault`. Wyniki są zapisywane osobno dla każdego zdarzenia i procedury obsługi, więc gdy ponawiana jest jedna procedura obsługi zdarzenia, te, które już się zakończyły, nie są wywoływane ponownie. Odrzucone i wyczerpane dostarczenia są widoczne na stronie zarządzania rozszerzeniem przez 30 dni albo do wygaśnięcia ich zdarzenia zgodnie z `DOMAIN_EVENT_RETENTION_DAYS`, jeśli nastąpi to wcześniej.

`command-handler` eksportuje `handle-command` dla [poleceń](#polecenia-dla-twojego-interfejsu), oddzielnie od obsługi zdarzeń.

## Zegar

`clock.now()` zwraca bieżący czas UTC jako sekundy i nanosekundy od epoki Uniksa. Używaj go do wygasania tokenów, planowania odpytywania i znaczników czasu, których potrzebuje miejsce docelowe.

## Odczytuj i zapisuj katalog

Z `attricat.read` typowana funkcja `api.read` odczytuje rekord (`record`), jego bezpośrednie wartości (`values`) albo wartości rozwiązane w kontekście przez łańcuch kontekstów zastępczych (`resolved`). Odpowiedzi zawierają przypiętą wersję Schematu rekordu, więc możesz przełożyć identyfikatory atrybutów na kody.

Z `attricat.write` funkcja `api.write` zapisuje wartości skalarne w jawnie wskazanym kontekście, wskazując każdy atrybut przez `attribute-id` lub `attribute-code`. Zapisy przechodzą zwykłą ścieżką: kontrole typów, schematy, audyt i nowe zdarzenie domenowe.

Zapis wykonany podczas obsługi zdarzenia jest przypisywany użytkownikowi lub tokenowi, który stoi za pierwotną zmianą, zachowuje identyfikator korelacji zdarzenia i jest publikowany ze źródłem `extension:<extension-id>`. Twoja procedura obsługi otrzyma zdarzenie spowodowane tym zapisem. `event-handler` z wersji 1.1 może zakończyć obsługę od razu, gdy `delivery.source-name` to `extension:<identyfikator-twojego-rozszerzenia>`. Eksport `handler` z wersji 1.0 nie otrzymuje źródła, więc tam porównuj przed zapisem: jeśli wartość jest już taka, jaką chcesz zapisać, pomiń zapis, a pętla się zakończy.

`attricat.read.v1` dodaje odczyty stronicowane, kanały zmian i wyszukiwanie. Żądanie wskazuje `operation`:

```json
{ "operation": "page", "blueprint_id": "…", "blueprint_version": 3, "context_id": "…", "cursor": null, "limit": 100 }
{ "operation": "changes", "blueprint_id": "…", "blueprint_version": 3, "cursor": null, "limit": 100 }
{ "operation": "lookup", "blueprint_id": "…", "blueprint_version": 3, "attribute_id": "…", "value": "ABC-1" }
```

`page` zwraca `{records, next_cursor}`, do 100 rekordów. Przekazuj `next_cursor` z powrotem, dopóki nie będzie `null`. `changes` przegląda zmiany rekordów Schematu w kolejności zdarzeń; zapisz jego kursor, aby wznowić później. `lookup` zwraca jeden rekord albo `null`.

`attricat.command.v1` stosuje partię intencji:

```json
{
  "operation": "batch",
  "batch": {
    "batch_key": "sync-2026-03-01",
    "dry_run": false,
    "intents": [
      { "kind": "update", "intent_key": "price-ABC-1", "record_id": "…",
        "values": [{ "kind": "scalar", "attribute_code": "price", "context_id": "…", "value": 19.99 }] }
    ]
  }
}
```

Rodzaje intencji to `create`, `update`, `relationships`, `upsert` i [`annotate`](/pl/extensions/operations/#adnotacje-rekordów). Partia mieści do 100 intencji. Odpowiedź zawiera po jednym wyniku na intencję, `{intent_key, status, record_id, error}`, gdzie `status` to `applied`, `already_applied`, `validated` (przy `dry_run: true`) lub `rejected`. Zachowuj te same klucze intencji przy ponowieniach: już zastosowany klucz intencji jest zgłaszany jako `already_applied` i nie jest zapisywany drugi raz.

Upsert dopasowuje rekord po zadeklarowanym atrybucie klucza biznesowego (`lookup_attribute_id` i `lookup_value`), tworzy go tylko wtedy, gdy żaden rekord nie pasuje, i kończy się błędem, jeśli pasuje więcej niż jeden. Zbiory relacji z upsertu są stosowane zarówno wtedy, gdy aktualizuje on dopasowany rekord, jak i wtedy, gdy go tworzy.

Wyszukiwanie działa dokładnie tak samo jak dopasowanie w upsercie. Jeśli sam atrybut tekstowy jest zadeklarowanym kluczem unikalnym, wyszukiwanie korzysta ze znormalizowanych wartości tego klucza we wszystkich wersjach Schematu; w przeciwnym razie dopasowuje dokładny tekst wśród rekordów żądanej wersji. Wartość pasująca do więcej niż jednego rekordu kończy się błędem `lookup matched multiple records`, zamiast zwracać jedną z nich.

## Magazyn

Z `storage.extension` rozszerzenie dostaje własny magazyn klucz-wartość w obszarze roboczym. Wartości to dowolny JSON. Dostęp ma tylko zainstalowane wydanie, a wpisy są zachowywane przy uaktualnieniu rozszerzenia.

| Operacja | Żądanie | Odpowiedź |
| --- | --- | --- |
| `storage.get.v1` | `{key}` | `{value, revision}` albo `null` |
| `storage.set.v1` | `{key, value, expected_revision?}` | `{revision}` |
| `storage.delete.v1` | `{key, expected_revision?}` | `null` |
| `storage.list.v1` | `{prefix?, cursor?, limit?}` | `{entries: [{key, value, revision}], cursor}` |

`expected_revision` sprawia, że `set` i `delete` kończą się błędem, jeśli klucz zmienił się od Twojego odczytu. Instalacja może przechowywać do 65 536 kluczy i 256 MiB, a pojedyncza wartość może mieć najwyżej 64 KiB.

## Konfiguracja

`configuration.get.v1` zwraca konfigurację instalacji (wymaga `configuration.read`). Konfigurację zakresową dla Schematu lub atrybutu odczytuje się i zapisuje przez typowane funkcje interfejsu `api` (wymaga `configuration.write`).

## Polecenia dla Twojego interfejsu

Polecenia serwerowe pozwalają komponentowi klienckiemu rozszerzenia poprosić jego komponent serwerowy o wykonanie działania. Zadeklaruj je w `server.commands` wraz ze schematami żądania i odpowiedzi i przyznaj `client.commands`:

```json
"commands": [{
  "id": "recalculate",
  "handler": "recalculate",
  "request_schema": { "type": "object", "required": ["record_id"] },
  "response_schema": { "type": "object" }
}]
```

Klient wywołuje `attricat.command({ command_id: 'recalculate', payload: { record_id } })`. Attricat waliduje ładunek względem `request_schema`, sprawdza sesję wywołującego, kontrybucję, wydanie i uprawnienia, a następnie wywołuje `handle-command` z `handler: "recalculate"` i ładunkiem jako tekstem JSON. Zwróć odpowiedź jako tekst JSON; przed przekazaniem do klienta jest walidowana względem `response_schema`. Zwrócony błąd nie obejmuje rozszerzenia kwarantanną, ale obietnica po stronie klienta jest odrzucana z ogólnym komunikatem `extension command failed`. Treść Twojego błędu nie jest przekazywana, więc problemy, które użytkownik powinien zobaczyć, np. nieprawidłowe dane, zgłaszaj w odpowiedzi.

Polecenie działa z uprawnieniami rozszerzenia, a nie użytkownika. Sprawdź w procedurze obsługi, czy działanie ma sens dla wskazanego rekordu.

## Sekrety

Z `secrets.read` wywołanie `secrets.get.v1` z `{"name": "destination-token"}` zwraca `{"value": "…"}` tylko na potrzeby bieżącego wywołania. Wartości sekretów nigdy nie pojawiają się w konfiguracji, dziennikach, śladach, błędach ani rekordach audytu.

## Wywołuj usługi zewnętrzne

Z `network.request` i przyznanym uprawnieniem hosta wywołanie `network.request.v1` wykonuje jedno żądanie HTTPS:

```json
{
  "host_permission_id": "inventory-api",
  "method": "GET",
  "url": "https://api.inventory.example/v2/stock/ABC-1",
  "headers": { "accept": "application/json" },
  "secret_headers": [{ "secret": "inventory-token", "header": "authorization", "prefix": "Bearer " }]
}
```

Host:

- przy każdym wywołaniu ponownie sprawdza wydanie, konfigurację i uprawnienia;
- wymaga HTTPS i adresu URL bez ciągu zapytania, pasującego do reguły;
- rozwiązuje DNS, odrzuca każdy adres niepubliczny i łączy się tylko ze sprawdzonym adresem;
- weryfikuje TLS i nie podąża za przekierowaniami;
- egzekwuje limity rozmiaru i limit czasu z reguły oraz jej [limit żądań](/pl/extensions/manifest/#limity-żądań): własny `rate_limit` reguły albo 60 żądań na minutę wspólnych dla reguł wydania, które go nie mają (liczonych osobno w każdym procesie API, a przy `CACHE_BACKEND=redis` wspólnie dla wszystkich replik);
- zwraca status, wybrane bezpieczne nagłówki i treść w base64.

Broker nigdy nie ponawia żądań. Przekroczenie limitu czasu lub błąd połączenia to **niepewny wynik**: druga strona mogła otrzymać żądanie. Ponawiaj tylko wtedy, gdy miejsce docelowe obsługuje klucz idempotencji i wysyłasz stały klucz. Żądanie v1 ponad limit kończy się błędem `network request rate limit exceeded`.

### network.request.v2

`network.request.v2` przyjmuje pola v1 oraz:

- ciąg zapytania w `url`, jeśli pasująca reguła ustawia `allow_query: true`, dla parametrów takich jak `?marketplaceIds=` albo podpisanych adresów URL. Wartości zapytania nigdy nie trafiają do dzienników, śladów, metryk, błędów ani zapisanej historii dostarczeń;
- `body_base64` dla treści binarnej, zamiast `body`;
- `secret_body: {"format": "form" | "json", "fields": [{"field", "secret"}]}`, które wstawia sekrety obszaru roboczego do treści formularza lub JSON. `field` w JSON to klucz najwyższego poziomu albo wskaźnik JSON (JSON pointer). Komponent nigdy nie widzi sekretu;
- `connection`, które dodaje do żądania token dostępu [połączenia](/pl/extensions/exports/#połączenia);
- `idempotency_key`, wysyłany jako nagłówek `Idempotency-Key`.

```json
{
  "host_permission_id": "amazon-sp",
  "method": "GET",
  "url": "https://sellingpartnerapi-eu.amazon.com/listings/2021-08-01/items/A1/SKU-1?marketplaceIds=A1PA6795UKMFR9",
  "connection": "amazon-eu"
}
```

Wynik ma pole `outcome`:

| `outcome` | Pola | Znaczenie |
| --- | --- | --- |
| `response` | `status`, `headers`, `body_base64` | Miejsce docelowe odpowiedziało, z dowolnym statusem. |
| `rate_limited` | `retry_after_ms` | Limit żądań reguły jest wyczerpany. Nic nie zostało wysłane. Dla reguły bez zadeklarowanego limitu czas oczekiwania to wspólne okno jednej minuty. |
| `unreachable` | | Nie udało się nawiązać połączenia ani uzgodnić TLS. Nic nie zostało wysłane, więc możesz wysłać żądanie ponownie. |
| `uncertain` | | Inny błąd transportu lub przekroczenie limitu czasu. Miejsce docelowe mogło otrzymać żądanie. |
| `connection_unavailable` | `message` | Nie udało się odświeżyć tokenu połączenia z przyczyny przejściowej. |

`headers` zawiera `content-type`, `etag`, `retry-after` oraz nagłówki wymienione w `response_headers` reguły. Przekierowania nigdy nie są śledzone: odpowiedź 3xx jest zwracana bez zmian, a jej `Location` możesz pobrać przez przyznaną regułę. Odpowiedź 401 usuwa zapamiętany token połączenia, więc następne żądanie otrzyma nowy.

## Eksport do marketplace'ów i sklepów

Połączenia, mapowania eksportu, publiczne linki do plików i zadania eksportu uruchamiane zmianami opisuje strona [Konektory eksportu](/pl/extensions/exports/).

## Dziennik

Z `logging.write` funkcja `api.log(level, message)` zapisuje wpis w dzienniku serwera Attricat, oznaczony identyfikatorem rozszerzenia. `level` to `trace`, `debug`, `info`, `warn` lub `error`, a wpisy są ograniczone do 16 KiB. Bez tego uprawnienia albo z innym poziomem wywołanie zwraca błąd. Nigdy nie zapisuj w dzienniku sekretów, tokenów ani danych osobowych.

## Publikuj zdarzenia dla innych rozszerzeń

Rozszerzenia komunikują się przez trwałe zdarzenia. Zadeklaruj eksport:

```json
"permissions": ["events.emit"],
"event_contracts": {
  "exports": [{
    "id": "inventory.changed",
    "version": "1.0.0",
    "event_type": "plugin.acme.inventory.inventory_changed.v1",
    "schema": { "type": "object", "required": ["sku"] },
    "max_payload_bytes": 4096
  }]
}
```

i publikuj za pomocą `events.emit.v1`:

```json
{ "contract_id": "inventory.changed", "aggregate_kind": "inventory_item",
  "aggregate_id": "<uuid>", "payload": { "sku": "ABC-1" } }
```

Host uzupełnia typ zdarzenia, źródło oraz identyfikatory korelacji i przyczyny. Konsument deklaruje kontrakt w `event_contracts.consumes` i potrzebuje `events.subscribe`.

Administratorzy muszą przyznać `event_publish` dla każdego eksportu (`--grant-id inventory.changed`) oraz `event_subscribe` dla każdego konsumowanego kontraktu (`--grant-id acme.inventory:inventory.changed`). Konsumenta można włączyć tylko wtedy, gdy jego dostawca jest włączony i zgodny, a dostawcy nie można wyłączyć, usunąć ani uaktualnić w sposób, który zepsułby włączonego konsumenta.

Między rozszerzeniami nie ma wywołań typu żądanie-odpowiedź ani współdzielonego stanu.

## Limity

- JSON przekazywany przez granicę hosta: 64 KiB na wywołanie.
- Komunikaty dziennika: 16 KiB.
- Żądania i odpowiedzi poleceń: domyślnie 64 KiB.
- Żądania i punkty kontrolne operacji: 64 KiB.
