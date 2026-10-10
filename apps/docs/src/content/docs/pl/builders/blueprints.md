---
title: Tworzenie schematu
description: Zbuduj schemat od pustego pliku do opublikowanej wersji z relacjami, widokami, walidacją i mixinem.
---

Schemat to dokument TOML opisujący jeden rodzaj rekordu katalogu: jego atrybuty, ich zachowanie w kontekstach, sposób walidacji i układ w aplikacji internetowej. Ten przewodnik krok po kroku buduje mały katalog produktów. Każdy użyty tu klucz jest opisany w [dokumentacji TOML schematu](/pl/reference/blueprint/).

## Gdzie pisać schematy

Schematy możesz pisać w dwóch miejscach:

- W aplikacji internetowej, w **Zarządzanie → Schematy → Nowy schemat**. Edytor waliduje treść podczas pisania i pokazuje podgląd powstałej strony rekordu, na którym możesz wypróbować edycję przykładowych wartości.
- W dowolnym edytorze tekstu, a następnie przesłać je przez CLI:

  ```sh
  acli blueprint create --file product.toml
  ```

Obie drogi zapisują TOML dokładnie w takiej postaci, w jakiej go napisano. Dobrze sprawdza się trzymanie plików schematów w systemie kontroli wersji obok kodu integracji; CLI wysyła je bez zmian.

Do tworzenia szkiców potrzebujesz uprawnienia `blueprints.write`, a do ich publikowania `blueprints.publish`.

## Krok 1: najmniejszy poprawny schemat

```toml
format_version = 1
code = "category"
name = "Category"
kind = "record"

[[attributes]]
code = "name"
value_type = "string"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
```

- `code` identyfikuje schemat przez cały czas jego istnienia. Wybierz go starannie; nie można go zmienić w kolejnych wersjach.
- `kind = "record"` oznacza, że można z niego tworzyć rekordy.
- Każdy schemat rekordu potrzebuje `views.dropdown_option`. Określa on, jak Attricat opisuje kategorię wszędzie tam, gdzie pojawia się ona na liście: w selektorach relacji, etykietach filtrów i wynikach wyszukiwania.

Zapisz to jako szkic. Szkic można dowolnie edytować, ale nie może jeszcze zawierać rekordów.

## Krok 2: opublikuj

Publikacja zamraża wersję. Od tej chwili nigdy się nie zmienia i można z niej tworzyć rekordy.

W aplikacji internetowej otwórz schemat i wybierz **Opublikuj**. Za pomocą CLI:

```sh
acli blueprint publish <blueprint-id> 1
```

Aby zmienić opublikowany schemat, utwórz nową wersję. Następna sekcja wyjaśnia, dlaczego to ważne.

## Krok 3: produkt z typowanymi atrybutami

```toml
format_version = 1
code = "product"
name = "Product"
kind = "record"

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "sku"
value_type = "string"
context_editable = "default"

[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'

[[attributes]]
code = "stock_on_hand"
value_type = "integer"
default_value = 0

[[attributes]]
code = "available"
value_type = "boolean"

[[attributes]]
code = "available_on"
value_type = "date"

[[attributes]]
code = "order_cutoff"
value_type = "time"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title", "sku"]
separator = " / "
```

Kilka decyzji w tym pliku:

- `sku` ma `context_editable = "default"`. SKU jest wszędzie takie samo, więc można je edytować tylko w kontekście domyślnym. W każdym innym kontekście jest wyświetlane jako tylko do odczytu.
- `price` ma `value_schema`, który odrzuca liczby ujemne. Schemat to JSON zapisany w łańcuchu znaków TOML. Zobacz [Walidacja](/pl/builders/validation/).
- `stock_on_hand` w nowych rekordach zaczyna od `0` dzięki `default_value`.
- `order_cutoff` jest typu `time`: to godzina zegarowa wraz ze strefą czasową IANA, np. 09:30 w `Europe/Warsaw`.

## Krok 4: relacje

Powiąż każdy produkt z jego kategoriami i jedną marką:

```toml
[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[[attributes]]
code = "brand"
value_type = "relationship"
target_blueprint = "brand"
cardinality = "one"
```

`target_blueprint` sprawia, że Attricat odrzuca powiązanie z czymkolwiek, co nie jest `category` (lub `brand`). `categories` pozwala na wiele celów; `brand` na jeden na produkt. Wiele produktów nadal może mieć tę samą markę. Jeśli cel musi należeć do dokładnie jednego źródła, ustaw także `target_cardinality = "one"`.

Schematy docelowe muszą istnieć, zanim opublikujesz schemat produktu.

Za pomocą relacji modeluje się w Attricat tagi, etykiety i taksonomie. [Modelowanie katalogu](/pl/builders/modeling/) wyjaśnia, dlaczego kategoria powinna być rekordem, a nie łańcuchem znaków.

## Krok 5: zachowanie w kontekstach

[Konteksty](/pl/guides/contexts/) pozwalają, by wartość różniła się zależnie od rynku, języka lub kanału. Każdy atrybut określa dwie rzeczy:

