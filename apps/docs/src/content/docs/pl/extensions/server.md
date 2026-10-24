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

Wszystkie importy są zawsze dostępne, ale część działa tylko we właściwym miejscu. Interfejsy operacji (`artifacts`, `attricat-data`, `catalog`, `transfer`, `selection`) zwracają błąd poza przebiegiem operacji. W przebiegu błąd zwracają typowane funkcje `read` i `write` oraz wywołania `attricat.read.v1` i `attricat.command.v1`; używaj wtedy interfejsów katalogu przypisanych do przebiegu.

Kolejne wersje 1.x tylko rozszerzają 1.0. Komponent zbudowany dla wcześniejszej wersji 1.x działa na nowszych hostach bez przebudowy.

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

`event-handler` z wersji 1.1 eksportuje `handle-event(event, delivery)`, które zwraca `result<_, failure>`. `delivery` zawiera `sequence` zdarzenia, `record-revision` rekordu dla zdarzeń zmieniających treść rekordu oraz numer próby `attempt`, zaczynający się od 1.

`failure` z interfejsu `outcomes` przyjmuje jedną z postaci:

| Przypadek | Co się dzieje |
| --- | --- |
| `rejected { code, message, record-id?, connection? }` | Niepowodzenie jest zapisywane i nigdy nie jest ponawiane; dostarczenie się kończy. Z `record-id` [stan eksportu](#mapowania-eksportu) rekordu zmienia się na `rejected` z tym kodem i komunikatem. |
| `retry { message, after-ms? }` | Ponawiana jest tylko ta procedura obsługi, po `after-ms` albo z rosnącym odstępem od 5 sekund do godziny, najwyżej 10 razy. Potem dostarczenie jest zapisywane jako `exhausted`, a rekordy, które procedura nadal dzierżawi, przechodzą w stan `retrying` z błędem `retries_exhausted`. |
| `fault(message)` | Instalacja zostaje poddana kwarantannie, a rekordy dzierżawione przez procedurę przechodzą w stan `retrying` z błędem `handler_fault`. |

Pułapki, wyczerpanie paliwa lub pamięci i przekroczenie limitu czasu są traktowane jak `fault`. Wyniki są zapisywane osobno dla każdego zdarzenia i procedury obsługi, więc gdy ponawiana jest jedna procedura obsługi zdarzenia, te, które już się zakończyły, nie są wywoływane ponownie. Odrzucone i wyczerpane dostarczenia są widoczne przez 30 dni na stronie zarządzania rozszerzeniem.

`command-handler` eksportuje `handle-command` dla [poleceń](#polecenia-dla-twojego-interfejsu), oddzielnie od obsługi zdarzeń.

## Zegar

`clock.now()` zwraca bieżący czas UTC jako sekundy i nanosekundy od epoki Uniksa. Używaj go do wygasania tokenów, planowania odpytywania i znaczników czasu, których potrzebuje miejsce docelowe.

## Odczytuj i zapisuj katalog

Z `attricat.read` komponent może odczytać rekord, jego bezpośrednie wartości albo wartości rozwiązane w kontekście. Odpowiedzi zawierają przypiętą wersję Schematu rekordu.

Z `attricat.write` może zapisywać wartości skalarne w jawnie wskazanym kontekście. Zapisy przechodzą zwykłą ścieżką: kontrole typów, schematy, audyt i nowe zdarzenie domenowe.

Zapis wykonany podczas obsługi zdarzenia jest przypisywany użytkownikowi lub tokenowi, który stoi za pierwotną zmianą, zachowuje identyfikator korelacji zdarzenia i jest publikowany ze źródłem `extension:<extension-id>`. **Ignoruj zdarzenia z własnego źródła**, inaczej procedura obsługi, która zapisuje, będzie wyzwalać samą siebie.

Wywołania JSON `attricat.read.v1` i `attricat.command.v1` dodają odczyty stronicowane, kanały zmian, wyszukiwanie pojedynczego atrybutu oraz partie intencji `create`, `update`, `relationships` i `upsert`. Upsert dopasowuje rekord po zadeklarowanym atrybucie klucza biznesowego, tworzy go tylko wtedy, gdy żaden rekord nie pasuje, i kończy się błędem, jeśli pasuje więcej niż jeden. Zbiory relacji z upsertu są stosowane zarówno wtedy, gdy aktualizuje on dopasowany rekord, jak i wtedy, gdy go tworzy.

Wyszukiwanie działa dokładnie tak samo jak dopasowanie w upsercie. Jeśli sam atrybut tekstowy jest zadeklarowanym kluczem unikalnym, wyszukiwanie korzysta ze znormalizowanych wartości tego klucza we wszystkich wersjach Schematu; w przeciwnym razie dopasowuje dokładny tekst wśród rekordów żądanej wersji. Wartość pasująca do więcej niż jednego rekordu kończy się błędem `lookup matched multiple records`, zamiast zwracać jedną z nich.

## Magazyn

Z `storage.extension` wywołania `storage.get.v1`, `storage.set.v1`, `storage.delete.v1` i `storage.list.v1` dają rozszerzeniu własny magazyn klucz-wartość, ograniczony do obszaru roboczego i wydania. `set` i `delete` przyjmują `expected_revision` na potrzeby optymistycznej współbieżności. Instalacja może przechowywać do 65 536 kluczy i 256 MiB, a pojedyncza wartość może mieć najwyżej 64 KiB.

## Konfiguracja

`configuration.get.v1` zwraca konfigurację instalacji (wymaga `configuration.read`). Konfigurację zakresową dla Schematu lub atrybutu odczytuje się i zapisuje przez typowane funkcje interfejsu `api` (wymaga `configuration.write`).

## Polecenia dla Twojego interfejsu

Polecenia serwerowe pozwalają komponentowi klienckiemu rozszerzenia poprosić jego komponent serwerowy o wykonanie działania. Zadeklaruj je w `server.commands` wraz ze schematami żądania i odpowiedzi. Klient wywołuje `attricat.command({ command_id, payload })`. Attricat waliduje ładunek, sprawdza sesję wywołującego, kontrybucję, wydanie i uprawnienia, a następnie wywołuje Twoją procedurę obsługi.

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
- `connection`, które dodaje do żądania token dostępu [połączenia](#połączenia);
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

## Połączenia

Połączenie to konto w miejscu docelowym, np. sklep Shopify lub konto sprzedawcy Amazon, które operator konfiguruje dla Twojego rozszerzenia. Rodzaje połączeń deklarujesz w [manifeście](/pl/extensions/manifest/#połączenia). Attricat przechowuje dane uwierzytelniające w postaci zaszyfrowanej, przeprowadza przepływy OAuth i odświeża tokeny. Twój komponent nigdy nie widzi sekretu ani tokenu.

Z `connections.use`:

- `connections.list.v1` z `{}` zwraca `{"connections": [...]}`.
- `connections.get.v1` z `{"connection": "shop-eu"}` zwraca jedno połączenie.

Każde połączenie opisują pola `connection` (jego nazwa), `kind`, `display_name`, `parameters` (niesekretne ustawienia, np. domena sklepu lub identyfikator sprzedawcy, walidowane względem `parameters_schema` rodzaju) oraz `status` (`pending_authorization`, `active`, `failed` lub `revoked`).

Aby wywołać miejsce docelowe, przekaż `connection` do `network.request.v2` albo [przesyłania v2](/pl/extensions/operations/#przesyłanie-plików). Host dodaje token dostępu, jako `Authorization: Bearer` albo w nagłówku `token_header` rodzaju, tylko do żądań pasujących do jednej z reguł `api_host_permissions` tego rodzaju.

## Mapowania eksportu

Z `exports.mappings` Attricat przechowuje jedno mapowanie na instalację, połączenie i rekord: zewnętrzny identyfikator, nazwane zewnętrzne identyfikatory, skrót treści i rewizję rekordu z ostatniego wysłania, status, ostatni błąd oraz znaczniki czasu. Mapowania są przygotowane na pełne katalogi i mają indeks na zewnętrznym identyfikatorze. Mapowanie jest usuwane razem ze swoim rekordem lub połączeniem; rekord usunięty miękko zachowuje mapowanie, aby można było wycofać pozycję z miejsca docelowego.

Status mapowania to stan eksportu rekordu, który użytkownicy widzą na stronie rekordu, w przeglądarce rekordów i na stronie Stan danych. Statusy to `pending`, `synced`, `rejected`, `retrying`, `uncertain` i `withdrawn`.

Każdy rekord ma `content_revision`: numer sekwencyjny ostatniego zdarzenia, które zmieniło jego wartości, relacje, publikację lub cykl życia. Ta wartość tylko rośnie. Zmiany adnotacji jej nie zwiększają, więc Twoje własne zapisy pomocnicze nigdy nie sprawiają, że rekord wygląda na zmieniony. Zmiany publikacji liczą się tylko dla odczytywanego kanału: wiersz strony konektora uwzględnia zmiany w kanale przebiegu, a odczyt bez kanału nie uwzględnia żadnych, więc publikacja w innym kanale nigdy nie sprawia, że eksport wygląda na nieaktualny. Zawierają ją odczyty typowane, wiersze [strony konektora v2](/pl/extensions/operations/#strony-eksportu-i-wycofania) i dostarczenia do `event-handler`. Dostarczenie zdarzenia publikacji lub wycofania publikacji zawiera rewizję bez zmian publikacji, tak jak odczyt bez kanału.

Poniższe operacje `api.call` działają tak samo w procedurach obsługi zdarzeń i w przebiegach operacji. `connection` jest wszędzie opcjonalne; bez niego mapowanie należy do domyślnego miejsca docelowego rozszerzenia.

| Operacja | Żądanie | Wynik |
| --- | --- | --- |
| `mappings.get.v1` | `{connection?, record_id}` lub `{connection?, external_id}` | `{mapping}` albo `{mapping: null}` |
| `mappings.list.v1` | `{connection?, status?, after?, limit?}` (od 1 do 100) | `{mappings, next}` |
| `mappings.begin.v1` | `{connection?, record_id, record_revision, lease_ms?, force?}` | `{outcome: "proceed", lease_token, mapping}`, `{outcome: "stale", mapping}` lub `{outcome: "busy", lease_until}` |
| `mappings.commit.v1` | `{connection?, record_id, lease_token?, status, record_revision?, external_id?, external_ids?, content_hash?, error?}` | `{mapping}` |
| `mappings.release.v1` | `{connection?, record_id, lease_token}` | `{released}` |
| `mappings.delete.v1` | `{connection?, record_id}` | `{deleted}` |

W przebiegu operacji `begin`, `commit`, `release` i `delete` przyjmują tylko rekordy objęte przebiegiem: rekordy schematu przebiegu konektora (także usunięte lub nieopublikowane, które wycofuje) albo rekordy wybrane w przebiegu interaktywnym. Przebieg konektora zapisuje tylko mapowania swojego połączenia, a przebieg ograniczony do wybranych rekordów tylko ich mapowania. Przebieg konektora z połączeniem wysyła też tylko token tego połączenia.

Wywołaj `begin` przed wysłaniem rekordu. Chroni ono przed pracą wykonaną w złej kolejności lub dwukrotnie:

- Odpowiada `stale` dla rewizji starszej niż rewizja mapowania, niezależnie od jego statusu, ponieważ zatwierdzenie zostałoby odrzucone po wysłaniu, a także dla usuniętego rekordu: jego mapowanie przyjmuje tylko zatwierdzenie `withdrawn`.
- Dla rewizji samego mapowania odpowiada `stale`, gdy mapowanie ma już status `synced`, `withdrawn` lub `rejected`, chyba że operator poprosił o ponowienie albo przekazujesz `force`.
- W przeciwnym razie dzierżawi rekord na `lease_ms` (domyślnie dwie minuty, najwyżej 15), aby procedura obsługi zdarzeń i przebieg konektora nigdy nie wysyłały tego samego rekordu jednocześnie. Odpowiada `busy`, dopóki dzierżawę trzyma ktoś inny.

`commit` zapisuje wynik i zwalnia dzierżawę. Bez tokenu dzierżawy kończy się błędem, gdy inny proces trzyma ważną dzierżawę, a wynik `synced` nigdy nie zastępuje nowszej zsynchronizowanej rewizji. Jeśli partia się nie powiedzie, zwolnij dzierżawy, które trzyma.

Po wyniku `uncertain` w miejscu docelowym bez kluczy idempotencji zapisz mapowanie jako `uncertain`. Zanim ponownie wyślesz ten rekord, wyszukaj go w miejscu docelowym, np. po SKU, i zapisz, co znalazłeś. `mappings.get.v1` z `external_id` znajduje rekord dla pozycji widocznej w miejscu docelowym.

Duże dane referencyjne miejsca docelowego, takie jak drzewa kategorii czy definicje typów produktów, przechowuj w magazynie rozszerzenia.

## Publiczne linki do plików

Niektóre miejsca docelowe same pobierają obrazy, czasem kilka godzin po zgłoszeniu oferty. Z `attricat.files.public_links`, `attricat.files.read` i `attricat.read` wywołanie `files.link.v1` wydaje publiczny, trudny do odgadnięcia link do pliku, do którego odwołuje się rekord:

```json
{ "file_id": "…", "record_id": "…", "expires_in_seconds": 86400 }
```

Zwraca `{"url": "…", "expires_at": "…"}`. Link jest ważny od 60 sekund do 7 dni. W przebiegu operacji można linkować tylko rekordy objęte przebiegiem: wybór przebiegu konektora (a w eksporcie do kanału opublikowane rekordy) i aktywne rekordy, do których one linkują, albo rekordy wybrane w przebiegu interaktywnym. Każdy, kto ma link, może pobrać plik, dopóki link nie wygasł, a rozszerzenie jest włączone i ma wszystkie trzy uprawnienia. Przechowywany jest tylko skrót tokenu. Wdrożenie musi ustawić [`EXTENSION_PUBLIC_FILE_BASE_URL`](/pl/reference/configuration/#rozszerzenia); w przeciwnym razie wywołanie kończy się błędem.

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
