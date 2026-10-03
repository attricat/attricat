---
title: Reguły jakości danych
description: Definiuj wersjonowane kontrole, które oznaczają encje z brakującymi, nieaktualnymi lub błędnie otagowanymi danymi, i zarządzaj ustaleniami.
---

Reguła sprawdza encje jednego Schematu pod kątem prostego warunku i zapisuje **ustalenie** dla każdej encji, która go nie spełnia. Ustalenia rozwiązują się same, gdy encja zostanie poprawiona. Reguły tylko odczytują dane; nigdy nie zmieniają encji.

Używaj reguł do pytań w rodzaju „które opublikowane produkty nie mają tytułu?” albo „których cen nikt nie zmieniał od roku?”. Aby zmieniać dane automatycznie, użyj [przepływu pracy](/pl/builders/workflows/).

## Zdefiniuj regułę

Reguła ma kod, nazwę, wagę, od jednego do ośmiu wyzwalaczy i dokładnie jeden predykat.

```toml
format_version = 1
code = "title-required"
name = "Products have a title"
severity = "error"

[[triggers]]
type = "schedule"
cron = "0 0 6 * * *"
timezone = "UTC"

[[triggers]]
type = "event"
event_type = "entity.updated.v1"

[[triggers]]
type = "manual"

[predicate]
type = "required"
attribute_code = "title"
```

