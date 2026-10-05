---
title: Kontrybucje klienckie
description: Dodawaj strony, panele, akcje i komórki tabeli do aplikacji webowej z izolowanych ramek i korzystaj z klienckiego API katalogu.
---

Kontrybucja kliencka to moduł JavaScript, który Attricat ładuje do izolowanej ramki w stałym miejscu aplikacji webowej. Host odpowiada za wszystko wokół ramki: układ, stany ładowania i błędów, fokus i punkty orientacyjne dostępności. Twój moduł odpowiada za to, co jest wewnątrz.

## Piaskownica

Każda kontrybucja dostaje własny element `<iframe sandbox="allow-scripts">` z nieprzezroczystym pochodzeniem (opaque origin) i zasadami Content Security Policy blokującymi dostęp do sieci. Ramka nie ma dostępu do strony Attricat, plików cookie, magazynu ani ramek innych rozszerzeń. Nie może wywoływać `fetch` wobec API.

Wszystko przechodzi przez obiekt `catalog`, który host przekazuje do Twojego modułu. Każde wywołanie jest sprawdzane pod kątem przyznanych uprawnień i kontekstu kontrybucji.

## Kontrakt modułu

Artefakt kliencki musi eksportować `mount(root, catalog)`. Może zwrócić funkcję czyszczącą, synchroniczną lub asynchroniczną, która zostanie uruchomiona po usunięciu ramki.

```js
export const mount = (root, catalog) => {
  const render = () => {
    root.dataset.mode = catalog.theme.color_mode;
    root.textContent = `Entity: ${catalog.context.entity_id}`;
  };
  root.addEventListener('catalog:context-changed.v1', render);
  root.addEventListener('catalog:theme-changed.v1', render);
  render();
  return () => root.replaceChildren();
};
```

Ramka pozostaje zamontowana, gdy użytkownik przełącza kontekst lub motyw. Nasłuchuj zdarzeń zmiany i aktualizuj widok, a wszelką pracę związaną z poprzednim kontekstem porzucaj.

## API `catalog`

