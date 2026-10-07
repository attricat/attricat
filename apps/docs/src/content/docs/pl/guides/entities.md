---
title: Praca z encjami
description: Twórz, edytuj i usuwaj encje, pracuj z wartościami kontekstowymi, plikami i relacjami oraz korzystaj z historii zmian.
---

Encja to jeden rekord katalogu, na przykład produkt lub kategoria. Jej schemat określa, jakie ma atrybuty, jak są one walidowane i jak wygląda układ jej strony.

## Tworzenie encji

W Przeglądarce wybierz **Utwórz encję** (lub **Utwórz: *Schemat***, gdy wybrany jest schemat). Wypełnij formularz i zapisz. Nowe encje zawsze korzystają z bieżącej opublikowanej wersji schematu.

Formularz tworzenia przechowuje niezapisane wartości na czas trwania karty przeglądarki. Jeśli przeładujesz stronę lub do niej wrócisz, Attricat zaproponuje **Przywróć szkic** lub **Odrzuć szkic**. Szkic jest usuwany po zapisaniu. Szkice nigdy nie zawierają haseł ani plików.

Wartości wpisane podczas tworzenia encji są zapisywane w kontekście domyślnym. Atrybuty z `default_value` są od początku wypełnione.

**Duplikuj encję**, w menu **Akcje** encji lub w menu wiersza w Przeglądarce, prosi o potwierdzenie, a następnie tworzy kopię z wartościami, relacjami i plikami encji i otwiera kopię. Wartości [kluczy unikalnych](/pl/reference/blueprint/#klucze-unikalne) schematu nie są kopiowane, ponieważ kopia nie może ich współdzielić z oryginałem: dla klucza unikalnego w całym obszarze roboczym pomijana jest wartość z kontekstu domyślnego, a dla klucza unikalnego w kontekście – wartości ze wszystkich kontekstów. Uzupełnij je w kopii. Jeśli atrybut klucza jest wymagany, duplikowanie kończy się błędem walidacji.

## Edycja wartości

Encję edytujesz bezpośrednio na jej stronie; nie ma osobnego formularza edycji. Każde pole, które możesz zmienić, jest edytowalną kontrolką w układzie schematu i pokazuje wartości dla jednego kontekstu naraz. Przełączaj je selektorem **Kontekst**. Jeśli nie możesz edytować encji, widzisz tylko jej wartości.

Każde pole zapisuje się osobno:

- Pola tekstowe, liczbowe i daty zapisują się, gdy opuścisz pole. Pola jednowierszowe zapisują się też po naciśnięciu Enter.
- Listy wyboru, pola wyboru, statusy, pola użytkowników i zespołów, relacje i pliki zapisują się od razu po zmianie.
- Escape przywraca zapisaną wartość.

Każde zapisane pole to osobna zmiana z własnym wpisem w historii i dzienniku audytu. Jeśli encja jest opublikowana, pierwsze zapisane pole cofa jej publikacje, chyba że Twoja rola je zachowuje; zobacz [Publikowanie](/pl/guides/publishing/#co-cofa-publikację).

Pola wyświetlane w nagłówku strony są dostępne do edycji na samej górze. Edytowalne pola, których układ nie pokazuje, znajdują się w sekcji **Pozostałe atrybuty**.

Każde pole informuje, skąd pochodzi jego wartość:

- Wartość wpisana bezpośrednio w tym kontekście jest jego własną wartością.
- **Dziedziczone z kontekstu *X*** oznacza, że ten kontekst nie ma wartości i pokazuje wartość przodka. Wpisanie wartości tworzy nadpisanie tylko dla tego kontekstu. Wyczyszczenie nadpisania przywraca wartość dziedziczoną.
- **Zarządzane w kontekście domyślnym** oznacza, że atrybut można edytować tylko w kontekście domyślnym.
- **Zarządzane przez działania systemowe** oznacza, że atrybut jest `readonly`: utrzymuje go integracja, przepływ pracy lub agent.
- **Zablokowane, gdy rekord ma status *X*** oznacza, że status rekordu zamraża to pole. Zobacz [Rekordy kontrolowane](#rekordy-kontrolowane).

Każdy zapis waliduje całą encję w każdym kontekście. Jeśli zmiana unieważniłaby którykolwiek kontekst, nie zostaje zapisana: pozostaje w polu wraz z błędem, a strona pokazuje, ile zmian nie jest jeszcze zapisanych. Odrzucone zmiany są wysyłane ponownie z następnym zapisywanym polem, więc kilka brakujących wymaganych wartości możesz uzupełniać po kolei. Escape w polu porzuca jego odrzuconą zmianę. Zobacz [Walidacja](/pl/builders/validation/).

Dopóki są niezapisane zmiany, selektor **Kontekst** jest wyłączony.

Schemat może też zawierać kontrole, na przykład „data ważności nie może być wcześniejsza niż data rozpoczęcia” albo „każdy zakład należy do tego dostawcy”, a obszar roboczy może egzekwować reguły jakości danych. Gdy któraś z nich nie przejdzie, komunikat podaje nazwę kontroli lub reguły i opisuje problem. Zmień wskazane pola w wymienionych kontekstach. Jeśli komunikat dotyczy powiązanego rekordu, na przykład niezatwierdzonego zakładu, najpierw popraw lub zastąp ten rekord.

Jeśli ktoś inny zmieni encję, gdy masz niezapisane zmiany, ostrzeżenie zaproponuje **Zachowaj moje zmiany** lub **Użyj najnowszych wartości**.

## Zmiana statusu

Pole statusu, na przykład *Szkic*, *W przeglądzie* lub *Wydany*, to lista wyboru. Schemat może ograniczać, które statusy mogą nastąpić po bieżącym, i określać warunki zmiany. Statusu może nie dać się wybrać lub zapisać z jednego z tych powodów:

- **Opcja jest wyłączona.** Schemat nie pozwala na tę zmianę z bieżącego statusu. Na przykład wydany dokument może najpierw wymagać powrotu do *Szkicu*, zanim znów trafi do przeglądu. Przechodź przez dozwolone statusy.
- **Opcja jest wyłączona i pokazuje *Zablokowane*** wraz z niespełnionymi warunkami. Zmiana jest dozwolona, ale zapisana encja nie spełnia jeszcze jej warunków. Uzupełnij to, czego wymagają, a potem wybierz status.
- **Zmiana zostaje odrzucona** z komunikatem takim jak *status transition conditions are not met: Set an approver before release (approver-set)*. Zmiana jest dozwolona, ale zapisywane wartości nie spełniają jej warunków. Podobny komunikat zaczynający się od *enforcing rules are violated* oznacza, że zmianę chroni reguła jakości danych. Nic nie zostaje zapisane.

Aby to naprawić, przeczytaj wymienione warunki i uzupełnij to, czego wymagają. Odrzucony status pozostaje w polu i jest wysyłany ponownie z następnym zapisywanym polem, więc możesz wybrać *Wydany*, a potem uzupełnić osobę zatwierdzającą. Niektóre warunki zależą od innych rekordów, np. „każde działanie korygujące jest zamknięte”; najpierw zaktualizuj te rekordy, a potem ponownie zmień status.

Warunki są sprawdzane w każdym kontekście, w którym zmienia się status. Jeśli status jest dziedziczony, zmiana w kontekście domyślnym musi spełniać warunki również w każdym kontekście potomnym.

Integracje mogą sprawdzić, które statusy są dostępne, bez próby zapisu; zobacz [Warunki przejść](/pl/builders/validation/#warunki-przejść).

## Relacje

Pola relacji otwierają okno wyboru. Wyszukaj cele, **Wybierz** je i kliknij **Zastosuj**. Relacje jednokrotnego wyboru zastępują bieżący cel. Podgląd każdej opcji można otworzyć w nowej karcie.

Wartości relacji są kontekstowe jak każda inna wartość: produkt może mieć inny zestaw kategorii na jednym rynku.

## Pliki

Pola plików przyjmują przesłane pliki po zapisaniu encji. Wybierz lub upuść pliki; każdy z nich jest sprawdzany pod kątem reguł atrybutu dotyczących typu, rozszerzenia i rozmiaru.

Nowy plik przechodzi przez następujące stany: **Przesyłanie**, **W kolejce**, **Przetwarzanie** i **Gotowy**. Obrazy otrzymują miniaturę i wersję w rozmiarze do wyświetlania. Plik można pobrać, gdy jest gotowy. Jeśli przetwarzanie się nie powiedzie, plik pokazuje stan **Niepowodzenie**; administrator może ponowić próbę.

Pola plików przyjmujące tylko obrazy wyświetlają galerię. Wybierz obraz, aby otworzyć większy podgląd z powiększaniem i przechodzeniem do poprzedniego lub następnego obrazu. Jeśli możesz edytować encję, możesz dodawać i usuwać obrazy oraz przesuwać je wcześniej lub później, jeśli kolejność plików atrybutu ma znaczenie. Usunięcie obrazu odłącza go od encji.

## Rekordy kontrolowane

Niektóre schematy używają statusów do kontrolowania cyklu życia rekordu. Wtedy:

- Lista wyboru statusu wyłącza przejścia, których nie możesz wykonać, i wyjaśnia dlaczego: przejście wymaga uprawnienia lub roli, której nie masz, albo musi je wykonać ktoś inny (np. osoba, która przesłała dokument do przeglądu, nie może go zatwierdzić).
- W statusie ostatecznym, takim jak *Wydany*, część pól lub wszystkie są tylko do odczytu, nie można dodawać ani usuwać plików, a encji nie można usunąć. Aby poprawić rekord, zmień jego status przejściem korygującym przewidzianym w schemacie, a potem edytuj. Korekta jest zapisywana w dzienniku audytu.
- Gdy rekord zostaje zatwierdzony, zatwierdzenie jest powiązane z dokładnie tą treścią, którą przejrzano. Edycja tej treści unieważnia zatwierdzenie, a rekord w tym samym zapisie wraca do wcześniejszego statusu.
- Pliki sfinalizowanych rekordów mogą być objęte blokadą retencji do określonej daty.

Panel **Kontrola rekordu** na stronie encji wyświetla zatwierdzenia wraz z informacją, kto i kiedy zatwierdził oraz czy zatwierdzenie jest nadal ważne, a także blokady retencji plików encji.

## Komentarze

Strona encji ma panel **Komentarze**. Każdy, kto może odczytać encję, może dodać komentarz w Markdownie; **Podgląd Markdown** pokazuje, jak będzie wyglądał. Możesz edytować tylko własne komentarze. Jeśli komentarz zmieni się, zanim zapiszesz edycję, zostanie ona odrzucona; odśwież komentarze i przywróć szkic, aby uzgodnić tekst.

## Dodatkowe atrybuty

Niektóre encje potrzebują pola, którego nie ma ich schemat. Jeśli obszar roboczy ma opublikowane [atrybuty wielokrotnego użytku](/pl/builders/modeling/#wielokrotne-użycie-atrybutów), wybierz **Dodaj niestandardowy atrybut lub grupę atrybutów** pod polami, aby dołączyć atrybut lub całą grupę tylko do tej encji. Dołączone atrybuty pojawiają się w sekcji **Dodatkowe atrybuty**.

## Inteligentne wypełnianie

Gdy agenci są włączeni i możesz edytować encję, otwórz **Zapytaj o tę encję** na pasku narzędzi encji i wklej tekst, na przykład opis produktu od dostawcy. Agent zaproponuje wartości dla bieżącego kontekstu. Nic nie zostaje zapisane, dopóki nie zastosujesz propozycji; zastosowanie zapisuje wszystkie proponowane wartości jako jedną zmianę.

## Historia

**Zmiany**, w menu **Akcje** encji, zawierają listę wszystkich zmian encji: co się zmieniło, w którym kontekście, kiedy i kto to zrobił. Zmiany wprowadzone przez agenta pokazują agenta i osobę, która je zatwierdziła.

Poprzednie wartości atrybutów są domyślnie przechowywane przez 90 dni. Z historii wartości możesz przywrócić wcześniejszą wartość; przywrócenie jest zapisywane jako nowa zmiana.

## Ustalenia dotyczące jakości danych

Jeśli [reguły jakości danych](/pl/builders/rules/) oznaczą encję, jej strona pokazuje liczbę otwartych ustaleń.

## Aktualizacja do nowszej wersji

Gdy schemat ma nowszą opublikowaną wersję, ikona ostrzeżenia obok odnośnika do schematu u góry strony encji informuje, że **Schemat jest nieaktualny**, a menu **Akcje** oferuje **Zaktualizuj schemat**. Strona aktualizacji porównuje wartości encji z nową wersją i prosi o uzupełnienie brakujących danych. Zobacz [Wersje i migracja](/pl/builders/revisions/).

## Publikowanie

Jeśli obszar roboczy korzysta z kanałów publikacji, strona encji pokazuje jej stan w każdym kanale oraz działania **Opublikuj** / **Cofnij publikację**. Zobacz [Publikowanie](/pl/guides/publishing/).

## Pytanie do agenta

**Zapytaj o tę encję** rozpoczyna [rozmowę z agentem](/pl/guides/agents/) o encji, którą właśnie przeglądasz.

## Usuwanie encji

**Usuń encję** jest dostępne z uprawnieniem `entities.delete` w menu **Akcje** encji lub w menu wiersza w Przeglądarce i wymaga potwierdzenia. Usunięta encja znika z wyszukiwania, podglądów i okien wyboru relacji. Jej historia zostaje zachowana. Rekordu w zablokowanym statusie nie można usunąć.

## Z wiersza poleceń

Wszystko, co opisuje ta strona, jest też dostępne przez [CLI](/pl/reference/cli/) i [API](/pl/reference/api/). Na przykład:

```sh
acli entity create --blueprint product --values product.toml
acli entity update <entity-id> --values changes.toml --context-id <context-id>
acli entity value-history <entity-id>
acli entity restore-value <entity-id> <history-id>
acli file upload <entity-id> main_photo --file photo.jpg
```
