---
title: Kontrybucje klienckie
description: Dodawaj strony, panele, akcje i komórki tabeli do aplikacji webowej z izolowanych ramek i korzystaj z klienckiego API katalogu.
---

Kontrybucja kliencka to moduł JavaScript, który Attricat ładuje do izolowanej ramki w stałym miejscu aplikacji webowej. Host odpowiada za wszystko wokół ramki: układ, stany ładowania i błędów, fokus i punkty orientacyjne dostępności. Twój moduł odpowiada za to, co jest wewnątrz.

## Piaskownica

Każda kontrybucja dostaje własny element `<iframe sandbox="allow-scripts">` z nieprzezroczystym pochodzeniem (opaque origin) i zasadami Content Security Policy blokującymi dostęp do sieci. Ramka nie ma dostępu do strony Attricat, plików cookie, magazynu ani ramek innych rozszerzeń. Nie może wywoływać `fetch` wobec API.

Wszystko przechodzi przez obiekt `attricat`, który host przekazuje do Twojego modułu. Każde wywołanie jest sprawdzane pod kątem przyznanych uprawnień i kontekstu kontrybucji.

## Kontrakt modułu

Artefakt kliencki musi eksportować `mount(root, attricat)`. Może zwrócić funkcję czyszczącą, synchroniczną lub asynchroniczną, która zostanie uruchomiona po usunięciu ramki.

```js
export const mount = (root, attricat) => {
  const render = () => {
    root.dataset.mode = attricat.theme.color_mode;
    root.textContent = `Record: ${attricat.context.record_id}`;
  };
  root.addEventListener('attricat:context-changed.v1', render);
  root.addEventListener('attricat:theme-changed.v1', render);
  render();
  return () => root.replaceChildren();
};
```

Ramka pozostaje zamontowana, gdy użytkownik przełącza kontekst lub motyw. Nasłuchuj zdarzeń zmiany i aktualizuj widok, a wszelką pracę związaną z poprzednim kontekstem porzucaj.

## API `attricat`

