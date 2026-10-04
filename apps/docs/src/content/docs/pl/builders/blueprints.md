---
title: Tworzenie schematu
description: Zbuduj schemat od pustego pliku do opublikowanej wersji z relacjami, widokami, walidacją i domieszką.
---

Schemat to dokument TOML opisujący jeden rodzaj rekordu katalogu: jego atrybuty, ich zachowanie w kontekstach, sposób walidacji i układ w aplikacji internetowej. Ten przewodnik krok po kroku buduje mały katalog produktów. Każdy użyty tu klucz jest opisany w [dokumentacji TOML schematu](/pl/reference/blueprint/).

## Gdzie pisać schematy

Schematy możesz pisać w dwóch miejscach:

- W aplikacji internetowej, w **Zarządzanie → Schematy → Nowy schemat**. Edytor waliduje treść podczas pisania i pokazuje podgląd powstałego formularza.
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
kind = "entity"

[[attributes]]
code = "name"
value_type = "string"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
```

- `code` identyfikuje schemat przez cały czas jego istnienia. Wybierz go starannie; nie można go zmienić w kolejnych wersjach.
- `kind = "entity"` oznacza, że można z niego tworzyć encje.
- Każdy schemat encji potrzebuje `views.dropdown_option`. Określa on, jak Attricat opisuje kategorię wszędzie tam, gdzie pojawia się ona na liście: w selektorach relacji, etykietach filtrów i wynikach wyszukiwania.

Zapisz to jako szkic. Szkic można dowolnie edytować, ale nie może jeszcze zawierać encji.

## Krok 2: opublikuj

Publikacja zamraża wersję. Od tej chwili nigdy się nie zmienia i można z niej tworzyć encje.

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
kind = "entity"

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
- `stock_on_hand` w nowych encjach zaczyna od `0` dzięki `default_value`.
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

Za pomocą relacji modeluje się w Attricat tagi, etykiety i taksonomie. [Modelowanie katalogu](/pl/builders/modeling/) wyjaśnia, dlaczego kategoria powinna być encją, a nie łańcuchem znaków.

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

Bez widoków aplikacja internetowa pokazuje atrybuty w kolejności deklaracji. Dodaj widoki, gdy chcesz mieć karty, siatki lub dopracowaną tabelę w **Przeglądarce encji**.

```toml
[views.detail]
type = "stack"

[[views.detail.children]]
type = "stack"
component = { id = "catalog.entity_heading", version = 1 }

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

Stos `catalog.entity_heading` zamienia swoje pierwsze pole w tytuł strony, a pozostałe w podtytuł. Kolumna tabeli `brand.name` podąża za relacją `brand` i pokazuje `name` marki. Ponieważ `brand` ma `cardinality = "one"`, tę kolumnę można też sortować.

[Widoki i układy](/pl/builders/views/) opisują wszystkie bloki i komponenty.

## Krok 8: walidacja wielu pól

`value_schema` sprawdza jedną wartość. `entity_schema` sprawdza całą encję, więc może wyrażać reguły typu „produkt w promocji wymaga ceny promocyjnej”. Ten przykład zakłada, że schemat ma też atrybut logiczny `on_sale` i liczbowy `sale_price`:

```toml
entity_schema = '''
{
  "type": "object",
  "required": ["title", "sku"],
  "if": { "properties": { "on_sale": { "const": true } }, "required": ["on_sale"] },
  "then": { "required": ["sale_price"] }
}
'''
```

Umieść `entity_schema` razem z pozostałymi kluczami najwyższego poziomu, przed pierwszym `[[attributes]]`. W TOML klucz zapisany po nagłówku tabeli należy do tej tabeli.

Attricat sprawdza schemat w każdym kontekście po każdej zmianie. Zapis, który pozostawiłby którykolwiek kontekst w niepoprawnym stanie, zostaje odrzucony z `422 entity_schema_mismatch` i nic nie jest zapisywane.

