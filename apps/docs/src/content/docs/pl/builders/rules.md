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

Encja **nie spełnia** reguły, gdy predykat nie jest spełniony.

| `type` | Klucze | Spełniony, gdy |
| --- | --- | --- |
| `required` | `attribute_code` | Atrybut ma wartość. |
| `stale` | `attribute_code`, `max_age_seconds` (od 1 do 31536000) | Atrybut został zaktualizowany w ciągu ostatnich `max_age_seconds` sekund. |
| `has_tag` | `tag` | Encja ma ten tag systemowy. |
| `missing_tag` | `tag` | Encja nie ma tego tagu systemowego. |

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