| Element | Wymaga | Opis |
| --- | --- | --- |
| `attricat.context` | | Identyfikatory dla miejsca osadzenia, np. `record_id` i `context_id`. Obecne są tylko pola udokumentowane dla danego miejsca osadzenia. |
| `attricat.theme` | | `{ color_mode: 'light' \| 'dark' }`. Właściwość `color-scheme` ramki jest ustawiana odpowiednio przed `mount`, więc kolory systemowe, takie jak `Canvas` i `CanvasText`, za nią podążają. |
| `attricat.configuration` | `configuration.read` | Konfiguracja instalacji. |
| `attricat.request(path)` | `attricat.read` | `GET` jednego z adresów `/api/records`, `/api/v1/records/<uuid>` lub `/api/blueprints/<uuid>/versions/<n>`. Odpowiedzi są ograniczone do 1 MiB. Odczyty wersji w `blueprint_attribute_configuration` są ograniczone do wersji Schematu tego miejsca osadzenia. |
| `attricat.command({ command_id, payload })` | `client.commands` | Wywołuje jedno z zadeklarowanych poleceń serwerowych rozszerzenia. |
| `attricat.storage.get/set/delete/list(…)` | `storage.extension` | Magazyn klucz-wartość rozszerzenia. `set` i `delete` przyjmują `expected_revision`. |
| `attricat.navigate({ record_id })` | `client.navigation` | Otwiera stronę rekordu. |
| `attricat.notify({ message, severity })` | `client.notification` | Wyświetla powiadomienie hosta. Komunikaty są przycinane do 512 znaków. |
| `attricat.refresh({ target: 'current_record' })` | `client.refresh` | Ponownie ładuje widoki bieżącego rekordu po zmianie wprowadzonej przez Twoje polecenie. Dostępne w miejscach osadzenia na stronach rekordów. |
| `attricat.dialog.close()` | `client.action_dialog` | Zamyka okno. Dostępne tylko wewnątrz okna akcji; okna otwiera host, gdy użytkownik wybierze [akcję](#akcje). |
| `attricat.operations.start({ operation_id, input, idempotency_key })` | `client.operations.start` | Uruchamia [operację interaktywną](/pl/extensions/operations/#operacje-interaktywne) dla rekordów akcji, która otworzyła okno, i zwraca `{ run_id }`. Dostępne tylko w oknie akcji. |
| `attricat.operations.list()` / `get({ run_id })` / `download({ run_id, artifact_id })` | `client.operations.read` | Tylko uruchomienia tego rozszerzenia rozpoczęte przez zalogowanego użytkownika, także gdy jest on operatorem. Pobieranie wykonuje host. |
| `attricat.operations.cancel({ run_id })` | `client.operations.cancel` | Anuluje jedno z tych uruchomień. |

| Zdarzenie na `root` | Wymaga | Wywoływane |
| --- | --- | --- |
| `attricat:context-changed.v1` | `client.events` | Na starcie i przy każdej zmianie kontekstu miejsca osadzenia. |
| `attricat:theme-changed.v1` | | Za każdym razem, gdy użytkownik przełącza tryb jasny i ciemny. |

Ramka nie ma innych składowych. W szczególności nie ma `fetch` do API, dostępu do adresu URL przeglądarki ani możliwości wywołania innego rozszerzenia. Manifest przyjmuje niektóre uprawnienia [zarezerwowane](/pl/extensions/manifest/#interakcja-z-klientem) dla przyszłych API ramki; ich przyznanie na razie niczego nie daje.

Wywołania zwracają obietnice. Wywołanie, na które nie pozwalają Twoje uprawnienia albo które host odrzuci, kończy się odrzuceniem z obiektem `Error`, którego komunikat podaje przyczynę.

Twój interfejs musi dostarczać własne przetłumaczone teksty i dostępne etykiety.

## Miejsca osadzenia

### Pełne strony

Kontrybucja `route` to strona pod adresem `/extensions/<extension-id>/<contribution-id>`. Może to być cała aplikacja: wieloetapowy kreator importu albo warsztat z własnymi ekranami listy i szczegółów. Używaj routera działającego w pamięci wewnątrz ramki; adres URL przeglądarki pozostaje taki sam i nie możesz dodawać tras hosta.

Dodaj kontrybucję `navigation`, aby umieścić do niej link na pasku bocznym. Administratorzy obszaru roboczego decydują, czy pojawi się w grupie rozszerzeń, czy zostanie przeniesiony do głównej nawigacji.

### Strony rekordów

| Miejsce osadzenia | Rodzaj | Uprawnienie | Kontekst |
| --- | --- | --- | --- |
| `record_preview_panel` | `embedded` | | `record_id`, opcjonalnie `context_id` |
| `record_action` | `action` | `client.record_action` | [Kontekst akcji](#akcje) z tym rekordem |
| `record_header_action` | `action` | `client.record_header_action` | [Kontekst akcji](#akcje) z tym rekordem |
| `record_attribute_action` | `action` | `client.record_field_notes` | [Kontekst akcji](#akcje) z tym rekordem i `attribute_code` pola. Dostępne tylko z odznaki [notatki pola](#notatki-pól). |

`record_preview_panel` pojawia się na karcie **Rozszerzenia** rekordu. Akcje rekordu to przyciski pod polami, cztery przed menu przepełnienia; akcje nagłówka to pozycje menu **Akcje** rekordu. Panele pokazują do trzech kontrybucji przed przepełnieniem. Pola i pliki nie mają ramek; opisuj je [notatkami pól](#notatki-pól).

### Notatki pól

Notatki pól to odznaki i krótkie notatki, które Attricat pokazuje przy polach i plikach rekordu oraz jako odznaki w komórkach przeglądarki rekordów. Wskaż w manifeście jedno źródło: polecenie serwerowe albo procedurę kliencką.

```json
"record_field_notes": { "version": 1, "source": { "command": "describe-fields" } }
"record_field_notes": { "version": 1, "source": { "client": "notes-handler" } }
```

Oba wymagają `client.record_field_notes`; polecenie wymaga też `client.commands`. Attricat pyta polecenie o najwyżej 25 rekordów naraz, `{ records: [{ record_id, blueprint_id, blueprint_version }], context_id, locale }`, i oczekuje:

```json
{
  "records": {
    "<id rekordu>": {
      "attributes": {
        "price_gross": {
          "badge": { "label": "Wyliczane", "tone": "info", "icon": "calculator", "tooltip": "price_net * 1.23", "action": "recalculate-field" },
          "note": "price_gross = price_net * 1.23"
        }
      },
      "files": { "<id pliku>": { "note": "Ostatni dokument: szablon summary." } },
      "actions": { "recalculate-record": { "disabled": "Formuły są nieprawidłowe" } }
    }
  },
  "cacheable": false
}
```

| Pole | Ograniczenie |
| --- | --- |
| `badge.label` | od 1 do 24 znaków |
| `badge.tone` | `neutral` (domyślnie), `info`, `success`, `warning`, `error` |
| `badge.icon` | jedna z [ikon hosta](#akcje) |
| `badge.tooltip` | do 200 znaków; podpowiedź podaje też nazwę Twojego rozszerzenia |
| `badge.action` | Twoja akcja w miejscu `record_attribute_action`; wybranie odznaki uruchamia ją z `attribute_code` pola w kontekście |
| `note` | do 280 znaków zwykłego tekstu |
| `actions` | dla każdej Twojej akcji: `{ "disabled": "<powód>" }` albo `{ "hidden": true }` |
| `cacheable` | `true` pozwala Attricat używać odpowiedzi przez godzinę, dopóki rekord i jego adnotacje się nie zmienią; w przeciwnym razie przez 30 sekund |

Na stronie rekordu odznaki i notatka pola pojawiają się w jednym wierszu pod nim, a notatka pliku pod plikiem. W przeglądarce rekordów odznaki stoją za wartością komórki, a notatka trafia do podpowiedzi odznaki. Notatki dla rekordów, atrybutów, plików lub akcji, których rozszerzenie nie ma, są pomijane, podobnie jak odpowiedź łamiąca te ograniczenia. Odpowiedź wolniejsza niż dwie sekundy jest pomijana, a Attricat wkrótce pyta ponownie, więc pierwsze wywołanie może skompilować Twój komponent bez wstrzymywania strony.

Procedura kliencka to komponent kliencki uruchamiany bez interfejsu w izolowanej ramce. Eksportuje `handle(request, attricat)`, otrzymuje `{ record_ids, context_id, locale }` i zwraca tę samą odpowiedź. Jedynym wywołaniem hosta jest `attricat.request`, które wymaga `attricat.read`.

### Przeglądarka rekordów

| Miejsce osadzenia | Rodzaj | Uprawnienie | Kontekst |
| --- | --- | --- | --- |
| `explorer_row_action` | `action` | `client.explorer_row_action` | [Kontekst akcji](#akcje) z rekordem wiersza |
| `explorer_action` | `action` | `client.explorer_action` | [Kontekst akcji](#akcje) bez rekordów |
| `explorer_bulk_action` | `action` | `client.explorer_bulk_action` | [Kontekst akcji](#akcje) z zaznaczonymi rekordami (od 1 do 50) |
| `explorer_table_cell` | `embedded` | `client.explorer_table_cell` | Wartość komórki, dla kolumny używającej Twojego [renderera komórek](/pl/extensions/manifest/#renderery-komórek). |

Akcje wiersza to pozycje menu akcji wiersza, akcje zbiorcze to pozycje menu **Akcje** zaznaczenia, a akcje paska narzędzi kryją się za jednym przyciskiem **Akcje rozszerzeń**. Konteksty przeglądarki rekordów nigdy nie zawierają zapytania wyszukiwania, filtrów ani wartości wierszy. Zaznaczenie to wskazówka, na co patrzy użytkownik, a nie autoryzacja: polecenia nadal sprawdzają uprawnienia na serwerze.

### Akcje

Akcja to pozycja menu lub przycisk, które Attricat rysuje sam. Deklarujesz jej tytuł i to, co robi jej wybranie; żadna ramka nie ładuje się, dopóki użytkownik jej nie wybierze.

```json
{ "id": "check", "version": 1, "kind": "action", "outlet": "record_header_action",
  "title": "Check formulas", "icon": "check", "invoke": { "command": "check-formulas" },
  "translations": { "pl": { "title": "Sprawdź formuły" } } }
{ "id": "recalculate-now", "version": 1, "kind": "action", "outlet": "explorer_bulk_action",
  "title": "Recalculate now", "invoke": { "operation": { "id": "recalculate", "input": { "mode": "write" } } },
  "when": { "requires_write": true, "max_records": 50 },
  "confirm": { "message": "Write recalculated values to the selected records?" } }
```

| `invoke` | Wymaga | Działanie |
| --- | --- | --- |
| `{ "command": "<id>" }` | `client.commands` | Wywołuje jedno z Twoich [poleceń serwerowych](/pl/extensions/manifest/#serwer); akcja może wywołać tylko to polecenie. |
| `{ "dialog": "<id>" }` | `client.action_dialog` | Otwiera jedną z Twoich kontrybucji `dialog`. |
| `{ "operation": { "id", "input" } }` | `client.operations.start` | Uruchamia jedną z Twoich [operacji interaktywnych](/pl/extensions/operations/#operacje-interaktywne) dla rekordów akcji z `input` ustalonym w manifeście. Niedostępne w `explorer_action`, które nie ma rekordów. |

| Pole opcjonalne | Działanie |
| --- | --- |
| `when` | Oferuje akcję tylko tam, gdzie ma sens: `blueprints` (kody), `min_records`, `max_records` (najwyżej 50), `has_tags`, `missing_tags`, `requires_write`. Attricat odrzuca też polecenia i uruchomienia akcji z `requires_write` dla rekordów, których użytkownik nie może zmieniać. |
| `confirm` | `{ "message", "destructive" }`: Attricat pyta przed uruchomieniem akcji. |
| `icon` | Jedna z `alert`, `calculator`, `check`, `clock`, `download`, `file`, `info`, `link`, `refresh`, `search`, `send`, `settings`, `sparkles`, `tag`, `wand`. |
| `translations` | Tytuły w innych językach, np. `{ "pl": { "title": "…" } }`. Może je mieć każda kontrybucja z tytułem. |

W jednym miejscu osadzenia możesz dodać kilka akcji. Zakończ tytuł znakiem `…`, gdy akcja otwiera okno. Attricat pokazuje też akcje strony w palecie poleceń (Ctrl/⌘+K). Twoje [notatki pól](#notatki-pól) mogą dla danego rekordu wyłączyć akcję z podaniem powodu albo ją ukryć.

Polecenie otrzymuje, a okno dostaje jako `attricat.context`, kontekst akcji:

```json
{
  "context_version": 1,
  "action_id": "generate",
  "outlet": "explorer_bulk_action",
  "blueprint_id": "…",
  "blueprint_version": 3,
  "context_id": null,
  "record_ids": ["…"],
  "locale": "pl"
}
```

`record_ids` zawiera zapisane rekordy jednej wersji Schematu w kolejności wyświetlania: jeden na stronie rekordu lub w wierszu, od 1 do 50 dla zaznaczenia, żadnego dla akcji paska narzędzi. Akcja odznaki dodaje `attribute_code`. `context_id` to kontekst rozwiązywania wartości na danej powierzchni albo `null` dla domyślnego. `locale` to język oglądającego.

Odpowiedź polecenia może powiedzieć Attricat, co zrobić dalej; pozostałe pola są pomijane.

| Pole | Działanie |
| --- | --- |
| `notification` | `{ "message", "severity" }` wyświetlane jako powiadomienie. |
| `refresh` | `true` ponownie ładuje widoki rekordów akcji i wyniki przeglądarki rekordów. |
| `navigate` | `{ "record_id" }` otwiera rekord; `{ "route" }` otwiera jedną z Twoich stron. |
| `download` | `{ "run_id", "artifact_id" }` pobiera wynik jednego z uruchomień Twojego rozszerzenia przez użytkownika. |
| `open_dialog` | ID jednego z Twoich okien, otwieranego z tym samym kontekstem. |

Attricat pokazuje, że polecenie trwa, a jeśli się nie powiedzie, podaje, która akcja zawiodła. Akcja operacji lub okno, które uruchamia operację, dostaje powiadomienie o jej zakończeniu, a rekordy uruchomienia są ponownie ładowane.

### Okno akcji

| Miejsce osadzenia | Rodzaj | Uprawnienie | Kontekst |
| --- | --- | --- | --- |
| `action_dialog` | `dialog` | `client.action_dialog` | Kontekst akcji, która je otworzyła, utrwalony w chwili otwarcia. |

Kontrybucja `dialog` wymaga pola `title`, a rozszerzenie może mieć kilka okien. Host rysuje okno wokół Twojej ramki, pokazuje, ilu rekordów dotyczy, i ostrzega o niezapisanych zmianach. Okno pozostaje otwarte po zamknięciu menu wiersza lub zmianie zaznaczenia i zamyka się przy nawigacji. Naciśnięcia klawiszy w Twojej ramce nie docierają do hosta, więc wywołaj `attricat.dialog.close()`, gdy użytkownik naciśnie Escape. Zamknięcie przed uruchomieniem niczego nie tworzy; zamknięcie później nie anuluje uruchomienia.

### Schematy

| Miejsce osadzenia | Rodzaj | Uprawnienie | Kontekst |
| --- | --- | --- | --- |
| `blueprint_attribute_configuration` | `embedded` | `client.blueprint_configuration` | Schemat, wersja, atrybut |
| `blueprint_detail_panel` | `panel` | `client.blueprint_detail_panel` | `blueprint_id`, `blueprint_version` |
| `blueprint_panel` | `panel` | `client.blueprint_panel` | `blueprint_id`, `blueprint_version`. Pozostaje widoczny na wszystkich kartach szczegółów. |
| `blueprint_publish_check` | `panel` | `client.blueprint_publish_check` | `blueprint_id`, `blueprint_version`. Wyświetlany w oknie publikowania. Wyłącznie informacyjny; nie może zablokować publikacji. |

### Inne miejsca

| Miejsce osadzenia | Rodzaj | Uprawnienie | Kontekst |
| --- | --- | --- | --- |
| `navigation` | `embedded` | | Kompaktowa ramka na pasku bocznym. |
| `audit_event_panel` | `panel` | `client.audit_event_panel` | `event_id` otwartego zdarzenia audytu. |
| `data_health_card` | `panel` | `client.data_health_card` | Brak. Karta na stronie Stan danych. |
| `export_mapping` | `panel` | `client.export_mapping` | `export_id`. Wyświetlany w sekcji stanu eksportu na stronie zarządzania Twoim rozszerzeniem, raz dla każdego miejsca docelowego eksportu zadeklarowanego w wydaniu. Służy do edycji dużych mapowań miejsca docelowego przechowywanych w magazynie rozszerzenia, np. atrybutów na atrybuty typu produktu. W przeciwieństwie do innych paneli może wywoływać Twoje polecenia serwerowe (z `client.commands`), więc polecenie może zwalidować mapowanie przed zapisaniem. Z tego panelu mogą je wywoływać tylko osoby z `extensions.manage`. |

Każdy pozostały obiekt kontekstu zawiera `context_version: 1`.

## Kolejność i usuwanie

Kontrybucje w tym samym miejscu osadzenia są uporządkowane według identyfikatora rozszerzenia, a następnie identyfikatora kontrybucji, chyba że administrator obszaru roboczego lub Schemat ustawi układ. Wyłączenie, kwarantanna, uaktualnienie lub cofnięcie uprawnienia usuwa kontrybucje; otwarte ramki sprawdzają to cyklicznie i są zamykane w ciągu 15 sekund, a ich oczekujące wywołania są odrzucane.

Jeśli ramka nie uruchomi się, host wyświetla w jej miejscu ostrzeżenie, nie ujawniając Twojego kodu ani szczegółów hosta.
