---
title: Operacje i konektory
description: Długotrwałe operacje rozszerzeń z punktami kontrolnymi do importów, eksportów i przesyłania plików, z harmonogramami i zadaniami konektorów w Schematach.
---

**Operacja** to długotrwała praca po stronie serwera, np. eksport 200 000 produktów do CSV albo import pliku od dostawcy. Operacje działają w tle w partiach, po każdej partii zapisują punkt kontrolny i przetrwają ponowne uruchomienia.

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

Komponent implementuje `prepare`, `start`, `process-batch`, `checkpoint`, `finish` i `cancel`. Każde wywołanie otrzymuje `operation-request` z identyfikatorem wykonania, wartością `handler` operacji jako `operation-id`, migawką konfiguracji, zwalidowanymi danymi wejściowymi, ostatnim punktem kontrolnym i **kluczem partii**. Konfiguracja, dane wejściowe i punkt kontrolny są tekstami JSON.

Attricat wykonuje partie po kolei:

1. Przy pierwszej partii wywołuje `prepare`, a potem `start`. Jeśli proces roboczy zatrzyma się przed ukończeniem pierwszej partii, zostaną wywołane ponownie, więc nie wykonuj w nich skutków ubocznych, których nie można powtórzyć.
2. Wywołuje `process-batch`. Wykonaj ograniczoną porcję pracy i zwróć `{checkpoint, progress, done}`. `checkpoint` i `progress` muszą być obiektami JSON; punkt kontrolny ma najwyżej 64 KiB, więc przechowuj w nim kursory i liczniki, a nie dane.
3. Zapisuje punkt kontrolny i wywołuje z nim `checkpoint`.
4. Gdy `done` ma wartość true, wywołuje `finish`. W przeciwnym razie następna partia dostaje zapisany punkt kontrolny i nowy klucz partii.

Punkt kontrolny początkowo ma wartość `{}`. Gdy ktoś anuluje trwającą operację, następna partia wywołuje `cancel` zamiast `process-batch` i wykonanie się kończy.

## Odraczanie i niepowodzenia partii (1.1)