- `context_fallback`: co pokazuje kontekst bez własnej wartości. Domyślne `"default"` dziedziczy wartość z najbliższego kontekstu nadrzędnego, który ją ma. `"none"` nie pokazuje niczego.
- `context_editable`: gdzie można zapisać atrybut. Domyślne `"all"` pozwala na każdy kontekst. `"default"` pozwala tylko na kontekst główny.

```toml
[[attributes]]
code = "description"
value_type = "string"
# Inherit from the parent market when a channel has no description.
context_fallback = "default"

[[attributes]]
code = "promo_banner"
value_type = "string"
# A banner shown in one channel must not leak into its children.
context_fallback = "none"
```

## Krok 6: pliki

```toml
[[attributes]]
code = "main_photo"
value_type = "file"
allowed_mime_groups = ["image"]
image_only = true
max_bytes = 10485760

[[attributes]]
code = "manuals"
value_type = "file"
cardinality = "many"
allowed_extensions = ["pdf"]
```

Atrybut plikowy domyślnie przechowuje jeden plik; `cardinality = "many"` zamienia go w uporządkowaną listę. Zasady są sprawdzane przy każdym przesłaniu: sygnatura zawartości pliku, typ MIME, rozszerzenie i rozmiar. Dla obrazów w tle generowane są warianty WebP `thumbnail` i `display`.

## Krok 7: układ

Bez widoków aplikacja internetowa pokazuje atrybuty w kolejności deklaracji. Dodaj widoki, gdy chcesz mieć karty, siatki lub dopracowaną tabelę w **Przeglądarce rekordów**.

```toml
[views.detail]
type = "stack"

[[views.detail.children]]
type = "stack"
component = { id = "catalog.record_heading", version = 1 }

[[views.detail.children.children]]
type = "field"
field = "title"

[[views.detail.children.children]]
type = "field"
field = "sku"

[[views.detail.children]]
type = "tabs"

[[views.detail.children.tabs]]
label = "Overview"

[[views.detail.children.tabs.children]]
type = "grid"
children = [
  { type = "field", field = "price" },
  { type = "field", field = "stock_on_hand" },
]

[[views.detail.children.tabs.children]]
type = "relationship_list"
field = "categories"

[[views.detail.children.tabs]]
label = "Media"

[[views.detail.children.tabs.children]]
type = "field"
field = "main_photo"

[views.table]
type = "table"

[[views.table.columns]]
field = "main_photo"
label = "Image"
renderer = { id = "catalog.table_image", version = 1 }

[[views.table.columns]]
field = "title"

[[views.table.columns]]
field = "brand.name"
label = "Brand"

[[views.table.columns]]
field = "price"
```

Stos `catalog.record_heading` zamienia swoje pierwsze pole w tytuł strony, a pozostałe w podtytuł. Kolumna tabeli `brand.name` podąża za relacją `brand` i pokazuje `name` marki. Ponieważ `brand` ma `cardinality = "one"`, tę kolumnę można też sortować.

[Widoki i układy](/pl/builders/views/) opisują wszystkie bloki i komponenty.

## Krok 8: walidacja wielu pól

`value_schema` sprawdza jedną wartość. `record_schema` sprawdza cały rekord, więc może wyrażać reguły typu „produkt w promocji wymaga ceny promocyjnej”. Ten przykład zakłada, że schemat ma też atrybut logiczny `on_sale` i liczbowy `sale_price`:

```toml
record_schema = '''
{
  "type": "object",
  "required": ["title", "sku"],
  "if": { "properties": { "on_sale": { "const": true } }, "required": ["on_sale"] },
  "then": { "required": ["sale_price"] }
}
'''
```

Umieść `record_schema` razem z pozostałymi kluczami najwyższego poziomu, przed pierwszym `[[attributes]]`. W TOML klucz zapisany po nagłówku tabeli należy do tej tabeli.

Attricat sprawdza schemat w każdym kontekście po każdej zmianie. Zapis, który pozostawiłby którykolwiek kontekst w niepoprawnym stanie, zostaje odrzucony z `422 record_schema_mismatch` i nic nie jest zapisywane.

JSON Schema nie potrafi porównać dwóch atrybutów. Dla reguł w rodzaju „cena promocyjna musi być niższa od ceny” dodaj do tego samego schematu nazwaną kontrolę w `x-attricat-checks`:

```toml
record_schema = '''
{
  "type": "object",
  "required": ["title", "sku"],
  "x-attricat-checks": [
    { "code": "sale-below-price", "message": "The sale price must be lower than the price",
      "predicate": { "type": "compare", "attribute_code": "sale_price", "op": "lt", "other_attribute_code": "price" } }
  ]
}
'''
```

