---
title: Praca z encjami
description: Twórz, edytuj i usuwaj encje, pracuj z wartościami kontekstowymi, plikami i relacjami oraz korzystaj z historii zmian.
---

Encja to jeden rekord katalogu, na przykład produkt lub kategoria. Jej schemat określa, jakie ma atrybuty, jak są one walidowane i jak wygląda układ jej strony.

## Tworzenie encji

W Przeglądarce wybierz **Utwórz encję** (lub **Utwórz: *Schemat***, gdy wybrany jest schemat). Wypełnij formularz i zapisz. Nowe encje zawsze korzystają z bieżącej opublikowanej wersji schematu.

Wartości wpisane podczas tworzenia encji są zapisywane w kontekście domyślnym. Atrybuty z `default_value` są od początku wypełnione.

**Duplikuj encję** przy istniejącej encji otwiera formularz tworzenia wstępnie wypełniony jej wartościami.

## Edycja wartości

Otwórz encję i wybierz **Edytuj encję**. Formularz pokazuje wartości dla jednego kontekstu naraz; przełączaj je selektorem **Kontekst**.

Formularz informuje, skąd pochodzi każda wartość:

- Wartość wpisana bezpośrednio w tym kontekście jest jego własną wartością.
- **Dziedziczone z kontekstu *X*** oznacza, że ten kontekst nie ma wartości i pokazuje wartość przodka. Wpisanie wartości tworzy nadpisanie tylko dla tego kontekstu. Wyczyszczenie nadpisania przywraca wartość dziedziczoną.
- **Zarządzane w kontekście domyślnym** oznacza, że atrybut można edytować tylko w kontekście domyślnym.
- **Zarządzane przez działania systemowe** oznacza, że atrybut jest `readonly`: utrzymuje go integracja, przepływ pracy lub agent.
- **Zablokowane, gdy rekord ma status *X*** oznacza, że status rekordu zamraża to pole. Zobacz [Rekordy kontrolowane](#rekordy-kontrolowane).

Zapis waliduje całą encję w każdym kontekście. Jeśli zmiana unieważniłaby którykolwiek kontekst, nic nie zostaje zapisane, a formularz wskazuje problem. Zobacz [Walidacja](/pl/builders/validation/).

Schemat może też zawierać kontrole, na przykład „data ważności nie może być wcześniejsza niż data rozpoczęcia” albo „każdy zakład należy do tego dostawcy”, a obszar roboczy może egzekwować reguły jakości danych. Gdy któraś z nich nie przejdzie, komunikat podaje nazwę kontroli lub reguły i opisuje problem. Zmień wskazane pola w wymienionych kontekstach i zapisz ponownie. Jeśli komunikat dotyczy powiązanego rekordu, na przykład niezatwierdzonego zakładu, najpierw popraw lub zastąp ten rekord.

### Niezapisane szkice

Formularz edycji przechowuje niezapisane zmiany na czas trwania karty przeglądarki. Jeśli przeładujesz stronę lub do niej wrócisz, Attricat zaproponuje **Przywróć szkic** lub **Odrzuć szkic**. Szkic jest usuwany po zapisaniu. Szkice nigdy nie zawierają haseł ani plików.

## Zmiana statusu

Pole statusu, na przykład *Szkic*, *W przeglądzie* lub *Wydany*, to lista wyboru. Schemat może ograniczać, które statusy mogą nastąpić po bieżącym, i określać warunki zmiany. Statusu może nie dać się wybrać lub zapisać z jednego z tych powodów:

- **Opcja jest wyłączona.** Schemat nie pozwala na tę zmianę z bieżącego statusu. Na przykład wydany dokument może najpierw wymagać powrotu do *Szkicu*, zanim znów trafi do przeglądu. Przechodź przez dozwolone statusy.
- **Opcja jest wyłączona i pokazuje *Zablokowane*** wraz z niespełnionymi warunkami. Zmiana jest dozwolona, ale zapisana encja nie spełnia jeszcze jej warunków. Uzupełnij to, czego wymagają, zapisz, a potem wybierz status.
- **Zapis zostaje odrzucony** z komunikatem takim jak *status transition conditions are not met: Set an approver before release (approver-set)*. Zmiana jest dozwolona, ale zapisywane wartości nie spełniają jej warunków. Podobny komunikat zaczynający się od *enforcing rules are violated* oznacza, że zmianę chroni reguła jakości danych. Nic nie zostaje zapisane.

Aby to naprawić, przeczytaj wymienione warunki i uzupełnij to, czego wymagają. Brakujące wartości możesz podać w tym samym zapisie co zmianę statusu: uzupełnij osobę zatwierdzającą i jednocześnie wybierz *Wydany*. Niektóre warunki zależą od innych rekordów, np. „każde działanie korygujące jest zamknięte”; najpierw zaktualizuj te rekordy, a potem ponownie zmień status.

Warunki są sprawdzane w każdym kontekście, w którym zmienia się status. Jeśli status jest dziedziczony, zmiana w kontekście domyślnym musi spełniać warunki również w każdym kontekście potomnym.

Aby sprawdzić, które statusy są dostępne i dlaczego inne są zablokowane, bez próby zapisu, wywołaj `GET /v1/entities/{id}/status-transitions` z opcjonalnym `context_id`. Każde przejście ma pola `allowed`, `denial_reason` (gdy jest zablokowane) i `unmet` z niespełnionymi warunkami.

## Relacje

Pola relacji otwierają okno wyboru. Wyszukaj cele, **Wybierz** je i kliknij **Zastosuj**. Relacje jednokrotnego wyboru zastępują bieżący cel. Podgląd każdej opcji można otworzyć w nowej karcie.

Wartości relacji są kontekstowe jak każda inna wartość: produkt może mieć inny zestaw kategorii na jednym rynku.

## Pliki

Pola plików przyjmują przesłane pliki po zapisaniu encji. Wybierz lub upuść pliki; każdy z nich jest sprawdzany pod kątem reguł atrybutu dotyczących typu, rozszerzenia i rozmiaru.

Nowy plik przechodzi przez następujące stany: **Przesyłanie**, **W kolejce**, **Przetwarzanie** i **Gotowy**. Obrazy otrzymują miniaturę i wersję w rozmiarze do wyświetlania. Plik można pobrać, gdy jest gotowy. Jeśli przetwarzanie się nie powiedzie, plik pokazuje stan **Niepowodzenie**; administrator może ponowić próbę.

Pola plików przyjmujące tylko obrazy wyświetlają galerię. Wybierz obraz, aby otworzyć większy podgląd z powiększaniem i przechodzeniem do poprzedniego lub następnego obrazu. W formularzu edycji możesz dodawać i usuwać obrazy oraz przesuwać je wcześniej lub później, jeśli kolejność plików atrybutu ma znaczenie. Usunięcie obrazu odłącza go od encji.

## Rekordy kontrolowane

Niektóre schematy używają statusów do kontrolowania cyklu życia rekordu. Wtedy:

- Lista wyboru statusu wyłącza przejścia, których nie możesz wykonać, i wyjaśnia dlaczego: przejście wymaga uprawnienia lub roli, której nie masz, albo musi je wykonać ktoś inny (np. osoba, która przesłała dokument do przeglądu, nie może go zatwierdzić).
- W statusie ostatecznym, takim jak *Released*, część pól lub wszystkie są tylko do odczytu, nie można dodawać ani usuwać plików, a encji nie można usunąć. Aby poprawić rekord, zmień jego status przejściem korygującym przewidzianym w schemacie, zapisz, a potem edytuj. Korekta jest zapisywana w dzienniku audytu.
- Gdy rekord zostaje zatwierdzony, zatwierdzenie jest powiązane z dokładnie tą treścią, którą przejrzano. Edycja tej treści unieważnia zatwierdzenie, a rekord w tym samym zapisie wraca do wcześniejszego statusu.
- Pliki sfinalizowanych rekordów mogą być objęte blokadą retencji do określonej daty.

Panel **Kontrola rekordu** na stronie encji wyświetla zatwierdzenia wraz z informacją, kto i kiedy zatwierdził oraz czy zatwierdzenie jest nadal ważne, a także blokady retencji plików encji.

## Komentarze

Strona encji ma panel **Komentarze**. Każdy, kto może odczytać encję, może dodać komentarz w Markdownie; **Podgląd Markdown** pokazuje, jak będzie wyglądał. Możesz edytować tylko własne komentarze. Jeśli komentarz zmieni się, zanim zapiszesz edycję, zostanie ona odrzucona; odśwież komentarze i przywróć szkic, aby uzgodnić tekst.

## Dodatkowe atrybuty

Niektóre encje potrzebują pola, którego nie ma ich schemat. Jeśli obszar roboczy ma opublikowane [atrybuty wielokrotnego użytku](/pl/builders/modeling/#wielokrotne-użycie-atrybutów), wybierz **Dodaj niestandardowy atrybut lub grupę atrybutów** w formularzu edycji, aby dołączyć atrybut lub całą grupę tylko do tej encji.

## Inteligentne wypełnianie

Gdy agenci są włączeni, **Inteligentne wypełnianie** w formularzu edycji przyjmuje wklejony tekst, na przykład opis produktu od dostawcy, i proponuje wartości dla bieżącego kontekstu. Propozycje trafiają do formularza; przejrzyj je i zapisz jak zwykle. Nic nie jest zapisywane automatycznie.

## Historia

**Zmiany** zawierają listę wszystkich zmian encji: co się zmieniło, w którym kontekście, kiedy i kto to zrobił. Zmiany wprowadzone przez agenta pokazują agenta i osobę, która je zatwierdziła.

Poprzednie wartości atrybutów są domyślnie przechowywane przez 90 dni. Z historii wartości możesz przywrócić wcześniejszą wartość; przywrócenie jest zapisywane jako nowa zmiana.

## Ustalenia dotyczące jakości danych

Jeśli [reguły jakości danych](/pl/builders/rules/) oznaczą encję, jej strona pokazuje liczbę otwartych ustaleń.

## Aktualizacja do nowszej wersji

Gdy schemat ma nowszą opublikowaną wersję, encja pokazuje komunikat **Schemat jest nieaktualny** i oferuje **Zaktualizuj encję**. Strona aktualizacji porównuje wartości encji z nową wersją i prosi o uzupełnienie brakujących danych. Zobacz [Wersje i migracja](/pl/builders/revisions/).

## Publikowanie

Jeśli obszar roboczy korzysta z kanałów publikacji, strona encji pokazuje jej stan w każdym kanale oraz działania **Opublikuj** / **Cofnij publikację**. Zobacz [Publikowanie](/pl/guides/publishing/).

## Pytanie do agenta

**Zapytaj o tę encję** rozpoczyna [rozmowę z agentem](/pl/guides/agents/) o encji, którą właśnie przeglądasz.

## Usuwanie encji

**Usuń encję** jest dostępne z uprawnieniem `entities.delete` na pasku narzędzi encji lub w menu wiersza w Przeglądarce. Usunięta encja znika z wyszukiwania, podglądów i okien wyboru relacji. Jej historia zostaje zachowana. Rekordu w zablokowanym statusie nie można usunąć.

## Z wiersza poleceń

Wszystko, co opisuje ta strona, jest też dostępne przez [CLI](/pl/reference/cli/) i [API](/pl/reference/api/). Na przykład:

```sh
acli entity create --blueprint product --values product.toml
acli entity update <entity-id> --values changes.toml --context-id <context-id>
acli entity value-history <entity-id>
acli entity restore-value <entity-id> <history-id>
acli file upload <entity-id> main_photo --file photo.jpg
```
