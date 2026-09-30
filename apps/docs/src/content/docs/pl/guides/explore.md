---
title: Przeglądanie encji
description: Znajduj encje za pomocą wyszukiwania, filtrów, faset relacji i sortowania, a następnie zapisuj i udostępniaj wyniki.
---

**Przeglądarka encji** to strona główna Attricat. Wyświetla encje jednego schematu naraz i pozwala zawęzić je zapytaniem wyszukiwania, filtrami atrybutów i fasetami relacji.

## Wybierz, co chcesz przeglądać

Wybierz schemat z selektora lub ze skrótu na pasku bocznym. Administratorzy obszaru roboczego decydują, które schematy pojawiają się jako skróty na pasku bocznym.

Selektor **Zakres wersji** określa, które wersje schematu widzisz:

- **Aktualny** (domyślnie) pokazuje encje w najnowszej opublikowanej wersji. Jeśli istnieją starsze encje, komunikat informuje, ile z nich jest ukrytych, i zawiera link do nich.
- Konkretna starsza **Wersja** pokazuje tylko encje przypięte do tej wersji.
- **Wszystkie wersje** pokazuje wszystko. Dodaj kolumnę **Schemat** i posortuj według niej, aby zobaczyć najpierw najstarsze encje. Zobacz [Wersje i migracja](/pl/builders/revisions/).

## Wyszukiwanie

Wpisz tekst w polu wyszukiwania i naciśnij Enter. Zwykły termin, taki jak `linen`, dopasowuje wartości własnych atrybutów wybranego schematu. Nie przeszukuje powiązanych encji.

Aby wyszukiwać szerzej, dodaj kwalifikator do terminu:

| Zapytanie | Znajduje |
| --- | --- |
| `linen` | Encje, które mają „linen” w dowolnej ze swoich wartości tekstowych. |
| `sku:ABC*` | Encje, których `sku` zaczyna się od `ABC`. |
| `colors:red` | Encje powiązane przez `colors` z encją, która ma „red” w dowolnej wartości. |
| `colors.name:red` | To samo, ale tylko w `name` powiązanej encji. |
| `family.product_type.name:laptop` | Przechodzi przez dwie relacje, a następnie dopasowuje `name`. |
| `*:red` | Encje, które mają „red” w swoich wartościach lub w czymkolwiek powiązanym w odległości do trzech kroków. |
| `@id:ID-1,ID-2` | Dokładnie wskazane encje. |
| `colors.@id:ID-1,ID-2` | Encje powiązane przez `colors` z jedną ze wskazanych encji. |

Kilka terminów oddzielonych spacjami musi pasować jednocześnie. Pełną gramatykę opisuje [Składnia wyszukiwania](/pl/guides/search-syntax/).

Otwórz **Informacje o wyszukiwaniu** przy wyniku, aby zobaczyć, dlaczego został dopasowany, na przykład *red przez 1 relację*.

## Filtry

**Dodaj filtr** zawęża wyniki według wartości atrybutu w kontekście domyślnym. Dostępne operatory zależą od typu:

| Typ | Operatory |
| --- | --- |
| Łańcuch znaków | Równa się, Zawiera, Zaczyna się od |
| Liczba, liczba całkowita, data, data i godzina, godzina | Równa się, Większe niż, Większe lub równe, Mniejsze niż, Mniejsze lub równe |
| Wartość logiczna | Równa się |

Możesz też filtrować według atrybutu powiązanej encji, na przykład `brand.name`, przez maksymalnie trzy relacje.

Filtr możesz również utworzyć z poziomu tabeli: otwórz menu komórki i wybierz **Filtruj**.

Każdy aktywny filtr jest widoczny jako etykieta pod polem wyszukiwania. Wszystkie filtry, fasety i zapytanie wyszukiwania muszą pasować jednocześnie.

## Fasety relacji

Każdy atrybut relacji schematu ma fasetę na pasku bocznym. Wybierz jeden lub więcej celów, aby zachować tylko encje z nimi powiązane.

