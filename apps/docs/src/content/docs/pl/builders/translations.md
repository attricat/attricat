---
title: Tłumaczenie etykiet
description: Wyświetlaj nazwy schematów, nazwy atrybutów, etykiety statusów i etykiety widoków w języku każdego użytkownika dzięki leksykonowi obszaru roboczego.
---

Etykiety schematów są domyślnie zwykłym tekstem i wyświetlają się dokładnie tak, jak je zapisano. Aby przetłumaczyć etykietę, umieść ją w podwójnych nawiasach klamrowych. Tekst w nawiasach jest wyszukiwany w **leksykonie** obszaru roboczego, czyli zbiorze tłumaczeń przechowywanym poza schematami:

```toml
name = "{{Product}}"

[[attributes]]
code = "price"
name = "{{Price}}"
value_type = "number"

[[views.detail.tabs]]
label = "{{Overview}}"
```

Użytkownik z interfejsem w języku polskim zobaczy polski wpis dla `Product`. Jeśli go nie ma, zobaczy wpis angielski, a gdy brakuje i jego, tekst z nawiasów. Nieprzetłumaczona etykieta nadal poprawnie wyświetla się po angielsku.

Ponieważ leksykon jest oddzielony od schematów, możesz poprawić tłumaczenie lub dodać język bez publikowania nowej wersji schematu. Tłumaczenia zależą od języka interfejsu użytkownika, a nie od wybranego [kontekstu](/pl/guides/contexts/), który wybiera wartości katalogu, a nie etykiety.

## Które etykiety są tłumaczone

| Etykieta | Gdzie |
| --- | --- |
| Nazwa schematu | `name` |
| Nazwa atrybutu | `[[attributes]] name` atrybutu zdefiniowanego w schemacie |
| Nazwa atrybutu wielokrotnego użytku | `name` w jego definicji |
| Etykieta opcji statusu | `label` każdej opcji w adnotacji `x-attricat-status`, w atrybutach zdefiniowanych w schemacie i wielokrotnego użytku |
| Etykieta karty i sekcji akordeonu | `label` w blokach `tabs` i `accordion` |
| Nagłówek kolumny tabeli | `label` w `[views.table] columns` |
| Przycisk i tytuł okna relacji przychodzących | `label` w `incoming_relationship_list` |

Pozostały tekst, na przykład bloki `heading` i `text`, separatory wyświetlania i wartości katalogu, zawsze wyświetla się tak, jak go zapisano.

### Etykiety statusów

