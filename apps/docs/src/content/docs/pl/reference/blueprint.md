---
title: Dokumentacja YAML schematu
description: Wszystkie klucze akceptowane w definicji schematu, z typami, wartościami domyślnymi i regułami walidacji.
---

Ta strona wymienia wszystkie klucze akceptowane przez kompilator schematów. Przewodnik, który krok po kroku buduje schemat, znajdziesz w [Tworzenie schematu](/pl/builders/blueprints/).

Kompilator jest rygorystyczny. Nieznany klucz, wartość złego typu lub odwołanie do czegoś, co nie istnieje, powoduje odrzucenie całej definicji z `422 invalid_blueprint_definition` i komunikatem wskazującym problem.

## Składnia YAML

W YAML liczą się wcięcia: wcinaj spacjami, nigdy tabulatorami, a elementy jednej listy trzymaj na tej samej głębokości. Ujmij w cudzysłów ciąg znaków, który zawiera `: ` lub ` #` albo zaczyna się od `{`, `[`, `*`, `&`, `!`, `|`, `>`, `'`, `"`, `%` lub `@`. Dotyczy to każdej etykiety zaczynającej się od odwołania do leksykonu: pisz `name: '{{Product}}'`, a nie `name: {{Product}}`. Wartościami logicznymi są tylko `true` i `false`; `yes`, `no`, `on` i `off` to ciągi znaków.

## Kody

Kody schematów, kody atrybutów, aliasy include, cele relacji, kody zadań konektorów i kody ról w `publication` muszą być **kodami**: niepustymi ciągami złożonymi z liter ASCII, cyfr, `-` i `_`. `product`, `seo-fields` i `stock_on_hand` są prawidłowe. `product type` i `prodükt` nie są.

## Tłumaczone etykiety

Pola `name` i `description` schematu, `name` i `description` atrybutu, `label` kart i sekcji akordeonu, `columns[].label` tabeli oraz `label` bloku `incoming_relationship_list` mogą zawierać odwołania do leksykonu: `{{Product}}` lub `{{Order|purchase}}` z kontekstem. Tekst poza nawiasami jest dosłowny, a `\{{` zapisuje dosłowne `{{`. Nieprawidłowe odwołania są odrzucane przy zapisie schematu. Zobacz [Tłumaczenie etykiet](/pl/builders/translations/).

## Poziom główny

```yaml
format_version: 1
code: product
name: Product
kind: record
record_schema: '{ "type": "object", "required": ["title"] }'
```

Schematy rekordów mają `kind: record`.

