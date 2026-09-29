---
title: Widoki i układy
description: Steruj układem stron encji, formularzy i tabel Przeglądarki encji za pomocą deklaratywnych bloków i zarejestrowanych komponentów.
---

Widoki są częścią wersji schematu. Opisują wyłącznie układ: które atrybuty się pojawiają, w jakiej kolejności i jak są pogrupowane. Nie mogą uruchamiać kodu. Aplikacja internetowa renderuje znane sobie bloki i zarejestrowane w niej komponenty.

Widoki są opcjonalne. Bez nich aplikacja internetowa pokazuje atrybuty w kolejności deklaracji, z uwzględnieniem [tagów widoczności](/pl/reference/blueprint/#tagi-widoczności).

## Widoki, które może definiować schemat

| Widok | Gdzie się pojawia |
| --- | --- |
| `dropdown_option` | Etykieta encji w selektorach, etykietach filtrów i wyszukiwaniu. Wymagany w schematach encji. |
| `detail` | Strona encji i jej podgląd. |
| `edit` | Formularze tworzenia i edycji. |
| `table` | Kolumny wyników w **Przeglądarce encji**. |
| `extension_layout` | Kolejność i widoczność paneli i akcji rozszerzeń na stronach encji tego schematu. |

## Budowanie układu szczegółów lub edycji

Układ jest drzewem. Kontenery zawierają inne bloki; liście pokazują pole lub treść statyczną.

| Kontenery | Liście |
| --- | --- |
| `stack` (pionowo), `grid` (responsywne kolumny), `section`, `tabs`, `accordion` | `field`, `relationship_list`, `incoming_relationship_list`, `heading`, `text`, `divider` |

TOML pozwala zapisać to samo drzewo na dwa sposoby. Zagnieżdżone tablice tabel są rozwlekłe, ale łatwo porównywać ich zmiany:

```toml
[views.edit]
type = "stack"

[[views.edit.children]]
type = "field"
field = "title"

[[views.edit.children]]
type = "grid"

[[views.edit.children.children]]
type = "field"
field = "price"

[[views.edit.children.children]]
type = "field"
field = "stock_on_hand"
```

Tabele wbudowane są zwięzłe:

```toml
[views.edit]
type = "stack"
children = [
  { type = "field", field = "title" },
  { type = "grid", children = [
    { type = "field", field = "price" },
    { type = "field", field = "stock_on_hand" },
  ] },
]
```

Karty i akordeony przyjmują listę grup z etykietami:

```toml
[views.detail]
type = "tabs"
tabs = [
  { label = "Overview", children = [{ type = "field", field = "title" }] },
  { label = "Logistics", children = [
    { type = "accordion", sections = [
      { label = "Stock", children = [{ type = "field", field = "stock_on_hand" }] },
    ] },
  ] },
]
```

`field` przyjmuje atrybuty skalarne i plikowe. `relationship_list` przyjmuje atrybuty relacji. Odwołanie do atrybutu, którego schemat nie ma, lub do atrybutu niewłaściwego rodzaju kończy się błędem walidacji.

## Nagłówek encji

Aby nadać stronie szczegółów właściwy tytuł, umieść pola w `stack` z komponentem `catalog.entity_heading`. Jego pierwszy element podrzędny musi być polem skalarnym i staje się nagłówkiem strony. Kolejne elementy, tekst lub pola skalarne, tworzą podtytuł.

```toml
[[views.detail.children]]
type = "stack"
component = { id = "catalog.entity_heading", version = 1 }
children = [
  { type = "field", field = "title" },
  { type = "field", field = "sku" },
]
```

Jeśli pole tytułu jest puste, wyświetlany jest identyfikator encji. Nagłówek jest usuwany z treści strony, aby nie pojawiał się dwa razy.

## Pokazywanie powiązań przychodzących

`incoming_relationship_list` pokazuje encje wskazujące na bieżącą, np. produkty w kategorii. To przycisk otwierający stronicowaną listę, więc nic nie jest wczytywane, dopóki ktoś o to nie poprosi.

```toml
{ type = "incoming_relationship_list",
  label = "Products in this category",
  page_size = 10,
  relationships = [{ source_blueprint = "product", field = "categories" }] }
```

Wymień kilka pozycji w `relationships`, aby połączyć źródła. Encja pasująca do więcej niż jednego źródła pojawia się raz.

## Hierarchie

`catalog.relationship_hierarchy` pokazuje łańcuchy przodków jako ścieżkę nawigacyjną, rozstrzygniętą w wybranym kontekście.

W relacji odwołującej się do własnego schematu, takiej jak `category.parent`, pokazuje przodków bieżącej encji:

```toml
{ type = "relationship_list", field = "parent",
  component = { id = "catalog.relationship_hierarchy", version = 1 } }
```

W relacji do innego schematu ustaw `parent_field` na pole celu odwołujące się do jego własnego schematu. Na produkcie pokazuje to pełną ścieżkę każdej powiązanej kategorii, np. *Apparel › Shirts › Linen*:

```toml
{ type = "relationship_list", field = "categories",
  component = { id = "catalog.relationship_hierarchy", version = 1, props = { parent_field = "parent" } } }
```

## Tabela Przeglądarki encji

```toml
[views.table]
type = "table"

[[views.table.columns]]
field = "title"
label = "Product"

[[views.table.columns]]
field = "family.product_type.name"
label = "Product type"
```

Kolumna to lokalny atrybut skalarny albo ścieżka przez maksymalnie trzy relacje, zakończona atrybutem skalarnym. Każdy krok jest rozstrzygany według wersji schematu powiązanej encji. Jeśli starsza powiązana encja nie ma danego atrybutu, komórka jest pusta. Ścieżka o wielu wartościach może pokazać kilka wartości w jednej komórce.

Kolumny można sortować, gdy prowadzą do wartości skalarnej, a każda relacja na ścieżce ma `cardinality = "one"`.

### Miniatury obrazów

`catalog.table_image` pokazuje miniaturę dla atrybutu z jednym obrazem. Atrybut musi mieć `value_type = "file"`, `cardinality = "one"` (domyślnie) i `image_only = true`.

```toml
[[views.table.columns]]
field = "main_photo"
label = "Image"
renderer = { id = "catalog.table_image", version = 1 }
```

### Renderery komórek z rozszerzeń

Włączone rozszerzenie może dostarczać renderery komórek, np. formatowanie walut. Odwołaj się do niego przez identyfikator i wersję, a jego opcje przekaż w `props`:

```toml
[[views.table.columns]]
field = "price"
renderer = { id = "example.currency", version = 1, props = { currency = "USD" } }
```

Renderer musi być zadeklarowany przez włączone rozszerzenie dla typu wartości kolumny. Działa w odizolowanej ramce; zobacz [Kontrybucje klienckie](/pl/extensions/client/).

## Komponenty

Każdy blok może wskazać komponent przez `component = { id, version, props }`. Identyfikator i wersja muszą odpowiadać zarejestrowanemu komponentowi, komponent musi obsługiwać dany blok i typ wartości, a `props` może zawierać tylko zadeklarowane przez niego opcje. Wbudowane komponenty są wymienione w [dokumentacji schematu](/pl/reference/blueprint/#odwołania-do-komponentów).

## Panele rozszerzeń na stronach encji

Administratorzy obszaru roboczego ustawiają domyślną kolejność elementów rozszerzeń dla całego obszaru roboczego. Schemat może nadpisać trzy miejsca osadzenia dla własnych encji:

```toml
[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.entity_preview_panel]
order = ["acme.inventory:summary", "acme.pricing:margin"]
hidden = ["acme.legacy:panel"]
```

Miejsca osadzenia, które można tu ustawić, to `entity_preview_panel`, `entity_attribute_decoration` i `entity_action`. Układ nigdy nie nadaje uprawnień ani nie włącza rozszerzenia. Wpisy dla brakujących lub wyłączonych rozszerzeń są zachowywane i ignorowane, więc układ przetrwa wyłączenie i ponowne włączenie rozszerzenia.
