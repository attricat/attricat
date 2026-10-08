---
title: Dokumentacja TOML schematu
description: Wszystkie klucze akceptowane w definicji schematu, z typami, wartościami domyślnymi i regułami walidacji.
---

Ta strona wymienia wszystkie klucze akceptowane przez kompilator schematów. Przewodnik, który krok po kroku buduje schemat, znajdziesz w [Tworzenie schematu](/pl/builders/blueprints/).

Kompilator jest rygorystyczny. Nieznany klucz, wartość złego typu lub odwołanie do czegoś, co nie istnieje, powoduje odrzucenie całej definicji z `422 invalid_blueprint_definition` i komunikatem wskazującym problem.

## Kody

Kody schematów, kody atrybutów, aliasy include, cele relacji, kody reguł, kody zadań konektorów i kody ról w `[publication]` muszą być **kodami**: niepustymi ciągami złożonymi z liter ASCII, cyfr, `-` i `_`. `product`, `seo-fields` i `stock_on_hand` są prawidłowe. `product type` i `prodükt` nie są.

## Tłumaczone etykiety

Pola `name` i `description` schematu, `name` i `description` atrybutu, `label` kart i sekcji akordeonu, `columns[].label` tabeli oraz `label` bloku `incoming_relationship_list` mogą zawierać odwołania do leksykonu: `{{Product}}` lub `{{Order|purchase}}` z kontekstem. Tekst poza nawiasami jest dosłowny, a `\{{` zapisuje dosłowne `{{`. Nieprawidłowe odwołania są odrzucane przy zapisie schematu. Zobacz [Tłumaczenie etykiet](/pl/builders/translations/).