| Element | Wymaga | Opis |
| --- | --- | --- |
| `catalog.context` | | Identyfikatory dla miejsca osadzenia, np. `entity_id` i `context_id`. Obecne są tylko pola udokumentowane dla danego miejsca osadzenia. |
| `catalog.theme` | | `{ color_mode: 'light' \| 'dark' }`. Właściwość `color-scheme` ramki jest ustawiana odpowiednio przed `mount`, więc kolory systemowe, takie jak `Canvas` i `CanvasText`, za nią podążają. |
| `catalog.configuration` | `configuration.read` | Konfiguracja instalacji. |
| `catalog.request(path)` | `catalog.read` | `GET` jednego z adresów `/api/entities`, `/api/v1/entities/<uuid>` lub `/api/blueprints/<uuid>/versions/<n>`. Odpowiedzi są ograniczone do 1 MiB. Odczyty wersji w `blueprint_attribute_configuration` są ograniczone do wersji Schematu tego miejsca osadzenia. |
| `catalog.command({ command_id, payload })` | `client.commands` | Wywołuje jedno z zadeklarowanych poleceń serwerowych rozszerzenia. |
| `catalog.storage.get/set/delete/list(…)` | `storage.extension` | Magazyn klucz-wartość rozszerzenia. `set` i `delete` przyjmują `expected_revision`. |
| `catalog.navigate({ entity_id })` | `client.navigation` | Otwiera stronę encji. |
| `catalog.notify({ message, severity })` | `client.notification` | Wyświetla powiadomienie hosta. Komunikaty są przycinane do 512 znaków. |
| `catalog.refresh({ target: 'current_entity' })` | `client.refresh` | Ponownie ładuje widoki bieżącej encji po zmianie wprowadzonej przez Twoje polecenie. Dostępne w miejscach osadzenia encji. |
| `catalog.dialog.open()` / `catalog.dialog.close()` | `client.action_dialog` | Otwiera `action_dialog` rozszerzenia z akcji zaznaczenia w wersji 2, z zaznaczeniem tej akcji; `close` działa wewnątrz okna. |
| `catalog.operations.start({ operation_id, input, idempotency_key })` | `client.operations.start` | Uruchamia [operację interaktywną](/pl/extensions/operations/#operacje-interaktywne) dla zaznaczenia ramki i zwraca `{ run_id }`. Dostępne w akcjach zaznaczenia w wersji 2 i w oknie akcji. |
| `catalog.operations.list()` / `get({ run_id })` / `download({ run_id, artifact_id })` | `client.operations.read` | Tylko uruchomienia tego rozszerzenia rozpoczęte przez zalogowanego użytkownika, także gdy jest on operatorem. Pobieranie wykonuje host. |
| `catalog.operations.cancel({ run_id })` | `client.operations.cancel` | Anuluje jedno z tych uruchomień. |

| Zdarzenie na `root` | Wymaga | Wywoływane |
| --- | --- | --- |
| `catalog:context-changed.v1` | `client.events` | Na starcie i przy każdej zmianie kontekstu miejsca osadzenia. |
| `catalog:theme-changed.v1` | | Za każdym razem, gdy użytkownik przełącza tryb jasny i ciemny. |

Kolejne pośredniczone operacje, każda z własnym uprawnieniem, obejmują okna potwierdzenia, pobieranie plików, otwieranie dozwolonych adresów HTTPS, odczyt i przesyłanie plików w kontekście pliku, wyszukiwanie w katalogu, aktualizacje na żywo, zapis do schowka i odczyt ustawień regionalnych. Okna dialogowe i postęp tych operacji rysuje host.

Twój interfejs musi dostarczać własne przetłumaczone teksty i dostępne etykiety.

## Miejsca osadzenia

### Pełne strony

Kontrybucja `route` to strona pod adresem `/extensions/<extension-id>/<contribution-id>`. Może to być cała aplikacja: wieloetapowy kreator importu albo warsztat z własnymi ekranami listy i szczegółów. Używaj routera działającego w pamięci wewnątrz ramki; adres URL przeglądarki pozostaje taki sam i nie możesz dodawać tras hosta.

Dodaj kontrybucję `navigation`, aby umieścić do niej link na pasku bocznym. Administratorzy obszaru roboczego decydują, czy pojawi się w grupie rozszerzeń, czy zostanie przeniesiony do głównej nawigacji.

### Strony encji

| Miejsce osadzenia | Rodzaj | Uprawnienie | Kontekst |
| --- | --- | --- | --- |
| `entity_preview_panel` | `embedded` | | `entity_id`, opcjonalnie `context_id` |
| `entity_action` | `embedded` | `client.entity_action` | encja, atrybut, kontekst |
| `entity_attribute_decoration` | `embedded` | `client.entity_decoration` | encja, atrybut, Schemat i wersja, opcjonalnie kontekst |
| `entity_header_action` | `action` | `client.entity_header_action` | `entity_id`, `blueprint_id`, `blueprint_version` |
| `entity_attribute_panel` | `panel` | `client.entity_attribute_panel` | `entity_id`, `attribute_id`, `blueprint_id`, `blueprint_version`, `context_id` |
| `file_panel` | `panel` | `client.file_panel` | `file_id`, `entity_id`, `attribute_id`, `blueprint_id`, `blueprint_version` |

`entity_preview_panel` pojawia się w szufladzie rozszerzeń encji. `entity_action` w wersji 2 otrzymuje zamiast tego [kontekst zaznaczenia](#kontekst-zaznaczenia). Paski akcji pokazują jedną akcję główną i trzy dodatkowe przed menu przepełnienia. Panele pokazują do trzech kontrybucji przed przepełnieniem.

### Przeglądarka encji

| Miejsce osadzenia | Rodzaj | Uprawnienie | Kontekst |
| --- | --- | --- | --- |
| `explorer_row_action` | `action` | `client.explorer_row_action` | `entity_id`, `blueprint_id`, `blueprint_version` |
| `explorer_action` | `action` | `client.explorer_action` | `blueprint_id`, `blueprint_version` |
| `explorer_bulk_action` | `action` | `client.explorer_bulk_action` | `blueprint_id`, `blueprint_version`, zaznaczone `entity_ids` (od 1 do 50) |
| `explorer_table_cell` | `embedded` | `client.explorer_table_cell` | Wartość komórki, dla kolumny używającej Twojego [renderera komórek](/pl/extensions/manifest/#renderery-komórek). |

Konteksty przeglądarki encji nigdy nie zawierają zapytania wyszukiwania, filtrów ani wartości wierszy. Zaznaczenie to wskazówka, na co patrzy użytkownik, a nie autoryzacja: polecenia nadal sprawdzają uprawnienia na serwerze. Akcje wiersza i akcje zbiorcze w wersji 2 otrzymują [kontekst zaznaczenia](#kontekst-zaznaczenia).

### Kontekst zaznaczenia

Kontrybucje do `entity_action`, `explorer_row_action` i `explorer_bulk_action` mogą zadeklarować `"version": 2`. Otrzymują wtedy jeden kształt kontekstu na każdej powierzchni:

```json
{
  "context_version": 2,
  "selection_source": "entity_preview",
  "blueprint_id": "…",
  "blueprint_version": 3,
  "context_id": null,
  "entity_ids": ["…"]
}
```

`selection_source` to `entity_preview`, `explorer_row` lub `explorer_selection`. `entity_ids` zawiera od 1 do 50 zapisanych encji jednej wersji schematu w kolejności wyświetlania. `context_id` to kontekst rozwiązywania wartości na danej powierzchni albo `null` dla domyślnego. Kontrybucje w wersji 1 zachowują opisane wyżej konteksty.

### Okno akcji

| Miejsce osadzenia | Rodzaj | Uprawnienie | Kontekst |
| --- | --- | --- | --- |
| `action_dialog` | `dialog` | `client.action_dialog` | Kontekst zaznaczenia akcji, która je otworzyła, utrwalony w chwili otwarcia. |

Kontrybucja `dialog` wymaga pola `title`. Twoje akcje w wersji 2 otwierają ją przez `catalog.dialog.open()`. Host rysuje okno wokół Twojej ramki, pokazuje, ilu encji dotyczy, i ostrzega o niezapisanych zmianach. Okno pozostaje otwarte po zamknięciu menu wiersza lub zmianie zaznaczenia i zamyka się przy nawigacji. Naciśnięcia klawiszy w Twojej ramce nie docierają do hosta, więc wywołaj `catalog.dialog.close()`, gdy użytkownik naciśnie Escape. Zamknięcie przed uruchomieniem niczego nie tworzy; zamknięcie później nie anuluje uruchomienia.

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

Każdy pozostały obiekt kontekstu zawiera `context_version: 1`.

## Kolejność i usuwanie

Kontrybucje w tym samym miejscu osadzenia są uporządkowane według identyfikatora rozszerzenia, a następnie identyfikatora kontrybucji, chyba że administrator obszaru roboczego lub Schemat ustawi układ. Wyłączenie, kwarantanna, uaktualnienie lub cofnięcie uprawnienia usuwa kontrybucje; otwarte ramki sprawdzają to cyklicznie i są zamykane w ciągu 15 sekund, a ich oczekujące wywołania są odrzucane.

Jeśli ramka nie uruchomi się, host wyświetla w jej miejscu ostrzeżenie, nie ujawniając Twojego kodu ani szczegółów hosta.
