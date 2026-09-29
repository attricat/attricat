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