JSON Schema nie potrafi porównać dwóch atrybutów. Dla reguł w rodzaju „cena promocyjna musi być niższa od ceny” dodaj do tego samego schematu nazwaną kontrolę w `x-attricat-checks`:

```toml
entity_schema = '''
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

Kontrola, która nie przejdzie, powoduje odrzucenie zapisu z `422 entity_check_failed`. Kontrole mogą też sprawdzać powiązane rekordy. Zobacz [Walidacja](/pl/builders/validation/#porównuj-atrybuty-za-pomocą-kontroli).

Atrybut [statusu](/pl/builders/validation/#statusy) może ograniczać dozwolone zmiany, a każda dozwolona zmiana może mieć [warunki](/pl/builders/validation/#warunki-przejść), np. „osoba zatwierdzająca jest ustawiona”, zanim `review` zmieni się w `released`. Zmiana z niespełnionymi warunkami zostaje odrzucona z `422 transition_conditions_unmet`.

## Krok 9: udostępnij atrybuty w domieszce

Gdy kilka schematów potrzebuje tych samych pól, np. metadanych SEO, umieść je w domieszce:

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

Opublikuj ją, a następnie dołącz do `product` i wybierz potrzebne atrybuty:

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

Dołączenie przypina konkretną wersję domieszki. Opublikowanie wersji 2 `seo` nie zmienia `product`, dopóki nie opublikujesz nowej wersji `product`, która dołącza wersję 2.

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
        "permission": "entities.publish" },
      { "from": "released", "to": "draft", "code": "correct",
        "roles": ["owner", "admin"] }
    ]
  }
}'''
```

### Kto może wykonać przejście

Przejście może określać wymagania. Osoba zapisująca zmianę musi spełnić je wszystkie, oprócz posiadania `entities.write`:

- `permission`: [uprawnienie](/pl/reference/permissions/), które musi mieć dla tej encji, np. `entities.publish`.
- `roles`: musi mieć co najmniej jedną z tych ról (wbudowaną lub [niestandardową](/pl/operate/workspaces/#role)), przydzieloną w całym obszarze roboczym, dla schematu lub dla tej encji.
- `separate_from`: rozdzielenie obowiązków. Nie może to być osoba, która jako ostatnia wykonała w tej encji i w tym kontekście przejście o jednym z podanych kodów `code`. W powyższym przykładzie osoba, która przesłała dokument do przeglądu, nie może go też zatwierdzić.

Nadaj przejściu `code`, aby wskazywać je w `separate_from` i w historii. Odrzucone przejście zwraca `403 status_transition_forbidden` lub `403 status_separation_of_duties` i nic nie zostaje zapisane. Formularz edycji wyłącza przejścia, których nie możesz wykonać, i wyjaśnia dlaczego.

Te same kontrole obowiązują każdego, kto zapisuje dane: API, CLI, przepływy pracy (jako osobę, której zmiana uruchomiła przepływ), rozszerzenia i agentów (jako osobę, która zatwierdziła zmianę). Zapis bez możliwej do ustalenia osoby, np. z zaplanowanego zadania, nie może wykonać ograniczonego przejścia.

### Blokuj sfinalizowane rekordy

`lock` w statusie sprawia, że treść jest tylko do odczytu, dopóki rekord ma ten status:

- `"lock": "all"` zamraża wszystkie atrybuty, relacje i pliki oprócz samego statusu oraz uniemożliwia usunięcie encji.
- `"lock": ["title", "procedure"]` zamraża tylko te atrybuty.

Blokady są egzekwowane na serwerze dla każdej drogi zapisu: formularza edycji, API, CLI, przepływów pracy, rozszerzeń, agentów, przywracania z historii wartości, przesyłania plików i zmiany ich kolejności oraz migracji. Odrzucony zapis zwraca `409 record_locked`. Formularz pokazuje zablokowane pola jako tylko do odczytu wraz z powodem.

Status, który deklaruje blokadę, wymaga jawnej listy `transitions`, więc wyjście z niego jest zawsze nazwanym, ograniczonym przejściem. Aby poprawić wydany rekord, najpierw wykonaj przejście korygujące (w przykładzie `released` → `draft`, dostępne tylko dla właścicieli i administratorów), a dopiero potem edytuj. Korekta może zmienić wyłącznie status. Odblokowanie jest zapisywane w dzienniku audytu jako `entity.record.unlock`.

Blokady działają w obrębie kontekstu: rekord wydany na jednym rynku nadal można edytować na innym, gdzie jest szkicem, o ile zmiana nie trafi do zablokowanego rynku przez dziedziczenie.

### Wiąż zatwierdzenia z przejrzaną treścią

`approval` w statusie rejestruje zatwierdzenie za każdym razem, gdy rekord przechodzi do tego statusu: kto zatwierdził, kiedy, oraz skrót SHA-256 objętej treści. `covers` przyjmuje `"all"` lub listę atrybutów; objęte relacje i pliki wchodzą do skrótu, pliki według ich dokładnej zawartości bajtowej.

Gdy objęta treść się później zmieni, zatwierdzenie zostaje unieważnione w tym samym zapisie, a jeśli rekord wciąż ma status zatwierdzony, przechodzi do `void_to`. W przykładzie edycja tytułu zatwierdzonego dokumentu odsyła go z powrotem do przeglądu. Zmiany atrybutów, których zatwierdzenie nie obejmuje, nie naruszają zatwierdzenia.

Zatwierdzenia i unieważnienia pojawiają się w dzienniku audytu (`entity.approval.record`, `entity.approval.void`) oraz w panelu **Kontrola rekordu** na stronie encji.

### Zachowuj wydane pliki

`retention_days` w zablokowanym statusie zakłada blokadę retencji na każdy plik, do którego odwołują się zablokowane atrybuty, gdy rekord przechodzi do tego statusu. Zablokowane pliki nigdy nie są usuwane z magazynu przed wygaśnięciem blokady, nawet jeśli późniejsza korekta je odłączy. Blokady są wyświetlane na stronie encji i w dzienniku audytu. Zobacz [Blokady retencji](/pl/operate/workspaces/#blokady-retencji).

## Wersje a istniejące encje

Każda encja pamięta dokładną wersję schematu, z którą została utworzona. Gdy opublikujesz wersję 2 schematu `product`, istniejące produkty pozostają przy wersji 1: ich wartości i walidacja zachowują znaczenie z chwili zapisu.

Attricat oznacza je wtedy jako nieaktualne i proponuje migrację. Wersje, które tylko dodają opcjonalne atrybuty, można migrować zbiorczo. Zmiany, które usuwają atrybuty lub zmieniają ich typ albo dodają atrybuty wymagane, wymagają decyzji dla każdej encji. Zobacz [Wersje i migracja](/pl/builders/revisions/).

Dwie praktyki oszczędzają później kłopotów:

- Nigdy nie używaj ponownie kodu atrybutu w innym znaczeniu w późniejszej wersji. Zamiast tego dodaj nowy kod.
- Dodawaj atrybuty zamiast je zmieniać. Wersje, które tylko dodają, migrują się bez niczyjej pomocy.

## Więcej możliwości

Schemat może też zawierać:

- [Reguły](/pl/builders/rules/), które oznaczają problemy z jakością danych, np. brak tytułu, i mogą je [egzekwować](/pl/builders/rules/#egzekwowanie-reguły) przy zapisie lub zmianie statusu.
- [Politykę publikacji](/pl/guides/publishing/#zachowaj-publikację-po-zaufanych-edycjach), która pozwala zaufanym rolom edytować bez cofania zatwierdzeń w kanałach.
- [Zadania konektorów](/pl/reference/blueprint/#zadania-konektorów), które importują lub eksportują encje przez rozszerzenie konektora.
- Atrybuty, których typ pochodzi z [rozszerzenia](/pl/reference/blueprint/#typy-atrybutów-z-rozszerzeń).
- [Układ paneli i akcji rozszerzeń](/pl/reference/blueprint/#extension_layout) na stronach encji.