| Klucz | Typ | Wymagany | Opis |
| --- | --- | --- | --- |
| `format_version` | liczba całkowita | Tak | Musi wynosić `1`. |
| `code` | kod | Tak | Identyfikator rodziny schematów. Nie może się zmieniać między wersjami. |
| `name` | ciąg znaków | Tak | Nazwa wyświetlana. Może się zmieniać między wersjami. |
| `description` | ciąg znaków | Nie | Czym są rekordy schematu, do 500 znaków, np. „Grupy produktów, np. Narzędzia podstawowe”. Agent czyta go, by dopasować słowa użytkowników do rekordów; aplikacja webowa jeszcze go nie wyświetla. |
| `kind` | `"record"` lub `"mixin"` | Tak | Schemat `record` może mieć rekordy. Mixin jedynie dostarcza atrybuty innym schematom przez `includes`. |
| `attributes` | lista mapowań | Tak | Co najmniej jeden atrybut. Zobacz [Atrybuty](#atrybuty). |
| `includes` | lista mapowań | Nie | Mixiny, z których ten schemat pobiera atrybuty. Zobacz [Include](#include). |
| `views` | mapowanie | Dla `record`: tak | Układy dla aplikacji webowej. Schematy rekordów muszą definiować `views.dropdown_option`. Zobacz [Widoki](#widoki). |
| `record_schema` | ciąg znaków (JSON) | Nie | JSON Schema dla całego rekordu, opcjonalnie z [`x-attricat-checks`](#kontrole-rekordów). Tylko schematy rekordów. Zobacz [Walidacja](/pl/builders/validation/). |
| `publication` | mapowanie | Nie | Zasady ponownego zatwierdzania publikacji. Zobacz [Publikacja](#publikacja). |
| `unique_keys` | lista mapowań | Nie | Klucze biznesowe, których wartości muszą być unikalne. Tylko schematy rekordów. Zobacz [Klucze unikalne](#klucze-unikalne). |
| `connector_jobs` | lista mapowań | Nie | Zaplanowane lub ręczne zadania importu i eksportu wykonywane przez rozszerzenie konektora. Tylko schematy rekordów. Zobacz [Zadania konektorów](#zadania-konektorów). |
| `movements` | lista mapowań | Nie | Jak utworzenie rekordu zmienia saldo innego rekordu, np. wydanie towaru obniża stan magazynowy. Najwyżej 4. Tylko schematy rekordów. Zobacz [Salda i przesunięcia](#salda-i-przesunięcia). |
| `extensions` | mapowanie | Nie | Dowolne dane dla rozszerzeń, w przestrzeniach nazw `extensions.<extension-id>`. Kompilator rdzenia je ignoruje; rozszerzenia odczytują je z zapisanej definicji. |

## Atrybuty

Każdy wpis w `attributes` deklaruje dokładnie jeden z kluczy `value_type`, `from` lub `extension_type`.

```yaml
attributes:
  - code: title
    value_type: string
```

### Klucze wspólne

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `code` | kod | Wymagany | Unikalny w obrębie schematu, łącznie z atrybutami pobranymi z domieszek. |
| `name` | ciąg znaków | `code` w czytelnej formie | Czytelna etykieta wyświetlana w formularzach, filtrach, podglądach oraz jako domyślny nagłówek kolumny tabeli. Nie może być pusta. Niedozwolona razem z `from`; wybrany atrybut zachowuje nazwę atrybutu mixinu. |
| `description` | ciąg znaków | | Co przechowuje atrybut, do 500 znaków. Formularze rekordów pokazują go pod polem, a agent go czyta. Niedozwolony razem z `from`. |
| `value_type` | ciąg znaków | | Jeden z [typów wartości](#typy-wartości). |
| `from` | ciąg znaków | | `"<include-alias>.<attribute-code>"`. Materializuje atrybut z mixinu wskazanego przez include. `code` musi być równy kodowi atrybutu mixinu. |
| `extension_type` | ciąg znaków | | `"<extension-id>:<type-id>@<semver-range>"`. Używa typu atrybutu zadeklarowanego przez włączone rozszerzenie. Zobacz [Typy atrybutów z rozszerzeń](#typy-atrybutów-z-rozszerzeń). |
| `context_fallback` | `"default"` lub `"none"` | `"default"` | Co pokazuje kontekst bez własnej wartości. `default` przechodzi w górę drzewa kontekstów do najbliższego przodka z wartością. `none` nie pokazuje niczego. |
| `context_editable` | `"all"` lub `"default"` | `"all"` | Gdzie można zapisywać wartości. `default` ogranicza zapisy do kontekstu domyślnego: inne konteksty pokazują pole tylko do odczytu, a API odrzuca w nich zapisy. |
| `readonly` | wartość logiczna | `false` | Pokazuje pole w aplikacji webowej, ale uniemożliwia jego edycję w niej. API, CLI, agenci, przepływy pracy i rozszerzenia nadal mogą je zapisywać. Używaj dla wartości, którymi zarządza integracja. |
| `tags` | tablica ciągów znaków | `[]` | Dowolne metadane. Muszą być unikalne i niepuste. Niektóre tagi ukrywają atrybut w aplikacji webowej; zobacz [Tagi widoczności](#tagi-widoczności). |
| `default_value` | zgodny z typem | Nieustawiony | Wartość zapisywana w kontekście domyślnym, gdy rekord zostaje utworzony bez niej. Tylko typy skalarne. |
| `value_schema` | ciąg znaków (JSON) | Nieustawiony | JSON Schema dla jednej wartości. Tylko typy skalarne. Schemat atrybutu `string` może uczynić go [statusem](#statusy) lub [przypisaniem użytkownika lub zespołu](#przypisania-użytkowników-i-zespołów). Zobacz [Walidacja](/pl/builders/validation/). |
| `generated` | mapowanie | Nieustawiony | Serwer numeruje nowe rekordy, np. numerami faktur lub zamówień. Tylko atrybuty `string` i `integer` w schematach rekordów. Zobacz [Wartości generowane](#wartości-generowane). |
| `balance` | mapowanie | Nieustawiony | Suma bieżąca, którą zmieniają tylko [przesunięcia](#salda-i-przesunięcia), np. stan magazynowy. Tylko atrybuty `number` i `integer` w schematach rekordów. |

### Typy wartości

| `value_type` | Przechowywana wartość | Przykład CLI/YAML | Uwagi |
| --- | --- | --- | --- |
| `string` | Tekst | `value: Blue shirt` | Wyczyszczenie ciągu znaków w kontekście innym niż domyślny usuwa nadpisanie zamiast zapisywać `""`. Może też być [statusem](#statusy) lub [przypisaniem użytkownika lub zespołu](#przypisania-użytkowników-i-zespołów). |
| `number` | Liczba dziesiętna | `value: 19.99` | |
| `integer` | 64-bitowa liczba całkowita | `value: 12` | |
| `boolean` | `true` lub `false` | `value: true` | |
| `date` | Data kalendarzowa | `value: 2026-03-01` | |
| `datetime` | Znacznik czasu z przesunięciem | `value: 2026-03-01T09:30:00Z` | RFC 3339. |
| `time` | Godzina zegarowa i strefa czasowa IANA | `value: {time: '09:30:00', time_zone: Europe/Warsaw}` | Obie części są wymagane. |
| `json` | Dowolna wartość JSON | | Nie można według niej sortować ani używać jej w filtrach przeglądarki rekordów. Preferuj atrybuty typowane lub relacje. |
| `relationship` | Powiązania z innymi rekordami | | Zobacz [Klucze relacji](#klucze-relacji). |
| `file` | Przesłane pliki | | Zobacz [Klucze plików](#klucze-plików). |

### Wskazówki wzorca

Opcjonalny łańcuch `x-attricat-pattern-hint` opisuje słowami `pattern` schematu na potrzeby formularzy rekordów. Wymaga `pattern` i ma od 1 do 200 znaków. Zobacz [Co pokazują formularze](/pl/builders/validation/#co-pokazują-formularze).

### Listy wyboru

Atrybut `string`, którego `value_schema` ma `enum`, edytuje się przez wybór jednej wartości: przyciski opcji dla najwyżej pięciu wartości, lista z wyszukiwaniem dla większej liczby. Opcjonalny obiekt `x-attricat-enum-labels` przypisuje wartościom `enum` etykiety; każdy klucz musi być wartością `enum`, a etykieta ma od 1 do 200 znaków. Nie można go łączyć z `x-attricat-status`. Zobacz [Listy wyboru](/pl/builders/validation/#listy-wyboru).

### Statusy

Atrybut `string`, którego `value_schema` ma `enum` i adnotację `x-attricat-status`, jest statusem. Działanie statusów opisuje sekcja [Statusy](/pl/builders/validation/#statusy).

```yaml
attributes:
  - code: status
    value_type: string
    value_schema: |-
      {
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
            { "from": "released", "to": "draft", "code": "correct", "permission": "records.publish" }
          ]
        }
      }
```

| Klucz | Wartość | Opis |
| --- | --- | --- |
| `version` | `1` | Wymagany. |
| `options` | lista od 1 do 100 mapowań | Wymagany. Jedna opcja dla każdego kodu z `enum`, w kolejności wyświetlania. |
| `transitions` | lista do 10 000 mapowań | Dozwolone zmiany. Pomiń, aby dopuścić każdą zmianę; jeśli jest podana, dozwolone są tylko wymienione zmiany, a pusta tablica nie dopuszcza żadnej. |

Każda opcja:

| Klucz | Wartość | Opis |
| --- | --- | --- |
| `code` | kod, do 128 znaków | Wymagany. Musi odpowiadać dokładnie jednej wartości z `enum`. |
| `label` | ciąg znaków, od 1 do 200 znaków | Wymagany. Może zawierać [odwołania do leksykonu](/pl/builders/translations/#etykiety-statusów-i-list-wyboru). |
| `tone` | `default`, `success`, `warning`, `error` lub `info` | Kolor znacznika statusu. |
| `lock` | `"all"` lub od 1 do 500 kodów atrybutów | Sprawia, że wymienione atrybuty są tylko do odczytu, dopóki rekord ma ten status. `"all"` obejmuje wszystkie atrybuty, relacje i pliki oprócz samego statusu. Atrybut wielokrotnego użytku zapisuje się jako `namespace:code`. Rekordu ze statusem z blokadą nie można usunąć. Właściciele i administratorzy zawsze mogą go odblokować, zmieniając status. Wymaga `transitions`. |
| `approval` | mapowanie z `covers` i `void_to` | Rejestruje zatwierdzenie, gdy rekord przechodzi do tego statusu. `covers` to `"all"` lub od 1 do 500 kodów atrybutów; `void_to` to inna opcja, do której rekord przechodzi, gdy objęta treść się zmieni. |
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

```yaml
attributes:
  - code: assignee
    value_type: string
    value_schema: '{"type": "string", "x-attricat-principal": {"version": 1, "kinds": ["user", "team"]}}'
```

| Klucz | Wartość | Opis |
| --- | --- | --- |
| `version` | `1` | Wymagany. |
| `kinds` | `["user"]`, `["team"]` lub oba | Wymagany. Co przyjmuje atrybut. |

- Wartości mają postać `user:<id>` lub `team:<id>` z identyfikatorem zapisanym małymi literami, pobranym z `acli directory list` lub `GET /directory`. Brak wartości oznacza brak przypisania.
- Schemat musi mieć `"type": "string"` i nie może mieć `enum`, `const`, `pattern` ani `format`. Atrybut nie może mieć `default_value` ani być jednocześnie statusem. Atrybuty wielokrotnego użytku przyjmują tę samą adnotację.
- Nowa lub zmieniona wartość musi wskazywać aktywnego członka obszaru roboczego lub nieusunięty zespół, w dozwolonym rodzaju. W przeciwnym razie zapis zwraca `422 attribute_value_schema_mismatch`. Niezmienione wartości nie są sprawdzane ponownie.
- Filtry wyszukiwania porównują zapisaną wartość dokładnie. Wartość `@me` z operatorem `eq` pasuje do wywołującego i do każdego zespołu, do którego należy.
- [Predykaty](#predykaty) traktują wartość jak tekst: `required` sprawdza, czy rekord jest przypisany, a `compare` z `eq` lub `one_of` może dopasować konkretne `user:<id>` lub `team:<id>`. Żaden predykat ani [wymaganie przejścia](#statusy) nie może odwoływać się do osoby zapisującej zmianę, więc nie da się zadeklarować zasady „zamknąć może tylko osoba przypisana”.

### Klucze relacji

```yaml
attributes:
  - code: brand
    value_type: relationship
    target_blueprint: brand
    cardinality: one
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `target_blueprint` | kod | Dowolny schemat rekordu | Ogranicza cele do rekordów z tej rodziny schematów. |
| `target_blueprints` | tablica kodów | Dowolny schemat rekordu | Ogranicza cele do rekordów z dowolnej z tych rodzin schematów. Nie można łączyć z `target_blueprint`; lista z jednym elementem działa tak samo jak `target_blueprint`. |
| `cardinality` | `"one"`, `"many"` lub `"one_to_one"` | `"many"` | Z iloma celami jeden rekord może być powiązany w jednym kontekście. `one_to_one` to skrót dla `cardinality: one` razem z `target_cardinality: one` i nie można go łączyć z `target_cardinality`. |
| `target_cardinality` | `"one"` lub `"many"` | `"many"` | Ile rekordów może wskazywać ten sam cel przez ten atrybut w jednym kontekście. |
| `acyclic` | wartość logiczna | `false` | Odrzuca powiązania, które utworzyłyby cykl przez ten atrybut. Wymaga `context_editable: default`, a cele muszą obejmować sam schemat. Zobacz [Hierarchie](#hierarchie). |
| `tree` | wartość logiczna | `false` | Hierarchia acykliczna, w której każdy rekord ma co najwyżej jeden cel (rodzica). Oznacza `acyclic: true` i domyślnie ustawia `cardinality` na `"one"`; `cardinality: many` jest odrzucane. |
| `target_scope` | mapowanie | Nieustawiony | `{attribute, equals}`: wybór rekordu pokazuje tylko cele, których `attribute` wskazuje ten sam rekord co `equals` tego rekordu. Zobacz [Zakres celu](#zakres-celu). |

`cardinality: one` daje pole jednokrotnego wyboru, którego opcje mogą być współdzielone, np. marka. Dodaj `target_cardinality: one` tylko dla wyłącznego powiązania, w którym każdy cel może zostać zajęty raz. Zapis naruszający którykolwiek limit zwraca `409 relationship_cardinality_conflict`.

Zapis wiążący rekord schematu niedozwolonego przez `target_blueprint` lub `target_blueprints` zwraca `422 relationship_target_type_mismatch`. Okno wyboru rekordu w aplikacji webowej oferuje tylko dozwolone schematy; gdy jest ich kilka, ma pole **Schemat celu**. Blok `incoming_relationship_list` w każdym z dozwolonych schematów docelowych może wyświetlać to pole.

#### Hierarchie

```yaml
attributes:
  - code: parent
    value_type: relationship
    target_blueprint: location
    tree: true
    context_editable: default
```

`acyclic` i `tree` chronią struktury odwołujące się do siebie, takie jak hierarchie lokalizacji czy zasobów, oraz łańcuchy poprzedników, np. wersja → poprzednia wersja.

- Zapis, który zamknąłby cykl, zwraca `409 relationship_cycle`. `error.details.path` zawiera identyfikatory rekordów wzdłuż cyklu, zaczynając i kończąc na zapisywanym rekordzie. Powiązanie rekordu z nim samym jest cyklem o długości jeden.
- W drzewie (`tree`) nadanie rekordowi drugiego celu zwraca `409 relationship_cardinality_conflict`.
- Sprawdzenia uwzględniają powiązania ze wszystkich wersji schematu i działają w transakcji zapisu. Dwa równoczesne zapisy nie mogą każdy dodać połowy cyklu.
- Hierarchia dotyczy całej rodziny schematów zgodnie z jej najnowszą opublikowaną wersją, także rekordów przypiętych do starszych wersji.
- Publikacja wersji, która dodaje `acyclic` lub `tree`, najpierw sprawdza istniejące powiązania. Jeśli zawierają cykle albo drzewo ma rekordy z więcej niż jednym celem, publikacja kończy się błędem `409 relationship_hierarchy_violations`; `error.details` wymienia do 20 cykli i rekordów z nadmiarowymi celami. Popraw powiązania i opublikuj ponownie.

#### Zakres celu

```yaml
attributes:
  - code: product
    value_type: relationship
    target_blueprint: product
    cardinality: one
  - code: conversion
    value_type: relationship
    target_blueprint: unit_conversion
    cardinality: one
    target_scope: {attribute: product, equals: product}
```

Z `target_scope` wybór rekordu w polu `conversion` pokazuje tylko przeliczniki jednostek, których `product` jest produktem wybranym w tym samym formularzu. Dopóki produkt nie jest wybrany, wybór pokazuje wszystkie przeliczniki.

- Atrybut musi być relacją z dokładnie jednym schematem docelowym. `equals` musi być inną relacją tego schematu z `cardinality: one`, a `attribute` relacją schematu docelowego z `cardinality: one`. Publikacja sprawdza `attribute`, gdy schemat docelowy ma opublikowaną wersję.
- Zakres tylko filtruje wybór. Aby odrzucić niepasujące powiązanie przy zapisie, dodaj [kontrolę rekordu](#kontrole-rekordów), np. `{"type": "linked", "relationship_code": "conversion", "predicate": {"type": "compare", "attribute_code": "product", "op": "eq", "subject_attribute_code": "product"}}`.

Relacje nie mogą mieć `value_schema` ani `default_value`. Ograniczaj je zamiast tego przez `record_schema`.

### Klucze plików

```yaml
attributes:
  - code: gallery
    value_type: file
    cardinality: many
    allowed_mime_groups: [image]
    allowed_extensions: [jpg, png, webp]
    max_bytes: 10485760
    image_only: true
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `cardinality` | `"one"` lub `"many"` | `"one"` | Pojedynczy plik lub lista plików. |
| `ordered` | wartość logiczna | `true` dla `many` | Czy kolejność listy `many` ma znaczenie. Dla `one` musi mieć wartość `false` lub zostać pominięty. |
| `allowed_mime_groups` | tablica ciągów znaków | Dowolne | Akceptowane grupy MIME, np. `image`. Pliki CSV i inne pliki tekstowe są zapisywane jako `text/plain`, więc należą do grupy `text`; aby akceptować tylko `csv`, użyj `allowed_extensions`. |
| `allowed_extensions` | tablica ciągów znaków | Dowolne | Akceptowane rozszerzenia plików, wyłącznie litery i cyfry (`jpg`, a nie `*.jpg`). Początkowa `.` jest ignorowana. |
| `max_bytes` | dodatnia liczba całkowita | Limit serwera | Limit rozmiaru pojedynczego pliku. Gdy jest wyższy, nadal obowiązuje `FILE_UPLOAD_MAX_BYTES` serwera. |
| `purposes` | tablica kodów | `[]` | Etykiety opisujące przeznaczenie plików, np. `product_image`. |
| `image_only` | wartość logiczna | `false` | Akceptuje tylko obrazy. Wymagane dla renderera `attricat.table_image`. |

Te klucze są odrzucane w każdym atrybucie, który nie ma `value_type: file`. Atrybuty plikowe nie mogą mieć `value_schema`, `default_value` ani `target_blueprint`.

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

### Tagi wyszukiwania

`search:fuzzy` na atrybucie tekstowym sprawia, że wyszukiwanie w przeglądarce rekordów znajduje też wartości podobne do terminu, np. inną formę tego samego słowa albo wartość z brakującą lub dodatkową literą: `tarcze` znajduje „Tarcza tnąca”. Dotyczy zwykłych i kwalifikowanych terminów o długości co najmniej czterech liter, ale nie terminów zakończonych `*`. Atrybuty bez tego tagu nadal pasują tylko do wartości zawierających termin. Zobacz [Składnia wyszukiwania](/pl/guides/search-syntax/#podobne-słowa).

```yaml
attributes:
  - code: name
    value_type: string
    tags: ['search:fuzzy']
```

### Wartości generowane

`generated` sprawia, że serwer nadaje każdemu nowemu rekordowi kolejny numer z licznika, np. numer faktury, zamówienia lub dokumentu:

```yaml
attributes:
  - code: invoice_number
    value_type: string
    generated:
      format: 'INV-{yyyy}-{n:05}'
      start: 1
      reset: yearly
      time_zone: Europe/Warsaw
```

Pierwsza faktura utworzona w 2026 roku otrzymuje `INV-2026-00001`, następna `INV-2026-00002`, a pierwsza utworzona w 2027 roku `INV-2027-00001`.

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `format` | ciąg znaków, do 100 znaków | Wymagany dla `string` | Sposób budowania wartości z [tokenów formatu](#tokeny-formatu). Niedozwolony dla `integer`. |
| `start` | liczba całkowita od 0 do 999 999 999 999 999 999 | `1` | Pierwszy numer, który wydaje nowy licznik. |
| `reset` | `"never"`, `"yearly"`, `"monthly"` lub `"daily"` | `"never"` | Rozpoczyna liczenie od `start` w każdym nowym okresie. Niedozwolony dla `integer`. |
| `time_zone` | strefa czasowa IANA | `"UTC"` | Strefa, w której tokeny daty i okres resetu są odczytywane z czasu utworzenia rekordu. Niedozwolony dla `integer`. |

Generowane mogą być tylko atrybuty `string` i `integer`. `string` wymaga `format`; `integer` przechowuje sam numer i przyjmuje tylko `start`.

`generated` jest dozwolony tylko w lokalnym atrybucie schematu rekordu. Jest odrzucany w mixinie, w atrybucie wybranym przez `from`, w `extension_type`, razem z `default_value` oraz w [statusie](#statusy) lub [przypisaniu użytkownika lub zespołu](#przypisania-użytkowników-i-zespołów). Atrybut generowany jest zawsze tylko do odczytu i zapisywany wyłącznie w kontekście domyślnym (`context_editable: default`), niezależnie od wartości `readonly` i `context_editable`.

#### Tokeny formatu

| Token | Zapisuje |
| --- | --- |
| `{n}` | Licznik, bez dopełniania. Wymagany dokładnie raz. |
| `{n:0W}` | Licznik dopełniony zerami do `W` cyfr, gdzie `W` wynosi od 1 do 18. Dłuższe liczby nie są obcinane: po `999` `{n:03}` daje `1000`. |
| `{yyyy}` | Czterocyfrowy rok. |
| `{yy}` | Dwucyfrowy rok. |
| `{mm}` | Dwucyfrowy miesiąc. |
| `{dd}` | Dwucyfrowy dzień. |
| `{{` i `}}` | Dosłowne `{` i `}`. |

Każdy inny symbol zastępczy `{…}` jest odrzucany. `reset` wymaga tokenów, które identyfikują jego okres, aby wartości z różnych okresów nigdy nie wyglądały tak samo: `yearly` wymaga `{yyyy}` lub `{yy}`, `monthly` dodatkowo `{mm}`, a `daily` dodatkowo `{dd}`. W przeciwnym razie schemat jest odrzucany.

#### Jak nadawane są numery

- Serwer przypisuje wartość w transakcji, która tworzy rekord. Pojawia się ona w odpowiedzi na utworzenie, w zdarzeniu `record.created.v1` i w dzienniku audytu, a `required` dla tego atrybutu w `record_schema` jest spełnione.
- W formularzu tworzenia rekordu w aplikacji internetowej pole pozostaje puste i informuje, że wartość zostanie przypisana przy zapisie rekordu. Nigdy nie jest tam oznaczone jako wymagane, nawet gdy wymaga go `record_schema`, ponieważ nie ma czego wypełniać.
- Numeracja jest ciągła, bez luk. Utworzenie, które się nie powiedzie, np. z powodu walidacji lub konfliktu klucza unikalnego, zwraca swój numer. Usunięte rekordy zachowują swoje numery, a numery nigdy nie są używane ponownie.
- Istnieje jeden licznik na rodzinę schematów i kod atrybutu, a przy ustawionym `reset` jeden na każdy okres. Nowe wersje schematu kontynuują ten sam licznik. Późniejsza zmiana `start` nie wpływa na już istniejący licznik, a zmiana `format` dotyczy tylko nowych wartości. Nowa wersja nie może zmienić `reset`, gdy atrybut był generowany we wcześniejszej wersji, ponieważ numeracja zaczęłaby się od nowa w nowym okresie i powtórzyła wydane już numery. Taka wersja jest odrzucana z `409 generated_reset_changed`, a `error.details` zawiera `attribute` oraz jego poprzedni (`previous`) i żądany (`requested`) reset.
- Zduplikowanie rekordu nadaje kopii nowy numer. Wsad rekordów numeruje po kolei każdy tworzony rekord. Przykładowe rekordy pakietu rozwiązań zużywają numery, a usunięcie danych przykładowych ich nie zwraca.
- Żaden klient nie może zapisać atrybutu generowanego. Utworzenia, aktualizacje, czyszczenie wartości, przywracanie z historii, wsady rekordów, przepływy pracy, agenci i rozszerzenia, które tego próbują, są odrzucane z `422 generated_attribute_readonly`; `error.details.attribute` wskazuje atrybut.
- Gdy istniejący rekord migruje do wersji, która dodaje atrybut generowany, otrzymuje numer podczas migracji. Rekordy są numerowane w kolejności migracji, a nie utworzenia, a tokeny daty i okres resetu korzystają z pierwotnej daty utworzenia każdego rekordu. Wartość, którą rekord miał już w poprzedniej wersji, zostaje zachowana.

#### Liczniki

`GET /blueprints/{id}/sequences` wyświetla liczniki schematu i wymaga `blueprints.read`. Każdy wpis zawiera `attribute_code`, okres `period` (pusty dla licznika, który nigdy się nie resetuje), `next_value` (numer, który otrzyma następny rekord) i `updated_at` (`null`, dopóki licznik nie został użyty).

Aby kontynuować numerację po przejściu z innego systemu, podnieś licznik przez `POST /blueprints/{id}/sequences/{attribute_code}/advance`:

```json
{ "next_value": 10000, "period": "2026" }
```

- `period` jest opcjonalny i domyślnie oznacza bieżący okres. Ma postać `2026` dla `yearly`, `2026-10` dla `monthly`, `2026-10-27` dla `daily` i jest pusty dla licznika, który nigdy się nie resetuje.
- Wywołanie wymaga `blueprints.publish` i jest zapisywane w dzienniku audytu.
- Licznik można tylko podnieść. Niższa wartość `next_value` kończy się błędem `409 generated_sequence_not_raised`, a `error.details` zawiera wartości `current` i `requested`. Ta sama wartość niczego nie zmienia.

W CLI użyj `acli blueprint sequences <blueprint-id>` i `acli blueprint advance-sequence <blueprint-id> <attribute-code> <next-value> [--period <period>]`.

### Typy atrybutów z rozszerzeń

Włączone rozszerzenie może deklarować typy atrybutów, np. typ kwoty pieniężnej z ustawieniem waluty:

```yaml
attributes:
  - code: price
    extension_type: 'com.acme.commerce:money@^1'
    extension_configuration: '{"currency":"USD"}'
```

Gdy schemat jest zapisywany, Attricat odnajduje włączone rozszerzenie `com.acme.commerce`, wybiera najwyższą zadeklarowaną wersję typu `money` pasującą do `^1`, weryfikuje `extension_configuration` względem schematu konfiguracji typu i zapisuje rozstrzygnięty typ w wersji schematu. Wartość jest przechowywana w bazowym typie prostym tego typu (`string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time` lub `json`) i sprawdzana względem schematu wartości typu. Wartości pozostają czytelne, jeśli rozszerzenie zostanie później wyłączone.

`extension_type` nie można łączyć z `value_schema`, kluczami relacji ani kluczami plików.

## Include

Mixiny współdzielą definicje atrybutów między schematami. Include przypina jedną konkretną opublikowaną wersję mixinu. Każdy potrzebny atrybut trzeba następnie wybrać przez `from`.

```yaml
includes:
  - alias: seo
    code: seo
    version: 2

attributes:
  - code: meta_title
    from: seo.meta_title
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
| `page_title` | Tytuł karty przeglądarki na stronie rekordu. Opcjonalny w schematach rekordów. | `page_title` |
| `detail` | Strona rekordu i formularz tworzenia. Pola, które użytkownik może zmienić, są edytowane w miejscu. | Blok układu |
| `edit` | Przestarzały i ignorowany przez aplikację internetową. Nadal akceptowany, a komponenty edycji w nim nadal są walidowane. | Blok układu |
| `table` | Kolumny przeglądarki rekordów. | `table` |
| `explorer` | Ustawienia domyślne przeglądarki rekordów, np. atrybut wyświetlany w panelu **Przeglądaj według**. | `explorer` |
| `extension_layout` | Kolejność i widoczność kontrybucji z rozszerzeń na stronach rekordów tego schematu. | `extension_layout` |

Gdy brakuje `detail` lub `table`, aplikacja webowa wyświetla atrybuty w kolejności deklaracji.

### `dropdown_option`

```yaml
views:
  dropdown_option:
    type: dropdown_option
    fields: [name, sku]
    separator: ' / '
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `fields` | tablica kodów atrybutów | Wymagany | Jeden lub więcej unikalnych atrybutów łączonych w etykietę. |
| `separator` | ciąg znaków | `" · "` | Tekst wstawiany między pola. |

Etykiety są rozstrzygane dla każdego kontekstu i respektują `context_fallback` każdego atrybutu.

### `page_title`

```yaml
views:
  page_title:
    type: page_title
    fields: [name]
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `fields` | tablica kodów atrybutów | Wymagany | Jeden lub więcej unikalnych atrybutów skalarnych łączonych w tytuł. Puste wartości są pomijane. |
| `separator` | ciąg znaków | `" · "` | Tekst wstawiany między pola. |

Może go zdefiniować tylko schemat rekordów i tylko pod nazwą `page_title`. Tytuł korzysta z wartości widocznych w wybranym kontekście. Bez tego widoku lub gdy wszystkie jego pola są puste, strona rekordu używa etykiety `dropdown_option`, a następnie skróconego identyfikatora rekordu.

### `table`

```yaml
views:
  table:
    type: table
    columns:
      - {field: title, label: Product}
      - {field: category.name, label: Category}
      - field: main_photo
        label: Image
        renderer: {id: attricat.table_image, version: 1}
```

| Klucz | Typ | Opis |
| --- | --- | --- |
| `columns` | lista mapowań | Definicje kolumn. Musi być niepusta, z unikalnymi wartościami `field`. |
| `columns[].field` | ciąg znaków | Lokalny atrybut skalarny, lokalna relacja (etykiety powiązanych rekordów, najwyżej pięć, dowolnego schematu docelowego), lokalny atrybut plikowy ze zgodnym rendererem lub ścieżka o maksymalnie trzech krokach relacji zakończona atrybutem skalarnym, np. `family.product_type.name`. Krok może wskazywać kilka schematów; następny atrybut musi wtedy mieć jeden typ wartości w każdym schemacie, który go deklaruje. |
| `columns[].label` | ciąg znaków | Nagłówek kolumny. Domyślnie nazwa atrybutu. |
| `columns[].renderer` | odwołanie do komponentu | `attricat.table_image@1`, komponent wyświetlania tekstu (`attricat.color_display@1`, `attricat.email_display@1`, `attricat.url_display@1` lub `attricat.phone_display@1`) albo komórka tabeli z rozszerzenia. |
| `component` | odwołanie do komponentu | Opcjonalny komponent tabeli. |

Kolumnę można sortować w przeglądarce rekordów tylko wtedy, gdy jest skalarna, a każdy krok relacji w jej ścieżce ma `cardinality: one`.

### `explorer`

```yaml
views:
  explorer:
    type: explorer
    browse_by: categories
```

| Klucz | Typ | Opis |
| --- | --- | --- |
| `browse_by` | kod atrybutu, ścieżka relacji lub `false` | Atrybut relacji wyświetlany w panelu **Przeglądaj według**, dopóki użytkownik nie wybierze innego. Relacja do kilku schematów wyświetla ich rekordy razem, bez drzewa. Ścieżka z kropkami, na przykład `model.brand`, przechodzi przez maksymalnie trzy relacje, z których każda ma jeden schemat docelowy i `cardinality: one`, i wyświetla cele ostatniej z nich. `false` pozostawia panel zamknięty, dopóki użytkownik go nie otworzy. |

Może go zdefiniować tylko schemat rekordów i tylko pod nazwą `explorer`. Bez `browse_by` panel otwiera się dla pierwszej relacji, której schemat docelowy ma hierarchię. Kolejne relacje na ścieżce są sprawdzane względem opublikowanych schematów docelowych podczas zapisywania i publikowania schematu.

### Bloki układu

`detail` i `edit` zaczynają się od bloku kontenera i zagnieżdżają w nim inne bloki. Bloki to mapowania YAML z kluczem `type`. Możesz je zapisać w stylu blokowym, z jednym kluczem w wierszu, lub w stylu przepływowym jako `{type: field, field: title}`; oba zapisy dają ten sam wynik.

| `type` | Klucze | Opis |
| --- | --- | --- |
| `stack` | `children` | Elementy podrzędne ułożone pionowo. |
| `grid` | `children` | Elementy podrzędne w responsywnej siatce. |
| `section` | `children` | Zgrupowany obszar. |
| `tabs` | `tabs`: lista `{label, children}` | Jedna karta na wpis. |
| `accordion` | `sections`: lista `{label, children}` | Zwijane sekcje. |
| `field` | `field`; opcjonalnie `editor` | Jeden atrybut skalarny lub plikowy. W `detail` klucz `editor` wskazuje edytor atrybutu z rozszerzenia przez `extension_id`, `id`, `version` i `props`; zobacz [Edytory atrybutów](/pl/extensions/manifest/#edytory-atrybutów). |
| `relationship_list` | `field` | Jeden atrybut relacji i jego cele. |
| `incoming_relationship_list` | `label`, `relationships`, `page_size`, `display`, `editable`, `columns` | Link otwierający Przeglądarkę rekordów w nowej karcie, przefiltrowaną do rekordów wskazujących na ten rekord; osobny link dla każdej relacji. `relationships` to lista `{source_blueprint, field}`. `page_size` jest ograniczane przez `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE` serwera. Z `display: table` (domyślnie `links`) rekordy dokładnie jednej relacji są zamiast tego wyświetlane w tabeli na stronie, najwyżej `page_size`, z atrybutami źródła wskazanymi w `columns` (bez niego ze wszystkimi poza relacją zwrotną). `editable: true` pozwala edytować komórki tabeli i dodaje do niej **Dodaj** oraz **Usuń**; zobacz [Pozycje zlecenia](#pozycje-zlecenia). Dozwolone tylko w `detail`. |
| `heading` | `text` | Statyczny nagłówek. |
| `text` | `text` | Statyczny akapit. |
| `divider` | | Linia pozioma. |

Każdy blok oprócz `explorer` przyjmuje opcjonalne odwołanie `component`. Każdy `field` i `relationship_list` musi wskazywać atrybut, który schemat posiada, łącznie z atrybutami wybranymi z domieszek.

### Odwołania do komponentów

```yaml
component:
  id: attricat.url_display
  version: 1

renderer:
  id: example.currency
  version: 1
  props:
    currency: USD
```

| Klucz | Typ | Opis |
| --- | --- | --- |
| `id` | ciąg znaków | Identyfikator zarejestrowanego komponentu. |
| `version` | liczba całkowita | Wersja komponentu. |
| `props` | mapowanie | Opcje komponentu. Akceptowane są tylko właściwości zadeklarowane przez komponent. |

Wbudowane komponenty:

| ID | Wersja | Używany w | Typy wartości | Właściwości |
| --- | --- | --- | --- | --- |
| `attricat.field_display` | 1 | `field` (detail) | Skalarne | |
| `attricat.relationship_list_display` | 1 | `relationship_list` (detail) | `relationship` | |
| `attricat.relationship_hierarchy` | 1 | `relationship_list` (detail) | `relationship` | `parent_field` |
| `attricat.incoming_relationship_list_display` | 1 | `incoming_relationship_list` | | |
| `attricat.record_heading` | 1 | `stack` (detail) | | |
| `attricat.table_display` | 1 | `table` | Skalarne | |
| `attricat.table_image` | 1 | `renderer` kolumny tabeli | `file` z `cardinality: one` i `image_only: true` | |
| `attricat.color_display` | 1 | `field` (detail), `renderer` kolumny tabeli | `string` | |
| `attricat.email_display` | 1 | `field` (detail), `renderer` kolumny tabeli | `string` | |
| `attricat.url_display` | 1 | `field` (detail), `renderer` kolumny tabeli | `string` | |
| `attricat.phone_display` | 1 | `field` (detail), `renderer` kolumny tabeli | `string` | |
| `attricat.markdown_display` | 1 | `field` (detail) | `string` | |

Tam, gdzie pole w `detail` jest edytowalne, aplikacja internetowa używa kontrolki powiązanej z jego komponentem wyświetlania albo standardowego edytora dla typu wartości. Działanie tych komponentów opisano w sekcji [Kontrolki pól](/pl/builders/views/#kontrolki-pól).

Identyfikator i wersja komórki z rozszerzenia muszą odpowiadać [komórce tabeli](/pl/extensions/manifest/#komórki-tabeli) zadeklarowanej przez włączone rozszerzenie dla typu wartości kolumny, a jej `props` muszą pasować do zadeklarowanych typów właściwości.

### `extension_layout`

```yaml
views:
  extension_layout:
    type: extension_layout
    version: 1
    outlets:
      record_tab:
        order: ['acme.inventory:summary']
        hidden: ['acme.legacy:panel']
```

| Klucz | Opis |
| --- | --- |
| `version` | Musi wynosić `1`. |
| `outlets` | Klucze `record_tab` lub `record_action`. |
| `outlets.<outlet>.order` | Klucze kontrybucji (`<extension-id>:<contribution-id>`) w kolejności wyświetlania. |
| `outlets.<outlet>.hidden` | Klucze kontrybucji do ukrycia. |

Punkt osadzenia wymieniony tutaj zastępuje domyślny układ obszaru roboczego dla tego punktu na rekordach tego schematu. Punkty, których nie wymienisz, zachowują układ obszaru roboczego. Klucze rozszerzeń, które nie są zainstalowane, są zachowywane, więc układ przetrwa wyłączenie i ponowne włączenie rozszerzenia.

## Publikacja

```yaml
publication:
  retain_on_edit_roles: [editor]
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `retain_on_edit_roles` | tablica kodów ról | `[]` | Role obszaru roboczego, których edycje zachowują istniejące publikacje rekordu w kanałach. Edycje wszystkich pozostałych osób je wycofują. Role muszą istnieć w chwili publikacji wersji. |

Nie przyznaje to żadnych uprawnień. Zobacz [Publikowanie](/pl/guides/publishing/).

## Reguły

Schematy nie zawierają reguł jakości danych. Regułę tworzy się osobno i dotyczy ona rekordów Schematu; zobacz [Reguły](/pl/builders/rules/). Definicja z listą `rules` jest odrzucana.

## Predykaty

Reguły, kontrole rekordów, warunki przejść i kontrole kanałów publikacji używają jednego języka predykatów. Predykat to mapowanie (w YAML) lub obiekt (w JSON) rozróżniany kluczem `type`. Jest **spełniony**, gdy dane są poprawne. Nieznane klucze są odrzucane.

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
| `referenced_by` | `blueprint_code`, `relationship_code`, opcjonalnie `predicate`, `min` i/lub `max` (od 0 do 1000) | Liczba rekordów `blueprint_code`, których relacja `relationship_code` wskazuje ten rekord i które spełniają `predicate`, mieści się między `min` a `max`. `max: 0` oznacza „żaden”. |
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

`x-attricat-checks` to tablica wewnątrz `record_schema`:

```yaml
record_schema: |
  {
    "type": "object",
    "x-attricat-checks": [
      { "code": "valid-range", "message": "Valid until must not be before valid from",
        "predicate": { "type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from" } }
    ]
  }
```

| Klucz | Typ | Wymagany | Opis |
| --- | --- | --- | --- |
| `code` | kod | Tak | Unikalny w obrębie tablicy. |
| `message` | ciąg znaków | Nie | Od 1 do 500 znaków. Zastępuje wygenerowany komunikat. |
| `predicate` | obiekt | Tak | [Predykat](#predykaty) z wyjątkiem `stale`, `unique` i `acyclic`. |

Najwyżej 32 kontrole. Niepowodzenie zwraca `422 record_check_failed`. Zobacz [Walidacja](/pl/builders/validation/#porównuj-atrybuty-za-pomocą-kontroli).

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

```yaml
unique_keys:
  - code: manufacturer_part
    attributes: [manufacturer, part_number]

  - code: slug
    attributes: [slug]
    scope: context
    case_sensitive: true
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `code` | kod | Wymagany | Unikalny w obrębie schematu. Podawany w błędach. |
| `attributes` | tablica kodów atrybutów | Wymagany | Od jednego do ośmiu atrybutów, których łączne wartości muszą być unikalne. Każdy musi być wartością skalarną innego typu niż `json` albo relacją z `cardinality: one`. |
| `scope` | `"workspace"` lub `"context"` | `"workspace"` | `workspace` porównuje wartości w kontekście domyślnym. `context` porównuje wartości wyświetlane w każdym kontekście, także dziedziczone, osobno dla każdego kontekstu. |
| `case_sensitive` | wartość logiczna | `false` | Porównuje tekst dokładnie zamiast bez rozróżniania wielkości liter. |

Jak porównywane są wartości:

- Tekst jest przycinany, a każdy ciąg białych znaków zamieniany na jedną spację. Jeśli nie ustawiono `case_sensitive: true`, tekst jest też porównywany małymi literami, więc `ABC-1  Rev` i ` abc-1 rev` to ten sam klucz.
- Liczby są porównywane według wartości (`1.50` równa się `1.5`), daty z czasem według chwili, a relacje według powiązanego rekordu.
- Rekord, który nie ma wartości (albo ma tylko pusty tekst) dla któregokolwiek atrybutu klucza, nie jest sprawdzany względem tego klucza. Jeśli każdy rekord musi mieć klucz, oznacz atrybuty jako wymagane w `record_schema`.
- Klucz obejmuje całą rodzinę schematów zgodnie z jej najnowszą opublikowaną wersją, także rekordy przypięte do starszych wersji. Atrybuty są dopasowywane według kodu.

Zapis, który nadałby drugiemu rekordowi tę samą wartość klucza, zwraca `409 unique_key_conflict`. `error.details` zawiera `key`, kod kontekstu `context`, znormalizowane wartości `values` oraz `conflicting_record_id` rekordu, który już je ma. Sprawdzenie odbywa się w bazie danych w transakcji zapisu, więc gdy dwie osoby zapisują tę samą wartość w tym samym momencie, udaje się dokładnie jeden zapis.

Publikacja wersji, która dodaje lub zmienia klucze unikalne, najpierw sprawdza istniejące rekordy. Jeśli niektóre już współdzielą wartość, publikacja kończy się błędem `409 unique_key_duplicates`, a `error.details.duplicates` wymienia do 20 grup z kluczem, kontekstem, wartościami i identyfikatorami rekordów (`error.details.total` podaje liczbę wszystkich grup). Zmień lub usuń duplikaty i opublikuj ponownie.

## Salda i przesunięcia

Niektóre rekordy prowadzą sumę bieżącą, którą zmieniają inne rekordy, np. stan magazynowy, który przyjęcia towaru zwiększają, a wydania zmniejszają. Sumę przechowuje atrybut **salda**. **Przesunięcie** w innym schemacie określa, jak utworzenie jego rekordu zmienia tę sumę. Serwer stosuje przesunięcie w tej samej transakcji, w której tworzy rekord, więc suma zawsze zgadza się z dokumentami, a dwie osoby zapisujące w tej samej chwili nie zejdą poniżej minimum.

```yaml
# stock_item: jeden rekord na produkt
unique_keys:
  - code: product
    attributes: [product]
attributes:
  - code: product
    value_type: relationship
    target_blueprint: product
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    balance: {min: 0}
```

```yaml
# goods_issue: zmniejsza stan magazynowy swojego produktu
attributes:
  - code: product
    value_type: relationship
    target_blueprint: product
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: conversion
    value_type: relationship
    target_blueprint: unit_conversion
    cardinality: one
    context_editable: default
    target_scope: {attribute: product, equals: product}
  - code: stock_item
    value_type: relationship
    target_blueprint: stock_item
    cardinality: one
  - code: base_quantity
    value_type: number
  - code: reverses
    value_type: relationship
    target_blueprint: goods_issue
    cardinality: one
    context_editable: default
movements:
  - code: stock
    direction: out
    quantity: quantity
    factor: {relationship: conversion, attribute: factor}
    result: base_quantity
    target:
      relationship: stock_item
      blueprint: stock_item
      balance: quantity
      key: product
      values: {product: product}
    reversal: {relationship: reverses}
```

Przyjęcie towaru wygląda tak samo, z `direction: in` i `create_missing: true`, więc pierwsze przyjęcie produktu tworzy jego pozycję magazynową.

### Atrybuty sald

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `balance.min` | liczba | Bez minimum | Najniższa wartość, jaką może zostawić przesunięcie, np. `0` dla stanu, który nie może być ujemny. Dla salda `integer` musi być liczbą całkowitą. |
| `balance.reserved` | kod atrybutu | Brak | Inne saldo tego samego rekordu, które trzyma część tego salda, np. stan zarezerwowany. `min` dotyczy wtedy tego salda pomniejszonego o rezerwację. Zobacz [Rezerwacje](#rezerwacje). |

- `balance` jest dozwolone tylko w lokalnym atrybucie `number` lub `integer` schematu rekordów. Jest odrzucane w mixinie, z `from`, razem z `generated` lub `default_value` oraz w statusie lub przypisaniu użytkownika lub zespołu.
- Saldo jest zawsze tylko do odczytu i tylko w kontekście domyślnym (`context_editable: default`). Rekord bez wartości ma saldo zero.
- Saldo zmieniają tylko przesunięcia. Tworzenie, aktualizacje, czyszczenie wartości, przywracanie z historii, partie rekordów, przepływy pracy, agenci i rozszerzenia, które próbują je zapisać, są odrzucane z `422 balance_attribute_readonly`.
- Rekordu z niezerowym saldem nie można usunąć: `409 balance_record_not_empty` z `attribute` i jego `balance`. Rekordy przykładowe pakietów rozwiązań są wyjątkiem.
- Atrybuty klucza unikalnego, po którym przesunięcie odnajduje rekord, np. produkt pozycji magazynowej, nie mogą się zmienić, gdy mają już wartość: `409 movement_target_key_immutable`. Przesunięcia zostały już zastosowane do rekordu według tego klucza.

### Klucze przesunięcia

| Klucz | Typ | Wymagany | Opis |
| --- | --- | --- | --- |
| `code` | kod | Tak | Unikalny w obrębie schematu. |
| `direction` | `"in"` lub `"out"` | Tak | `in` dodaje przesuniętą ilość do salda, `out` ją odejmuje. |
| `quantity` | kod atrybutu | Tak | Atrybut `number` lub `integer` z ilością. Musi być większa od zera. |
| `factor.relationship` | kod atrybutu | Nie | Relacja z `cardinality: one` i jednym schematem docelowym do rekordu z przelicznikiem. |
| `factor.attribute` | kod | Z `factor` | Atrybut `number` lub `integer` powiązanego rekordu. Przesunięta ilość to `quantity × factor`; bez powiązania jest równa ilości. |
| `result` | kod atrybutu | Dla `reversal` | Atrybut `number`, w którym serwer zapisuje przesuniętą ilość w jednostce salda. `integer` tylko dla ilości całkowitej bez przelicznika. |
| `target.relationship` | kod atrybutu | Tak | Relacja z `cardinality: one` i `target_blueprint: <target.blueprint>`, w której serwer zapisuje cel. |
| `target.blueprint` | kod | Tak | Schemat rekordów celu. |
| `target.balance` | kod | Tak | Atrybut salda schematu docelowego. |
| `target.key` | kod | Tak | Klucz unikalny schematu docelowego z `scope: workspace`, który odnajduje cel. |
| `target.values` | mapowanie | Tak | Każdy atrybut klucza przypisany do atrybutu tego rekordu z jego wartością, np. `{product: product}`. Typy muszą się zgadzać. |
| `target.create_missing` | wartość logiczna | `false` | Utwórz cel z wartościami klucza, gdy żaden rekord ich nie ma. W przeciwnym razie przesunięcie się nie powiedzie. |
| `reversal.relationship` | kod atrybutu | Nie | Relacja z `cardinality: one` do tego samego schematu. Rekord, który ją ustawia, wycofuje powiązany rekord. Zobacz [Wycofania](#wycofania). |
| `apply_on` | lista | Przy utworzeniu | Do 8 zmian statusu, które księgują przesunięcie zamiast utworzenia rekordu: `{attribute, to, from?}`, opcjonalnie z `relationship`. Zobacz [Przesunięcia wyzwalane statusem](#przesunięcia-wyzwalane-statusem). |
| `requires` | lista kodów | Brak | Wcześniej zadeklarowane przesunięcia tego schematu, które muszą być już zaksięgowane na rekordzie; w przeciwnym razie to przesunięcie jest pomijane. |

Atrybuty odczytywane przez przesunięcie (ilość, relacja przelicznika, wartości klucza, relacja wycofania i relacja wyzwalacza) muszą mieć `context_editable: default`. Publikacja sprawdza schematy celu i przelicznika w ich najnowszych opublikowanych wersjach, więc najpierw opublikuj schematy pozycji magazynowej i przelicznika jednostek.

### Jak działa przesunięcie

Gdy przesunięcie jest księgowane (przy utworzeniu rekordu, chyba że ma `apply_on`):

1. Ilość musi być większa od zera, inaczej zapis zwraca `422 movement_quantity_invalid`. Powiązany przelicznik musi być większy od zera, inaczej zapis zwraca `409 movement_factor_invalid`.
2. Serwer odnajduje cel przez `target.key` i wartości klucza rekordu. Z `create_missing: true` brakujący cel zostaje utworzony; w przeciwnym razie zapis zwraca `409 movement_target_missing` z `blueprint` i `key`. Równoczesne pierwsze przyjęcia jednego produktu tworzą jeden cel.
3. Saldo celu zmienia się o `quantity × factor`. Jeśli spadłoby poniżej `min`, zapis zwraca `409 balance_below_minimum`, a `error.details` zawiera `attribute`, `record_id` celu, dostępne saldo `available`, żądaną ilość `requested` i `minimum`. Nic nie zostaje zapisane: ani rekord, ani zmiana.
4. Serwer zapisuje cel w `target.relationship`, a ilość w `result`. Klienci nie mogą ich zapisać: `422 movement_attribute_readonly`.

Zmiana celu jest zwykłą aktualizacją. Obowiązują jego blokady statusów, jest audytowana jako `attricat.records.movement.balance_update` z rekordem przesunięcia w metadanych i emituje `record.updated.v1`.

Przesunięcie jest księgowane raz. Po zaksięgowaniu atrybuty, które odczytało lub ustawiło, nie mogą się zmienić, a zapis, który próbuje je zmienić, zwraca `409 movement_immutable`. Pozostałe atrybuty rekordu można edytować, podobnie jak dane przesunięć jeszcze niezaksięgowanych, np. w zleceniu w wersji roboczej. Rekordu z zaksięgowanym przesunięciem nie można usunąć (`409 movement_record_undeletable`); zamiast tego go wycofaj. Odpowiedź formularza rekordu podaje jego zaksięgowane przesunięcia w `applied_movements`. Duplikat rekordu przesunięcia tworzy nowe przesunięcie.

### Przesunięcia wyzwalane statusem

Z `apply_on` przesunięcie jest księgowane przy zmianie statusu, a nie przy utworzeniu rekordu, np. rezerwacja materiału przy zatwierdzeniu zlecenia produkcyjnego i jego zużycie przy zakończeniu.

```yaml
# production_order: jeden materiał, rezerwowany przy zatwierdzeniu
movements:
  - code: reserve
    direction: in
    quantity: quantity
    target: {relationship: stock_item, blueprint: stock_item, balance: reserved,
             key: product, values: {product: material}}
    apply_on: [{attribute: status, to: approved}]
  - code: release
    direction: out
    quantity: quantity
    target: {relationship: stock_item, blueprint: stock_item, balance: reserved,
             key: product, values: {product: material}}
    apply_on: [{attribute: status, to: completed}, {attribute: status, to: cancelled}]
    requires: [reserve]
  - code: consume
    direction: out
    quantity: quantity
    target: {relationship: stock_item, blueprint: stock_item, balance: quantity,
             key: product, values: {product: material}}
    apply_on: [{attribute: status, to: completed}]
    requires: [reserve]
```

- `attribute` to atrybut statusu, a `to` (i opcjonalne `from`) to kody jego opcji. Przejście do `to` z `from` albo z dowolnej opcji księguje przesunięcie.
- Każde przesunięcie jest księgowane raz na rekord, przy pierwszym pasującym przejściu. Ponowne wejście w status niczego nie zmienia.
- Z `requires` przesunięcie jest księgowane dopiero po wskazanych przesunięciach. Anulowanie zlecenia w wersji roboczej niczego nie zwalnia, bo nic nie zostało zarezerwowane.
- Przesunięcia, które odnajdują ten sam rekord, mogą dzielić relację celu, jak `stock_item` powyżej.
- Przesunięcia są księgowane w transakcji zapisu, który zmienia status, w kolejności deklaracji, niezależnie od tego, co zmienia status: formularz, partia rekordów, przepływ pracy, agent czy rozszerzenie. Jeśli któreś się nie powiedzie, np. z `409 balance_below_minimum`, zapis się nie udaje, a status pozostaje bez zmian.

#### Pozycje zlecenia

Wyzwalacz z `relationship` księguje przesunięcie rekordu pozycji, gdy rekord, który ona wskazuje, przechodzi zmianę statusu. Zlecenie produkcyjne z kilkoma materiałami ma swój status, a każda pozycja materiałowa rezerwuje i zużywa własny materiał:

```yaml
# production_order_line
movements:
  - code: reserve
    # ...
    apply_on: [{relationship: production_order, attribute: status, to: approved}]
```

- Relacja ma `cardinality: one`, jeden schemat docelowy i `context_editable: default`. Publikacja sprawdza, czy status i jego opcje istnieją w tym schemacie.
- Jedna zmiana statusu księguje przesunięcia najwyżej 200 pozycji; więcej zwraca `422 movement_lines_exceeded`. Jeśli którakolwiek pozycja się nie powiedzie, cała zmiana statusu się nie udaje.
- Gdy pozycje zlecenia jednego schematu są zaksięgowane, nie można już dodać takiej pozycji do zlecenia, przenieść jej do innego ani jej usunąć: `409 movement_lines_locked` z `order_id`. Pozycje innego schematu są zamykane, gdy zaksięgują się ich własne przesunięcia, więc zatwierdzone zlecenie nadal przyjmuje pozycje wyrobów aż do zakończenia. Formularz rekordu zlecenia podaje je w `settled_lines` jako `{blueprint, relationship}`.

Aby pracować na pozycjach ze strony zlecenia, pokaż je w edytowalnej tabeli:

```yaml
# production_order: views.detail
- type: incoming_relationship_list
  label: Materials
  display: table
  editable: true
  columns: [item, quantity, conversion]
  page_size: 50
  relationships:
    - {source_blueprint: production_order_line, field: production_order}
```

Komórki tabeli edytują każdą pozycję na miejscu, a zmieniona komórka zapisuje się po jej opuszczeniu, tak jak pola na stronie rekordu. Komórki, których pozycja nie pozwala zmienić, pokazują jej wartość. **Dodaj** otwiera nowy wiersz na dole tabeli: wypełnij jego komórki i naciśnij Enter lub jego przycisk zapisu, a pozycja zostanie utworzona z już ustawionym zleceniem, a wiersz opróżni się na kolejną. Escape lub przycisk odrzucenia go zamyka. Niewypełnione wymagane wartości są oznaczane w komórkach. **Otwórz** pokazuje cały rekord pozycji obok zlecenia, a **Usuń** ją usuwa. Gdy pozycje są zaksięgowane, tabela pozwala je tylko wyświetlać i otwierać.

### Rezerwacje

Saldo z `reserved` odkłada część siebie na bok. `min` dotyczy salda pomniejszonego o saldo rezerwacji, po każdej zmianie któregokolwiek z nich:

```yaml
# stock_item
attributes:
  - code: quantity
    value_type: number
    balance: {min: 0, reserved: reserved}
  - code: reserved
    value_type: number
    balance: {min: 0}
```

- Wydanie nie może zabrać zarezerwowanego stanu, a rezerwacja nie może przekroczyć wolnego stanu.
- W `409 balance_below_minimum` pole `available` to wtedy wolna ilość, a `details.reserved` zawiera ilość zarezerwowaną.
- `reserved` wskazuje inne saldo tego samego schematu, które nie ma własnego `reserved`.

### Wycofania

Rekord, który ustawia `reversal.relationship`, wycofuje rekord, który wskazuje. Cofa każde przesunięcie zaksięgowane na oryginale, od najpóźniejszego, na celach oryginału. Przesunięcia niezaksięgowane są pomijane; rekordu bez zaksięgowanych przesunięć nie można wycofać (`409 movement_not_applied`). Wycofanie jest księgowane przy utworzeniu, a jego własny status nigdy nie księguje przesunięć ponownie.

- Wycofanie przejmuje ilość, powiązanie przelicznika i wartości klucza oryginału. Przesłane wartości muszą się z nimi zgadzać, inaczej zapis zwraca `422 movement_reversal_mismatch`; liczby są porównywane według wartości.
- Rekord można wycofać raz. Drugie wycofanie zwraca `409 movement_already_reversed` z `record_id` i istniejącym wycofaniem w `reversed_by`. Wycofania nie można wycofać: `409 movement_reversal_of_reversal`.
- Wycofanie podlega minimum salda. Wycofanie przyjęcia, którego towar został już wydany, zwraca `409 balance_below_minimum`; najpierw wycofaj wydania.

## Zadania konektorów

Zadania konektorów uruchamiają operację zainstalowanego rozszerzenia konektora, np. import lub eksport CSV, dla tego schematu.

```yaml
connector_jobs:
  - code: csv_export
    direction: export
    extension_id: attricat-connector-csv
    operation_id: export
    interval_seconds: 3600
    input:
      profile:
        version: 1
        columns:
          - {header: ID, attribute: external_id, kind: string}
```

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `code` | kod | Wymagany | Stały identyfikator. Zadania są dopasowywane między wersjami według kodu. |
| `direction` | `"import"` lub `"export"` | Wymagany | |
| `extension_id` | ciąg znaków | Wymagany | Identyfikator rozszerzenia konektora. |
| `operation_id` | ciąg znaków | Wymagany | Operacja zadeklarowana przez rozszerzenie. |
| `input` | mapowanie | Wymagany | Dane wejściowe operacji, weryfikowane względem schematu operacji. |
| `context` | kod kontekstu | | Wymagany dla importów: zaimportowane wartości są zapisywane w tym kontekście. Dla eksportów kanał publikacji do eksportu (musi nim być); bez niego zadanie uruchamia się raz dla każdego włączonego kanału. Zadanie rozszerzenia, które prowadzi mapowania eksportu, musi go ustawić, gdy włączonych jest kilka kanałów. |
| `input_file_id` | UUID | | Tylko importy. Gotowy plik obszaru roboczego do zaimportowania, np. wartość atrybutu plikowego. |
| `interval_seconds` | liczba całkowita | Tylko ręcznie | Uruchamia zadanie co N sekund, od 60 do 2592000. |
| `enabled` | wartość logiczna | `true` | Ustaw `false`, aby wstrzymać zadanie. |
| `connection` | ciąg znaków | Domyślne miejsce docelowe | Tylko eksporty. [Połączenie](/pl/builders/extensions/#połączenia) rozszerzenia, do którego zadanie wysyła dane: od 1 do 128 liter ASCII, cyfr, `.`, `_` lub `-`. |
| `trigger` | mapowanie | | Tylko eksporty. Uruchamia zadanie także wtedy, gdy rekordy się zmieniają: `{events: [...], debounce_seconds: N}`. |

`trigger.events` zawiera co najmniej jedno z: `record.created.v1`, `record.updated.v1`, `record.deleted.v1`, `record.migrated.v1`, `record.published.v1`, `record.unpublished.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1` i `relationship.changed.v1`. `trigger.debounce_seconds` ma wartość od 1 do 3600: wykonanie rusza, gdy od najnowszego pasującego zdarzenia rekordów schematu minie tyle sekund albo gdy najstarsza oczekująca zmiana ma dziesięciokrotność tego czasu, i obejmuje tylko zmienione rekordy. Zdarzenie rekordu, do którego rekordy schematu odwołują się przez relację, liczy się jako zmiana rekordów odwołujących się.

```yaml
connector_jobs:
  - code: shopify
    direction: export
    extension_id: acme.shopify
    operation_id: export
    connection: shop-eu
    trigger:
      events: [record.updated.v1, record.published.v1]
      debounce_seconds: 30
    input: {}
```

Zadania są sprawdzane względem włączonego rozszerzenia podczas publikacji wersji; nieprawidłowe zadanie blokuje publikację. Zadanie usunięte w późniejszej wersji zostaje wyłączone, a jego historia uruchomień jest zachowywana. Zadanie eksportu uruchamia się raz dla każdego włączonego kanału eksportu i eksportuje tylko rekordy opublikowane w tym kanale. Zobacz [Operacje i konektory](/pl/extensions/operations/).

## Limity i błędy

Kilka reguł kompilacji, które łatwo przeoczyć:

- `format_version` inny niż `1` jest odrzucany.
- Schemat rekordu bez `views.dropdown_option` jest odrzucany.
- `record_schema` w mixinie jest odrzucany.
- `record_schema` może wskazywać tylko atrybuty, które schemat posiada, w swoich kluczach najwyższego poziomu `required`, `properties`, `dependentRequired` i `dependentSchemas`.
- Predykaty w `x-attricat-checks` i warunkach przejść (`conditions`) muszą wskazywać atrybuty, które schemat posiada, o typach pasujących do predykatu. Porównanie porządkujące na łańcuchu znaków albo porównanie daty z liczbą jest odrzucane.
- `target_blueprint`, `target_blueprints`, `acyclic` i `tree` w atrybucie innym niż relacja są odrzucane.
- `unique_keys` w mixinie, klucz wskazujący nieznany atrybut, atrybut `json`, plikowy lub relację z wieloma celami, a także klucz wymieniający atrybut dwa razy są odrzucane.
- `from` musi mieć postać `alias.code`, gdzie `code` odpowiada kodowi samego atrybutu.
- `balance` w mixinie, w atrybucie wybranym przez `from`, w typie innym niż `number` lub `integer` albo razem z `generated` lub `default_value` jest odrzucane.
- Przesunięcie wskazujące nieznany atrybut, używające atrybutu w dwóch rolach, odczytujące atrybut bez `context_editable: default` albo deklarujące `reversal` bez `result` jest odrzucane. Publikacja odrzuca przesunięcie, którego schemat docelowy nie jest opublikowany albo nie ma salda lub klucza, oraz przelicznik, który nie jest liczbą.
- `generated` w mixinie, w atrybucie wybranym przez `from`, w `extension_type`, w statusie lub w przypisaniu użytkownika lub zespołu, w typie innym niż `string` lub `integer` albo razem z `default_value` jest odrzucany. Odrzucany jest też `format` bez dokładnie jednego `{n}`, z nieznanym symbolem zastępczym lub dłuższy niż 100 znaków, a także `reset`, którego okresu `format` nie identyfikuje.
