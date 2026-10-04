---
title: Operacje i konektory
description: Długotrwałe operacje rozszerzeń z punktami kontrolnymi do importów, eksportów i przesyłania plików, z harmonogramami i zadaniami konektorów w Schematach.
---

**Operacja** to długotrwała praca po stronie serwera, np. eksport 200 000 produktów do CSV albo import pliku od dostawcy. Operacje działają w tle w partiach, po każdej partii zapisują punkt kontrolny i przetrwają ponowne uruchomienia.

Operacje wymagają wydania zbudowanego dla `catalog:host@1.2.0` lub nowszego.

## Zadeklaruj operację

```json
"server": {
  "operations": [{
    "id": "export",
    "handler": "export",
    "request_schema": {
      "type": "object",
      "properties": { "profile": { "type": "object" } },
      "required": ["profile"]
    }
  }]
}
```

Komponent implementuje `prepare`, `start`, `process-batch`, `checkpoint`, `finish` i `cancel`. Każde wywołanie otrzymuje identyfikator przebiegu, migawkę konfiguracji, zwalidowane dane wejściowe, ostatni punkt kontrolny i **klucz partii**.

## Jak przebiegi zachowują trwałość

- Przebieg jest tworzony raz dla danego obszaru roboczego, wydania, operacji i klucza idempotencji. Ponowne uruchomienie z tym samym kluczem zwraca ten sam przebieg.
- Przebieg jest przypięty do wydania, na którym się rozpoczął. Wyłączenie, kwarantanna, uaktualnienie lub cofnięcie uprawnienia wstrzymuje go, dopóki to dokładnie wydanie nie zostanie ponownie autoryzowane; nigdy nie przechodzi na nowy kod.
- Po każdej partii zapisywany jest punkt kontrolny. Klucz partii zmienia się dopiero po zapisaniu punktu kontrolnego.
- Jeśli proces roboczy ulegnie awarii przed punktem kontrolnym, partia jest wykonywana ponownie z **tym samym kluczem partii**. Jeśli Twój komponent zapisuje do katalogu lub systemu zewnętrznego, zadbaj, aby te zapisy były idempotentne względem klucza partii.
- Anulowanie przebiegu oczekującego w kolejce kończy go natychmiast. Anulowanie trwającego prosi komponent o zatrzymanie się przy następnej partii.
- Przebieg, który stale kończy się błędem, staje się martwą wiadomością (dead letter). Operator może go odtworzyć.

## Uruchamiaj, obserwuj i pobieraj

```sh
acli extension-operation start acme.export --operation-id export \
  --input '{"profile":{"version":1}}' --idempotency-key export-2026-03-01
acli extension-operation list
acli extension-operation show <run-id>
acli extension-operation artifacts <run-id>
acli extension-operation download <run-id> <artifact-id> --output products.csv
acli extension-operation cancel <run-id>
acli extension-operation replay <run-id>
```

`--input-file-id` dołącza gotowy plik obszaru roboczego jako dane wejściowe operacji. Komponent otwiera go jako `open-input("source")`; nigdy nie widzi identyfikatorów plików ani kluczy magazynu.

Listy przebiegów nigdy nie zawierają danych wejściowych, konfiguracji ani punktów kontrolnych. Wartości w konfiguracji i diagnostyce, których nazwy sugerują sekrety, dane uwierzytelniające, hasła, tokeny, klucze lub autoryzację, są zastępowane przez `[redacted]`.

## Dane wyjściowe

Z `artifacts.write` komponent buduje pliki wyjściowe po kawałku:

- `artifacts.append-output(name, media-type, batch-key, bytes)` dodaje od 1 do 65 536 bajtów dla bieżącej partii. Powtórzenie tych samych bajtów dla tego samego klucza partii jest ignorowane; inne bajty są odrzucane.
- `artifacts.finalize-output(name)` składa kawałki w jeden plik z sumą kontrolną.

Limity wynoszą 1 GiB na plik wyjściowy, 2 GiB na przebieg i 8 GiB na obszar roboczy. Dane wyjściowe można pobrać po zakończeniu przebiegu i są przechowywane przez 30 dni.

## Operacje interaktywne

Dodaj `interactive` do operacji, aby zalogowani użytkownicy mogli ją uruchomić dla encji lub zaznaczenia z Twoich akcji w wersji 2:

```json
{"id": "generate", "handler": "generate", "request_schema": {"type": "object"}, "interactive": {"version": 1, "max_selection": 50}}
```

