---
title: Dokumentacja TOML schematu
description: Wszystkie klucze akceptowane w definicji schematu, z typami, wartościami domyślnymi i regułami walidacji.
---

Ta strona wymienia wszystkie klucze akceptowane przez kompilator schematów. Przewodnik, który krok po kroku buduje schemat, znajdziesz w [Tworzenie schematu](/pl/builders/blueprints/).

Kompilator jest rygorystyczny. Nieznany klucz, wartość złego typu lub odwołanie do czegoś, co nie istnieje, powoduje odrzucenie całej definicji z `422 invalid_blueprint_definition` i komunikatem wskazującym problem.

## Kody

Kody schematów, kody atrybutów, aliasy dołączeń, cele relacji, kody reguł, kody zadań konektorów i kody ról w `[publication]` muszą być **kodami**: niepustymi ciągami złożonymi z liter ASCII, cyfr, `-` i `_`. `product`, `seo-fields` i `stock_on_hand` są prawidłowe. `product type` i `prodükt` nie są.

## Tłumaczone etykiety

Pola `name` schematu, `name` atrybutu, `label` kart i sekcji akordeonu, `columns[].label` tabeli oraz `label` bloku `incoming_relationship_list` mogą zawierać odwołania do leksykonu: `{{Product}}` lub `{{Order|purchase}}` z kontekstem. Tekst poza nawiasami jest dosłowny, a `\{{` zapisuje dosłowne `{{`. Nieprawidłowe odwołania są odrzucane przy zapisie schematu. Zobacz [Tłumaczenie etykiet](/pl/builders/translations/).

