---
title: Przeglądanie rekordów
description: Znajduj rekordy za pomocą wyszukiwania, filtrów, faset relacji i sortowania, a następnie zapisuj i udostępniaj wyniki.
---

**Przeglądarka rekordów** to strona główna Attricat. Wyświetla rekordy jednego schematu naraz i pozwala zawęzić je zapytaniem wyszukiwania, filtrami atrybutów i fasetami relacji.

## Wybierz, co chcesz przeglądać

Wybierz schemat z selektora lub ze skrótu na pasku bocznym. Administratorzy obszaru roboczego decydują, które schematy pojawiają się jako skróty na pasku bocznym.

Selektor **Zakres wersji** określa, które wersje schematu widzisz:

- **Aktualny** (domyślnie) pokazuje rekordy w najnowszej opublikowanej wersji. Jeśli istnieją starsze rekordy, komunikat informuje, ile z nich jest ukrytych, i zawiera link do nich.
- Konkretna starsza **Wersja** pokazuje tylko rekordy przypięte do tej wersji.
- **Wszystkie wersje** pokazuje wszystko. Dodaj kolumnę **Schemat** i posortuj według niej, aby zobaczyć najpierw najstarsze rekordy. Zobacz [Wersje i migracja](/pl/builders/revisions/).

## Wyszukiwanie

Wpisz tekst w polu wyszukiwania i naciśnij Enter. Zwykły termin, taki jak `linen`, dopasowuje wartości własnych atrybutów wybranego schematu. Nie przeszukuje powiązanych rekordów.

Aby wyszukiwać szerzej, dodaj kwalifikator do terminu:

| Zapytanie | Znajduje |
| --- | --- |
| `linen` | Rekordy, które mają „linen” w dowolnej ze swoich wartości tekstowych. |
| `sku:ABC*` | Rekordy, których `sku` zaczyna się od `ABC`. |
| `colors:red` | Rekordy powiązane przez `colors` z rekordem, który ma „red” w dowolnej wartości. |
| `colors.name:red` | To samo, ale tylko w `name` powiązanego rekordu. |
| `family.product_type.name:laptop` | Przechodzi przez dwie relacje, a następnie dopasowuje `name`. |
| `*:red` | Rekordy, które mają „red” w swoich wartościach lub w czymkolwiek powiązanym w odległości do trzech kroków. |
| `@id:ID-1,ID-2` | Dokładnie wskazane rekordy. |
| `colors.@id:ID-1,ID-2` | Rekordy powiązane przez `colors` z jednym ze wskazanych rekordów. |

Kilka terminów oddzielonych spacjami musi pasować jednocześnie. Pełną gramatykę opisuje [Składnia wyszukiwania](/pl/guides/search-syntax/).

Otwórz **Informacje o wyszukiwaniu** przy wyniku, aby zobaczyć, dlaczego został dopasowany, na przykład *red przez 1 relację*.

## Filtry