[Status](/pl/builders/validation/#statusy) przechowuje stały kod, na przykład `live`, i wyświetla etykietę jego opcji. Etykietę tłumaczy się tak jak każdą inną:

```toml
"options": [
  { "code": "draft", "label": "{{Draft|status}}" },
  { "code": "live", "label": "{{Live}}", "tone": "success" }
]
```

Przetłumaczoną etykietę, z takim samym zastępowaniem brakujących tłumaczeń jak w innych etykietach, pokazują znaczniki statusu, lista wyboru statusu na stronie encji i w formularzu tworzenia, lista wartości filtra i etykiety filtrów w Przeglądarce oraz wartości wyświetlane jako tekst. Wyszukiwanie, filtry, zapisane wyszukiwania, API i eksporty nadal używają kodu, więc przetłumaczenie lub zmiana etykiety nigdy nie zmienia zapisanych danych. Kontekst, na przykład `{{Draft|status}}`, oddziela status „Draft” od innych użyć tego słowa.

## Składnia odwołań

| Zapis | Znaczenie |
| --- | --- |
| `{{Product}}` | Przetłumacz `Product`. |
| `{{Order\|purchase}}` | Przetłumacz `Order` w znaczeniu `purchase`. Część po `\|` to kontekst; nigdy się nie wyświetla. |
| `SKU {{Price}}` | Tekst wokół odwołania pozostaje bez zmian. |
| `\{{` | Dosłowne `{{`. |
| `Brand™` | Bez nawiasów: wyświetla się bez zmian i nigdy nie jest tłumaczone. |

- **Klucze to tekst angielski.** Tekst w nawiasach jest jednocześnie kluczem i angielskim tekstem zastępczym. Spacje na początku, na końcu i powtórzone spacje są pomijane, ale wielkość liter ma znaczenie: `Product` i `product` to osobne klucze.
- **Wspólne terminy tłumaczysz raz.** Każdy schemat używający `{{Price}}` pokazuje to samo tłumaczenie.
- **Kontekst rozróżnia homonimy i formy gramatyczne.** `{{Order|purchase}}` i `{{Order|sorting}}` to osobne wpisy, podobnie jak słowa wymagające w innym języku innego rodzaju gramatycznego. Odwołanie z kontekstem nigdy nie korzysta z wpisu bez kontekstu.
- **Obejmuj całe wyrażenie.** Pisz `"{{Products in this category}}"`, a nie `"{{Products}} in this category"`. Szyk, przypadek i rodzaj różnią się między językami, więc tłumacz potrzebuje całego wyrażenia.

Zapisanie schematu kończy się błędem, jeśli tłumaczona etykieta zawiera nieprawidłowe odwołanie: niezamknięte `{{`, puste nawiasy, pusty kontekst, więcej niż jeden znak `|` albo nawias klamrowy wewnątrz odwołania.

## Zarządzanie tłumaczeniami

Tłumaczenia to dane obszaru roboczego. Wczytuje je każdy, kto może czytać katalog; ich zmiana wymaga uprawnienia `blueprints.write`.

W aplikacji internetowej otwórz **Zarządzanie → Tłumaczenia** i wybierz język. Strona pokazuje etykiety, które nadal wymagają tłumaczenia na ten język, w tym brakujące formy liczby mnogiej, a przy każdej z nich przycisk otwierający formularz tłumaczenia z wypełnionym kluczem. Poniżej widać wszystkie wpisy w danym języku wraz ze źródłem; wpisy, których nie używa już żaden schemat, są oznaczone jako **Nieużywane**. Możesz tam dodawać, edytować i usuwać wpisy oraz importować i eksportować cały język. Zmiany od razu obowiązują w całej aplikacji.

Te same operacje są dostępne w CLI:

```sh
acli lexicon set --key Product --language pl --text Produkt
acli lexicon set --key Order --context purchase --language pl --text Zamówienie
acli lexicon list --language pl
acli lexicon delete --key Order --context purchase --language pl
```

Aby pracować na całym języku naraz, wyeksportuj go, zmień plik i zaimportuj:

```sh
acli lexicon export --language pl > pl.json
acli lexicon import --file pl.json            # dodaje i aktualizuje wpisy
acli lexicon import --file pl.json --replace  # usuwa też wpisy, których nie ma w pliku
```

Pliki importu można też zapisać w formacie TOML (z rozszerzeniem `.toml`):

```toml
format_version = 1
language = "pl"

[[entries]]
key = "Product"
text = "Produkt"

[[entries]]
key = "Order"
context = "purchase"
text = "Zamówienie"
```

Języki to znaczniki BCP 47, na przykład `pl`, `de` lub `pt-BR`. Wszystkie opcje opisuje [dokumentacja CLI](/pl/reference/cli/#tłumaczenia). Aplikacja internetowa udostępnia obecnie interfejs po angielsku i po polsku.

## Zmiana angielskiego brzmienia

Wpis angielski (`en`) zastępuje wyświetlany tekst angielski bez zmiany klucza, więc istniejące tłumaczenia pozostają przypisane:

```sh
acli lexicon set --key Product --language en --text Item
```

W ten sposób zmienisz nazwę etykiety lub poprawisz literówkę. Klucz w schemacie zmieniaj tylko wtedy, gdy zmienia się znaczenie, a następnie przetłumacz nowy klucz.

## Formy liczby mnogiej

Etykiety bez liczby, takie jak etykiety kart i nagłówki kolumn, używają osobnych kluczy dla liczby pojedynczej i mnogiej: `{{Product}}` i `{{Products}}`.

Tam, gdzie aplikacja pokazuje liczbę encji, na przykład w liczbie wyników Przeglądarki encji, może użyć nazwy schematu w formie odpowiedniej dla tej liczby: „1 Produkt”, „3 Produkty”, „5 Produktów”. W tym celu nadaj wpisowi nazwy schematu po jednej formie dla każdej kategorii liczby mnogiej języka:

```sh
acli lexicon set --key Product --language en --plural-category one --text Product
acli lexicon set --key Product --language en --plural-category other --text Products
acli lexicon set --key Product --language pl --plural-category one --text Produkt
acli lexicon set --key Product --language pl --plural-category few --text Produkty
acli lexicon set --key Product --language pl --plural-category many --text Produktów
acli lexicon set --key Product --language pl --plural-category other --text Produktu
```

| Język | Kategorie liczby mnogiej |
| --- | --- |
| angielski, niemiecki, niderlandzki, szwedzki | `one`, `other` |
| francuski, hiszpański, włoski, portugalski | `one`, `many`, `other` |
| polski, czeski, słowacki, ukraiński, rosyjski | `one`, `few`, `many`, `other` |
| japoński, chiński, koreański | `other` |

Wpis bez kategorii liczby mnogiej używa `other`. Forma zawiera tylko rzeczownik; liczbę wstawia aplikacja. Gdy wpis ma formy liczby mnogiej, etykiety bez liczby używają formy `one`, więc zapisuj formy z wielkością liter używaną w etykietach. Dopóki nazwa schematu nie ma form liczby mnogiej, aplikacja zachowuje ogólne sformułowanie, na przykład „5 wyników”.

## Brakujące i nieużywane tłumaczenia

```sh
acli lexicon report
acli lexicon report --language pl --language de
```

Raport pokazuje dla każdego języka:

- **Nieprzetłumaczone** odwołania z etykiet katalogu, które nie mają wpisu w tym języku. Angielski nigdy się tu nie pojawia, ponieważ klucz jest tekstem angielskim.
- **Brakujące kategorie liczby mnogiej** dla nazw schematów (które aplikacja pokazuje z liczbami) oraz dla wpisów, które mają już formy liczby mnogiej. Dla angielskiego tu pojawiają się brakujące formy `one` i `other`.

Raport pokazuje też wpisy **osierocone**, których klucza i kontekstu nie używa już żadna wersja schematu ani atrybut wielokrotnego użytku. Przejrzyj je przed usunięciem: starsze wersje, których nadal używają encje, liczą się jako używane.

## Tłumaczenia z pakietów rozwiązań

[Pakiet rozwiązań](/pl/builders/solution-packs/) może zawierać tłumaczenia. Zastosowanie pakietu dodaje jego wpisy do leksykonu. Wpisy zapisane w obszarze roboczym, przed instalacją lub po niej, zawsze mają pierwszeństwo: pakiet nigdy ich nie nadpisuje, a edycja wpisu dostarczonego przez pakiet czyni go wpisem obszaru roboczego. Nowsze wydanie pakietu może zaktualizować dostarczone przez siebie wpisy, których nikt nie edytował.
