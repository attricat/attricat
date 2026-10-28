---
title: Reguły jakości danych
description: Definiuj wersjonowane kontrole, które oznaczają rekordy z brakującymi, nieaktualnymi lub błędnie otagowanymi danymi, i zarządzaj ustaleniami.
---

Reguła sprawdza rekordy jednego Schematu pod kątem prostego warunku i zapisuje **ustalenie** dla każdego rekordu, który go nie spełnia. Ustalenia rozwiązują się same, gdy rekord zostanie poprawiony. Reguły tylko odczytują dane; nigdy nie zmieniają rekordów.

Używaj reguł do pytań w rodzaju „które opublikowane produkty nie mają tytułu?” albo „których cen nikt nie zmieniał od roku?”. Aby zmieniać dane automatycznie, użyj [przepływu pracy](/pl/builders/workflows/).

## Zdefiniuj regułę

Reguła ma kod, nazwę, wagę, od jednego do ośmiu wyzwalaczy i dokładnie jeden predykat.

```toml
format_version = 1
code = "title-required"
name = "Products have a title"
description = "Shoppers search by title, so every product needs one before it goes live."
severity = "error"

[[triggers]]
type = "schedule"
cron = "0 0 6 * * *"
timezone = "UTC"

[[triggers]]
type = "event"
event_type = "record.updated.v1"

[[triggers]]
type = "manual"

[predicate]
type = "required"
attribute_code = "title"
```