Gdy schemat docelowy ma relację do samego siebie, na przykład `category.parent`, faseta staje się drzewem:

- Liczniki przy każdym węźle obejmują wszystko, co znajduje się pod nim. Encja przypisana do dwóch podkategorii jest liczona raz.
- Wybranie węzła nadrzędnego dopasowuje też encje przypisane do jego potomków.
- Kilka zaznaczeń w jednej fasecie oznacza *dowolny z nich*. Zaznaczenia w różnych fasetach muszą pasować jednocześnie.
- Liczniki uwzględniają zapytanie wyszukiwania i zakres wersji, ale nie własne zaznaczenie fasety, więc po wybraniu węzła nadal widzisz liczniki węzłów sąsiednich. Filtry atrybutów nie wpływają na liczniki faset.

**Opcje drzewa** pozwalają wybrać, które pole odwołujące się do tego samego schematu buduje drzewo i w którym kontekście rozwiązywane są relacje. Zmiana dowolnej z tych opcji czyści zaznaczenie fasety.

## Kontekst

Selektor **Kontekst** pokazuje wartości rozstrzygnięte w danym kontekście: własną wartość kontekstu, jeśli ją ma, a w przeciwnym razie wartość dziedziczoną. Zobacz [Konteksty](/pl/guides/contexts/).

Jeśli wybrany kontekst jest [kanałem publikacji](/pl/guides/publishing/), tabela zyskuje kolumnę **Publikacja**, która pokazuje, czy każda encja jest w nim opublikowana. Można ją posortować, aby nieopublikowane encje znalazły się na początku.

## Sortowanie i układ kolumn

Kliknij nagłówek kolumny, aby sortować. Kolumnę można sortować, gdy zawiera jedną wartość skalarną na encję. Kolumny, które podążają za relacją wielowartościową, są wyświetlane, ale nie można ich sortować.

W widoku **Wszystkie wersje** sortowanie według powiązanej wartości działa tylko wtedy, gdy wszystkie pasujące encje są w tej samej wersji.

**Kolumny** pozwalają pokazywać, ukrywać i zmieniać kolejność kolumn. Układ jest zapisywany w przeglądarce.

## Zapisywanie i udostępnianie wyszukiwań

Wszystko, co ustawisz w Przeglądarce, jest przechowywane w adresie URL strony, więc przeładowanie strony lub wysłanie linku odtwarza ten stan.

- **Zapisz wyszukiwanie** zapisuje wyszukiwanie pod nazwą. Wybierz **Prywatne**, aby zachować je dla siebie, albo **Obszar roboczy**, aby udostępnić je każdemu członkowi, który może odczytywać encje. Zapisane wyszukiwania otwierasz z listy **Zapisane wyszukiwania**.
- Zmiana zapisanego wyszukiwania oznacza je jako mające niezapisane zmiany. **Zapisz zmiany** je aktualizuje, a **Zapisz wyszukiwanie** zapisuje kopię.
- **Udostępnij wyszukiwanie** kopiuje link. Długie wyszukiwania są zapisywane jako migawka i udostępniane jako krótki link.

Link nigdy nie nadaje dostępu. Osoba, która go otwiera, musi być członkiem obszaru roboczego z uprawnieniem do odczytu encji i widzi tylko to, na co pozwala jej rola.

## Działania na wielu encjach

**Wybierz encje** włącza tryb zaznaczania. Zaznacz do 50 wczytanych encji, aby:

- wysłać je do [rozmowy z agentem](/pl/guides/agents/) wraz z instrukcjami;
- uruchomić działania zbiorcze udostępniane przez zainstalowane rozszerzenia.

## Otwieranie encji

Kliknij wynik, aby otworzyć jego podgląd. Stamtąd możesz edytować encję, zobaczyć jej historię lub otworzyć powiązane encje. Zobacz [Praca z encjami](/pl/guides/entities/).