Kontrola, która nie przejdzie, powoduje odrzucenie zapisu z `422 record_check_failed`. Kontrole mogą też sprawdzać powiązane rekordy. Zobacz [Walidacja](/pl/builders/validation/#porównuj-atrybuty-za-pomocą-kontroli).

## Krok 9: udostępnij atrybuty w mixinie

Gdy kilka schematów potrzebuje tych samych pól, np. metadanych SEO, umieść je w mixinie:

```toml
format_version = 1
code = "seo"
name = "SEO fields"
kind = "mixin"

[[attributes]]
code = "meta_title"
value_type = "string"

[[attributes]]
code = "meta_description"
value_type = "string"
```

Opublikuj go, a następnie dodaj do `product` jako include i wybierz potrzebne atrybuty:

```toml
[[includes]]
alias = "seo"
code = "seo"
version = 1

[[attributes]]
code = "meta_title"
from = "seo.meta_title"

[[attributes]]
code = "meta_description"
from = "seo.meta_description"
```

Include przypina konkretną wersję mixinu. Opublikowanie wersji 2 `seo` nie zmienia `product`, dopóki nie opublikujesz nowej wersji `product`, której include wskazuje wersję 2.

## Krok 10: kontroluj cykl życia rekordu

[Status](/pl/builders/validation/#statusy) może nie tylko ograniczać dostępne przejścia. W dokumentach kontrolowanych, inspekcjach czy ocenach może też decydować, kto może wykonać przejście, zamrażać rekord, gdy jest już ostateczny, wiązać zatwierdzenie z dokładnie tą treścią, którą przejrzano, i zachowywać wydane pliki przez okres retencji:

```toml
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["draft", "review", "approved", "released"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "draft", "label": "Draft" },
      { "code": "review", "label": "In review" },
      { "code": "approved", "label": "Approved",
        "approval": { "covers": ["title", "procedure"], "void_to": "review" } },
      { "code": "released", "label": "Released", "tone": "success",
        "lock": "all", "retention_days": 3650 }
    ],
    "transitions": [
      { "from": null, "to": "draft" },
      { "from": "draft", "to": "review", "code": "submit" },
      { "from": "review", "to": "approved", "code": "approve",
        "roles": ["reviewer"], "separate_from": ["submit"] },
      { "from": "approved", "to": "released", "code": "release",
        "permission": "records.publish" },
      { "from": "released", "to": "draft", "code": "correct",
        "roles": ["owner", "admin"] }
    ]
  }
}'''
```

W tym przykładzie:

- Osoba, która przesłała dokument do przeglądu, nie może go też zatwierdzić, a zatwierdzać mogą tylko osoby z rolą `reviewer`. Wydanie wymaga `records.publish`.
- Edycja tytułu lub procedury zatwierdzonego dokumentu unieważnia zatwierdzenie i odsyła dokument z powrotem do przeglądu.
- Wydany dokument jest tylko do odczytu, nie można go usunąć, a jego pliki są zachowywane przez dziesięć lat. Właściciele i administratorzy poprawiają go przejściem `correct`, a potem edytują.

Działanie każdej z tych kontroli opisuje sekcja [Kontroluj cykl życia rekordu](/pl/builders/validation/#kontroluj-cykl-życia-rekordu), a wszystkie klucze sekcja [Statusy](/pl/reference/blueprint/#statusy).

## Wersje a istniejące rekordy

Każdy rekord pamięta dokładną wersję schematu, z którą został utworzony. Gdy opublikujesz wersję 2 schematu `product`, istniejące produkty pozostają przy wersji 1: ich wartości i walidacja zachowują znaczenie z chwili zapisu.

Attricat oznacza je wtedy jako nieaktualne i proponuje migrację. Wersje, które tylko dodają opcjonalne atrybuty, można migrować zbiorczo. Zmiany, które usuwają atrybuty lub zmieniają ich typ albo dodają atrybuty wymagane, wymagają decyzji dla każdego rekordu. Zobacz [Wersje i migracja](/pl/builders/revisions/).

Dwie praktyki oszczędzają później kłopotów:

- Nigdy nie używaj ponownie kodu atrybutu w innym znaczeniu w późniejszej wersji. Zamiast tego dodaj nowy kod.
- Dodawaj atrybuty zamiast je zmieniać. Wersje, które tylko dodają, migrują się bez niczyjej pomocy.

## Więcej możliwości

Schemat może też zawierać:

- [Reguły](/pl/builders/rules/), które oznaczają problemy z jakością danych, np. brak tytułu, i mogą je [egzekwować](/pl/builders/rules/#egzekwowanie-reguły) przy zapisie lub zmianie statusu.
- [Atrybuty statusu](/pl/builders/validation/#statusy) z dozwolonymi przejściami i [przetłumaczonymi etykietami](/pl/builders/translations/#etykiety-statusów).
- [Politykę publikacji](/pl/guides/publishing/#zachowaj-publikację-po-zaufanych-edycjach), która pozwala zaufanym rolom edytować bez cofania zatwierdzeń w kanałach.
- [Zadania konektorów](/pl/reference/blueprint/#zadania-konektorów), które importują lub eksportują rekordy przez rozszerzenie konektora.
- Atrybuty, których typ pochodzi z [rozszerzenia](/pl/reference/blueprint/#typy-atrybutów-z-rozszerzeń).
- [Układ paneli i akcji rozszerzeń](/pl/reference/blueprint/#extension_layout) na stronach rekordów.