Komponent zbudowany dla [świata](/pl/extensions/server/#wersja-api-hosta) `outcome-extension` eksportuje `deferrable-operations` zamiast `operations`. Jego `process-batch` zwraca `result<batch-outcome, failure>`.

`batch-outcome` dodaje do wyniku z wersji 1.0 pole `resume-after-ms`. Attricat zapisuje punkt kontrolny, zwalnia wykonanie i ponownie umieszcza je w kolejce po tym czasie (najwyżej 24 godziny) zamiast od razu. Używaj tego, aby poczekać, aż miejsce docelowe przetworzy plik zasilający (feed), aby odpytywać długotrwałe zadanie albo aby odczekać po ograniczeniu liczby żądań. Lista wykonań pokazuje `resume_at`, dopóki wykonanie czeka.

`failure` przyjmuje jedną z postaci:

- `rejected { code, message, record-id?, connection? }`: gdy wskazuje rekord, a rozszerzenie ma `exports.mappings`, status eksportu rekordu zmienia się na `rejected`, a partia uruchamia się ponownie po sekundzie. Mapowanie jest już rozstrzygnięte, więc komponent pomija rekord, a reszta wykonania jest kontynuowana. Każdy nowo odrzucony rekord to postęp, więc partia może odrzucić dowolnie wiele rekordów; powtórzenie odrzucenia albo odrzucenie rekordu, który trzyma inny proces, liczy się jak opóźnienie ponowienia, najwyżej 100 razy z rzędu. Rekord trzymany przez inny proces jest odtwarzany po wygaśnięciu jego dzierżawy. Odrzucenie, którego nie da się zapisać, np. dla nieznanego połączenia lub rekordu spoza wykonania, oraz każde inne odrzucenie zamieniają wykonanie w martwą wiadomość z kodem `rejected`, bez ponowień.
- `retry { message, after-ms? }`: partia jest wykonywana ponownie z tym samym kluczem partii po podanym czasie. Z `after-ms` czeka bez zużywania budżetu niepowodzeń wykonania, najwyżej 100 razy z rzędu, zanim jakaś partia się zakończy; potem, a także bez `after-ms`, każde ponowienie liczy się do tego budżetu.
- `fault(message)`: wykonanie kończy się błędem, a instalacja zostaje poddana kwarantannie.

Pułapki (trap), wyczerpanie paliwa lub pamięci, przekroczenie limitu czasu i nieprawidłowy wynik partii również są traktowane jak `fault`. Błąd zwrócony przez `prepare`, `start`, `checkpoint`, `finish` lub `cancel` kończy wykonanie błędem bez kwarantanny instalacji.

Wykonanie, którego wydanie zostanie wyłączone, poddane kwarantannie lub utraci uprawnienie w trakcie partii, jest wstrzymywane, a nie kończy się błędem.

## Jak wykonania zachowują trwałość

- Wykonanie jest tworzone raz dla danego obszaru roboczego, wydania, operacji i klucza idempotencji. Ponowne uruchomienie z tym samym kluczem zwraca to samo wykonanie.
- Wykonanie jest przypięte do wydania, na którym się rozpoczęło. Wyłączenie, kwarantanna, uaktualnienie lub cofnięcie uprawnienia wstrzymuje je, dopóki to dokładnie wydanie nie zostanie ponownie autoryzowane; nigdy nie przechodzi na nowy kod.
- Po każdej partii zapisywany jest punkt kontrolny. Klucz partii zmienia się dopiero po zapisaniu punktu kontrolnego.
- Jeśli proces roboczy ulegnie awarii przed punktem kontrolnym, partia jest wykonywana ponownie z **tym samym kluczem partii**. Jeśli Twój komponent zapisuje do katalogu lub systemu zewnętrznego, zadbaj, aby te zapisy były idempotentne względem klucza partii.
- Anulowanie wykonania oczekującego w kolejce kończy je natychmiast. Anulowanie trwającego prosi komponent o zatrzymanie się przy następnej partii.
- Wykonanie, które stale kończy się błędem, staje się martwą wiadomością (dead letter), a rekordy, które dzierżawił, ale ich nie rozstrzygnął, przechodzą w stan `retrying` z błędem. Operator może je odtworzyć; odtworzone wykonanie zaczyna z nowym budżetem niepowodzeń i 100 nowymi odroczeniami.

## Uruchamiaj, obserwuj i pobieraj

```sh
acli extension operation start acme.export --operation-id export \
  --input '{"profile":{"version":1}}' --idempotency-key export-2026-03-01
acli extension operation list
acli extension operation get <run-id>
acli extension operation artifacts <run-id>
acli extension operation download <run-id> <artifact-id> --output products.csv
acli extension operation cancel <run-id>
acli extension operation replay <run-id>
```

`--input-file-id` dołącza gotowy plik obszaru roboczego jako dane wejściowe operacji. Komponent otwiera go jako `open-input("source")`; nigdy nie widzi identyfikatorów plików ani kluczy magazynu.

Listy wykonań nigdy nie zawierają danych wejściowych, konfiguracji ani punktów kontrolnych. Wartości w konfiguracji i diagnostyce, których nazwy sugerują sekrety, dane uwierzytelniające, hasła, tokeny, klucze lub autoryzację, są zastępowane przez `[redacted]`.

## Dane wyjściowe

Z `artifacts.write` komponent buduje pliki wyjściowe po kawałku:

- `artifacts.append-output(name, media-type, batch-key, bytes)` dodaje od 1 do 65 536 bajtów dla bieżącej partii. Powtórzenie tych samych bajtów dla tego samego klucza partii jest ignorowane; inne bajty są odrzucane.
- `artifacts.finalize-output(name)` składa kawałki w jeden plik z sumą kontrolną.

Limity wynoszą 1 GiB na plik wyjściowy, 2 GiB na wykonanie i 8 GiB na obszar roboczy. Dane wyjściowe można pobrać po zakończeniu wykonania i są przechowywane przez 30 dni.

## Operacje interaktywne

Dodaj `interactive` do operacji, aby zalogowani użytkownicy mogli ją uruchomić dla rekordu lub zaznaczenia z [okna akcji](/pl/extensions/client/#okno-akcji) otwieranego przez jedną z Twoich akcji:

```json
{"id": "generate", "handler": "generate", "request_schema": {"type": "object"}, "interactive": {"version": 1, "max_selection": 50}}
```

Wymaga to `client.operations.start`. Zbuduj komponent dla [świata](/pl/extensions/server/#wersja-api-hosta) `attricat-extension` lub `operation-extension`; jego interfejs `selection` działa w uruchomieniu interaktywnym.

Przy starcie Attricat sprawdza, czy użytkownik może odczytać każdy zaznaczony rekord, i utrwala użytkownika, wydanie, dane wejściowe, kontekst oraz uporządkowane zaznaczenie. Następnie:

- `selection.describe()` zwraca liczbę rekordów, wersję schematu i kontekst.
- `selection.page(cursor, limit)` zwraca od 1 do 10 rekordów z zapisanymi wartościami rozwiązanymi w kontekście uruchomienia, Twoimi adnotacjami i `read_at`. Rekordy, których użytkownik nie może już odczytać, mają status `unavailable`, a usunięte `deleted`.
- `attricat-data.read` i wywołania konektorowego interfejsu `catalog` są odrzucane. `attricat-data.batch` przyjmuje intencje `update`, `relationships` i `annotate` tylko dla zaznaczonych rekordów, sprawdzane względem bieżących uprawnień użytkownika.
- Jeśli użytkownik opuści obszar roboczy, uruchomienie zatrzymuje się z bezpiecznym powodem zamiast działać dalej z uprawnieniami rozszerzenia.

Pobierz z każdego rekordu to, czego potrzebujesz, raz i zapisz w punkcie kontrolnym, aby ponowiona paczka tworzyła te same bajty. Raportuj postęp jako `{"completed": n, "total": n, "outcome": {"succeeded": n, "failed": n, "skipped": n}}`; Attricat pokazuje te liczby niezależnie od statusu uruchomienia, więc uruchomienie może się zakończyć mimo niepowodzeń części rekordów.

Użytkownicy widzą swoje uruchomienia w **Profil → Uruchomienia rozszerzeń**. Uruchomienie widzą tylko osoba, która je rozpoczęła, i osoby z `extensions.manage`. Osoba, która je rozpoczęła, zawsze może je anulować, ale otworzyć je i pobrać wyniki może tylko dopóty, dopóki może odczytać każdy zaznaczony rekord. Osoby z `extensions.manage` mogą otwierać, anulować i pobierać każde uruchomienie.

## Adnotacje rekordów

Z `attricat.annotations.write` dodawaj do paczki intencje `annotate`, aby zapisać informacje o rekordzie we własnej przestrzeni nazw: tagi `<extension-id>:<tag>` i obiekt w `system_metadata[<extension-id>]`. Intencje wskazują rekord przez `record_id`.

```json
{"kind": "annotate", "intent_key": "doc-<run>-<record>", "record_id": "…",
 "add_tags": ["document-generated"], "set_metadata": {"last_document": {"template_version": 2}},
 "remove_tags": [], "remove_metadata": [], "expected_revision": null}
```

Podajesz tylko lokalne nazwy tagów i kluczy; Attricat dodaje przestrzeń nazw. Łatka zawiera od 1 do 32 operacji. Ustawienie klucza zastępuje jego wartość (`null` jest dozwolone). `expected_revision` odrzuca zapis, jeśli przestrzeń nazw zmieniła się od odczytu; ponowiony klucz intencji jest najpierw zgłaszany jako `already_applied`. Inni zapisujący, w tym użytkownicy edytujący rekord, nie mogą zmienić Twojej przestrzeni nazw, a Twoje zapisy nie zmieniają `updated_at` rekordu.

Jeśli rekordy mają już dane pod identyfikatorem Twojego rozszerzenia, operator musi przejąć przestrzeń nazw przed Twoim pierwszym zapisem. Nie zapisuj w adnotacjach podpisanych adresów URL ani sekretów i nie traktuj tagu jako dowodu, że plik nadal można pobrać: wyniki wygasają.

## Przesyłanie plików

Z `network.request` i uprawnieniem hosta, które ustawia `max_transfer_bytes`, komponent może przenosić duże pliki przez HTTPS bez przekazywania bajtów przez JSON:

- `transfer.fetch-input` pobiera jeden zakres bajtów, do 16 MiB, do artefaktu wejściowego. Kolejne zakresy wymagają ETag, aby źródło nie mogło się zmienić między zakresami.
- `transfer.deliver-output` strumieniuje gotowy plik wyjściowy metodą `PUT` albo `POST`, jeśli uprawnienie hosta deklaruje `idempotent_delivery: true`. Każde dostarczenie zawiera stały nagłówek `Idempotency-Key`.

Próba dostarczenia jest zapisywana przed jakimkolwiek ruchem sieciowym. Po przekroczeniu limitu czasu lub awarii wynik to `uncertain` i nigdy nie jest wysyłany ponownie automatycznie. `acli extension operation deliveries <run-id>` pokazuje historię dostarczeń.

API hosta 1.1 dodaje wersje dopasowane do przesyłania plików do marketplace'ów i magazynów w chmurze:

- `transfer.fetch-input-v2` i `transfer.deliver-output-v2` przyjmują pola v1 oraz `headers` (nagłówki, nad którymi masz kontrolę, np. dokładny `Content-Type`, dla którego podpisano adres URL) i `connection` (dodaje token [połączenia](/pl/extensions/exports/#połączenia)). Przyjmują ciągi zapytania, np. podpisane adresy URL S3 lub GCS, gdy reguła ustawia `allow_query`. Gdy pobieranie z `connection` otrzyma 401, token jest odrzucany, a wywołanie odpowiada `{"outcome": "failed", "status": 401}`, więc możesz ponowić je z nowym tokenem. `deliver-output-v2`, którego połączenie lub uzgodnienie TLS się nie powiedzie, odpowiada `{"outcome": "unreachable"}`: nic nie zostało wysłane i to samo dostarczenie z tym samym kluczem można wysłać ponownie.
- Dostarczenie v2 wysyła `Idempotency-Key` tylko wtedy, gdy przekażesz `idempotency_key`. `PUT` na podpisany adres URL jest idempotentny sam w sobie. `POST` nadal wymaga `idempotent_delivery: true` w regule.
- `multipart: {"fields": [{"name", "value"}], "file_field", "filename"?}` wysyła formularz `multipart/form-data` z artefaktem jako częścią z plikiem, na potrzeby przesyłania etapowego w postaci formularza.
- Dostarczenie v2 może przesłać nie tylko dane wyjściowe, ale też ukończony artefakt wejściowy wykonania.
- `transfer.open-record-file({"record_id", "file_id", "variant"?})` kopiuje plik, do którego odwołuje się aktywny rekord, do 16 MiB, do artefaktu wejściowego wykonania i zwraca jego identyfikator, aby można go było przesłać bezpośrednio. `variant` wybiera wariant obrazu zamiast oryginału. Rekordy, które współdzielą plik, dostają ten sam artefakt. Wymaga `attricat.read`, `attricat.files.read` i `artifacts.read`. Wykonania konektorów mogą otwierać tylko pliki eksportowanych rekordów i aktywnych rekordów, do których one linkują (rekordów powiązanych rozwijanych przez `page-v2`), a wykonania interaktywne tylko pliki z zaznaczenia.

Każde żądanie wykonane przy przesyłaniu jest wliczane do [limitu żądań](/pl/extensions/manifest/#limity-żądań) jego reguły.

## Dostęp do katalogu w operacjach

Operacje mogą wywoływać `attricat-data.read` i `attricat-data.batch` (ten sam JSON co `attricat.read.v1` i `attricat.command.v1`) oraz wywołania w kształcie konektora: `schema`, `page` i `upsert-batch`. Strony mieszczą do 100 rekordów, a partie do 100 intencji. Partie muszą zawierać bieżący klucz partii, a klucz każdej intencji jest zapisywany, więc odtworzona partia zwraca `already_applied` zamiast zapisywać dwukrotnie.

Kursory stron rozwiązują wartości według stanu z pierwszej strony, korzystając z historii wartości, i wygasają po 30 dniach. To nie jest migawka bazy danych: rekordy tworzone, usuwane lub migrowane w trakcie długiego eksportu nadal mogą zmienić to, które rekordy się pojawią. Jeśli potrzebujesz dokładnego eksportu, zamroź źródło.

W wykonaniu konektora `attricat-data.batch` przyjmuje też partie zawierające wyłącznie adnotacje rekordów Schematu zadania, dzięki czemu eksporter może zapisać w [adnotacjach](#adnotacje-rekordów) zewnętrzne identyfikatory lub stan synchronizacji tam, gdzie widzą je kontrole i przeglądarka rekordów. Wykonania eksportu nadal nie mogą zapisywać wartości.

Wykonania eksportu odczytują zamiast tego rekordy wywołaniami `attricat.page-v2` i `attricat.withdrawn` z wersji 1.1. Zobacz [Konektory eksportu](/pl/extensions/exports/#odczyt-rekordów-do-eksportu).

## Harmonogramy

```sh
acli extension schedule create acme.export --operation-id export \
  --input '{"profile":{"version":1}}' --interval-seconds 86400
acli extension schedule update <schedule-id> --enabled false --interval-seconds 86400
```

Harmonogram uruchamia operację w odstępach od 60 sekund do 30 dni. Przegapione interwały i nakładające się wykonania są pomijane. Harmonogram jest wstrzymywany, gdy jego wydanie jest wyłączone, poddane kwarantannie, uaktualnione lub nie ma wymaganego uprawnienia.

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
- Zadanie **eksportu** tworzy jedno wykonanie dla każdego włączonego [kanału eksportu](/pl/guides/publishing/) albo tylko dla kanału wskazanego jako `context`. Każde wykonanie odczytuje wartości w kontekście tego kanału i obejmuje tylko rekordy do niego opublikowane.
- Zadanie **importu** tworzy jedno wykonanie, które zapisuje do swojego `context`.
- Późniejsza wersja dopasowuje zadania po `code`. Zadanie pominięte w nowej wersji zostaje wyłączone; jego historia pozostaje.
- Ustaw `enabled = false`, aby wstrzymać zadanie.
- Zadanie eksportu może wskazać `connection`, do którego wysyła dane. Bez niego używa domyślnego miejsca docelowego rozszerzenia.
- W przypadku konektora CSV Attricat uzupełnia identyfikator Schematu, wersję i kontekst w profilu dla każdego wykonania, więc TOML potrzebuje tylko kolumn (oraz `business_key` przy importach).

Uruchom zadanie ręcznie:

```sh
acli extension connector-job list <blueprint-id>
acli extension connector-job run <job-id> --idempotency-key manual-2026-03-01
```

Zarządzanie zadaniami konektorów wymaga `extensions.manage`.

Zadania eksportu mogą też uruchamiać się po zmianach rekordów, wysyłać dane do wskazanego połączenia i być ponownie synchronizowane. Zobacz [Konektory eksportu](/pl/extensions/exports/).