## Poziom główny

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"
entity_schema = '''{ "type": "object", "required": ["title"] }'''
```

Schematy rekordów mają `kind = "entity"`: w kluczach schematu, tak jak w API, rekordy występują pod nazwą `entity`.

| Klucz | Typ | Wymagany | Opis |
| --- | --- | --- | --- |
| `format_version` | liczba całkowita | Tak | Musi wynosić `1`. |
| `code` | kod | Tak | Identyfikator rodziny schematów. Nie może się zmieniać między wersjami. |
| `name` | ciąg znaków | Tak | Nazwa wyświetlana. Może się zmieniać między wersjami. |
| `description` | ciąg znaków | Nie | Czym są rekordy schematu, do 500 znaków, np. „Grupy produktów, np. Narzędzia podstawowe”. Agent czyta go, by dopasować słowa użytkowników do rekordów; aplikacja webowa jeszcze go nie wyświetla. |
| `kind` | `"entity"` lub `"mixin"` | Tak | Schemat `entity` może mieć rekordy. Mixin jedynie dostarcza atrybuty innym schematom przez `[[includes]]`. |
| `attributes` | tablica tabel | Tak | Co najmniej jeden atrybut. Zobacz [Atrybuty](#atrybuty). |
| `includes` | tablica tabel | Nie | Mixiny, z których ten schemat pobiera atrybuty. Zobacz [Include](#include). |
| `views` | tabela | Dla `entity`: tak | Układy dla aplikacji webowej. Schematy rekordów muszą definiować `views.dropdown_option`. Zobacz [Widoki](#widoki). |
| `entity_schema` | ciąg znaków (JSON) | Nie | JSON Schema dla całego rekordu, opcjonalnie z [`x-attricat-checks`](#kontrole-rekordów). Tylko schematy rekordów. Zobacz [Walidacja](/pl/builders/validation/). |
| `publication` | tabela | Nie | Zasady ponownego zatwierdzania publikacji. Zobacz [Publikacja](#publikacja). |
| `rules` | tablica tabel | Nie | Reguły jakości danych należące do tego schematu. Zobacz [Reguły](/pl/builders/rules/). |
| `unique_keys` | tablica tabel | Nie | Klucze biznesowe, których wartości muszą być unikalne. Tylko schematy rekordów. Zobacz [Klucze unikalne](#klucze-unikalne). |
| `connector_jobs` | tablica tabel | Nie | Zaplanowane lub ręczne zadania importu i eksportu wykonywane przez rozszerzenie konektora. Tylko schematy rekordów. Zobacz [Zadania konektorów](#zadania-konektorów). |
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
| `name` | ciąg znaków | `code` w czytelnej formie | Czytelna etykieta wyświetlana w formularzach, filtrach, podglądach oraz jako domyślny nagłówek kolumny tabeli. Nie może być pusta. Niedozwolona razem z `from`; wybrany atrybut zachowuje nazwę atrybutu mixinu. |
| `description` | ciąg znaków | | Co przechowuje atrybut, do 500 znaków. Czyta go agent; aplikacja webowa jeszcze go nie wyświetla. Niedozwolony razem z `from`. |
| `value_type` | ciąg znaków | | Jeden z [typów wartości](#typy-wartości). |
| `from` | ciąg znaków | | `"<include-alias>.<attribute-code>"`. Materializuje atrybut z mixinu wskazanego przez include. `code` musi być równy kodowi atrybutu mixinu. |
| `extension_type` | ciąg znaków | | `"<extension-id>:<type-id>@<semver-range>"`. Używa typu atrybutu zadeklarowanego przez włączone rozszerzenie. Zobacz [Typy atrybutów z rozszerzeń](#typy-atrybutów-z-rozszerzeń). |
| `context_fallback` | `"default"` lub `"none"` | `"default"` | Co pokazuje kontekst bez własnej wartości. `default` przechodzi w górę drzewa kontekstów do najbliższego przodka z wartością. `none` nie pokazuje niczego. |
| `context_editable` | `"all"` lub `"default"` | `"all"` | Gdzie można zapisywać wartości. `default` ogranicza zapisy do kontekstu domyślnego: inne konteksty pokazują pole tylko do odczytu, a API odrzuca w nich zapisy. |
| `readonly` | wartość logiczna | `false` | Pokazuje pole w aplikacji webowej, ale uniemożliwia jego edycję w niej. API, CLI, agenci, przepływy pracy i rozszerzenia nadal mogą je zapisywać. Używaj dla wartości, którymi zarządza integracja. |
| `tags` | tablica ciągów znaków | `[]` | Dowolne metadane. Muszą być unikalne i niepuste. Niektóre tagi ukrywają atrybut w aplikacji webowej; zobacz [Tagi widoczności](#tagi-widoczności). |
| `default_value` | zgodny z typem | Nieustawiony | Wartość zapisywana w kontekście domyślnym, gdy rekord zostaje utworzony bez niej. Tylko typy skalarne. |
| `value_schema` | ciąg znaków (JSON) | Nieustawiony | JSON Schema dla jednej wartości. Tylko typy skalarne. Schemat atrybutu `string` może uczynić go [statusem](#statusy) lub [przypisaniem użytkownika lub zespołu](#przypisania-użytkowników-i-zespołów). Zobacz [Walidacja](/pl/builders/validation/). |

### Typy wartości

| `value_type` | Przechowywana wartość | Przykład CLI/TOML | Uwagi |
| --- | --- | --- | --- |
| `string` | Tekst | `value = "Blue shirt"` | Wyczyszczenie ciągu znaków w kontekście innym niż domyślny usuwa nadpisanie zamiast zapisywać `""`. Może też być [statusem](#statusy) lub [przypisaniem użytkownika lub zespołu](#przypisania-użytkowników-i-zespołów). |
| `number` | Liczba dziesiętna | `value = 19.99` | |
| `integer` | 64-bitowa liczba całkowita | `value = 12` | |
| `boolean` | `true` lub `false` | `value = true` | |
| `date` | Data kalendarzowa | `value = 2026-03-01` | |
| `datetime` | Znacznik czasu z przesunięciem | `value = 2026-03-01T09:30:00Z` | RFC 3339. |
| `time` | Godzina zegarowa i strefa czasowa IANA | `value = { time = "09:30:00", time_zone = "Europe/Warsaw" }` | Obie części są wymagane. |
| `json` | Dowolna wartość JSON | | Nie można według niej sortować ani używać jej w filtrach przeglądarki rekordów. Preferuj atrybuty typowane lub relacje. |
| `relationship` | Powiązania z innymi rekordami | | Zobacz [Klucze relacji](#klucze-relacji). |
| `file` | Przesłane pliki | | Zobacz [Klucze plików](#klucze-plików). |

### Statusy

Atrybut `string`, którego `value_schema` ma `enum` i adnotację `x-attricat-status`, jest statusem. Działanie statusów opisuje sekcja [Statusy](/pl/builders/validation/#statusy).

```toml
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["draft", "review", "released"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "draft", "label": "Draft" },
      { "code": "review", "label": "In review",
        "approval": { "covers": "all", "void_to": "draft" } },
      { "code": "released", "label": "Released", "tone": "success",
        "lock": "all", "retention_days": 3650 }
    ],
    "transitions": [
      { "from": null, "to": "draft" },
      { "from": "draft", "to": "review", "code": "submit" },
      { "from": "review", "to": "released", "code": "release",
        "roles": ["reviewer"], "separate_from": ["submit"] },
      { "from": "released", "to": "draft", "code": "correct", "permission": "entities.publish" }
    ]
  }
}'''
```

| Klucz | Wartość | Opis |
| --- | --- | --- |
| `version` | `1` | Wymagany. |
| `options` | tablica od 1 do 100 tabel | Wymagany. Jedna opcja dla każdego kodu z `enum`, w kolejności wyświetlania. |
| `transitions` | tablica do 10 000 tabel | Dozwolone zmiany. Pomiń, aby dopuścić każdą zmianę; jeśli jest podana, dozwolone są tylko wymienione zmiany, a pusta tablica nie dopuszcza żadnej. |

Każda opcja:

| Klucz | Wartość | Opis |
| --- | --- | --- |
| `code` | kod, do 128 znaków | Wymagany. Musi odpowiadać dokładnie jednej wartości z `enum`. |
| `label` | ciąg znaków, od 1 do 200 znaków | Wymagany. Może zawierać [odwołania do leksykonu](/pl/builders/translations/#etykiety-statusów). |
| `tone` | `default`, `success`, `warning`, `error` lub `info` | Kolor znacznika statusu. |
| `lock` | `"all"` lub od 1 do 500 kodów atrybutów | Sprawia, że wymienione atrybuty są tylko do odczytu, dopóki rekord ma ten status. `"all"` obejmuje wszystkie atrybuty, relacje i pliki oprócz samego statusu. Atrybut wielokrotnego użytku zapisuje się jako `namespace:code`. Rekordu ze statusem z blokadą nie można usunąć. Wymaga `transitions`. |
| `approval` | tabela z `covers` i `void_to` | Rejestruje zatwierdzenie, gdy rekord przechodzi do tego statusu. `covers` to `"all"` lub od 1 do 500 kodów atrybutów; `void_to` to inna opcja, do której rekord przechodzi, gdy objęta treść się zmieni. |
| `retention_days` | liczba całkowita od 1 do 36 600 | Zakłada blokadę retencji na pliki zablokowanych atrybutów, gdy rekord przechodzi do tego statusu. Wymaga `lock`. |

Każde przejście:

| Klucz | Wartość | Opis |
| --- | --- | --- |
| `from` | kod opcji lub `null` | Wymagany. `null` oznacza brak wartości, więc krawędź z `null` pozwala ustawić pierwszą wartość, także domyślną. |
| `to` | kod opcji lub `null` | Wymagany. Krawędź do `null` pozwala wyczyścić wartość. |
| `code` | kod, do 128 znaków | Nazywa przejście w `separate_from` i w historii. |
| `permission` | kod uprawnienia | Osoba zapisująca zmianę musi mieć to [uprawnienie](/pl/reference/permissions/) dla rekordu. |
| `roles` | od 1 do 20 kodów ról | Osoba zapisująca zmianę musi mieć co najmniej jedną z tych ról w obszarze roboczym, dla schematu lub dla rekordu. |
| `separate_from` | od 1 do 20 kodów przejść | Osoba zapisująca zmianę nie może być tą, która jako ostatnia wykonała jedno z tych przejść w tym rekordzie i tym kontekście. |
| `conditions` | do 16 kontroli | Wymagania dotyczące danych. Zobacz [Warunki przejść](#warunki-przejść). |

Każda para `from`/`to` może wystąpić tylko raz. `permission`, `roles` i `separate_from` zawsze dotyczą osoby zapisującej zmianę; nie mogą odwoływać się do [przypisania użytkownika lub zespołu](#przypisania-użytkowników-i-zespołów) w rekordzie. Odmowy zwracają `403 status_transition_forbidden` lub `403 status_separation_of_duties`, a zmiany zablokowanej treści zwracają `409 record_locked`. Zobacz [Kontroluj cykl życia rekordu](/pl/builders/validation/#kontroluj-cykl-życia-rekordu).

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
- [Predykaty](#predykaty) traktują wartość jak tekst: `required` sprawdza, czy rekord jest przypisany, a `compare` z `eq` lub `one_of` może dopasować konkretne `user:<id>` lub `team:<id>`. Żaden predykat ani [wymaganie przejścia](#statusy) nie może odwoływać się do osoby zapisującej zmianę, więc nie da się zadeklarować zasady „zamknąć może tylko osoba przypisana”.

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
| `target_blueprint` | kod | Dowolny schemat rekordu | Ogranicza cele do rekordów z tej rodziny schematów. |
| `target_blueprints` | tablica kodów | Dowolny schemat rekordu | Ogranicza cele do rekordów z dowolnej z tych rodzin schematów. Nie można łączyć z `target_blueprint`; lista z jednym elementem działa tak samo jak `target_blueprint`. |
| `cardinality` | `"one"`, `"many"` lub `"one_to_one"` | `"many"` | Z iloma celami jeden rekord może być powiązany w jednym kontekście. `one_to_one` to skrót dla `cardinality = "one"` razem z `target_cardinality = "one"` i nie można go łączyć z `target_cardinality`. |
| `target_cardinality` | `"one"` lub `"many"` | `"many"` | Ile rekordów może wskazywać ten sam cel przez ten atrybut w jednym kontekście. |
| `acyclic` | wartość logiczna | `false` | Odrzuca powiązania, które utworzyłyby cykl przez ten atrybut. Wymaga `context_editable = "default"`, a cele muszą obejmować sam schemat. Zobacz [Hierarchie](#hierarchie). |
| `tree` | wartość logiczna | `false` | Hierarchia acykliczna, w której każdy rekord ma co najwyżej jeden cel (rodzica). Oznacza `acyclic = true` i domyślnie ustawia `cardinality` na `"one"`; `cardinality = "many"` jest odrzucane. |

`cardinality = "one"` daje pole jednokrotnego wyboru, którego opcje mogą być współdzielone, np. marka. Dodaj `target_cardinality = "one"` tylko dla wyłącznego powiązania, w którym każdy cel może zostać zajęty raz. Zapis naruszający którykolwiek limit zwraca `409 relationship_cardinality_conflict`.

Zapis wiążący rekord schematu niedozwolonego przez `target_blueprint` lub `target_blueprints` zwraca `422 relationship_target_type_mismatch`. Okno wyboru rekordu w aplikacji webowej oferuje tylko dozwolone schematy; gdy jest ich kilka, ma pole **Schemat celu**. Blok `incoming_relationship_list` w każdym z dozwolonych schematów docelowych może wyświetlać to pole.

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

- Zapis, który zamknąłby cykl, zwraca `409 relationship_cycle`. `error.details.path` zawiera identyfikatory rekordów wzdłuż cyklu, zaczynając i kończąc na zapisywanym rekordzie. Powiązanie rekordu z nim samym jest cyklem o długości jeden.
- W drzewie (`tree`) nadanie rekordowi drugiego celu zwraca `409 relationship_cardinality_conflict`.
- Sprawdzenia uwzględniają powiązania ze wszystkich wersji schematu i działają w transakcji zapisu. Dwa równoczesne zapisy nie mogą każdy dodać połowy cyklu.
- Hierarchia dotyczy całej rodziny schematów zgodnie z jej najnowszą opublikowaną wersją, także rekordów przypiętych do starszych wersji.
- Publikacja wersji, która dodaje `acyclic` lub `tree`, najpierw sprawdza istniejące powiązania. Jeśli zawierają cykle albo drzewo ma rekordy z więcej niż jednym celem, publikacja kończy się błędem `409 relationship_hierarchy_violations`; `error.details` wymienia do 20 cykli i rekordów z nadmiarowymi celami. Popraw powiązania i opublikuj ponownie.

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
| `hidden:form` | Edytorach dodawanych poza układem, np. w sekcji **Pozostałe atrybuty**, oraz w automatycznie generowanym formularzu tworzenia |
| `hidden:detail` | Automatycznie generowanych widokach szczegółów rekordów |
| `hidden:explorer` | Opcjach filtrów i faset przeglądarki rekordów |
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

## Include

Mixiny współdzielą definicje atrybutów między schematami. Include przypina jedną konkretną opublikowaną wersję mixinu. Każdy potrzebny atrybut trzeba następnie wybrać przez `from`.

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
| `code` | kod | Kod schematu mixinu. |
| `version` | dodatnia liczba całkowita | Konkretna opublikowana wersja mixinu. |

Wybrany atrybut zachowuje wszystkie ustawienia z mixinu. Aby uwzględnić zmiany w mixinie, opublikuj nową wersję mixinu i wskaż ją w `version` w nowej wersji schematu, który z niej korzysta.

## Widoki

`views` to tabela z kluczami będącymi nazwami widoków. Aplikacja webowa używa następujących nazw:

| Widok | Przeznaczenie | Dozwolony `type` |
| --- | --- | --- |
| `dropdown_option` | Etykieta rekordu w selektorach relacji, etykietach filtrów i wynikach wyszukiwania. Wymagany w schematach rekordów. | `dropdown_option` |
| `detail` | Strona rekordu i formularz tworzenia. Pola, które użytkownik może zmienić, są edytowane w miejscu. | Blok układu |
| `edit` | Przestarzały i ignorowany przez aplikację internetową. Nadal akceptowany, a komponenty edycji w nim nadal są walidowane. | Blok układu |
| `table` | Kolumny przeglądarki rekordów. | `table` |
| `extension_layout` | Kolejność i widoczność kontrybucji z rozszerzeń na stronach rekordów tego schematu. | `extension_layout` |

Gdy brakuje `detail` lub `table`, aplikacja webowa wyświetla atrybuty w kolejności deklaracji.

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

Kolumnę można sortować w przeglądarce rekordów tylko wtedy, gdy jest skalarna, a każdy krok relacji w jej ścieżce ma `cardinality = "one"`.

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
| `incoming_relationship_list` | `label`, `relationships`, `page_size` | Przycisk otwierający stronicowaną listę rekordów wskazujących na ten rekord. `relationships` to tablica `{ source_blueprint, field }`. `page_size` jest ograniczane przez `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE` serwera. |
| `heading` | `text` | Statyczny nagłówek. |
| `text` | `text` | Statyczny akapit. |
| `divider` | | Linia pozioma. |

Każdy blok przyjmuje opcjonalne odwołanie `component`. Każdy `field` i `relationship_list` musi wskazywać atrybut, który schemat posiada, łącznie z atrybutami wybranymi z domieszek.

### Odwołania do komponentów

```toml
component = { id = "catalog.url_display", version = 1 }
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

Komponenty oznaczone *(edit)* są akceptowane tylko w przestarzałym widoku `edit`. Tam, gdzie pole w `detail` jest edytowalne, aplikacja internetowa używa komponentu edycji sparowanego z komponentem wyświetlania albo standardowego edytora dla typu wartości. Działanie tych komponentów opisano w sekcji [Kontrolki pól](/pl/builders/views/#kontrolki-pól).

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

Punkt osadzenia wymieniony tutaj zastępuje domyślny układ obszaru roboczego dla tego punktu na rekordach tego schematu. Punkty, których nie wymienisz, zachowują układ obszaru roboczego. Klucze rozszerzeń, które nie są zainstalowane, są zachowywane, więc układ przetrwa wyłączenie i ponowne włączenie rozszerzenia.

## Publikacja

```toml
[publication]
retain_on_edit_roles = ["editor"]
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `retain_on_edit_roles` | tablica kodów ról | `[]` | Role obszaru roboczego, których edycje zachowują istniejące publikacje rekordu w kanałach. Edycje wszystkich pozostałych osób je wycofują. Role muszą istnieć w chwili publikacji wersji. |

Nie przyznaje to żadnych uprawnień. Zobacz [Publikowanie](/pl/guides/publishing/).

## Reguły

Tabele `[[rules]]` używają składni reguł opisanej w [Reguły](/pl/builders/rules/), bez `format_version`. Kody reguł muszą być unikalne w obrębie schematu, a typy ich [predykatów](#predykaty) są sprawdzane względem atrybutów schematu.

```toml
[[rules]]
code = "released-documents-approved"
name = "Released documents have an approver"
severity = "error"
triggers = [{ type = "event", event_type = "entity.updated.v1" }]
predicate = { type = "required", attribute_code = "approved_by" }

[rules.enforcement]
on_save = false

[[rules.enforcement.transitions]]
attribute_code = "status"
from = "review"
to = "released"
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `enforcement.on_save` | wartość logiczna | `false` | Odrzuca każdy zapis, po którym rekord narusza regułę. |
| `enforcement.transitions` | tablica tabel | `[]` | Do 16 chronionych zmian statusu, każda z `attribute_code`, opcjonalnym `from` oraz `to`. Bez `from` chroniona jest każda zmiana na `to`. |

Tabela `enforcement` wymaga `on_save = true` lub co najmniej jednego przejścia, wagi (`severity`) `error` lub `critical` oraz predykatu bez `stale`, `unique` i `acyclic`. `attribute_code` każdego przejścia musi być atrybutem statusu, a `from` i `to` muszą być kodami z jego `enum`. Naruszenia zwracają `422 rule_violation`. Zobacz [Egzekwowanie reguły](/pl/builders/rules/#egzekwowanie-reguły).

## Predykaty

Reguły, kontrole rekordów, warunki przejść i kontrole kanałów publikacji używają jednego języka predykatów. Predykat to tabela (w TOML) lub obiekt (w JSON) rozróżniany kluczem `type`. Jest **spełniony**, gdy dane są poprawne. Nieznane klucze są odrzucane.

| `type` | Klucze | Spełniony, gdy |
| --- | --- | --- |
| `required` | `attribute_code` | Atrybut ma wartość. Relacja musi mieć co najmniej jeden cel. |
| `stale` | `attribute_code`, `max_age_seconds` (od 1 do 31536000) | Wartość zmieniła się w ciągu `max_age_seconds`. Tylko reguły zgłaszające ustalenia. |
| `has_tag` | `tag` | Rekord ma tag systemowy. |
| `missing_tag` | `tag` | Rekord nie ma tagu systemowego. |
| `compare` | `attribute_code`, `op` oraz dokładnie jeden z kluczy `other_attribute_code`, `subject_attribute_code` lub `value` | Porównanie jest prawdziwe. |
| `one_of` | `attribute_code`, `values` (od 1 do 100) | Wartość jest jedną z `values`. Nie dotyczy relacji ani plików. |
| `relative_date` | `attribute_code`, `op` (`lt`, `lte`, `gt`, `gte`), `offset_days` (od -36500 do 36500, domyślnie `0`) | Data lub data z godziną spełnia porównanie z bieżącym czasem przesuniętym o `offset_days`. |
| `unique` | `attribute_codes` (od 1 do 4 atrybutów typu string, number, integer, boolean, date lub datetime) | Żaden inny aktywny rekord z rodziny schematów, w dowolnej wersji, nie ma tych samych wartości w tym samym kontekście. Tylko reguły zgłaszające ustalenia. |
| `linked` | `relationship_code`, `quantifier` (`all`, `any`, `none`; domyślnie `all`), `predicate` | `all`: każdy powiązany rekord spełnia `predicate` (prawda, gdy nie ma powiązań). `any`: co najmniej jeden go spełnia. `none`: żaden go nie spełnia. |
| `referenced_by` | `blueprint_code`, `relationship_code`, opcjonalnie `predicate`, `min` i/lub `max` (od 0 do 1000) | Liczba rekordów `blueprint_code`, których relacja `relationship_code` wskazuje ten rekord i które spełniają `predicate`, mieści się między `min` a `max`. `max = 0` oznacza „żaden”. |
| `acyclic` | `relationship_code` | Podążanie za relacją nigdy nie wraca do rekordu. Tylko reguły zgłaszające ustalenia. |
| `all_of` | `predicates` (od 1 do 16) | Każdy zagnieżdżony predykat jest spełniony. |
| `any_of` | `predicates` (od 1 do 16) | Co najmniej jeden zagnieżdżony predykat jest spełniony. |

Operatory `compare`:

| `op` | Dotyczy |
| --- | --- |
| `eq`, `ne` | Każdego typu oprócz plików. Relacje są równe, gdy mają ten sam zbiór celów. |
| `lt`, `lte`, `gt`, `gte` | `number`, `integer`, `date`, `datetime`. |
| `disjoint` | Dwóch relacji bez wspólnego celu. |

- Obie strony muszą mieć zgodne typy: liczbę z liczbą lub liczbą całkowitą, datę z datą. Dosłowna wartość `value` musi pasować do typu atrybutu; daty i daty z godziną zapisuj jako łańcuchy znaków, np. `"2026-01-31"`. Relację można porównać tylko z inną relacją.
- `compare` z brakującym operandem jest spełniony; tak samo `one_of` i `relative_date` na pustym atrybucie. Połącz je z `required`, jeśli wartość musi istnieć.
- Wewnątrz `linked` i `referenced_by` klucze `attribute_code` i `other_attribute_code` dotyczą drugiego rekordu, a `subject_attribute_code` — sprawdzanego rekordu. Poza nimi `subject_attribute_code` jest odrzucany.
- `linked` i `referenced_by` przechodzą o jeden krok: ich zagnieżdżony predykat nie może używać `linked`, `referenced_by`, `unique`, `acyclic` ani `stale`.
- Predykat może mieć najwyżej 4 poziomy zagnieżdżenia i najwyżej 32 części.
- W trakcie działania `linked` nie przechodzi przy ponad 200 powiązanych rekordach na relację, `referenced_by` przy ponad 1000 rekordach wskazujących, a `acyclic`, gdy nie zdoła zakończyć sprawdzania w obrębie 1000 rekordów.

`stale`, `unique` i `acyclic` służą tylko do zgłaszania ustaleń, ponieważ nie da się ich sprawdzić w ramach jednego zapisu. Są odrzucane w egzekwowanych regułach, kontrolach rekordów i warunkach przejść.

### Kontrole rekordów

`x-attricat-checks` to tablica wewnątrz `entity_schema`:

```toml
entity_schema = '''
{
  "type": "object",
  "x-attricat-checks": [
    { "code": "valid-range", "message": "Valid until must not be before valid from",
      "predicate": { "type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from" } }
  ]
}
'''
```

| Klucz | Typ | Wymagany | Opis |
| --- | --- | --- | --- |
| `code` | kod | Tak | Unikalny w obrębie tablicy. |
| `message` | ciąg znaków | Nie | Od 1 do 500 znaków. Zastępuje wygenerowany komunikat. |
| `predicate` | obiekt | Tak | [Predykat](#predykaty) z wyjątkiem `stale`, `unique` i `acyclic`. |

Najwyżej 32 kontrole. Niepowodzenie zwraca `422 entity_check_failed`. Zobacz [Walidacja](/pl/builders/validation/#porównuj-atrybuty-za-pomocą-kontroli).

### Warunki przejść

Krawędź w `transitions` adnotacji `x-attricat-status` może mieć `conditions`, czyli tablicę do 16 kontroli z tymi samymi kluczami co [kontrole rekordów](#kontrole-rekordów):

```json
{ "from": "review", "to": "released", "conditions": [
  { "code": "approver-set", "message": "Set an approver before release",
    "predicate": { "type": "required", "attribute_code": "approved_by" } }
] }
```

Każda para `from`/`to` może wystąpić tylko raz. Niespełnione warunki zwracają `422 transition_conditions_unmet`. Zobacz [Warunki przejść](/pl/builders/validation/#warunki-przejść).

## Klucze unikalne

Klucz unikalny deklaruje identyfikator biznesowy, którego dwa rekordy z tej rodziny schematów nie mogą współdzielić, np. numer części, numer dokumentu albo kombinację, taką jak producent i numer części.

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
- Liczby są porównywane według wartości (`1.50` równa się `1.5`), daty z czasem według chwili, a relacje według powiązanego rekordu.
- Rekord, który nie ma wartości (albo ma tylko pusty tekst) dla któregokolwiek atrybutu klucza, nie jest sprawdzany względem tego klucza. Jeśli każdy rekord musi mieć klucz, oznacz atrybuty jako wymagane w `entity_schema`.
- Klucz obejmuje całą rodzinę schematów zgodnie z jej najnowszą opublikowaną wersją, także rekordy przypięte do starszych wersji. Atrybuty są dopasowywane według kodu.

Zapis, który nadałby drugiemu rekordowi tę samą wartość klucza, zwraca `409 unique_key_conflict`. `error.details` zawiera `key`, kod kontekstu `context`, znormalizowane wartości `values` oraz `conflicting_entity_id` rekordu, który już je ma. Sprawdzenie odbywa się w bazie danych w transakcji zapisu, więc gdy dwie osoby zapisują tę samą wartość w tym samym momencie, udaje się dokładnie jeden zapis.

Publikacja wersji, która dodaje lub zmienia klucze unikalne, najpierw sprawdza istniejące rekordy. Jeśli niektóre już współdzielą wartość, publikacja kończy się błędem `409 unique_key_duplicates`, a `error.details.duplicates` wymienia do 20 grup z kluczem, kontekstem, wartościami i identyfikatorami rekordów (`error.details.total` podaje liczbę wszystkich grup). Zmień lub usuń duplikaty i opublikuj ponownie.

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

Zadania są sprawdzane względem włączonego rozszerzenia podczas publikacji wersji; nieprawidłowe zadanie blokuje publikację. Zadanie usunięte w późniejszej wersji zostaje wyłączone, a jego historia uruchomień jest zachowywana. Zadanie eksportu uruchamia się raz dla każdego włączonego kanału eksportu i eksportuje tylko rekordy opublikowane w tym kanale. Zobacz [Operacje i konektory](/pl/extensions/operations/).

## Limity i błędy

Kilka reguł kompilacji, które łatwo przeoczyć:

- `format_version` inny niż `1` jest odrzucany.
- Schemat rekordu bez `views.dropdown_option` jest odrzucany.
- `entity_schema` w mixinie jest odrzucany.
- `entity_schema` może wskazywać tylko atrybuty, które schemat posiada, w swoich kluczach najwyższego poziomu `required`, `properties`, `dependentRequired` i `dependentSchemas`.
- Predykaty w `x-attricat-checks`, warunkach przejść (`conditions`) i `[[rules]]` muszą wskazywać atrybuty, które schemat posiada, o typach pasujących do predykatu. Porównanie porządkujące na łańcuchu znaków albo porównanie daty z liczbą jest odrzucane.
- `target_blueprint`, `target_blueprints`, `acyclic` i `tree` w atrybucie innym niż relacja są odrzucane.
- `unique_keys` w mixinie, klucz wskazujący nieznany atrybut, atrybut `json`, plikowy lub relację z wieloma celami, a także klucz wymieniający atrybut dwa razy są odrzucane.
- `from` musi mieć postać `alias.code`, gdzie `code` odpowiada kodowi samego atrybutu.