| Klucz | Opis |
| --- | --- |
| `format_version` | `1`. |
| `code` | Unikalny [kod](/pl/reference/blueprint/#kody). |
| `name` | Nazwa wyświetlana. |
| `description` | Opcjonalny. Co reguła sprawdza i dlaczego, prostymi słowami, do 2000 znaków. Jest wyświetlany na stronie reguły, na liście reguł, w podglądzie edytora i przy ustaleniach reguły na każdym rekordzie, aby osoby je poprawiające wiedziały, czego się oczekuje. Podziały wierszy są zachowywane; Markdown nie jest renderowany. |
| `severity` | `info`, `warning`, `error` lub `critical`. |
| `triggers` | Od 1 do 8 wyzwalaczy. Zobacz niżej. |
| `predicate` | Warunek, który rekord musi spełnić. Zobacz niżej. |

### Wyzwalacze

| `type` | Klucze | Uruchamia się |
| --- | --- | --- |
| `manual` | | Gdy ktoś wybierze **Uruchom teraz**. |
| `schedule` | `cron`, `timezone = "UTC"` | Według sześciopolowego harmonogramu cron (sekundy na początku), w UTC. `0 0 6 * * *` oznacza codziennie o 06:00. |
| `event` | `event_type` | Dla zmienionego rekordu, po jednym z: `record.created.v1`, `record.updated.v1`, `record.migrated.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1`, `relationship.changed.v1`. |
| `post_import` | | Zarezerwowany dla przyszłej integracji z importem. |

Wyzwalacze decydują, kiedy reguła sprawdza wszystkie rekordy swojego Schematu. Edycja rekordu zawsze ponownie sprawdza ten rekord względem każdej włączonej reguły jego Schematu, niezależnie od jej wyzwalaczy, więc ustalenia rekordu odpowiadają temu, co zapisano. Wyzwalacz `event` dodaje ponowne uruchomienia dla [zależnych rekordów](#zmiany-w-powiązanych-rekordach).

### Predykaty

Rekord **nie spełnia** reguły, gdy predykat nie jest spełniony. Reguły używają tych samych predykatów co [kontrole rekordów](/pl/builders/validation/#porównuj-atrybuty-za-pomocą-kontroli), [warunki przejść statusu](/pl/builders/validation/#warunki-przejść) i [kontrole kanałów publikacji](/pl/guides/publishing/#wymagaj-kontroli-przed-publikacją). Wszystkie klucze opisuje [dokumentacja Schematu](/pl/reference/blueprint/#predykaty).

- **Wartości:** `required`, `compare` (z innym atrybutem lub stałą wartością), `one_of` i `relative_date` (względem chwili obecnej przesuniętej o `offset_days`).
- **Tagi:** `has_tag` i `missing_tag` sprawdzają tagi systemowe.
- **Powiązane rekordy:** `linked` sprawdza rekordy, z którymi sprawdzany rekord jest powiązany; `referenced_by` liczy rekordy, które go wskazują.
- **Tylko zgłaszanie ustaleń:** `stale` (brak aktualizacji w zadanym czasie), `unique` (żaden inny rekord Schematu nie ma tych samych wartości) i `acyclic` (relacja nigdy nie prowadzi z powrotem do rekordu).
- **Kombinacje:** `all_of` i `any_of`.

Porównanie z brakującą wartością jest spełnione; to samo dotyczy `one_of` i `relative_date`, gdy atrybut jest pusty. Połącz je z `required`, jeśli wartość musi istnieć:

```toml
[predicate]
type = "all_of"
predicates = [
  { type = "required", attribute_code = "valid_until" },
  { type = "relative_date", attribute_code = "valid_until", op = "gt", offset_days = 30 },
]
```

`linked` i `referenced_by` przechodzą o jeden krok wzdłuż relacji. Ich zagnieżdżony predykat odczytuje drugi rekord i może porównywać go ze sprawdzanym rekordem przez `subject_attribute_code`:

```toml
# Każdy zakład na certyfikacie dostawcy należy do tego dostawcy.
[predicate]
type = "linked"
relationship_code = "facilities"
predicate = { type = "compare", attribute_code = "supplier", op = "eq", subject_attribute_code = "supplier" }
```

```toml
# Niezgodność nie ma otwartych działań korygujących.
[predicate]
type = "referenced_by"
blueprint_code = "corrective_action"
relationship_code = "nonconformance"
max = 0
predicate = { type = "one_of", attribute_code = "state", values = ["open"] }
```

### Wygasanie i inne kontrole zależne od czasu

`relative_date` porównuje datę z chwilą uruchomienia reguły. Nadaj takiej regule wyzwalacz `schedule`: rekord, którego nikt nie edytuje, może wygasnąć w nocy, a zauważy to tylko wykonanie według harmonogramu.

```toml
format_version = 1
code = "certificate-valid-30-days"
name = "Supplier certificates are valid for at least 30 more days"
severity = "warning"

[[triggers]]
type = "schedule"
cron = "0 0 5 * * *"
timezone = "UTC"

[predicate]
type = "relative_date"
attribute_code = "valid_until"
op = "gt"
offset_days = 30
```

Ujemne `offset_days` patrzy wstecz: `op = "gte"` z `offset_days = -365` oznacza „w ciągu ostatniego roku”.

### Konteksty

Reguła powiązana z kontekstem sprawdza rozstrzygnięte wartości rekordu w tym kontekście, łącznie z wartościami dziedziczonymi. Reguła bez kontekstu sprawdza każdy kontekst i nie jest spełniona, jeśli nie przejdzie w którymkolwiek z nich; dowody ustalenia podają kody tych kontekstów w polu `contexts`.

### Zmiany w powiązanych rekordach

`linked` i `referenced_by` zależą od innych rekordów. Gdy zmieni się rekord powiązany lub wskazujący, reguły z wyzwalaczem zdarzenia uruchamiają się też ponownie dla maksymalnie 100 zależnych od niego rekordów, dzięki czemu ich ustalenia pozostają aktualne. Dotyczy to też rekordu, który rekord wskazujący przestaje wskazywać, na przykład gdy działanie korygujące zostanie przeniesione do innej niezgodności, zgodnie z [faktami](/pl/reference/events/#które-zmiany-tworzą-fakty) zdarzenia. Migracja Schematu, która usuwa relację lub zmienia jej cele, nie zapisuje faktów, ale jej zdarzenie `record.migrated.v1` wymienia zwolnione rekordy, więc dla nich reguły również uruchamiają się ponownie. Reguły wyzwalane tylko harmonogramem lub ręcznie zauważą zmianę przy następnym wykonaniu.

Zasady porównywania dla każdego typu oraz limity zagnieżdżania i powiązanych rekordów opisuje [dokumentacja Schematu](/pl/reference/blueprint/#predykaty).

## Gdzie przechowywane są reguły

Reguły tworzy się w **Zarządzanie → Reguły jakości danych**, niezależnie od Schematów. Reguła należy do jednego Schematu i opcjonalnie do jednego kontekstu, a nie do rewizji Schematu. Wybierz **Nowa reguła**, wskaż Schemat i w razie potrzeby kontekst, a następnie napisz TOML w tym samym edytorze co Schematy: podpowiada klucze i kody atrybutów Schematu, oferuje gotowe wyzwalacze i warunki w menu **Wstaw** i na bieżąco sprawdza regułę względem najnowszej opublikowanej rewizji Schematu. Schemat musi mieć opublikowaną rewizję. **Zapisz** tworzy regułę jako wyłączoną.

Lista ma jeden wiersz na regułę z jej Schematem, ważnością, liczbą otwartych i potwierdzonych ustaleń (każda liczba prowadzi do tych ustaleń), bieżącą rewizją i czasem jej zmiany, przełącznikiem **Włączona** oraz przyciskami **Próbne wykonanie** i **Uruchom teraz**. Każda reguła ma własną stronę z kartą **Definicja**, która pokazuje, co reguła sprawdza, jej ważność, liczby ustaleń, bieżącą rewizję, definicję i ostatnie wykonania z odnośnikiem do wszystkich w **Historii wykonań**, oraz kartą **Historia zmian**. Na stronie reguły są też przyciski **Próbne wykonanie**, **Uruchom teraz**, **Włącz** i **Wyłącz**. **Historię wykonań** można zawęzić do jednej reguły.

To samo można zrobić w CLI:

```sh
acli rule create --blueprint-id <uuid> --file title-required.toml
acli rule enable <rule-id>
```

Nowa reguła jest wyłączona i zaczyna działać dopiero po włączeniu przełącznikiem lub w CLI. `acli rule disable` ją zatrzymuje. Wyłączenie reguły rozwiązuje jej ustalenia, więc przestają one obciążać rekordy i oznaczać je jako **Wymaga uwagi**; ponowne włączenie od razu sprawdza każdy rekord jej Schematu.

### Zmień regułę

Reguła ma jedną bieżącą definicję. **Edytuj** na stronie reguły otwiera edytor. Dopóki masz niezapisane zmiany, edytor pokazuje je w porównaniu z bieżącą rewizją i oferuje **Uruchom zmianę próbnie**. Zapisanie zmienionej definicji dodaje nową rewizję, która od razu staje się bieżąca; nie ma szkiców ani publikowania. Kodu reguły nie można zmienić.

```sh
acli rule update <rule-id> --file title-required.toml
```

Po zapisaniu nowej rewizji ustalenia wcześniejszych definicji zostają rozwiązane. Jeśli reguła jest włączona, jej harmonogramy przechodzą na nową rewizję, a reguła od razu ponownie sprawdza wszystkie rekordy swojego Schematu. Zapisanie definicji identycznej z bieżącą kończy się błędem `409 rule_revision_unchanged`.

Karta **Historia zmian** przechowuje wszystkie wcześniejsze rewizje wraz z tym, kto i kiedy je zapisał. **Porównaj z poprzednią** pokazuje rewizję obok poprzedniej, a pola **Od** i **Do** pozwalają porównać obok siebie dowolne dwie rewizje. Adres strony zachowuje wybraną parę, na przykład `?from=2&to=3`, więc porównanie można udostępnić. `acli rule revisions <rule-id>` i `acli rule revision <rule-id> <version>` odczytują tę samą historię. Ustalenia i wykonania podają rewizję reguły, która je utworzyła, na przykład „Products have a title v3”, i prowadzą do niej w historii zmian.

### Reguły i rewizje Schematu

Reguła sprawdza rekordy każdej rewizji swojego Schematu, do której pasuje. Pasuje do rewizji, gdy każdy atrybut wskazany w jej predykacie i chronionych przejściach istnieje w tej rewizji z odpowiednim typem, a w przypadku chronionych przejść istnieją też atrybut statusu i jego kody. Rekordy rewizji, do której reguła nie pasuje, są pomijane: reguła nie zapisuje dla nich ustaleń i niczego nie egzekwuje, a otwarte ustalenie takiego rekordu zostaje rozwiązane. Wykonania liczą te rekordy jako pominięte; karta **Historia wykonań** pokazuje na przykład „3 (pominięto: 1)”.

Zapisanie lub walidacja reguły sprawdza ją tylko względem najnowszej opublikowanej rewizji Schematu. Ikona ostrzeżenia na liście i ostrzeżenie na stronie reguły oznaczają regułę, która pomija niektóre rewizje swojego Schematu, i podają przyczynę.

Reguły nigdy nie blokują publikacji rewizji Schematu. Jeśli włączone reguły nie pasują do publikowanej rewizji, strona Schematu wymienia je w ostrzeżeniu, a każdy inny członek z uprawnieniem `rules.manage` dostaje [powiadomienie](/pl/guides/inbox/) z odnośnikiem do reguły. Zmień regułę tak, aby pasowała do nowej rewizji, albo zostaw ją, jeśli ma sprawdzać tylko starsze rekordy.

## Uruchom regułę

Włączone reguły uruchamiają się według swoich wyzwalaczy. Możesz też uruchomić regułę ręcznie ze strony reguły lub z CLI:

```sh
acli rule run-now <rule-id> --idempotency-key 2026-03-01-audit
acli rule run-now <rule-id> --idempotency-key check-one --record-id <uuid>
acli rule run-now <rule-id> --idempotency-key preview --dry-run
```

Próbne wykonanie (dry run) podaje, ile rekordów nie spełniłoby reguły, wymienia pierwsze 50 z nich wraz z tym, co każdy by zgłosił, i nie zapisuje żadnych ustaleń. **Próbne wykonanie** na liście otwiera wynik w oknie dialogowym; `GET /rule-runs/{id}` zwraca go jako `dry_run_failures`. Działa niezależnie od tego, czy reguła jest włączona; zwykłe wykonanie wymaga włączonej reguły. Ponowne użycie klucza idempotencji zwraca pierwotne wykonanie zamiast rozpoczynać nowe.

Każde wykonanie przetwarza kandydatów stronami po 500 i zatrzymuje się po 10 000. Harmonogram nigdy nie nakłada się sam na siebie: jeśli poprzednie wykonanie wciąż oczekuje, kolejne wystąpienie jest pomijane.

## Egzekwowanie reguły

Domyślnie reguła tylko zgłasza ustalenia. Reguła **egzekwowana** dodatkowo blokuje zapisy, po których rekord naruszałby regułę. Dodaj tabelę `enforcement`:

```toml
format_version = 1
code = "released-documents-approved"
name = "Released documents have an approver"
severity = "error"

[[triggers]]
type = "event"
event_type = "record.updated.v1"

[predicate]
type = "required"
attribute_code = "approved_by"

[enforcement]
on_save = false

[[enforcement.transitions]]
attribute_code = "status"
from = "review"
to = "released"
```

`on_save = true` odrzuca każdy zapis, po którym rekord narusza regułę. Każdy element `transitions` chroni zmianę atrybutu [statusu](/pl/builders/validation/#statusy) na `to`, opcjonalnie tylko z `from`. Chronione przejścia są sprawdzane na stanie, który powstaje po zmianie, więc wartość zapisana w tym samym zapisie się liczy.

| Klucz | Typ | Domyślnie | Opis |
| --- | --- | --- | --- |
| `enforcement.on_save` | wartość logiczna | `false` | Odrzuca każdy zapis, po którym rekord narusza regułę. |
| `enforcement.transitions` | tablica tabel | `[]` | Do 16 chronionych zmian statusu, każda z `attribute_code`, opcjonalnym `from` i `to`. Bez `from` chroniona jest każda zmiana na `to`. |

Tabela `enforcement` wymaga `on_save = true` lub co najmniej jednego przejścia, wagi `error` lub `critical` oraz [predykatu](/pl/reference/blueprint/#predykaty) bez `stale`, `unique` i `acyclic`, których nie da się sprawdzić podczas zapisu. `attribute_code` każdego przejścia musi być atrybutem statusu, a `from` i `to` muszą być kodami z jego `enum`.

Reguła powiązana z kontekstem jest egzekwowana tylko w tym kontekście. Reguła bez kontekstu jest egzekwowana w każdym kontekście.

Zapis naruszający egzekwowaną regułę zostaje odrzucony z `422 rule_violation` i nic nie jest zapisywane. Odpowiedź wymienia każdą naruszoną regułę, konteksty i atrybuty, których dotyczy. Zobacz [Walidacja](/pl/builders/validation/#błędy-i-ich-naprawa).

### Próbne wykonanie przed włączeniem

Egzekwowanie reguły na istniejących danych może zablokować osoby, które nic złego nie zrobiły. Dlatego gdy Schemat reguły ma już rekordy, Attricat wymaga ukończonego **pełnego próbnego wykonania** bieżącej definicji reguły, zanim ją włączysz. W przeciwnym razie włączenie kończy się błędem `409 rule_dry_run_required`.

1. Zapisz regułę.
2. Uruchom próbne wykonanie dla wszystkich rekordów, bez `--record-id`, i poczekaj, aż zakończy się na karcie **Historia wykonań**. Próbne wykonanie działa także dla wyłączonej reguły.

   ```sh
   acli rule run-now <rule-id> --idempotency-key enforce-preview --dry-run
   ```

3. Włącz regułę.

Jeśli próbne wykonanie znalazło naruszenia, włączenie kończy się błędem `409 rule_has_existing_violations`, a `details.existing_violations` podaje ich liczbę. Popraw te rekordy i powtórz próbne wykonanie albo zaakceptuj naruszenia jawnie:

```sh
acli rule enable <rule-id> --accept-existing-violations
```

W API wyślij `POST /rules/{id}/enable` z `{"accept_existing_violations": true}`.

Próbne wykonanie zatrzymuje się po 10 000 rekordów. Gdy Schemat ma ich więcej, wykonanie nie obejmuje pozostałych, więc włączenie nadal kończy się błędem `409 rule_dry_run_required`, chyba że zaakceptujesz istniejące naruszenia, jak powyżej. W takim przypadku komunikat błędu informuje, że wykonanie zatrzymało się przed czasem, a `details` ma postać `{"truncated": true, "existing_violations": …}` i podaje liczbę naruszeń znalezionych wśród sprawdzonych rekordów. Takie wykonanie ma w `GET /rule-runs` pole `"truncated": true`.

Zmiana włączonej egzekwowanej reguły przechodzi tę samą kontrolę dla nowej definicji. Próbne wykonanie może sprawdzić definicję jeszcze przed zapisaniem: w edytorze wybierz **Uruchom zmianę próbnie**, a jeśli znajdzie naruszenia, **Zapisz mimo to** je akceptuje. W CLI przekaż plik do próbnego wykonania, a przy zapisie zaakceptuj naruszenia:

```sh
acli rule run-now <rule-id> --idempotency-key change-preview --dry-run --definition-file rule.toml
acli rule update <rule-id> --accept-existing-violations --file rule.toml
```

W API `POST /rules/{id}/run-now` przyjmuje `"definition"` z kodem TOML tylko przy próbnym wykonaniu.

Rekordu, który już narusza egzekwowaną regułę, nie można zapisać, dopóki zapis nie usunie naruszenia.

## Pracuj z ustaleniami

**Zarządzanie → Reguły jakości danych → Ustalenia** wyświetla ustalenia wraz z regułą, rekordem, kontekstem i stanem. Dla każdej kombinacji reguły, rekordu i kontekstu istnieje jedno ustalenie. Gdy późniejsza ocena zakończy się powodzeniem, ustalenie zostaje rozwiązane. Jeśli problem wróci, to samo ustalenie zostaje ponownie otwarte.

**Potwierdź** ustalenie, aby zapisać, że ktoś je widział. Pozostaje ono, dopóki rekord nie spełni reguły. Lista ustaleń pokazuje, ile ustaleń pasuje do jej filtrów; zaznacz otwarte ustalenia i wybierz **Potwierdź zaznaczone** albo **Potwierdź wszystkie otwarte**, aby potwierdzić każde otwarte ustalenie pasujące do filtrów, do 5000 naraz. Potwierdzenia nie można cofnąć.

Strona rekordu wyświetla jego otwarte i potwierdzone ustalenia na karcie **Jakość danych**, po 20 naraz, a pozostałe wczytuje **Wczytaj więcej ustaleń**; oznaczenie jakości danych w nagłówku rekordu otwiera tę kartę. Nazwa pola w ustaleniu prowadzi do tego pola; przy ustaleniu dotyczącym innych kontekstów przełącza rekord na pierwszy kontekst, w którym ustalenie nie przechodzi. Po edycji lista odświeża się, gdy sprawdzanie rekordu się zakończy. **Sprawdź ponownie** od razu sprawdza rekord względem każdej włączonej reguły jego Schematu, na przykład po zmianie reguły.

W [Eksploratorze](/pl/guides/explore/#filtry) filtr **Jakość danych** zawęża wyniki do rekordów z otwartymi ustaleniami, tylko z potwierdzonymi albo bez ustaleń.

```sh
acli rule findings --record-id <uuid>
acli rule acknowledge <finding-id>
```

Karta **Historia wykonań** pokazuje wykonania i błędy. Wymienia wykonania obejmujące wszystkie rekordy, chyba że **Zakres** ustawisz na **Jeden rekord**, który pokazuje wykonania sprawdzające pojedynczy rekord, na przykład po każdej edycji, albo na **Dowolny**. **Wynik** podaje ustalenia, które wykonanie dodało i rozwiązało, a dla próbnego wykonania liczbę rekordów, które by nie przeszły. Wykonanie, które zakończy się niepowodzeniem pięć razy, staje się martwą wiadomością (dead letter); usuń przyczynę i odtwórz je poleceniem `acli rule run-replay <run-id>`.

## Uprawnienia

`rules.read` pozwala wyświetlać reguły, wykonania i ustalenia. `rules.manage` pozwala tworzyć, zmieniać, włączać, wyłączać i uruchamiać reguły oraz potwierdzać ustalenia. Oba uprawnienia są domyślnie przyznane rolom właściciela i administratora.

Każdy, kto może odczytać rekord, widzi jego ustalenia: kartę **Jakość danych**, oznaczenie w nagłówku i wyróżnione pola, więc osoby poprawiające dane widzą, co poprawić. Bez `rules.read` widzą nazwę reguły, ale nie jej stronę.