| Klucz | Opis |
| --- | --- |
| `format_version` | `1`. Pomiń go, gdy reguła jest osadzona w Schemacie. |
| `code` | Unikalny [kod](/pl/reference/blueprint/#kody). |
| `name` | Nazwa wyświetlana. |
| `severity` | `info`, `warning`, `error` lub `critical`. |
| `triggers` | Od 1 do 8 wyzwalaczy. Zobacz niżej. |
| `predicate` | Warunek, który encja musi spełnić. Zobacz niżej. |

### Wyzwalacze

| `type` | Klucze | Uruchamia się |
| --- | --- | --- |
| `manual` | | Gdy ktoś wybierze **Uruchom teraz**. |
| `schedule` | `cron`, `timezone = "UTC"` | Według sześciopolowego harmonogramu cron (sekundy na początku), w UTC. `0 0 6 * * *` oznacza codziennie o 06:00. |
| `event` | `event_type` | Dla zmienionej encji, po jednym z: `entity.created.v1`, `entity.updated.v1`, `entity.migrated.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1`, `relationship.changed.v1`. |
| `post_import` | | Zarezerwowany dla przyszłej integracji z importem. |

### Predykaty

Encja **nie spełnia** reguły, gdy predykat nie jest spełniony. Reguły używają tych samych predykatów co [kontrole encji](/pl/builders/validation/#porównuj-atrybuty-za-pomocą-kontroli), [warunki przejść statusu](/pl/builders/validation/#warunki-przejść) i [kontrole kanałów publikacji](/pl/guides/publishing/#wymagaj-kontroli-przed-publikacją). Wszystkie klucze opisuje [dokumentacja Schematu](/pl/reference/blueprint/#predykaty).

| `type` | Klucze | Spełniony, gdy |
| --- | --- | --- |
| `required` | `attribute_code` | Atrybut ma wartość. Relacja musi mieć co najmniej jeden cel. |
| `stale` | `attribute_code`, `max_age_seconds` (od 1 do 31536000) | Atrybut został zaktualizowany w ciągu ostatnich `max_age_seconds` sekund. |
| `has_tag` | `tag` | Encja ma ten tag systemowy. |
| `missing_tag` | `tag` | Encja nie ma tego tagu systemowego. |
| `compare` | `attribute_code`, `op` oraz jeden z kluczy `other_attribute_code` lub `value` | Porównanie jest prawdziwe, na przykład `valid_until` `gte` `valid_from`. |
| `one_of` | `attribute_code`, `values` (od 1 do 100) | Wartość jest jedną z wymienionych, na przykład kodem statusu. |
| `relative_date` | `attribute_code`, `op` (`lt`, `lte`, `gt`, `gte`), `offset_days` | Data lub data z godziną spełnia porównanie z chwilą obecną przesuniętą o `offset_days`. |
| `unique` | `attribute_codes` (od 1 do 4) | Żadna inna aktywna encja tego samego Schematu nie ma tych samych wartości w tym samym kontekście. |
| `linked` | `relationship_code`, `quantifier`, `predicate` | Powiązane rekordy spełniają zagnieżdżony predykat: wszystkie (`all`, domyślnie), co najmniej jeden (`any`) lub żaden (`none`). |
| `referenced_by` | `blueprint_code`, `relationship_code`, opcjonalnie `predicate`, `min` i/lub `max` | Liczba rekordów innego Schematu, które wskazują tę encję i spełniają `predicate`, mieści się w granicach. |
| `acyclic` | `relationship_code` | Podążanie za relacją od encji nigdy do niej nie wraca. |
| `all_of` | `predicates` (od 1 do 16) | Każdy zagnieżdżony predykat jest spełniony. |
| `any_of` | `predicates` (od 1 do 16) | Co najmniej jeden zagnieżdżony predykat jest spełniony. |

Porównanie z brakującą wartością jest spełnione; to samo dotyczy `one_of` i `relative_date`, gdy atrybut jest pusty. Połącz je z `required`, jeśli wartość musi istnieć:

```toml
[predicate]
type = "all_of"
predicates = [
  { type = "required", attribute_code = "valid_until" },
  { type = "relative_date", attribute_code = "valid_until", op = "gt", offset_days = 30 },
]
```

`compare` porządkuje liczby, liczby całkowite, daty i daty z godziną operatorami `lt`, `lte`, `gt` i `gte`. Łańcuchy znaków i wartości logiczne obsługują `eq` i `ne`. Dwie relacje są równe (`eq`), gdy mają ten sam zbiór celów, i rozłączne (`disjoint`), gdy nie mają wspólnego celu. Plików nie można porównywać. Daty zapisuj jako łańcuchy znaków, na przykład `"2026-01-31"`.

`linked` i `referenced_by` przechodzą o jeden krok wzdłuż relacji. Ich zagnieżdżony predykat odczytuje drugi rekord i może porównywać go ze sprawdzaną encją przez `subject_attribute_code`:

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

`relative_date` porównuje datę z chwilą uruchomienia reguły. Nadaj takiej regule wyzwalacz `schedule`: encja, której nikt nie edytuje, może wygasnąć w nocy, a zauważy to tylko przebieg według harmonogramu.

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

Reguła powiązana z kontekstem sprawdza rozstrzygnięte wartości encji w tym kontekście, łącznie z wartościami dziedziczonymi. Reguła bez kontekstu sprawdza każdy kontekst i nie jest spełniona, jeśli nie przejdzie w którymkolwiek z nich; dowody ustalenia podają kody tych kontekstów w polu `contexts`.

### Zmiany w powiązanych rekordach

`linked` i `referenced_by` zależą od innych rekordów. Gdy zmieni się rekord powiązany lub wskazujący, reguły z wyzwalaczem zdarzenia uruchamiają się też ponownie dla maksymalnie 100 zależnych od niego encji, dzięki czemu ich ustalenia pozostają aktualne. Reguły wyzwalane tylko harmonogramem lub ręcznie zauważą zmianę przy następnym przebiegu.

### Limity

- Predykaty można zagnieżdżać najwyżej na 4 poziomach i mogą mieć łącznie najwyżej 32 części.
- `linked` sprawdza najwyżej 200 powiązanych rekordów na relację, a `referenced_by` najwyżej 1000 rekordów wskazujących. Po przekroczeniu limitu encja nie spełnia reguły, a komunikat to wyjaśnia.
- `acyclic` odwiedza najwyżej 1000 rekordów. Jeśli nie zdoła zakończyć sprawdzania, encja nie spełnia reguły.
- `linked` i `referenced_by` nie mogą być zagnieżdżone w sobie nawzajem, a ich zagnieżdżony predykat nie może używać `unique`, `acyclic` ani `stale`.
- `offset_days` mieści się w zakresie od -36500 do 36500.

## Gdzie przechowywane są reguły

Reguły można zapisać na dwa sposoby.

**Wewnątrz Schematu**, jako tabele `[[rules]]` bez `format_version`. Są wersjonowane i publikowane razem ze Schematem:

```toml
[[rules]]
code = "price-fresh"
name = "Prices reviewed in the last 90 days"
severity = "warning"
triggers = [{ type = "schedule", cron = "0 0 3 * * 1", timezone = "UTC" }]
predicate = { type = "stale", attribute_code = "price", max_age_seconds = 7776000 }
```

**Jako samodzielne reguły** w **Zarządzanie → Reguły jakości danych**, powiązane z jedną opublikowaną wersją Schematu i opcjonalnie z jednym kontekstem:

```sh
acli rule create --blueprint-id <uuid> --blueprint-version 3 --file title-required.toml
acli rule publish <rule-id> 1
acli rule enable <rule-id> 1
```

Samodzielne reguły mają własne szkice i wersje. Wersja reguły musi zostać opublikowana, a następnie włączona, zanim zacznie działać. W danym momencie włączona jest tylko jedna wersja reguły; `acli rule disable` ją zatrzymuje.

## Uruchom regułę

Włączone reguły uruchamiają się według swoich wyzwalaczy. Możesz też uruchomić regułę ręcznie ze strony reguły lub z CLI:

```sh
acli rule run-now <rule-id> --idempotency-key 2026-03-01-audit
acli rule run-now <rule-id> --idempotency-key check-one --entity-id <uuid>
acli rule run-now <rule-id> --idempotency-key preview --dry-run
```

Przebieg próbny (dry run) podaje, ile encji nie spełniłoby reguły, i nie zapisuje żadnych ustaleń. Ponowne użycie klucza idempotencji zwraca pierwotny przebieg zamiast rozpoczynać nowy.

Każdy przebieg przetwarza kandydatów stronami po 500 i zatrzymuje się po 10 000. Harmonogram nigdy nie nakłada się sam na siebie: jeśli poprzedni przebieg wciąż oczekuje, kolejne wystąpienie jest pomijane.

## Egzekwowanie reguły

Domyślnie reguła tylko zgłasza ustalenia. Reguła **egzekwowana** dodatkowo blokuje zapisy, po których encja naruszałaby regułę. Dodaj tabelę `enforcement`:

```toml
format_version = 1
code = "released-documents-approved"
name = "Released documents have an approver"
severity = "error"

[[triggers]]
type = "event"
event_type = "entity.updated.v1"

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

| Klucz | Opis |
| --- | --- |
| `on_save` | `true` odrzuca każdy zapis, po którym encja narusza regułę. |
| `transitions` | Do 16 zmian statusu chronionych przez regułę. `attribute_code` to atrybut [statusu](/pl/builders/validation/#statusy); `to` to kod docelowy; `from` jest opcjonalny, a gdy go pominiesz, chroniona jest każda zmiana na `to`. |

Ustaw `on_save`, co najmniej jedno przejście albo oba. Chronione przejścia są sprawdzane na stanie, który powstaje po zmianie, więc wartość zapisana w tym samym zapisie się liczy. Wewnątrz Schematu zapisz tabelę jako `[rules.enforcement]` i `[[rules.enforcement.transitions]]` po nagłówku `[[rules]]` danej reguły.

Egzekwowanie ma kilka wymagań:

- Waga reguły to `error` lub `critical`.
- Predykat musi dać się bezpiecznie sprawdzić podczas zapisu. `stale`, `unique` i `acyclic` się do tego nie nadają; używaj ich w regułach zgłaszających ustalenia.
- W Schemacie atrybut przejścia musi być atrybutem statusu, a `from` i `to` muszą być jego kodami.

Reguła powiązana z kontekstem jest egzekwowana tylko w tym kontekście. Reguła bez kontekstu jest egzekwowana w każdym kontekście.

Zapis naruszający egzekwowaną regułę zostaje odrzucony z `422 rule_violation` i nic nie jest zapisywane. Odpowiedź wymienia każdą naruszoną regułę, konteksty i atrybuty, których dotyczy. Zobacz [Walidacja](/pl/builders/validation/#błędy-i-ich-naprawa).

### Przebieg próbny przed włączeniem

Egzekwowanie reguły na istniejących danych może zablokować osoby, które nic złego nie zrobiły. Dlatego gdy wersja Schematu reguły ma już encje, Attricat wymaga ukończonego **pełnego przebiegu próbnego** dokładnie tej wersji reguły, którą włączasz. W przeciwnym razie włączenie kończy się błędem `409 rule_dry_run_required`.

1. Opublikuj wersję.
2. Uruchom przebieg próbny dla wszystkich encji, bez `--entity-id`, i poczekaj, aż zakończy się na karcie **Historia przebiegów**. Przebieg próbny może dotyczyć opublikowanej wersji, która nie jest jeszcze włączona.

   ```sh
   acli rule run-now <rule-id> --idempotency-key enforce-preview --dry-run
   ```

   CLI uruchamia przebieg próbny włączonej wersji albo najnowszej opublikowanej, jeśli żadna nie jest włączona. Aby sprawdzić konkretną wersję, gdy włączona jest inna, wywołaj `POST /rules/{id}/run-now` z `{"dry_run": true, "idempotency_key": "…", "version": 2}`.
3. Włącz wersję.

Jeśli przebieg próbny znalazł naruszenia, włączenie kończy się błędem `409 rule_has_existing_violations`, a `details.existing_violations` podaje ich liczbę. Popraw te encje i powtórz przebieg próbny albo zaakceptuj naruszenia jawnie przez API:

```http
POST /rules/{id}/versions/{version}/enable
{"accept_existing_violations": true}
```

Encji, która już narusza egzekwowaną regułę, nie można zapisać, dopóki zapis nie usunie naruszenia.

## Pracuj z ustaleniami

**Zarządzanie → Reguły jakości danych → Ustalenia** wyświetla ustalenia wraz z regułą, encją, kontekstem i stanem. Dla każdej kombinacji reguły, encji i kontekstu istnieje jedno ustalenie. Gdy późniejsza ocena zakończy się powodzeniem, ustalenie zostaje rozwiązane. Jeśli problem wróci, to samo ustalenie zostaje ponownie otwarte.

**Potwierdź** ustalenie, aby zapisać, że ktoś je widział. Pozostaje ono, dopóki encja nie spełni reguły.

```sh
acli rule findings --entity-id <uuid>
acli rule acknowledge <finding-id>
```

Karta **Historia przebiegów** pokazuje historię przebiegów i błędy. Przebieg, który zakończy się niepowodzeniem pięć razy, staje się martwą wiadomością (dead letter); usuń przyczynę i odtwórz go poleceniem `acli rule run-replay <run-id>`.

## Uprawnienia

`rules.read` pozwala wyświetlać reguły, przebiegi i ustalenia. `rules.manage` pozwala tworzyć, publikować, włączać, wyłączać i uruchamiać reguły oraz potwierdzać ustalenia. Oba uprawnienia są domyślnie przyznane rolom właściciela i administratora.
