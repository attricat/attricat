---
title: Środowisko wykonawcze serwera
description: Uruchamiaj kod rozszerzenia na serwerze jako komponent WebAssembly, reaguj na zdarzenia, odczytuj i zapisuj katalog, wywołuj usługi zewnętrzne i publikuj zdarzenia.
---

Serwerowy kod rozszerzenia to **komponent** WebAssembly zadeklarowany jako artefakt `server_wasm`. Działa w piaskownicy bez kontekstu WASI: bez systemu plików, zmiennych środowiskowych, zegara, gniazd i wstępnie otwartych plików. Wszystko, co robi, przechodzi przez wywołania hosta, a host przy każdym wywołaniu sprawdza uprawnienia przyznane rozszerzeniu.

## Wersje API hosta

Interfejs hosta jest zdefiniowany w pakietach WIT w repozytorium Attricat, w katalogu `crates/extension-runtime/`. Każda opublikowana wersja jest niezmienna.

| Wersja | Katalog WIT | Dodaje |
| --- | --- | --- |
| `catalog:host@1.0.0` | `wit/` | `api.call` i `api.log` z żądaniami JSON. |
| `catalog:host@1.1.0` | `wit-next/` | Typowane funkcje `read`, `write` i konfiguracji zakresowej. |
| `catalog:host@1.2.0` | `wit-operations/` | Trwałe [operacje](/pl/extensions/operations/). |
| `catalog:host@1.3.0` | `wit-artifacts/` | Artefakty wejściowe i wyjściowe operacji. |
| `catalog:host@1.4.0` | `wit-connectors/` | Wywołania katalogu przez konektory i przesyłanie plików przez HTTPS. |
| `catalog:host@1.5.0` | `wit-interactive/` | [Operacje interaktywne](/pl/extensions/operations/#operacje-interaktywne-api-hosta-15) na zaznaczeniu. |
| `catalog:host@1.6.0` | `wit-host/` | **Ujednolicone ABI**: wszystko z 1.1 i 1.5 w jednym pakiecie. |

Ustaw `catalog.host_api` w manifeście na zakres, dla którego zbudowano Twój komponent.

### Używaj ujednoliconego ABI w nowych rozszerzeniach

Wersje do 1.5 tworzą dwie osobne rodziny. 1.0 i 1.1 obsługują zdarzenia i polecenia. 1.2–1.5 uruchamiają operacje. Wydanie oparte na jednej z nich nie może używać funkcji drugiej rodziny, więc jedno wydanie nie może mieć jednocześnie poleceń klienta i operacji interaktywnych.

Od 1.6 jest jedno ABI, które tylko się rozrasta. Ustaw `"host_api": ">=1.6.0, <2.0.0"`, a komponent może łączyć procedury obsługi zdarzeń, polecenia, konfigurację zakresową i wszystkie rodzaje operacji. Zbuduj go dla jednego ze światów z `wit-host/`:

| Świat | Eksportuje |
| --- | --- |
| `catalog-extension` | `handler` i `operations` |
| `handler-extension` | `handler` (zdarzenia i polecenia) |
| `operation-extension` | `operations` |

Wszystkie importy są zawsze dostępne, ale część działa tylko we właściwym miejscu. Interfejsy operacji (`artifacts`, `catalog-data`, `catalog`, `transfer`, `selection`) zwracają błąd poza przebiegiem operacji. W przebiegu błąd zwracają typowane funkcje `read` i `write` oraz wywołania `catalog.read.v1` i `catalog.command.v1`; używaj wtedy interfejsów katalogu przypisanych do przebiegu.

Kolejne wersje 1.x tylko rozszerzają 1.6. Komponent zbudowany dla 1.6 działa na nowszych hostach bez przebudowy.

Zakresy obejmujące 1.5 lub starsze wersje, np. `>=1.1.0, <2.0.0`, zachowują swój dotychczasowy świat.

## Obsługuj zdarzenia katalogu

Zadeklaruj procedurę obsługi i zasubskrybuj dokładne typy zdarzeń:

```json
"permissions": ["events.subscribe", "catalog.read", "catalog.write"],
"server": {
  "event_handlers": [{
    "id": "recalculate",
    "event_types": ["entity.updated.v1"],
    "handler": "handle-event"
  }]
}
```

Eksport `handle-event` komponentu otrzymuje zdarzenie: jego identyfikator, typ, rodzaj i identyfikator agregatu, identyfikatory korelacji i przyczyny oraz ładunek JSON. Zdarzenia encji zawierają identyfikator encji, jej Schemat i wersję oraz listę faktów opisujących każdy zmieniony atrybut. Zobacz [dokumentację zdarzeń](/pl/reference/events/).

Zasady dostarczania:

- **Co najmniej raz.** To samo zdarzenie może dotrzeć dwukrotnie, także po zakończeniu Twojej procedury obsługi, ale zanim Attricat to zapisał. Używaj identyfikatora zdarzenia jako klucza idempotencji.
- **Brak gwarancji kolejności.** Nie wnioskuj o przyczynowości na podstawie kolejności nadejścia.
- **Bieżący stan odczytujesz sam.** Zdarzenie mówi, co się zmieniło; jeśli potrzebujesz pełnego stanu encji, odczytaj ją.
- **Błędy powodują kwarantannę rozszerzenia.** Pułapka (trap), wyczerpanie paliwa lub pamięci, przekroczenie limitu czasu albo zwrócony błąd poddają instalację kwarantannie. Dostarczenie jest ponawiane i po skonfigurowanej liczbie prób staje się martwą wiadomością (dead letter).

Przed każdym dostarczeniem Attricat ponownie sprawdza, czy instalacja jest włączona, nadal korzysta z tego samego wydania i wciąż ma potrzebne uprawnienia.

## Odczytuj i zapisuj katalog

Z `catalog.read` komponent może odczytać encję, jej bezpośrednie wartości albo wartości rozwiązane w kontekście. Odpowiedzi zawierają przypiętą wersję Schematu encji.

Z `catalog.write` może zapisywać wartości skalarne w jawnie wskazanym kontekście. Zapisy przechodzą zwykłą ścieżką: kontrole typów, schematy, audyt i nowe zdarzenie domenowe.

Zapis wykonany podczas obsługi zdarzenia jest przypisywany użytkownikowi lub tokenowi, który stoi za pierwotną zmianą, zachowuje identyfikator korelacji zdarzenia i jest publikowany ze źródłem `extension:<extension-id>`. **Ignoruj zdarzenia z własnego źródła**, inaczej procedura obsługi, która zapisuje, będzie wyzwalać samą siebie.

Wywołania JSON `catalog.read.v1` i `catalog.command.v1` dodają odczyty stronicowane, kanały zmian, wyszukiwanie pojedynczego atrybutu oraz partie intencji `create`, `update`, `relationships` i `upsert`. Upsert dopasowuje encję po zadeklarowanym atrybucie klucza biznesowego, tworzy ją tylko wtedy, gdy żadna encja nie pasuje, i kończy się błędem, jeśli pasuje więcej niż jedna.

## Magazyn

Z `storage.extension` wywołania `storage.get.v1`, `storage.set.v1`, `storage.delete.v1` i `storage.list.v1` dają rozszerzeniu własny magazyn klucz-wartość, ograniczony do obszaru roboczego i wydania. `set` i `delete` przyjmują `expected_revision` na potrzeby optymistycznej współbieżności.

## Konfiguracja

`configuration.get.v1` zwraca konfigurację instalacji (wymaga `configuration.read`). Konfigurację zakresową dla Schematu lub atrybutu odczytuje się i zapisuje przez typowane funkcje API hosta 1.1 (wymaga `configuration.write`).

## Polecenia dla Twojego interfejsu

Polecenia serwerowe pozwalają komponentowi klienckiemu rozszerzenia poprosić jego komponent serwerowy o wykonanie działania. Zadeklaruj je w `server.commands` wraz ze schematami żądania i odpowiedzi. Klient wywołuje `catalog.command({ command_id, payload })`. Attricat waliduje ładunek, sprawdza sesję wywołującego, kontrybucję, wydanie i uprawnienia, a następnie wywołuje Twoją procedurę obsługi.

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
- egzekwuje limity rozmiaru i limit czasu z reguły oraz 60 żądań na minutę na wydanie;
- zwraca status, wybrane bezpieczne nagłówki i treść w base64.

Broker nigdy nie ponawia żądań. Przekroczenie limitu czasu lub błąd połączenia to **niepewny wynik**: druga strona mogła otrzymać żądanie. Ponawiaj tylko wtedy, gdy miejsce docelowe obsługuje klucz idempotencji i wysyłasz stały klucz.

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
