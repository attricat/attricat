---
title: Modelowanie katalogu
description: Zdecyduj, co ma być schematem, atrybutem, relacją, a co kontekstem. Poznaj wzorce dla taksonomii, tagów, hierarchii i tłumaczeń.
---

Większość pytań o modelowanie w Attricat sprowadza się do czterech elementów. Wybierz właściwy od początku; późniejsza zmiana oznacza migrację danych.

| Użyj | Do | Przykład |
| --- | --- | --- |
| **Schematu encji** | Czegoś z własną tożsamością, nazwą lub cyklem życia | Produkt, kategoria, marka, dostawca |
| **Atrybutu skalarnego** | Faktu dotyczącego jednej encji | SKU, tytuł, waga, data premiery |
| **Relacji** | Powiązania między encjami | Produkt → kategorie, produkt → marka |
| **Kontekstu** | Zakresu, w którym wartości się różnią | Rynek, język, kanał sprzedaży, sklep |

## Klasyfikacje to encje

Tagi, etykiety, kategorie, kolory, materiały i certyfikaty powinny być encjami powiązanymi relacjami. Nie modeluj ich jako łańcuchów znaków ani list łańcuchów znaków.

Klasyfikacja zwykle potrzebuje tego, czego łańcuch znaków nie zapewni: stałej tożsamości po zmianie nazwy, przetłumaczonej nazwy, rodzica w hierarchii, opisu, właściciela. Encja ma to wszystko, a relacja zapisuje, które klasyfikacje mają zastosowanie.

```toml
format_version = 1
code = "material"
name = "Material"
kind = "entity"

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "code"
value_type = "string"
context_editable = "default"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
```

```toml
# On product
[[attributes]]
code = "materials"
value_type = "relationship"
target_blueprint = "material"
```

Kilka wskazówek:

