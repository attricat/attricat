---
title: Walidacja
description: Ograniczaj pojedyncze wartości i całe encje za pomocą JSON Schema i dowiedz się, jak walidacja współdziała z kontekstami.
---

Attricat waliduje każdy zapis na serwerze, zanim cokolwiek zostanie zapisane. Walidacja pochodzi z trzech źródeł:

1. **Typ atrybutu.** Atrybut `number` odrzuca `"abc"`; `date` odrzuca `2026-13-01`.
2. **`value_schema`** w atrybucie: JSON Schema dla jednej wartości.
3. **`entity_schema`** w schemacie: JSON Schema dla całej encji.

Oba schematy walidacji używają JSON Schema Draft 2020-12. Aplikacja internetowa korzysta z tych samych schematów, aby ostrzegać Cię podczas pisania, ale liczy się odpowiedź serwera.

## Ogranicz jedną wartość

Zapisz schemat jako JSON w łańcuchu znaków TOML. Apostrofy pozwalają uniknąć escapowania.

```toml
[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'

[[attributes]]
code = "sku"
value_type = "string"
value_schema = '{"type":"string","pattern":"^[A-Z]{3}-[0-9]{4}$"}'

[[attributes]]
code = "size"
value_type = "string"
value_schema = '{"enum":["XS","S","M","L","XL"]}'
```

`value_schema` działa na atrybutach skalarnych: łańcuchach znaków, liczbach, liczbach całkowitych, wartościach logicznych, datach, datach z godziną i godzinach. Relacje i pliki nie mogą go mieć. Relacje ograniczaj przez `entity_schema`.

Wartość, która nie przejdzie walidacji, zwraca `422 attribute_value_schema_mismatch`.

## Statusy

Status to atrybut `string`, którego `value_schema` ma `enum` ze stałymi kodami oraz adnotację `x-attricat-status`. Adnotacja nadaje każdemu kodowi etykietę, opcjonalny odcień koloru i opcjonalnie dozwolone przejścia między kodami:

```toml
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["draft", "live", "retired"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "draft", "label": "Draft" },
      { "code": "live", "label": "Live", "tone": "success" },
      { "code": "retired", "label": "Retired" }
    ],
    "transitions": [
      { "from": null, "to": "draft" },
      { "from": "draft", "to": "live" },
      { "from": "live", "to": "retired" }
    ]
  }
}'''
```

- Każdy kod z `enum` musi mieć dokładnie jedną opcję, podaną w kolejności wyświetlania. Kody składają się z liter, cyfr, `_` i `-`; etykiety to zwykły tekst.
- `tone` przyjmuje wartość `default`, `success`, `warning`, `error` lub `info`. Etykieta jest zawsze widoczna, więc kolor nigdy nie jest jedyną informacją.
- Pomiń `transitions`, aby zezwolić na każdą zmianę. Z `transitions` dozwolone są tylko wymienione zmiany; pusta tablica nie zezwala na żadną. `null` oznacza „brak wartości”: przejście z `null` pozwala ustawić pierwszą wartość (także domyślną), a przejście do `null` pozwala ją wyczyścić. Pozostawienie tej samej wartości jest zawsze dozwolone.

Aplikacja internetowa pokazuje status jako etykietę i edytuje go listą wyboru, w której niedozwolone opcje są wyłączone. Przejścia sprawdza serwer dla każdego zapisu, także z API, CLI, przepływów pracy, przywracania historii i migracji. Porównywane są wartości efektywne, więc wartość odziedziczona z kontekstu nadrzędnego jest punktem wyjścia. Niedozwolona zmiana zwraca `422 attribute_value_schema_mismatch`.

Status może też ograniczać, kto może wykonać poszczególne przejścia, blokować sfinalizowane rekordy i wiązać zatwierdzenia z przejrzaną treścią. Zobacz [Kontroluj cykl życia rekordu](/pl/builders/blueprints/#krok-10-kontroluj-cykl-życia-rekordu).

## Ogranicz całą encję

`entity_schema` widzi encję jako jeden obiekt JSON. Używaj go dla reguł obejmujących więcej niż jeden atrybut:

```toml
entity_schema = '''
{
  "type": "object",
  "required": ["title", "price"],
  "allOf": [
    {
      "if": { "properties": { "on_sale": { "const": true } }, "required": ["on_sale"] },
      "then": { "required": ["sale_price"] }
    }
  ],
  "dependentRequired": { "discontinued_on": ["replacement"] }
}
'''
```

Obiekt walidowany przez Attricat wygląda tak:

```json
{
  "title": "Linen shirt",
  "price": 49.0,
  "on_sale": true,
  "sale_price": 39.0,
  "categories": ["e8b7a8d3-c954-4c0f-b658-0f686ba466a3"]
}
```

- Wartości skalarne pojawiają się w postaci JSON.
- Relacje są tablicami UUID encji docelowych. Użyj `minItems` i `maxItems`, aby wymagać co najmniej jednej kategorii lub najwyżej trzech tagów.
- Atrybuty bez wartości są pomijane, więc `required` oznacza „ma wartość”.

Najwyższego poziomu `required`, `properties`, `dependentRequired` i `dependentSchemas` mogą wskazywać tylko atrybuty, które ma schemat, w tym atrybuty wybrane z domieszek. Literówka w tych miejscach powoduje błąd kompilacji.

`entity_schema` jest dozwolony tylko w schematach encji. Encja, która nie przejdzie walidacji, zwraca `422 entity_schema_mismatch`.

## Walidacja a konteksty

Encja ma w każdym kontekście rozstrzygnięty zestaw wartości, zbudowany z jej własnych wartości i wartości odziedziczonych. Po każdej zmianie Attricat waliduje encję w **każdym** kontekście, nie tylko w edytowanym.

Załóżmy na przykład, że `title` jest wymagany i dziedziczy wartość z kontekstu domyślnego. Wyczyszczenie domyślnego tytułu pozostawiłoby każdy kontekst potomny bez tytułu, więc zapis zostaje odrzucony, mimo że edytowano kontekst domyślny.

Przeniesienie kontekstu pod nowego rodzica jest sprawdzane tak samo: przed zapisaniem przeniesienia każda aktywna encja jest walidowana względem nowego łańcucha dziedziczenia.

## Czego nie waliduje się

- Tagi widoczności i `readonly` są wskazówkami prezentacyjnymi. Nie blokują zapisu wartości przez API.
- `system_tags` i `system_metadata` encji znajdują się poza schematem i nie są sprawdzane.
- Schemat walidacji z nowszej wersji schematu nie dotyczy encji pozostających przy starszej wersji, dopóki nie zostaną zmigrowane.