Wymaga to `client.operations.start`. Nowe rozszerzenia buduj dla [ujednoliconego ABI hosta](/pl/extensions/server/#używaj-ujednoliconego-abi-w-nowych-rozszerzeniach): ustaw `"host_api": ">=1.6.0, <2.0.0"` i użyj świata `catalog-extension` lub `operation-extension` pakietu `catalog:host@1.6.0` z `crates/extension-runtime/wit-host/`, którego interfejs `selection` działa w uruchomieniu interaktywnym. Starsza ścieżka, czyli zakres `catalog.host_api` zgodny z 1.5, ale nie z 1.4, używa zamrożonego świata `catalog:host@1.5.0` z `wit-interactive/` (świat 1.4 z interfejsem `selection`) i nadal działa, ale nie da się jej łączyć z obsługą zdarzeń ani poleceniami.

Przy starcie Catalog sprawdza, czy użytkownik może odczytać każdą zaznaczoną encję, i utrwala użytkownika, wydanie, dane wejściowe, kontekst oraz uporządkowane zaznaczenie. Następnie:

- `selection.describe()` zwraca liczbę encji, wersję schematu i kontekst.
- `selection.page(cursor, limit)` zwraca od 1 do 10 encji z zapisanymi wartościami rozwiązanymi w kontekście uruchomienia, Twoimi adnotacjami i `read_at`. Encje, których użytkownik nie może już odczytać, mają status `unavailable`, a usunięte `deleted`.
- `catalog-data.read` i wywołania konektorowego interfejsu `catalog` są odrzucane. `catalog-data.batch` przyjmuje intencje `update`, `relationships` i `annotate` tylko dla zaznaczonych encji, sprawdzane względem bieżących uprawnień użytkownika.
- Jeśli użytkownik opuści obszar roboczy, uruchomienie zatrzymuje się z bezpiecznym powodem zamiast działać dalej z uprawnieniami rozszerzenia.

Pobierz z każdej encji to, czego potrzebujesz, raz i zapisz w punkcie kontrolnym, aby ponowiona paczka tworzyła te same bajty. Raportuj postęp jako `{"completed": n, "total": n, "outcome": {"succeeded": n, "failed": n, "skipped": n}}`; Catalog pokazuje te liczby niezależnie od statusu uruchomienia, więc uruchomienie może się zakończyć mimo niepowodzeń części encji.

Użytkownicy widzą swoje uruchomienia w **Profil → Uruchomienia rozszerzeń**. Uruchomienie widzą tylko osoba, która je rozpoczęła, i osoby z `extensions.manage`. Osoba, która je rozpoczęła, zawsze może je anulować, ale otworzyć je i pobrać wyniki może tylko dopóty, dopóki może odczytać każdą zaznaczoną encję. Osoby z `extensions.manage` mogą otwierać, anulować i pobierać każde uruchomienie.

## Adnotacje encji

Z `catalog.annotations.write` dodawaj do paczki intencje `annotate`, aby zapisać informacje o encji we własnej przestrzeni nazw: tagi `<extension-id>:<tag>` i obiekt w `system_metadata[<extension-id>]`.

```json
{"kind": "annotate", "intent_key": "doc-<run>-<entity>", "entity_id": "…",
 "add_tags": ["document-generated"], "set_metadata": {"last_document": {"template_version": 2}},
 "remove_tags": [], "remove_metadata": [], "expected_revision": null}
```

Podajesz tylko lokalne nazwy tagów i kluczy; Catalog dodaje przestrzeń nazw. Łatka zawiera od 1 do 32 operacji. Ustawienie klucza zastępuje jego wartość (`null` jest dozwolone). `expected_revision` odrzuca zapis, jeśli przestrzeń nazw zmieniła się od odczytu; ponowiony klucz intencji jest najpierw zgłaszany jako `already_applied`. Inni zapisujący, w tym użytkownicy edytujący encję, nie mogą zmienić Twojej przestrzeni nazw, a Twoje zapisy nie zmieniają `updated_at` encji.

Jeśli encje mają już dane pod identyfikatorem Twojego rozszerzenia, operator musi przejąć przestrzeń nazw przed Twoim pierwszym zapisem. Nie zapisuj w adnotacjach podpisanych adresów URL ani sekretów i nie traktuj tagu jako dowodu, że plik nadal można pobrać: wyniki wygasają.

## Przesyłanie plików (API hosta 1.4)

Z `network.request` i uprawnieniem hosta, które ustawia `max_transfer_bytes`, komponent może przenosić duże pliki przez HTTPS bez przekazywania bajtów przez JSON:

- `transfer.fetch-input` pobiera jeden zakres bajtów, do 16 MiB, do artefaktu wejściowego. Kolejne zakresy wymagają ETag, aby źródło nie mogło się zmienić między zakresami.
- `transfer.deliver-output` strumieniuje gotowy plik wyjściowy metodą `PUT` albo `POST`, jeśli uprawnienie hosta deklaruje `idempotent_delivery: true`. Każde dostarczenie zawiera stały nagłówek `Idempotency-Key`.

Próba dostarczenia jest zapisywana przed jakimkolwiek ruchem sieciowym. Po przekroczeniu limitu czasu lub awarii wynik to `uncertain` i nigdy nie jest wysyłany ponownie automatycznie. `acli extension-operation deliveries <run-id>` pokazuje historię dostarczeń.

## Dostęp do katalogu w operacjach (API hosta 1.4)

Operacje zbudowane dokładnie dla świata konektorów 1.4 mogą wywoływać `catalog-data.read` i `catalog-data.batch` (ten sam JSON co `catalog.read.v1` i `catalog.command.v1`) oraz wywołania w kształcie konektora: `schema`, `page` i `upsert-batch`. Strony mieszczą do 100 encji, a partie do 100 intencji. Partie muszą zawierać bieżący klucz partii, a klucz każdej intencji jest zapisywany, więc odtworzona partia zwraca `already_applied` zamiast zapisywać dwukrotnie.

Kursory stron rozwiązują wartości według stanu z pierwszej strony, korzystając z historii wartości, i wygasają po 30 dniach. To nie jest migawka bazy danych: encje tworzone, usuwane lub migrowane w trakcie długiego eksportu nadal mogą zmienić to, które encje się pojawią. Jeśli potrzebujesz dokładnego eksportu, zamroź źródło.

## Harmonogramy

```sh
acli extension-schedule create acme.export --operation-id export \
  --input '{"profile":{"version":1}}' --interval-seconds 86400
acli extension-schedule update <schedule-id> --enabled false --interval-seconds 86400
```

Harmonogram uruchamia operację w odstępach od 60 sekund do 30 dni. Przegapione interwały i nakładające się przebiegi są pomijane. Harmonogram jest wstrzymywany, gdy jego wydanie jest wyłączone, poddane kwarantannie, uaktualnione lub nie ma wymaganego uprawnienia.

## Zadania konektorów w Schematach

Zadanie konektora wiąże operację ze Schematem. Deklaruje się je w TOML Schematu, więc jest wersjonowane i przeglądane razem z modelem danych:

```toml
[[connector_jobs]]
code = "csv_export"
direction = "export"
extension_id = "attricat-connector-csv"
operation_id = "export"
interval_seconds = 3600
input = { profile = { version = 1, columns = [
  { header = "SKU", attribute = "sku", kind = "string" },
  { header = "Title", attribute = "title", kind = "string" },
] } }

[[connector_jobs]]
code = "csv_import"
direction = "import"
extension_id = "attricat-connector-csv"
operation_id = "import"
context = "default"
input = { profile = { version = 1, business_key = "sku", columns = [
  { header = "SKU", attribute = "sku", kind = "string" },
] } }
```

- Zadania są walidowane względem włączonego rozszerzenia podczas publikowania wersji Schematu. Nieprawidłowe zadanie blokuje publikację.
- Zadanie **eksportu** tworzy jeden przebieg dla każdego włączonego [kanału eksportu](/pl/guides/publishing/). Każdy przebieg odczytuje wartości w kontekście tego kanału i obejmuje tylko encje do niego opublikowane.
- Zadanie **importu** tworzy jeden przebieg, który zapisuje do swojego `context`.
- Późniejsza wersja dopasowuje zadania po `code`. Zadanie pominięte w nowej wersji zostaje wyłączone; jego historia pozostaje.
- Ustaw `enabled = false`, aby wstrzymać zadanie.
- W przypadku konektora CSV Attricat uzupełnia identyfikator Schematu, wersję i kontekst w profilu dla każdego przebiegu, więc TOML potrzebuje tylko kolumn (oraz `business_key` przy importach).

Uruchom zadanie ręcznie:

```sh
acli connector-job list <blueprint-id>
acli connector-job run <job-id> --idempotency-key manual-2026-03-01
```

Zadania konektorów wymagają wydania korzystającego z ujednoliconego ABI hosta (1.6+) lub ze świata konektorów `catalog:host@1.4.0`, a zarządzanie nimi wymaga `extensions.manage`. Zadań nie można jeszcze wyzwalać zdarzeniami katalogu; uruchamiaj je ręcznie lub cyklicznie.
