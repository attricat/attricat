---
title: Widoki i układy
description: Steruj układem stron rekordów, formularzy i tabel Przeglądarki rekordów za pomocą deklaratywnych bloków i zarejestrowanych komponentów.
---

Widoki są częścią wersji schematu. Opisują wyłącznie układ: które atrybuty się pojawiają, w jakiej kolejności i jak są pogrupowane. Nie mogą uruchamiać kodu. Aplikacja internetowa renderuje znane sobie bloki i zarejestrowane w niej komponenty.

Widoki są opcjonalne. Bez nich aplikacja internetowa pokazuje atrybuty w kolejności deklaracji, z uwzględnieniem [tagów widoczności](/pl/reference/blueprint/#tagi-widoczności).

## Widoki, które może definiować schemat

| Widok | Gdzie się pojawia |
| --- | --- |
| `dropdown_option` | Etykieta rekordu w selektorach, etykietach filtrów i wyszukiwaniu. Wymagany w schematach rekordów. |
| `page_title` | Tytuł karty przeglądarki na stronie rekordu. Gdy go brak, używana jest etykieta `dropdown_option`. |
| `detail` | Strona rekordu, jej podgląd i formularz tworzenia. W tym układzie pola są zarówno wyświetlane, jak i edytowane. |
| `table` | Kolumny wyników w **Przeglądarce rekordów**. |
| `explorer` | Ustawienia domyślne **Przeglądarki rekordów**, np. atrybut wyświetlany w panelu **Przeglądaj według**. |
| `extension_layout` | Kolejność i widoczność paneli i akcji rozszerzeń na stronach rekordów tego schematu. |

Nie ma osobnego widoku edycji: rekordy edytuje się na miejscu w układzie szczegółów. Schemat, który definiuje `views.edit`, zostaje odrzucony.

## Budowanie układu szczegółów

Układ jest drzewem. Kontenery zawierają inne bloki; liście pokazują pole lub treść statyczną.

| Kontenery | Liście |
| --- | --- |
| `stack` (pionowo), `grid` (responsywne kolumny), `section`, `tabs`, `accordion` | `field`, `relationship_list`, `incoming_relationship_list`, `heading`, `text`, `divider` |

YAML pozwala zapisać to samo drzewo na dwa sposoby. Styl blokowy, z jednym kluczem w wierszu, jest rozwlekły, ale łatwo porównywać jego zmiany:

```yaml
views:
  detail:
    type: stack
    children:
      - type: field
        field: title
      - type: grid
        children:
          - type: field
            field: price
          - type: field
            field: stock_on_hand
```

Styl przepływowy, z `{ }` i `[ ]`, jest zwięzły:

```yaml
views:
  detail:
    type: stack
    children:
      - {type: field, field: title}
      - {type: grid, children: [{type: field, field: price}, {type: field, field: stock_on_hand}]}
```

Karty i akordeony przyjmują listę grup z etykietami:

```yaml
views:
  detail:
    type: tabs
    tabs:
      - label: Overview
        children:
          - {type: field, field: title}
      - label: Logistics
        children:
          - type: accordion
            sections:
              - label: Stock
                children:
                  - {type: field, field: stock_on_hand}
```

`field` przyjmuje atrybuty skalarne i plikowe. `relationship_list` przyjmuje atrybuty relacji. Odwołanie do atrybutu, którego schemat nie ma, lub do atrybutu niewłaściwego rodzaju kończy się błędem walidacji.

Układ szczegółów steruje zarówno wyświetlaniem, jak i edycją. Na stronie rekordu każde pole, które użytkownik może zmienić, jest edytowalną kontrolką w swoim miejscu układu; pola tylko do odczytu, pola zablokowane przez status rekordu i pola zarządzane w kontekście domyślnym pokazują samą wartość. Edytowalne atrybuty pominięte w układzie pojawiają się pod nim w sekcji **Pozostałe atrybuty**, więc wymagany atrybut jest zawsze dostępny. Formularz tworzenia korzysta z tego samego układu.

## Nagłówek rekordu

Aby nadać stronie szczegółów właściwy tytuł, umieść pola w `stack` z komponentem `attricat.record_heading`. Jego pierwszy element podrzędny musi być polem skalarnym i staje się nagłówkiem strony. Kolejne elementy, tekst lub pola skalarne, tworzą podtytuł.

```yaml
views:
  detail:
    children:
      - type: stack
        component: {id: attricat.record_heading, version: 1}
        children:
          - {type: field, field: title}
          - {type: field, field: sku}
```

Jeśli pole tytułu jest puste, wyświetlany jest identyfikator rekordu. Nagłówek jest usuwany z treści strony, aby nie pojawiał się dwa razy. Nagłówek tylko wyświetla wartości, dlatego jego edytowalne pola są dostępne do edycji jako pierwsze, nad resztą układu.

## Pokazywanie powiązań przychodzących

`incoming_relationship_list` pokazuje rekordy wskazujące na bieżący, np. produkty w kategorii. To link otwierający Przeglądarkę rekordów w nowej karcie, przefiltrowaną do tych rekordów. Przeglądarka przeszukuje jeden schemat naraz, więc lista z kilkoma `relationships` pokazuje osobny link dla każdego wpisu, nazwany schematem źródłowym.

```yaml
- type: incoming_relationship_list
  label: Products in this category
  page_size: 10
  relationships:
    - {source_blueprint: product, field: categories}
```

Wymień kilka pozycji w `relationships`, aby połączyć źródła. Rekord pasujący do więcej niż jednego źródła pojawia się raz.

## Hierarchie

`attricat.relationship_hierarchy` pokazuje łańcuchy przodków jako ścieżkę nawigacyjną, rozstrzygniętą w wybranym kontekście.

W relacji odwołującej się do własnego schematu, takiej jak `category.parent`, pokazuje przodków bieżącego rekordu:

```yaml
- type: relationship_list
  field: parent
  component: {id: attricat.relationship_hierarchy, version: 1}
```

W relacji do innego schematu ustaw `parent_field` na pole celu odwołujące się do jego własnego schematu. Na produkcie pokazuje to pełną ścieżkę każdej powiązanej kategorii, np. *Apparel › Shirts › Linen*:

```yaml
- type: relationship_list
  field: categories
  component:
    id: attricat.relationship_hierarchy
    version: 1
    props: {parent_field: parent}
```

## Tabela Przeglądarki rekordów

```yaml
views:
  table:
    type: table
    columns:
      - {field: title, label: Product}
      - {field: family.product_type.name, label: Product type}
```

Kolumna to lokalny atrybut skalarny albo ścieżka przez maksymalnie trzy relacje, zakończona atrybutem skalarnym. Każdy krok jest rozstrzygany według wersji schematu powiązanego rekordu. Jeśli starszy powiązany rekord nie ma danego atrybutu, komórka jest pusta. Ścieżka o wielu wartościach może pokazać kilka wartości w jednej komórce.

Kolumny można sortować, gdy prowadzą do wartości skalarnej, a każda relacja na ścieżce ma `cardinality: one`.

### Miniatury obrazów

`attricat.table_image` pokazuje miniaturę dla atrybutu z jednym obrazem. Atrybut musi mieć `value_type: file`, `cardinality: one` (domyślnie) i `image_only: true`.

```yaml
views:
  table:
    columns:
      - field: main_photo
        label: Image
        renderer: {id: attricat.table_image, version: 1}
```

### Renderery komórek z rozszerzeń

Włączone rozszerzenie może deklarować komórki tabeli, np. kwotę ze stałą liczbą miejsc po przecinku i jednostką. Odwołaj się do komórki przez identyfikator i wersję, a jej opcje przekaż w `props`:

```yaml
views:
  table:
    columns:
      - field: price
        renderer:
          id: example.amount
          version: 1
          props: {precision: 2, unit: USD}
```

Komórka musi być zadeklarowana przez włączone rozszerzenie dla typu wartości kolumny, a każda właściwość musi pasować do zadeklarowanego typu i granic. Attricat rysuje komórkę sam; zobacz [Komórki tabeli](/pl/extensions/manifest/#komórki-tabeli).

### Edytory atrybutów z rozszerzeń

Na stronie rekordu pole w widoku `detail` może zamiast domyślnej kontrolki używać kontrolki z włączonego rozszerzenia, np. pola formuły podpowiadającego nazwy atrybutów. Wskaż edytor przez identyfikator i wersję, a jego opcje przekaż w `props`:

```yaml
views:
  detail:
    type: stack
    children:
      - type: field
        field: discount_formula
        editor: {id: formula, version: 1, props: {rows: 2}}
```

Edytor musi być zadeklarowany przez włączone rozszerzenie dla typu wartości pola, a każda właściwość musi pasować do zadeklarowanego typu i granic. Pole zachowuje swoją etykietę, pomoc, walidację i zapisywanie: nadal zapisuje się po opuszczeniu pola lub naciśnięciu Enter, a Escape przywraca zapisaną wartość. Gdy rozszerzenie jest wyłączone albo nie działa, pole pokazuje domyślną kontrolkę. Edytory działają tylko na stronie rekordu, nigdy w tabelach Przeglądarki rekordów.

## Panel przeglądania w Przeglądarce rekordów

Panel **Przeglądaj według** obok wyników **Przeglądarki rekordów** wyświetla cele jednej relacji. Schemat z własną [hierarchią](#hierarchie) wyświetla zamiast tego własne drzewo, pod nazwą schematu. Domyślnie panel otwiera się dla tego drzewa, a w przeciwnym razie dla pierwszej relacji, której schemat docelowy ma hierarchię. Aby wybrać atrybut, wskaż go w `views.explorer`:

```yaml
views:
  explorer:
    type: explorer
    browse_by: brand
```

Atrybut musi być relacją z jednym schematem docelowym. Jeśli schemat docelowy nie ma hierarchii, cele są wyświetlane jako płaska lista. Ustaw `browse_by: false`, aby panel pozostawał zamknięty, dopóki ktoś go nie otworzy. Użytkownicy nadal mogą wybrać inny atrybut lub ukryć panel; ich wybór ma pierwszeństwo w ich przeglądarce.

Aby przeglądać według schematu oddalonego o kilka relacji, wskaż ścieżkę złożoną z maksymalnie trzech relacji rozdzielonych kropkami. W przypadku pojazdu powiązanego z modelem, który jest powiązany z marką, panel wyświetli marki i policzy pojazdy każdej z nich:

```yaml
views:
  explorer:
    type: explorer
    browse_by: model.brand
```

Każda relacja na ścieżce musi mieć jeden schemat docelowy i `cardinality: one`. Jeśli ostatni schemat docelowy ma hierarchię, panel wyświetla jego drzewo. Lista **Przeglądaj według** w panelu również proponuje takie ścieżki, po relacjach bezpośrednich, z etykietami w rodzaju „Model → Marka”.

## Kontrolki pól

Atrybut `string` jest domyślnie zwykłym tekstem. Aby pokazać go jako kolor, adres e-mail, URL, numer telefonu lub Markdown, wskaż komponent wyświetlania w jego polu szczegółów (lub kolumnie tabeli). Tam, gdzie pole jest edytowalne, aplikacja internetowa używa sparowanego komponentu edycji, na przykład `attricat.url_edit` dla `attricat.url_display`:

```yaml
views:
  detail:
    type: stack
    children:
      - type: field
        field: website
        component: {id: attricat.url_display, version: 1}
      - type: field
        field: description
        component: {id: attricat.markdown_display, version: 1}
  table:
    columns:
      - field: website
        renderer: {id: attricat.url_display, version: 1}
```

| Kontrolka | Komponenty | Działanie |
| --- | --- | --- |
| Kolor | `attricat.color_display`, `attricat.color_edit` | Sześciocyfrowy kod szesnastkowy (`#RRGGBB`), wybierany w próbniku kolorów; przycisk edycji obok otwiera pole do wpisania dokładnej wartości. Wyświetlany jako próbka obok tekstu, który po kliknięciu kopiuje wartość. |
| E-mail | `attricat.email_display`, `attricat.email_edit` | Jeden zwykły adres ASCII, np. `name+tag@example.com`. Wyświetlany jako link `mailto:`. |
| URL | `attricat.url_display`, `attricat.url_edit` | Tylko bezwzględne adresy `http://` lub `https://`. Linki otwierają się w nowej karcie. |
| Telefon | `attricat.phone_display`, `attricat.phone_edit` | Zapisywany tak, jak go wpisano. Numery zaczynające się od `+` i numeru kierunkowego kraju stają się linkami `tel:`; numer wewnętrzny można poprzedzić `ext.` lub `x`. |
| Markdown | `attricat.markdown_display`, `attricat.markdown_edit` | CommonMark z kartami **Edytuj** i **Podgląd**. Na stronie rekordu ustawiona wartość jest pokazywana sformatowana, dopóki nie zdecydujesz się jej edytować. Surowy HTML jest pomijany, obrazy pokazują tekst alternatywny, a linki są ograniczone do HTTP(S), `mailto:`, ścieżek względnych i fragmentów. Tekst jest zapisywany dokładnie tak, jak go wpisano, łącznie z białymi znakami. Niedostępny w kolumnach tabeli. |

Wartości, które nie pasują do kontrolki, np. starsze dane, nadal są widoczne jako zwykły tekst bez linku i próbki. Pola bez komponentu korzystają ze standardowego edytora dla swojego typu wartości.

Kontrolki edycji walidują wartości tylko w aplikacji internetowej. Aby odrzucać błędne wartości również z API, CLI i importów, dodaj `value_schema` lub `record_schema`; zobacz [Walidacja](/pl/builders/validation/).

## Komponenty

Każdy blok może wskazać komponent przez `component = { id, version, props }`. Identyfikator i wersja muszą odpowiadać zarejestrowanemu komponentowi, komponent musi obsługiwać dany blok i typ wartości, a `props` może zawierać tylko zadeklarowane przez niego opcje. Wbudowane komponenty są wymienione w [dokumentacji schematu](/pl/reference/blueprint/#odwołania-do-komponentów).

## Panele rozszerzeń na stronach rekordów

Administratorzy obszaru roboczego ustawiają domyślną kolejność elementów rozszerzeń dla całego obszaru roboczego. Schemat może nadpisać trzy miejsca osadzenia na stronach własnych rekordów:

```yaml
views:
  extension_layout:
    type: extension_layout
    version: 1
    outlets:
      record_tab:
        order: ['acme.inventory:summary', 'acme.pricing:margin']
        hidden: ['acme.legacy:panel']
```

Miejsca osadzenia, które można tu ustawić, to `record_tab` i `record_action`. Układ nigdy nie nadaje uprawnień ani nie włącza rozszerzenia. Wpisy dla brakujących lub wyłączonych rozszerzeń są zachowywane i ignorowane, więc układ przetrwa wyłączenie i ponowne włączenie rozszerzenia.