## Poziom główny

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"
entity_schema = '''{ "type": "object", "required": ["title"] }'''
```

| Klucz | Typ | Wymagany | Opis |
| --- | --- | --- | --- |
| `format_version` | liczba całkowita | Tak | Musi wynosić `1`. |
| `code` | kod | Tak | Identyfikator rodziny schematów. Nie może się zmieniać między wersjami. |
| `name` | ciąg znaków | Tak | Nazwa wyświetlana. Może się zmieniać między wersjami. |
| `kind` | `"entity"` lub `"mixin"` | Tak | Schemat `entity` może mieć encje. Domieszka (`mixin`) jedynie dostarcza atrybuty innym schematom przez `[[includes]]`. |
| `attributes` | tablica tabel | Tak | Co najmniej jeden atrybut. Zobacz [Atrybuty](#atrybuty). |
| `includes` | tablica tabel | Nie | Domieszki, z których ten schemat pobiera atrybuty. Zobacz [Dołączenia](#dołączenia). |
| `views` | tabela | Encja: tak | Układy dla aplikacji webowej. Schematy encji muszą definiować `views.dropdown_option`. Zobacz [Widoki](#widoki). |
| `entity_schema` | ciąg znaków (JSON) | Nie | JSON Schema dla całej encji. Tylko schematy encji. Zobacz [Walidacja](/pl/builders/validation/). |
| `publication` | tabela | Nie | Zasady ponownego zatwierdzania publikacji. Zobacz [Publikacja](#publikacja). |
| `rules` | tablica tabel | Nie | Reguły jakości danych należące do tego schematu. Zobacz [Reguły](/pl/builders/rules/). |
| `unique_keys` | tablica tabel | Nie | Klucze biznesowe, których wartości muszą być unikalne. Tylko schematy encji. Zobacz [Klucze unikalne](#klucze-unikalne). |
| `connector_jobs` | tablica tabel | Nie | Zaplanowane lub ręczne zadania importu i eksportu wykonywane przez rozszerzenie konektora. Tylko schematy encji. Zobacz [Zadania konektorów](#zadania-konektorów). |
| `extensions` | tabela | Nie | Dowolne dane dla rozszerzeń, w przestrzeniach nazw `[extensions.<extension-id>]`. Kompilator rdzenia je ignoruje; rozszerzenia odczytują je z zapisanej definicji. |

## Atrybuty

Każda tabela `[[attributes]]` deklaruje dokładnie jeden z kluczy `value_type`, `from` lub `extension_type`.

```toml
[[attributes]]
code = "title"
value_type = "string"
```

### Klucze wspólne

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `code` | kod | Wymagany | Unikalny w obrębie schematu, łącznie z atrybutami pobranymi z domieszek. |
| `name` | ciąg znaków | `code` w czytelnej formie | Czytelna etykieta wyświetlana w formularzach, filtrach, podglądach oraz jako domyślny nagłówek kolumny tabeli. Nie może być pusta. Niedozwolona razem z `from`; wybrany atrybut zachowuje nazwę atrybutu domieszki. |
| `value_type` | ciąg znaków | | Jeden z [typów wartości](#typy-wartości). |
| `from` | ciąg znaków | | `"<include-alias>.<attribute-code>"`. Materializuje atrybut z dołączonej domieszki. `code` musi być równy kodowi atrybutu domieszki. |
| `extension_type` | ciąg znaków | | `"<extension-id>:<type-id>@<semver-range>"`. Używa typu atrybutu zadeklarowanego przez włączone rozszerzenie. Zobacz [Typy atrybutów z rozszerzeń](#typy-atrybutów-z-rozszerzeń). |
| `context_fallback` | `"default"` lub `"none"` | `"default"` | Co pokazuje kontekst bez własnej wartości. `default` przechodzi w górę drzewa kontekstów do najbliższego przodka z wartością. `none` nie pokazuje niczego. |
| `context_editable` | `"all"` lub `"default"` | `"all"` | Gdzie można zapisywać wartości. `default` ogranicza zapisy do kontekstu domyślnego: inne konteksty pokazują pole tylko do odczytu, a API odrzuca w nich zapisy. |
| `readonly` | wartość logiczna | `false` | Pokazuje pole w aplikacji webowej, ale uniemożliwia jego edycję w niej. API, CLI, agenci, przepływy pracy i rozszerzenia nadal mogą je zapisywać. Używaj dla wartości, którymi zarządza integracja. |
| `tags` | tablica ciągów znaków | `[]` | Dowolne metadane. Muszą być unikalne i niepuste. Niektóre tagi ukrywają atrybut w aplikacji webowej; zobacz [Tagi widoczności](#tagi-widoczności). |
| `default_value` | zgodny z typem | Nieustawiony | Wartość zapisywana w kontekście domyślnym, gdy encja zostaje utworzona bez niej. Tylko typy skalarne. |
| `value_schema` | ciąg znaków (JSON) | Nieustawiony | JSON Schema dla jednej wartości. Tylko typy skalarne. Zobacz [Walidacja](/pl/builders/validation/). |

### Typy wartości

| `value_type` | Przechowywana wartość | Przykład CLI/TOML | Uwagi |
| --- | --- | --- | --- |
| `string` | Tekst | `value = "Blue shirt"` | Wyczyszczenie ciągu znaków w kontekście innym niż domyślny usuwa nadpisanie zamiast zapisywać `""`. Może też być [statusem](/pl/builders/validation/#statusy) lub [przypisaniem użytkownika lub zespołu](#przypisania-użytkowników-i-zespołów). |
| `number` | Liczba dziesiętna | `value = 19.99` | |
| `integer` | 64-bitowa liczba całkowita | `value = 12` | |
| `boolean` | `true` lub `false` | `value = true` | |
| `date` | Data kalendarzowa | `value = 2026-03-01` | |
| `datetime` | Znacznik czasu z przesunięciem | `value = 2026-03-01T09:30:00Z` | RFC 3339. |
| `time` | Godzina zegarowa i strefa czasowa IANA | `value = { time = "09:30:00", time_zone = "Europe/Warsaw" }` | Obie części są wymagane. |
| `json` | Dowolna wartość JSON | | Nie można według niej sortować ani używać jej w filtrach przeglądarki encji. Preferuj atrybuty typowane lub relacje. |
| `relationship` | Powiązania z innymi encjami | | Zobacz [Klucze relacji](#klucze-relacji). |
| `file` | Przesłane pliki | | Zobacz [Klucze plików](#klucze-plików). |

### Przypisania użytkowników i zespołów

Atrybut `string`, którego `value_schema` ma adnotację `x-attricat-principal`, przechowuje odwołanie do użytkownika lub zespołu obszaru roboczego:

```toml
[[attributes]]
code = "assignee"
value_type = "string"
value_schema = '''{"type": "string", "x-attricat-principal": {"version": 1, "kinds": ["user", "team"]}}'''
```

| Klucz | Wartość | Opis |
| --- | --- | --- |
| `version` | `1` | Wymagany. |
| `kinds` | `["user"]`, `["team"]` lub oba | Wymagany. Co przyjmuje atrybut. |

- Wartości mają postać `user:<id>` lub `team:<id>` z identyfikatorem zapisanym małymi literami, pobranym z `acli directory` lub `GET /directory`. Brak wartości oznacza brak przypisania.
- Schemat musi mieć `"type": "string"` i nie może mieć `enum`, `const`, `pattern` ani `format`. Atrybut nie może mieć `default_value` ani być jednocześnie statusem. Atrybuty wielokrotnego użytku przyjmują tę samą adnotację.
- Nowa lub zmieniona wartość musi wskazywać aktywnego członka obszaru roboczego lub nieusunięty zespół, w dozwolonym rodzaju. W przeciwnym razie zapis zwraca `422 attribute_value_schema_mismatch`. Niezmienione wartości nie są sprawdzane ponownie.
- Filtry wyszukiwania porównują zapisaną wartość dokładnie. Wartość `@me` z operatorem `eq` pasuje do wywołującego i do każdego zespołu, do którego należy.

### Klucze relacji

```toml
[[attributes]]
code = "brand"
value_type = "relationship"
target_blueprint = "brand"
cardinality = "one"
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `target_blueprint` | kod | Dowolny schemat encji | Ogranicza cele do encji z tej rodziny schematów. |
| `target_blueprints` | tablica kodów | Dowolny schemat encji | Ogranicza cele do encji z dowolnej z tych rodzin schematów. Nie można łączyć z `target_blueprint`; lista z jednym elementem działa tak samo jak `target_blueprint`. |
| `cardinality` | `"one"`, `"many"` lub `"one_to_one"` | `"many"` | Z iloma celami jedna encja może być powiązana w jednym kontekście. `one_to_one` to skrót dla `cardinality = "one"` razem z `target_cardinality = "one"` i nie można go łączyć z `target_cardinality`. |
| `target_cardinality` | `"one"` lub `"many"` | `"many"` | Ile encji może wskazywać ten sam cel przez ten atrybut w jednym kontekście. |
| `acyclic` | wartość logiczna | `false` | Odrzuca powiązania, które utworzyłyby cykl przez ten atrybut. Wymaga `context_editable = "default"`, a cele muszą obejmować sam schemat. Zobacz [Hierarchie](#hierarchie). |
| `tree` | wartość logiczna | `false` | Hierarchia acykliczna, w której każda encja ma co najwyżej jeden cel (rodzica). Oznacza `acyclic = true` i domyślnie ustawia `cardinality` na `"one"`; `cardinality = "many"` jest odrzucane. |

`cardinality = "one"` daje pole jednokrotnego wyboru, którego opcje mogą być współdzielone, np. marka. Dodaj `target_cardinality = "one"` tylko dla wyłącznego powiązania, w którym każdy cel może zostać zajęty raz. Zapis naruszający którykolwiek limit zwraca `409 relationship_cardinality_conflict`.

Zapis wiążący encję schematu niedozwolonego przez `target_blueprint` lub `target_blueprints` zwraca `422 relationship_target_type_mismatch`. Okno wyboru encji w aplikacji webowej oferuje tylko dozwolone schematy; gdy jest ich kilka, ma pole **Schemat celu**. Blok `incoming_relationship_list` w każdym z dozwolonych schematów docelowych może wyświetlać to pole.

#### Hierarchie

```toml
[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "location"
tree = true
context_editable = "default"
```

`acyclic` i `tree` chronią struktury odwołujące się do siebie, takie jak hierarchie lokalizacji czy zasobów, oraz łańcuchy poprzedników, np. wersja → poprzednia wersja.

- Zapis, który zamknąłby cykl, zwraca `409 relationship_cycle`. `error.details.path` zawiera identyfikatory encji wzdłuż cyklu, zaczynając i kończąc na zapisywanej encji. Powiązanie encji z nią samą jest cyklem o długości jeden.
- W drzewie (`tree`) nadanie encji drugiego celu zwraca `409 relationship_cardinality_conflict`.
- Sprawdzenia uwzględniają powiązania ze wszystkich wersji schematu i działają w transakcji zapisu. Dwa równoczesne zapisy nie mogą każdy dodać połowy cyklu.
- Hierarchia dotyczy całej rodziny schematów zgodnie z jej najnowszą opublikowaną wersją, także encji przypiętych do starszych wersji.
- Publikacja wersji, która dodaje `acyclic` lub `tree`, najpierw sprawdza istniejące powiązania. Jeśli zawierają cykle albo drzewo ma encje z więcej niż jednym celem, publikacja kończy się błędem `409 relationship_hierarchy_violations`; `error.details` wymienia do 20 cykli i encji z nadmiarowymi celami. Popraw powiązania i opublikuj ponownie.

Relacje nie mogą mieć `value_schema` ani `default_value`. Ograniczaj je zamiast tego przez `entity_schema`.

### Klucze plików

```toml
[[attributes]]
code = "gallery"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["jpg", "png", "webp"]
max_bytes = 10485760
image_only = true
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `cardinality` | `"one"` lub `"many"` | `"one"` | Pojedynczy plik lub lista plików. |
| `ordered` | wartość logiczna | `true` dla `many` | Czy kolejność listy `many` ma znaczenie. Dla `one` musi mieć wartość `false` lub zostać pominięty. |
| `allowed_mime_groups` | tablica ciągów znaków | Dowolne | Akceptowane grupy MIME, np. `image`. |
| `allowed_extensions` | tablica ciągów znaków | Dowolne | Akceptowane rozszerzenia plików, wyłącznie litery i cyfry (`jpg`, a nie `*.jpg`). Początkowa `.` jest ignorowana. |
| `max_bytes` | dodatnia liczba całkowita | Limit serwera | Limit rozmiaru pojedynczego pliku. Nie może przekraczać `FILE_UPLOAD_MAX_BYTES` serwera. |
| `purposes` | tablica kodów | `[]` | Etykiety opisujące przeznaczenie plików, np. `product_image`. |
| `image_only` | wartość logiczna | `false` | Akceptuje tylko obrazy. Wymagane dla renderera `catalog.table_image`. |

Te klucze są odrzucane w każdym atrybucie, który nie ma `value_type = "file"`. Atrybuty plikowe nie mogą mieć `value_schema`, `default_value` ani `target_blueprint`.

Przesłane obrazy otrzymują dwa wygenerowane warianty, `thumbnail` i `display`, oba w formacie WebP. Pozostałe pliki są przechowywane w postaci, w jakiej je przesłano.

### Tagi widoczności

Niektóre tagi zmieniają miejsca, w których aplikacja webowa pokazuje atrybut, gdy automatycznie buduje układ. Widok, który jawnie wskazuje atrybut, nadal go pokazuje. Te tagi nie są kontrolą dostępu: API, CLI, agenci i rozszerzenia nadal zwracają wartości.

| Tag | Ukrywa atrybut w |
| --- | --- |
| `hidden` | Wszystkich miejscach wymienionych poniżej |
| `hidden:form` | Automatycznie generowanych formularzach tworzenia i edycji |
| `hidden:detail` | Automatycznie generowanych widokach szczegółów encji |
| `hidden:explorer` | Opcjach filtrów i faset przeglądarki encji |
| `hidden:metadata` | Tabeli atrybutów na stronie schematu |

### Typy atrybutów z rozszerzeń

Włączone rozszerzenie może deklarować typy atrybutów, np. typ kwoty pieniężnej z ustawieniem waluty:

```toml
[[attributes]]
code = "price"
extension_type = "com.acme.commerce:money@^1"
extension_configuration = '{"currency":"USD"}'
```

Gdy schemat jest zapisywany, Attricat odnajduje włączone rozszerzenie `com.acme.commerce`, wybiera najwyższą zadeklarowaną wersję typu `money` pasującą do `^1`, weryfikuje `extension_configuration` względem schematu konfiguracji typu i zapisuje rozstrzygnięty typ w wersji schematu. Wartość jest przechowywana w bazowym typie prostym tego typu (`string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time` lub `json`) i sprawdzana względem schematu wartości typu. Wartości pozostają czytelne, jeśli rozszerzenie zostanie później wyłączone.

`extension_type` nie można łączyć z `value_schema`, kluczami relacji ani kluczami plików.

## Dołączenia

Domieszki współdzielą definicje atrybutów między schematami. Dołączenie przypina jedną konkretną opublikowaną wersję domieszki. Każdy potrzebny atrybut trzeba następnie wybrać przez `from`.

```toml
[[includes]]
alias = "seo"
code = "seo"
version = 2

[[attributes]]
code = "meta_title"
from = "seo.meta_title"
```

| Klucz | Typ | Opis |
| --- | --- | --- |
| `alias` | kod | Lokalna nazwa używana w `from`. Unikalna w obrębie schematu. |
| `code` | kod | Kod schematu domieszki. |
| `version` | dodatnia liczba całkowita | Konkretna opublikowana wersja domieszki. |

Wybrany atrybut zachowuje wszystkie ustawienia z domieszki. Aby uwzględnić zmiany w domieszce, opublikuj nową wersję domieszki i wskaż ją w `version` w nowej wersji schematu, który z niej korzysta.

## Widoki

`views` to tabela z kluczami będącymi nazwami widoków. Aplikacja webowa używa następujących nazw:

| Widok | Przeznaczenie | Dozwolony `type` |
| --- | --- | --- |
| `dropdown_option` | Etykieta encji w selektorach relacji, etykietach filtrów i wynikach wyszukiwania. Wymagany w schematach encji. | `dropdown_option` |
| `detail` | Strona encji tylko do odczytu. | Blok układu |
| `edit` | Formularz tworzenia i edycji. | Blok układu |
| `table` | Kolumny przeglądarki encji. | `table` |
| `extension_layout` | Kolejność i widoczność kontrybucji z rozszerzeń na stronach encji tego schematu. | `extension_layout` |

Gdy brakuje `detail`, `edit` lub `table`, aplikacja webowa wyświetla atrybuty w kolejności deklaracji.

### `dropdown_option`

```toml
[views.dropdown_option]
type = "dropdown_option"
fields = ["name", "sku"]
separator = " / "
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `fields` | tablica kodów atrybutów | Wymagany | Jeden lub więcej unikalnych atrybutów łączonych w etykietę. |
| `separator` | ciąg znaków | `" · "` | Tekst wstawiany między pola. |

Etykiety są rozstrzygane dla każdego kontekstu i respektują `context_fallback` każdego atrybutu.

### `table`

```toml
[views.table]
type = "table"

[[views.table.columns]]
field = "title"
label = "Product"

[[views.table.columns]]
field = "category.name"
label = "Category"

[[views.table.columns]]
field = "main_photo"
label = "Image"
renderer = { id = "catalog.table_image", version = 1 }
```

| Klucz | Typ | Opis |
| --- | --- | --- |
| `columns` | tablica tabel | Definicje kolumn. Musi być niepusta, z unikalnymi wartościami `field`. |
| `columns[].field` | ciąg znaków | Lokalny atrybut skalarny, lokalny atrybut plikowy ze zgodnym rendererem lub ścieżka o maksymalnie trzech krokach relacji zakończona atrybutem skalarnym, np. `family.product_type.name`. |
| `columns[].label` | ciąg znaków | Nagłówek kolumny. Domyślnie nazwa atrybutu. |
| `columns[].renderer` | odwołanie do komponentu | `catalog.table_image@1`, komponent wyświetlania tekstu (`catalog.color_display@1`, `catalog.email_display@1`, `catalog.url_display@1` lub `catalog.phone_display@1`) albo renderer komórek z rozszerzenia. |
| `fields` | tablica kodów atrybutów | Starszy skrót dla lokalnych kolumn skalarnych. Nie można go łączyć z `columns`. |
| `component` | odwołanie do komponentu | Opcjonalny komponent tabeli. |

Kolumnę można sortować w przeglądarce encji tylko wtedy, gdy jest skalarna, a każdy krok relacji w jej ścieżce ma `cardinality = "one"`.

### Bloki układu

`detail` i `edit` zaczynają się od bloku kontenera i zagnieżdżają w nim inne bloki. Bloki to tabele TOML z kluczem `type`. Możesz je zapisać jako zagnieżdżone tablice tabel lub jako tabele wbudowane; oba zapisy dają ten sam wynik.

| `type` | Klucze | Opis |
| --- | --- | --- |
| `stack` | `children` | Elementy podrzędne ułożone pionowo. |
| `grid` | `children` | Elementy podrzędne w responsywnej siatce. |
| `section` | `children` | Zgrupowany obszar. |
| `tabs` | `tabs` = tablica `{ label, children }` | Jedna karta na wpis. |
| `accordion` | `sections` = tablica `{ label, children }` | Zwijane sekcje. |
| `field` | `field` | Jeden atrybut skalarny lub plikowy. |
| `relationship_list` | `field` | Jeden atrybut relacji i jego cele. |
| `incoming_relationship_list` | `label`, `relationships`, `page_size` | Przycisk otwierający stronicowaną listę encji wskazujących na tę encję. `relationships` to tablica `{ source_blueprint, field }`. `page_size` jest ograniczane przez `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE` serwera. |
| `heading` | `text` | Statyczny nagłówek. |
| `text` | `text` | Statyczny akapit. |
| `divider` | | Linia pozioma. |

Każdy blok przyjmuje opcjonalne odwołanie `component`. Każdy `field` i `relationship_list` musi wskazywać atrybut, który schemat posiada, łącznie z atrybutami wybranymi z domieszek.

### Odwołania do komponentów

```toml
component = { id = "catalog.field_edit", version = 1 }
renderer = { id = "example.currency", version = 1, props = { currency = "USD" } }
```

| Klucz | Typ | Opis |
| --- | --- | --- |
| `id` | ciąg znaków | Identyfikator zarejestrowanego komponentu. |
| `version` | liczba całkowita | Wersja komponentu. |
| `props` | tabela | Opcje komponentu. Akceptowane są tylko właściwości zadeklarowane przez komponent. |

Wbudowane komponenty:

| ID | Wersja | Używany w | Typy wartości | Właściwości |
| --- | --- | --- | --- | --- |
| `catalog.field_display` | 1 | `field` (detail) | Skalarne | |
| `catalog.field_edit` | 1 | `field` (edit) | Skalarne | |
| `catalog.relationship_list_display` | 1 | `relationship_list` (detail) | `relationship` | |
| `catalog.relationship_list_edit` | 1 | `relationship_list` (edit) | `relationship` | |
| `catalog.relationship_hierarchy` | 1 | `relationship_list` (detail) | `relationship` | `parent_field` |
| `catalog.incoming_relationship_list_display` | 1 | `incoming_relationship_list` | | |
| `catalog.entity_heading` | 1 | `stack` (detail) | | |
| `catalog.table_display` | 1 | `table` | Skalarne | |
| `catalog.table_edit` | 1 | `table` | Skalarne | |
| `catalog.table_image` | 1 | `renderer` kolumny tabeli | `file` z `cardinality = "one"` i `image_only = true` | |
| `catalog.color_display` | 1 | `field` (detail), `renderer` kolumny tabeli | `string` | |
| `catalog.color_edit` | 1 | `field` (edit) | `string` | |
| `catalog.email_display` | 1 | `field` (detail), `renderer` kolumny tabeli | `string` | |
| `catalog.email_edit` | 1 | `field` (edit) | `string` | |
| `catalog.url_display` | 1 | `field` (detail), `renderer` kolumny tabeli | `string` | |
| `catalog.url_edit` | 1 | `field` (edit) | `string` | |
| `catalog.phone_display` | 1 | `field` (detail), `renderer` kolumny tabeli | `string` | |
| `catalog.phone_edit` | 1 | `field` (edit) | `string` | |
| `catalog.markdown_display` | 1 | `field` (detail) | `string` | |
| `catalog.markdown_edit` | 1 | `field` (edit) | `string` | |

Działanie tych komponentów opisano w sekcji [Kontrolki pól](/pl/builders/views/#kontrolki-pól).

Identyfikator i wersja renderera komórek z rozszerzenia muszą odpowiadać rendererowi zadeklarowanemu przez włączone rozszerzenie dla typu wartości kolumny.

### `extension_layout`

```toml
[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.entity_preview_panel]
order = ["acme.inventory:summary"]
hidden = ["acme.legacy:panel"]
```

| Klucz | Opis |
| --- | --- |
| `version` | Musi wynosić `1`. |
| `outlets` | Klucze `entity_preview_panel`, `entity_attribute_decoration` lub `entity_action`. |
| `outlets.<outlet>.order` | Klucze kontrybucji (`<extension-id>:<contribution-id>`) w kolejności wyświetlania. |
| `outlets.<outlet>.hidden` | Klucze kontrybucji do ukrycia. |

Punkt osadzenia wymieniony tutaj zastępuje domyślny układ obszaru roboczego dla tego punktu na encjach tego schematu. Punkty, których nie wymienisz, zachowują układ obszaru roboczego. Klucze rozszerzeń, które nie są zainstalowane, są zachowywane, więc układ przetrwa wyłączenie i ponowne włączenie rozszerzenia.

## Publikacja

```toml
[publication]
retain_on_edit_roles = ["editor"]
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `retain_on_edit_roles` | tablica kodów ról | `[]` | Role obszaru roboczego, których edycje zachowują istniejące publikacje encji w kanałach. Edycje wszystkich pozostałych osób je wycofują. Role muszą istnieć w chwili publikacji wersji. |

Nie przyznaje to żadnych uprawnień. Zobacz [Publikowanie](/pl/guides/publishing/).

## Reguły

Tabele `[[rules]]` używają składni reguł opisanej w [Reguły](/pl/builders/rules/), bez `format_version`. Kody reguł muszą być unikalne w obrębie schematu, a predykaty `required` i `stale` muszą wskazywać atrybut schematu.

## Klucze unikalne

Klucz unikalny deklaruje identyfikator biznesowy, którego dwie encje z tej rodziny schematów nie mogą współdzielić, np. numer części, numer dokumentu albo kombinację, taką jak producent i numer części.

```toml
[[unique_keys]]
code = "manufacturer_part"
attributes = ["manufacturer", "part_number"]

[[unique_keys]]
code = "slug"
attributes = ["slug"]
scope = "context"
case_sensitive = true
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `code` | kod | Wymagany | Unikalny w obrębie schematu. Podawany w błędach. |
| `attributes` | tablica kodów atrybutów | Wymagany | Od jednego do ośmiu atrybutów, których łączne wartości muszą być unikalne. Każdy musi być wartością skalarną innego typu niż `json` albo relacją z `cardinality = "one"`. |
| `scope` | `"workspace"` lub `"context"` | `"workspace"` | `workspace` porównuje wartości w kontekście domyślnym. `context` porównuje wartości wyświetlane w każdym kontekście, także dziedziczone, osobno dla każdego kontekstu. |
| `case_sensitive` | wartość logiczna | `false` | Porównuje tekst dokładnie zamiast bez rozróżniania wielkości liter. |

Jak porównywane są wartości:

- Tekst jest przycinany, a każdy ciąg białych znaków zamieniany na jedną spację. Jeśli nie ustawiono `case_sensitive = true`, tekst jest też porównywany małymi literami, więc `ABC-1  Rev` i ` abc-1 rev` to ten sam klucz.
- Liczby są porównywane według wartości (`1.50` równa się `1.5`), daty z czasem według chwili, a relacje według powiązanej encji.
- Encja, która nie ma wartości (albo ma tylko pusty tekst) dla któregokolwiek atrybutu klucza, nie jest sprawdzana względem tego klucza. Jeśli każda encja musi mieć klucz, oznacz atrybuty jako wymagane w `entity_schema`.
- Klucz obejmuje całą rodzinę schematów zgodnie z jej najnowszą opublikowaną wersją, także encje przypięte do starszych wersji. Atrybuty są dopasowywane według kodu.

Zapis, który nadałby drugiej encji tę samą wartość klucza, zwraca `409 unique_key_conflict`. `error.details` zawiera `key`, kod kontekstu `context`, znormalizowane wartości `values` oraz `conflicting_entity_id` encji, która już je ma. Sprawdzenie odbywa się w bazie danych w transakcji zapisu, więc gdy dwie osoby zapisują tę samą wartość w tym samym momencie, udaje się dokładnie jeden zapis.

Publikacja wersji, która dodaje lub zmienia klucze unikalne, najpierw sprawdza istniejące encje. Jeśli niektóre już współdzielą wartość, publikacja kończy się błędem `409 unique_key_duplicates`, a `error.details.duplicates` wymienia do 20 grup z kluczem, kontekstem, wartościami i identyfikatorami encji (`error.details.total` podaje liczbę wszystkich grup). Zmień lub usuń duplikaty i opublikuj ponownie.

## Zadania konektorów

Zadania konektorów uruchamiają operację zainstalowanego rozszerzenia konektora, np. import lub eksport CSV, dla tego schematu.

```toml
[[connector_jobs]]
code = "csv_export"
direction = "export"
extension_id = "attricat-connector-csv"
operation_id = "export"
interval_seconds = 3600
input = { profile = { version = 1, columns = [{ header = "ID", attribute = "external_id", kind = "string" }] } }
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `code` | kod | Wymagany | Stały identyfikator. Zadania są dopasowywane między wersjami według kodu. |
| `direction` | `"import"` lub `"export"` | Wymagany | |
| `extension_id` | ciąg znaków | Wymagany | Identyfikator rozszerzenia konektora. |
| `operation_id` | ciąg znaków | Wymagany | Operacja zadeklarowana przez rozszerzenie. |
| `input` | tabela | Wymagany | Dane wejściowe operacji, weryfikowane względem schematu operacji. |
| `context` | kod kontekstu | | Wymagany dla importów, odrzucany dla eksportów. Zaimportowane wartości są zapisywane w tym kontekście. |
| `input_file_id` | UUID | | Tylko importy. Gotowy plik obszaru roboczego do zaimportowania. |
| `interval_seconds` | liczba całkowita | Tylko ręcznie | Uruchamia zadanie co N sekund, od 60 do 2592000. |
| `enabled` | wartość logiczna | `true` | Ustaw `false`, aby wstrzymać zadanie. |

Zadania są sprawdzane względem włączonego rozszerzenia podczas publikacji wersji; nieprawidłowe zadanie blokuje publikację. Zadanie usunięte w późniejszej wersji zostaje wyłączone, a jego historia uruchomień jest zachowywana. Zadanie eksportu uruchamia się raz dla każdego włączonego kanału eksportu i eksportuje tylko encje opublikowane w tym kanale. Zobacz [Operacje i konektory](/pl/extensions/operations/).

## Limity i błędy

Kilka reguł kompilacji, które łatwo przeoczyć:

- `format_version` inny niż `1` jest odrzucany.
- Schemat encji bez `views.dropdown_option` jest odrzucany.
- `entity_schema` w domieszce jest odrzucany.
- `entity_schema` może wskazywać tylko atrybuty, które schemat posiada, w swoich kluczach najwyższego poziomu `required`, `properties`, `dependentRequired` i `dependentSchemas`.
- `target_blueprint`, `target_blueprints`, `acyclic` i `tree` w atrybucie innym niż relacja są odrzucane.
- `unique_keys` w domieszce, klucz wskazujący nieznany atrybut, atrybut `json`, plikowy lub relację z wieloma celami, a także klucz wymieniający atrybut dwa razy są odrzucane.
- `from` musi mieć postać `alias.code`, gdzie `code` odpowiada kodowi samego atrybutu.