**Dodaj filtr** zawęża wyniki według wartości atrybutu w wybranym [kontekście](#kontekst). Dostępne operatory zależą od typu:

| Typ | Operatory |
| --- | --- |
| Łańcuch znaków | Równa się, Zawiera, Zaczyna się od |
| Liczba, liczba całkowita, data, data i godzina, godzina | Równa się, Większe niż, Większe lub równe, Mniejsze niż, Mniejsze lub równe |
| Wartość logiczna | Równa się |
| Status | Równa się, z wyborem spośród etykiet statusu |
| Użytkownik lub zespół | Równa się osobie lub zespołowi albo **Przypisane do mnie (lub moich zespołów)** |
| Plik | Tylko Obecność wartości |

Każdy typ ma też operator **Obecność wartości**: wybierz **Ma wartość** lub **Brak wartości**. Dla atrybutu plikowego **Ma wartość** znajduje rekordy z co najmniej jednym załączonym plikiem, a **Brak wartości** rekordy bez żadnego pliku.

Możesz też filtrować według atrybutu powiązanego rekordu, na przykład `brand.name`, przez maksymalnie trzy relacje.

Filtr możesz również utworzyć z poziomu tabeli: otwórz menu komórki i wybierz **Filtruj**.

Każdy aktywny filtr jest widoczny jako etykieta pod polem wyszukiwania. Wszystkie filtry, fasety i zapytanie wyszukiwania muszą pasować jednocześnie.

## Fasety relacji

Każdy atrybut relacji schematu ma fasetę na pasku bocznym. Wybierz jeden lub więcej celów, aby zachować tylko rekordy z nimi powiązane.

Gdy schemat docelowy ma relację do samego siebie, na przykład `category.parent`, faseta staje się drzewem:

- Liczniki przy każdym węźle obejmują wszystko, co znajduje się pod nim. Rekord przypisany do dwóch podkategorii jest liczony raz.
- Wybranie węzła nadrzędnego dopasowuje też rekordy przypisane do jego potomków.
- Kilka zaznaczeń w jednej fasecie oznacza *dowolny z nich*. Zaznaczenia w różnych fasetach muszą pasować jednocześnie.
- Liczniki uwzględniają zapytanie wyszukiwania i zakres wersji, ale nie własne zaznaczenie fasety, więc po wybraniu węzła nadal widzisz liczniki węzłów sąsiednich. Filtry atrybutów nie wpływają na liczniki faset.

**Opcje drzewa** pozwalają wybrać, które pole odwołujące się do tego samego schematu buduje drzewo i w którym kontekście rozwiązywane są relacje. Zmiana dowolnej z tych opcji czyści zaznaczenie fasety.

## Kontekst

Selektor **Kontekst** pokazuje wartości rozstrzygnięte w danym kontekście: własną wartość kontekstu, jeśli ją ma, a w przeciwnym razie wartość dziedziczoną. Zobacz [Konteksty](/pl/guides/contexts/).

Kolumny tabeli, etykieta rekordu, filtry i sortowanie korzystają z wybranego kontekstu. Na przykład po wybraniu kontekstu `pl` przetłumaczona nazwa jest wyświetlana, filtrowana i sortowana po polsku, a rekord bez polskiej nazwy pokazuje nazwę dziedziczoną. Wyszukiwanie tekstowe dopasowuje wartości we wszystkich kontekstach.

Jeśli wybrany kontekst jest [kanałem publikacji](/pl/guides/publishing/), tabela zyskuje kolumnę **Publikacja**, która pokazuje, czy każdy rekord jest w nim opublikowany. Można ją posortować, aby nieopublikowane rekordy znalazły się na początku.

## Sortowanie i układ kolumn

Kliknij nagłówek kolumny, aby sortować. Kolumnę można sortować, gdy zawiera jedną wartość skalarną na rekord. Kolumny, które podążają za relacją wielowartościową, są wyświetlane, ale nie można ich sortować.

W widoku **Wszystkie wersje** sortowanie według powiązanej wartości działa tylko wtedy, gdy wszystkie pasujące rekordy są w tej samej wersji.

**Kolumny** pozwalają pokazywać, ukrywać i zmieniać kolejność kolumn. Układ jest zapisywany w przeglądarce.

## Zapisywanie i udostępnianie wyszukiwań

Wszystko, co ustawisz w Przeglądarce, jest przechowywane w adresie URL strony, więc przeładowanie strony lub wysłanie linku odtwarza ten stan.

- **Zapisz wyszukiwanie** zapisuje wyszukiwanie pod nazwą. Wybierz **Prywatne**, aby zachować je dla siebie, albo **Obszar roboczy**, aby udostępnić je każdemu członkowi, który może odczytywać rekordy. Zapisane wyszukiwania otwierasz z listy **Zapisane wyszukiwania**.
- Zmiana zapisanego wyszukiwania oznacza je jako mające niezapisane zmiany. **Zapisz zmiany** je aktualizuje, a **Zapisz wyszukiwanie** zapisuje kopię.
- [Pakiet rozwiązania](/pl/builders/solution-packs/#reguły-przepływy-pracy-i-zapisane-wyszukiwania) może dodać gotowe wyszukiwania z widocznością **Obszar roboczy**, na przykład kolejki do przeglądu. Pojawiają się na tej samej liście, a ich właścicielem jest osoba, która zastosowała pakiet.
- **Udostępnij wyszukiwanie** kopiuje link. Długie wyszukiwania są zapisywane jako migawka i udostępniane jako krótki link.

Link nigdy nie nadaje dostępu. Osoba, która go otwiera, musi być członkiem obszaru roboczego z uprawnieniem do odczytu rekordów i widzi tylko to, na co pozwala jej rola.

## Działania na wielu rekordach

**Wybierz rekordy** włącza tryb zaznaczania. Możesz zaznaczyć do 50 rekordów. Zaznaczenie zostaje po zmianie zapytania, sortowania lub filtrów, więc możesz zebrać rekordy z kilku wyszukiwań tego samego schematu. Kliknij **Wybrano N rekordów**, aby przejrzeć listę i usunąć z niej rekordy.

Menu **Akcje** działa na zaznaczeniu:

- **Wyślij do rozmowy z agentem** wysyła rekordy do [rozmowy z agentem](/pl/guides/agents/) wraz z instrukcjami.
- **Utwórz zapisane wyszukiwanie** zapisuje wyszukiwanie obejmujące dokładnie te rekordy.
- Zainstalowane rozszerzenia mogą dodać własne działania zbiorcze.

## Akcje i uruchomienia rozszerzeń

Rozszerzenie może udostępnić akcję, na przykład generowanie dokumentów, w podglądzie rekordu, w menu wiersza lub dla bieżącego zaznaczenia. Takie akcje zawsze używają zapisanych danych. Jeśli w tej samej karcie przeglądarki masz niezapisane zmiany zaznaczonego rekordu, okno akcji o tym informuje i prowadzi do edytora, abyś mógł je najpierw zapisać.

Uruchomienie akcji tworzy zadanie działające w tle. Możesz zamknąć okno lub opuścić stronę; uruchomienie trwa dalej, a po jego zakończeniu zobaczysz powiadomienie. Otwórz **Profil → Uruchomienia rozszerzeń**, aby śledzić swoje uruchomienia, anulować trwające i pobrać ich wyniki. Status pokazuje, czy uruchomienie się zakończyło; rozszerzenie osobno podaje, ile rekordów przetworzono pomyślnie, ile się nie udało, a ile pominięto.

Twoje uruchomienia widzisz tylko Ty oraz członkowie zarządzający rozszerzeniami. Wyniki możesz pobrać tylko wtedy, gdy nadal masz dostęp do odczytu każdego rekordu z uruchomienia. Wyniki są dostępne po zakończeniu uruchomienia i przechowywane przez 30 dni.

## Otwieranie rekordu

Kliknij wynik, aby otworzyć jego podgląd. Stamtąd możesz edytować rekord, zobaczyć jego historię lub otworzyć powiązane rekordy. Zobacz [Praca z rekordami](/pl/guides/records/).
