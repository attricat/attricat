---
title: Szybki start
description: Zbuduj mały katalog z dwoma powiązanymi schematami, kontekstem i opublikowanym produktem.
---

Ten przewodnik zajmuje około piętnastu minut. Utworzysz schemat kategorii i schemat produktu, dodasz produkt w dwóch językach i opublikujesz go w kanale.

Potrzebujesz obszaru roboczego, w którym masz rolę właściciela lub administratora. Jeśli samodzielnie konfigurujesz Attricat, najpierw zobacz [Wdrażanie Attricat](/pl/operate/deployment/).

## 1. Utwórz schemat kategorii

Otwórz **Zarządzanie → Schematy → Nowy schemat** i wklej:

```toml
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "category"
cardinality = "one"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
```

Zapisz go, a następnie **Opublikuj** wersję 1.

`parent` wskazuje na inną kategorię, więc kategorie mogą tworzyć drzewo.

## 2. Utwórz schemat produktu

Utwórz drugi schemat:

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"
entity_schema = '{"type":"object","required":["sku","title"]}'

[[attributes]]
code = "sku"
value_type = "string"
context_editable = "default"

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title", "sku"]
separator = " / "

[views.table]
type = "table"
columns = [
  { field = "sku", label = "SKU" },
  { field = "title" },
  { field = "price" },
]
```

Opublikuj go.

`entity_schema` celowo znajduje się nad pierwszym `[[attributes]]`. W TOML każdy klucz po nagłówku `[table]` należy do tej tabeli, więc klucz najwyższego poziomu umieszczony niżej stałby się częścią ostatniego atrybutu.

## 3. Dodaj kategorie

Otwórz **Przeglądarkę encji**, wybierz **Category** i dwukrotnie użyj **Utwórz encję**:

1. `name` = *Apparel*
2. `name` = *Shirts*, `parent` = *Apparel*

## 4. Dodaj produkt

Wybierz **Product** w Przeglądarce i utwórz jeden produkt:

- `sku` = *SH-0001*
- `title` = *Linen shirt*
- `price` = *49*
- `categories` = *Shirts*

Spróbuj zapisać produkt bez tytułu: schemat encji go odrzuci. Spróbuj ustawić cenę *-1*: schemat wartości ją odrzuci.

## 5. Dodaj kontekst

Otwórz **Zarządzanie → Konteksty → Utwórz kontekst**. Utwórz `PL` pod korzeniem, z metadanymi `{"language": "pl"}`.

Otwórz produkt, przełącz **Kontekst** na `PL` i wybierz **Edytuj encję**. Każde pole pokazuje *Dziedziczone z kontekstu Domyślny*. Ustaw `title` na *Lniana koszula* i zapisz. `sku` jest tu tylko do odczytu, ponieważ jest zarządzane w kontekście domyślnym.

Przełączaj selektor **Kontekst** w Przeglądarce między `default` a `PL`, aby zobaczyć zmianę tytułu.

## 6. Opublikuj w kanale

Otwórz **Zarządzanie → Eksporty** i włącz **Kanał eksportu** dla `PL`.

Po powrocie do produktu sekcja **Publikacja** pokazuje `PL` jako *Nieopublikowano*. Wybierz **Opublikuj**.

Teraz zmień cenę produktu i zapisz. Publikacja zostaje wycofana: ponownie widać *Nieopublikowano*, ponieważ zatwierdzony stan nie jest już bieżący. Użyj **Opublikuj ponownie**, aby zatwierdzić zmianę.

## 7. Wypróbuj Przeglądarkę

Gdy masz już kilka produktów i kategorii:

- wyszukaj `categories.name:shirts`;
- otwórz fasetę **categories** i wybierz *Apparel*, aby uwzględnić wszystko, co się pod nią znajduje;
- użyj **Dodaj filtr** dla `price` mniejszej niż 50;
- użyj **Zapisz wyszukiwanie**, aby zachować wynik.

## Kolejne kroki

- [Tworzenie schematu](/pl/builders/blueprints/) omawia pliki, układy, domieszki i nie tylko.
- [Modelowanie katalogu](/pl/builders/modeling/) pomaga zdecydować, co powinno być encją, atrybutem, a co kontekstem.
- [Administrowanie obszarem roboczym](/pl/operate/workspaces/) wyjaśnia, jak zaprosić zespół.
