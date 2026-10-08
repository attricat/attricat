---
title: Podstawowe pojęcia
description: Idee stojące za modelem danych Attricat, wyjaśnione na jednym przykładzie.
---

Ta strona prowadzi jeden przykład, sprzedawcę odzieży działającego w Polsce i Niemczech, przez wszystkie pojęcia Attricat.

## Obszar roboczy

Obszar roboczy to jeden katalog. Ma własnych członków, role, schematy, rekordy, konteksty, rozszerzenia i dziennik audytu. Obszary robocze nie współdzielą niczego między sobą.

Użytkownicy logują się do obszaru roboczego za pomocą jego **identyfikatora logowania**, na przykład `retailer.example`, oraz swojego adresu e-mail i hasła.

## Schematy

Schemat opisuje jeden rodzaj rekordu. Sprzedawca ma schematy `product`, `category`, `brand` i `material`.

Schemat jest zapisany w TOML i zawiera listę **atrybutów**, z których każdy ma typ:

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
```

Schemat może też definiować schematy walidacji, układy stron, reguły jakości danych i zadania konektorów.

**Mixiny** to schematy, które istnieją tylko po to, by inne schematy pobierały z nich atrybuty przez include. Sprzedawca trzyma pola SEO w mixinie `seo`, używanej zarówno przez `product`, jak i `category`.

## Wersje

Schematy zmieniają się z czasem. Każda zmiana to nowa **wersja**: 1, 2, 3. Wersja zaczyna się jako szkic i po opublikowaniu staje się niezmienna.

Gdy sprzedawca dodaje `care_instructions` w wersji 2, produkty utworzone w wersji 1 zachowują wersję 1. Zostają oznaczone jako nieaktualne i można je **zmigrować**, gdy ktoś będzie gotowy. Stare wartości nigdy nie są interpretowane na nowo według nowych reguł.

## Rekordy

Rekord to jeden element katalogu: jeden produkt, jedna kategoria. Jest tworzony z bieżącej wersji schematu i pozostaje do niej przypięty aż do migracji.

W plikach TOML schematów, API i CLI rekordy występują pod nazwą `entity`, na przykład `kind = "entity"` i `entity_schema`.

Oprócz atrybutów każdy rekord ma **tagi systemowe** i **metadane systemowe** na potrzeby automatyzacji, na przykład tag `needs-review` ustawiany przez przepływ pracy.

## Konteksty

Kontekst to miejsce, w którym wartości mogą się różnić. Konteksty sprzedawcy to:

```text
default
├── PL
│   ├── PL-web
│   └── PL-marketplace
└── DE
    └── DE-web
```

Atrybut `title` produktu jest zapisany w `default` po angielsku i nadpisany w `PL` po polsku, a w `DE` po niemiecku. `PL-web` nie ma własnego tytułu, więc **dziedziczy** polski tytuł z `PL`.

Każdy atrybut decyduje, czy dziedziczy wartości (`context_fallback`) i czy można go nadpisać poza `default` (`context_editable`). SKU jest ustawiane tylko w `default`, a baner promocyjny w `PL-web` nie przenika do innych kanałów.

## Relacje

Atrybut relacji łączy rekord z innymi. `product.categories` wskazuje na rekordy `category`, a `category.parent` łączy kategorię z kategorią nadrzędną, tworząc drzewo.

Relacje mogą być jednokrotnego wyboru (`cardinality = "one"`) lub wielokrotnego wyboru i, jak każda inna wartość, mogą różnić się zależnie od kontekstu.

Klasyfikacje takie jak kategorie, marki i materiały są rekordami, a nie łańcuchami znaków. Dzięki temu każda z nich ma tożsamość, nazwę, którą można przetłumaczyć, i miejsce w hierarchii.

## Publikacja

Sprzedawca włącza `PL-web`, `PL-marketplace` i `DE-web` jako **kanały publikacji**. Specjalista ds. asortymentu przegląda produkt i **publikuje** go w `PL-web`. Eksport CSV dla `PL-web` zawiera tylko opublikowane produkty.

Jeśli ktokolwiek później edytuje produkt, publikacja zostaje wycofana, dopóki produkt nie zostanie ponownie przejrzany i opublikowany.

## Walidacja

Każdy zapis jest walidowany na serwerze:

- typ atrybutu (liczba musi być liczbą);
- `value_schema` atrybutu (cena nie może być ujemna);
- `entity_schema` schematu (produkt w promocji musi mieć cenę promocyjną).

Walidacja obejmuje każdy kontekst, którego dotyczy zmiana. Zmiana, która naruszyłaby którykolwiek kontekst, jest odrzucana w całości.

## Historia i audyt

Każda zmiana jest zapisywana wraz z informacją, kto ją wprowadził, kiedy i przez co: aplikację webową, token API, przepływ pracy, rozszerzenie lub agenta. Poprzednie wartości są przechowywane, domyślnie przez 90 dni, i można je przywrócić.

## Automatyzacja

- **Reguły** sprawdzają rekordy i zapisują ustalenia, na przykład „produkt nie ma tytułu”. Nigdy nie zmieniają danych.
- **Przepływy pracy** reagują na zdarzenia, tagując rekord lub zapisując wartość.
- **Rozszerzenia** uruchamiają kod w piaskownicy, aby integrować inne systemy, dodawać elementy interfejsu, obliczać wartości oraz importować lub eksportować dane.
- **Agenci** odpowiadają na pytania o katalog i proponują zmiany, z których każda czeka na zatwierdzenie przez człowieka.

## Dostęp

Użytkownicy otrzymują **role** (`owner`, `admin`, `editor`, `viewer` lub role niestandardowe). Rolę można przyznać dla całego obszaru roboczego albo ograniczyć do jednego schematu, jednego rekordu lub jednej gałęzi drzewa kontekstów. Niemiecki zespół sprzedawcy ma rolę edytora tylko w poddrzewie `DE`.