- **Jeden schemat na słownik.** Kolor nie jest kategorią tylko dlatego, że oba pojawiają się przy produkcie. Trzymaj je osobno, chyba że mają wspólne znaczenie i cykl życia.
- **Nazywaj relacje zgodnie z ich znaczeniem.** `materials` i `certifications` są czytelniejsze niż ogólne `tags`. Zachowaj ogólną relację `labels` dla adnotacji, które przecinają wiele dziedzin.
- **Zachowaj stały atrybut z kodem**, gdy potrzebują go integracje, i zadeklaruj go jako [klucz unikalny](/pl/builders/validation/#klucze-unikalne), aby nie dało się go użyć ponownie. Same relacje zawsze używają UUID encji.
- **Użyj atrybutu skalarnego** dla wartości wewnętrznych i niewspółdzielonych: SKU, notatki tekstowej.

### Pojedynczy wybór i powiązania wyłączne

`cardinality = "one"` sprawia, że relacja pozwala wybrać tylko jeden cel. Wiele produktów nadal może mieć tę samą markę:

```toml
[[attributes]]
code = "brand"
value_type = "relationship"
target_blueprint = "brand"
cardinality = "one"
```

Dodaj `target_cardinality = "one"` tylko wtedy, gdy każdy cel może zostać przypisany tylko raz, na przykład produkt i jego unikalny rekord kodu kreskowego.

### Powiązania z kilkoma rodzajami encji

Relacja może wskazywać dowolny schemat, jeden schemat albo listę schematów. Użyj listy, gdy powiązanie ma jasne znaczenie, ale więcej niż jeden rodzaj celu, np. przedmiot oceny zgodności:

```toml
[[attributes]]
code = "subject"
value_type = "relationship"
target_blueprints = ["product", "product_revision", "material", "part"]
```

Powiązania z innymi schematami są odrzucane, okno wyboru encji pozwala wskazać, który z wymienionych schematów przeszukać, a każdy z nich może pokazać powiązania blokiem `incoming_relationship_list`. Zamiast jednej nieograniczonej relacji wybieraj jedną relację na znaczenie.

### Klucze biznesowe

Numery części, dokumentów czy inwentarzowe identyfikują encję dla ludzi i innych systemów. Zadeklaruj je jako [klucze unikalne](/pl/builders/validation/#klucze-unikalne); łącz atrybuty, gdy identyfikator jest unikalny tylko w obrębie czegoś innego, np. oznaczenie wersji w obrębie dokumentu albo numer części w obrębie producenta.

## Hierarchie

Dodaj do schematu klasyfikacji relację do niego samego:

```toml
# On category
[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "category"
tree = true
context_editable = "default"
```

`tree = true` sprawia, że relacja jest jednokrotnego wyboru, i odrzuca każde powiązanie, które uczyniłoby kategorię własnym przodkiem (`409 relationship_cycle`). Dla struktur, w których encja może mieć kilka celów, ale nigdy nie może wrócić do siebie, takich jak zależności czy łańcuchy poprzedników, użyj `acyclic = true`. Oba ustawienia wymagają `context_editable = "default"`, więc hierarchia jest taka sama w każdym kontekście. Publikacja któregokolwiek z nich na istniejących danych zgłasza cykle, które już istnieją.

**Przeglądarka encji** wykrywa relacje odwołujące się do własnego schematu i zamienia filtr dla `product.categories` w drzewo z sumowanymi licznikami. Wybranie *Shirts* obejmuje także produkty przypisane do kategorii podrzędnych. Zobacz [Przeglądanie encji](/pl/guides/explore/#fasety-relacji).

Aby pokazać ścieżkę na stronie produktu, użyj komponentu [`catalog.relationship_hierarchy`](/pl/builders/views/#hierarchie). Aby wykryć kategorię, która stała się własnym przodkiem, dodaj regułę z predykatem [`acyclic`](/pl/builders/rules/#predykaty).

## Ograniczenia obejmujące relacje

Niektóre ograniczenia dotyczą dwóch rekordów: zakłady na certyfikacie muszą należeć do dostawcy z certyfikatu, a niezgodności nie można zamknąć, dopóki wskazujące ją działanie korygujące jest otwarte. Zapisz je w rekordzie, do którego ograniczenie należy, jako [kontrolę `linked` lub `referenced_by`](/pl/builders/validation/#sprawdzaj-powiązane-rekordy).

- **Jeden krok.** Kontrola odczytuje rekordy, które encja wskazuje, albo rekordy, które wskazują ją, i na tym się zatrzymuje. Jeśli ograniczenie wymaga dwóch kroków, dodaj relację lub atrybut, który przybliży potrzebną wartość o jeden krok.
- **Ograniczony zakres.** Kontrola odczytuje najwyżej 200 powiązanych rekordów na relację i 1000 rekordów wskazujących. Relacje, które mają być sprawdzane, utrzymuj małe; kontrole na większych zbiorach nie przechodzą.
- **Sprawdzane przy zapisie właściciela.** Zmiana powiązanego rekordu nie jest odrzucana. Połącz kontrolę z [regułą](/pl/builders/rules/#zmiany-w-powiązanych-rekordach) wyzwalaną zdarzeniem, aby rekordy, których to dotyczy, były zgłaszane jako ustalenia.
- **Dla każdego kontekstu.** Powiązane rekordy są odczytywane w tym samym kontekście co encja, więc relacja, która różni się między rynkami, jest sprawdzana osobno dla każdego rynku.

## Konteksty

Drzewo kontekstów opisuje, gdzie wartości się różnią. Zaplanuj je przed dodaniem wielu nadpisań, ponieważ dziedziczenie podąża za drzewem.

```text
default
├── PL            (language: pl)
│   ├── PL-web
│   └── PL-marketplace
└── DE            (language: de)
    └── DE-web
```

W tym drzewie opis produktu zapisany w `PL` jest używany przez `PL-web` i `PL-marketplace`, chyba że ustawią własny.

- **Konteksty nie są automatycznie językami.** Jeśli drzewo odpowiada lokalizacjom, zapisz to w `data` kontekstu (na przykład `{"language": "pl"}`) i w konwencjach zespołu.
- **Łącz wymiary ostrożnie.** Jedno drzewo może łączyć rynek i kanał, jak powyżej, ale każda potrzebna kombinacja musi istnieć jako węzeł.
- **Decyduj dla każdego atrybutu.** `context_fallback = "none"` zatrzymuje dziedziczenie wartości, które nigdy nie mogą trafić do kontekstu potomnego, takich jak promocja w jednym kanale. `context_editable = "default"` blokuje wartości globalne, takie jak SKU.
- **Tłumacz nazwy klasyfikacji na miejscu.** `name` materiału może mieć nadpisania w kontekstach językowych. Selektory relacji i kolumny tabeli pokazują etykietę rozstrzygniętą w wybranym kontekście.
- **Relacje też zależą od kontekstu.** Produkt może mieć inny zestaw kategorii na jednym z rynków.

Konteksty pełnią też rolę [kanałów eksportu](/pl/guides/publishing/), gdy włączysz dla nich eksport.

## Wielokrotne użycie atrybutów

Definicje atrybutów można współdzielić na dwa sposoby.

**Mixiny** współdzielą atrybuty między schematami. Mixin to schemat z `kind = "mixin"`; inne schematy przypinają przez include jedną konkretną wersję i wybierają jej atrybuty przez `from`. Używaj ich dla grup pól, których wiele schematów potrzebuje w tej samej postaci, np. SEO lub wymiarów. Zobacz [Tworzenie schematu](/pl/builders/blueprints/#krok-9-udostępnij-atrybuty-w-mixinie).

**Atrybuty wielokrotnego użytku** to rejestr pojedynczych definicji atrybutów w obszarze roboczym, zarządzany w **Zarządzanie → Atrybuty wielokrotnego użytku**. Edytorzy mogą dołączyć opublikowany atrybut wielokrotnego użytku lub ich grupę do pojedynczej encji. Używaj ich dla okazjonalnych pól, których potrzebują tylko niektóre encje i które nie należą do schematu. Definicja jest zapisana w TOML:

```toml
code = "country_of_origin"
name = "Country of origin"
value_type = "string"
searchable = true
```

Przyjmuje te same klucze atrybutów co schemat (`value_type`, `value_schema`, `default_value`, `tags`, `context_fallback`, `context_editable`, `readonly` oraz klucze relacji z `target_blueprint_code`), a także `searchable`, który uwzględnia wartości w wyszukiwaniu i filtrach **Przeglądarki encji**. Każda zmiana tworzy nową wersję; wersję trzeba opublikować, zanim będzie można ją dołączyć. Przestrzeń nazw pochodzi z obszaru roboczego, więc pełny kod atrybutu wielokrotnego użytku to `<namespace>:<code>`.

Tworzenie i publikowanie atrybutów wielokrotnego użytku wymaga uprawnienia `blueprints.write`. Dołączenie takiego atrybutu do encji wymaga uprawnienia `entities.write` do tej encji.

## Przypisz odpowiedzialność

Zapisuj osobę odpowiedzialną za rekord, na przykład wykonawcę, właściciela czy recenzenta, w **atrybucie użytkownika lub zespołu**, a nie jako dowolny tekst. Dowolny tekst nie sprawdza, czy osoba istnieje i nadal należy do obszaru roboczego, i nie nadaje się do pewnego filtrowania. Atrybut użytkownika lub zespołu to atrybut `string` z adnotacją `x-attricat-principal`, która określa, co przyjmuje:

```toml
[[attributes]]
code = "reviewer"
name = "Recenzent"
value_type = "string"
value_schema = '''{
  "type": "string",
  "x-attricat-principal": { "version": 1, "kinds": ["user", "team"] }
}'''
```

- Formularz pokazuje listę aktywnych członków obszaru roboczego i zespołów, a rekord wyświetla wybraną nazwę. Użyj `"kinds": ["user"]`, gdy odpowiedzialność musi należeć do jednej osoby.
- Zapis sprawdza, czy nowa wartość to aktywny członek lub istniejący zespół. Rekord przypisany do osoby, która odeszła, zachowuje przypisanie i pokazuje je jako nieaktywne, dopóki ktoś go nie zmieni.
- W Przeglądarce filtruj atrybut według osoby lub zespołu albo wybierz **Przypisane do mnie (lub moich zespołów)**. Zapisane wyszukiwania z takim filtrem działają dla każdego, kto je otworzy.
- [Zespołami](/pl/operate/workspaces/#zespoły) zarządza się w **Zarządzanie → Zarządzanie obszarem roboczym → Zespoły**. Przypisanie zapisuje zespół, więc zmiana jego składu nigdy nie zmienia rekordów.
- Przypisanie nie nadaje ani nie ogranicza dostępu. [Wymagania przejść](/pl/builders/validation/#kto-może-wykonać-przejście) statusu sprawdzają osobę zapisującą zmianę, a nie osobę przypisaną, a predykaty reguł mogą jedynie sprawdzić, czy rekord jest przypisany, lub porównać wartość ze stałym użytkownikiem lub zespołem. Nie da się zadeklarować zasady „zamknąć może tylko osoba przypisana”; użyj zamiast tego ról lub [rozdzielenia obowiązków](/pl/builders/validation/#kto-może-wykonać-przejście).

Dokładne zasady opisuje sekcja [Przypisania użytkowników i zespołów](/pl/reference/blueprint/#przypisania-użytkowników-i-zespołów).

## Tagi systemowe i metadane

Każda encja ma też `system_tags` (zbiór łańcuchów znaków) i `system_metadata` (obiekt JSON do 64 KiB). Znajdują się poza schematem, nie są wersjonowane i nie są pokazywane w widokach. Służą automatyzacji: oznaczaniu partii do przetworzenia, zapisywaniu źródła importu lub oznaczaniu rekordu do przeglądu. Przepływy pracy i reguły mogą je odczytywać i zapisywać. Wyszukiwanie może filtrować po tagach systemowych.

Do wszystkiego, co człowiek ma widzieć lub edytować, używaj atrybutów.
