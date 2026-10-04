---
title: Dokumentacja zdarzeń
description: Zdarzenia katalogu rejestrowane przez Attricat przy każdej zmianie, ich koperta i gwarancje dostarczania.
---

Każda zmiana w katalogu zapisuje zdarzenie w tej samej transakcji bazy danych co sama zmiana i jej wpis audytu. Jeśli zmiana zostanie zapisana, zdarzenie również; jeśli się nie powiedzie, nie istnieje ani jedno, ani drugie.

Zdarzenia napędzają [przepływy pracy](/pl/builders/workflows/), [reguły](/pl/builders/rules/) i [procedury obsługi zdarzeń w rozszerzeniach](/pl/extensions/server/#obsługuj-zdarzenia-katalogu). Nie są udostępniane jako publiczny strumień.

## Typy zdarzeń

| Typ | Rejestrowane, gdy |
| --- | --- |
| `entity.created.v1` | Encja zostaje utworzona. |
| `entity.updated.v1` | Zmieniają się wartości, relacje lub adnotacje encji. |
| `entity.deleted.v1` | Encja zostaje usunięta. |
| `entity.migrated.v1` | Encja przechodzi na nowszą wersję schematu. |
| `entity.published.v1` | Encja zostaje opublikowana w kanale. |
| `entity.unpublished.v1` | Publikacja encji zostaje wycofana. |
| `attribute_value.changed.v1` | Wartość atrybutu zostaje ustawiona, zastąpiona lub usunięta. |
| `attribute_value.restored.v1` | Wartość zostaje przywrócona z historii. |
| `relationship.changed.v1` | Cele relacji zostają dodane lub usunięte. |
| `blueprint.created.v1` | Schemat zostaje utworzony. |
| `blueprint.revision_created.v1` | Powstaje szkic nowej wersji schematu. |
| `blueprint.published.v1` | Wersja schematu zostaje opublikowana. |
| `context.created.v1`, `context.updated.v1`, `context.deleted.v1` | Kontekst się zmienia. |

Rozszerzenia publikują własne typy o nazwach `plugin.<extension-id>.<name>.vN`. Przestrzenie nazw `entity`, `attribute_value`, `relationship`, `blueprint` i `context` są zarezerwowane.

Przyrostek wersji nigdy nie zmienia znaczenia. Niezgodny ładunek otrzymuje nowy typ `.vN`, a konsumenci subskrybują dokładnie te wersje, które rozumieją.

## Koperta

| Pole | Opis |
| --- | --- |
| `id` | UUID zdarzenia. Używaj go do deduplikacji. |
| `event_type` | Na przykład `entity.updated.v1`. |
| `occurred_at` | Kiedy zmiana została zapisana. |
| `aggregate_kind`, `aggregate_id` | Co się zmieniło, np. `entity` i jej UUID. |
| `correlation_id` | Wspólny dla wszystkiego, co wynikło z jednego żądania lub zadania. |
| `causation_id` | Zdarzenie, które bezpośrednio spowodowało to zdarzenie, jeśli istnieje. |
| `source_kind` | `api`, `worker`, `plugin` lub `system`. |
| `source_name` | Który producent, np. przepływ pracy (`workflow:<id>`) lub rozszerzenie (`extension:<extension-id>`). |
| `metadata`, `payload` | Obiekty JSON, każdy do 64 KiB. |

## Ładunki encji

Zdarzenia encji i wartości opisują, co się zmieniło, a nie całą encję:

```json
{
  "entity_id": "7f1c…",
  "blueprint_id": "a2d4…",
  "blueprint_version": 3,
  "facts": [{
    "attribute_id": "c9e0…",
    "attribute_code": "price",
    "context_id": "00000000-0000-4000-8000-000000000001",
    "context_code": "default",
    "relationship_target_entity_id": null,
    "change_kind": "set",
    "before_value": 49.0,
    "after_value": 39.0
  }]
}
```

Aby działać na bieżącym stanie encji, odczytaj ją; ładunek jest wyłącznie opisem zmiany.

### Które zmiany tworzą fakty

`facts` zawiera po jednym wpisie dla każdej wartości atrybutu, która faktycznie się zmieniła. Zapis, który pozostawia wartość bez zmian, nie dodaje dla niej faktu. [Wyzwalacze przepływów pracy](/pl/builders/workflows/#reaguj-tylko-na-wybrane-atrybuty) mogą filtrować po `attribute_code` za pomocą `attributes`.

- **Atrybuty skalarne:** `change_kind` to `set`, `replace` lub `remove`; `restore` w `attribute_value.restored.v1`. `entity.created.v1` wymienia każdą wartość początkową, łącznie z domyślnymi, jako `set`.
- **Atrybuty relacji:** każdy dodany lub usunięty cel to osobny fakt z `change_kind` równym `relationship_add` lub `relationship_remove`, ustawionym `relationship_target_entity_id` i identyfikatorem celu jako wartością. Zapis zmieniający tylko relacje tworzy `relationship.changed.v1`; zapis zmieniający też inne wartości tworzy `entity.updated.v1`.
- **Atrybuty plikowe:** przesłanie, podłączenie, zmiana kolejności lub usunięcie plików trafia do dziennika audytu, ale nie tworzy zdarzenia ani faktu.
- Zmiany tagów i metadanych systemowych nie są wartościami atrybutów i nie dodają faktów.

## Gwarancje dostarczania

- **Co najmniej raz.** Konsument może otrzymać to samo zdarzenie dwa razy, np. jeśli ulegnie awarii po wykonaniu pracy, ale przed zapisaniem dostarczenia. Twórz idempotentne procedury obsługi, oparte na identyfikatorze zdarzenia.
- **Bez kolejności.** Zdarzenia mogą docierać w innej kolejności niż ta, w której zostały zapisane.
- **Od teraz.** Nowy konsument zaczyna od bieżącej pozycji i nie otrzymuje starszych zdarzeń.
- **Ponowienia.** Nieudane dostarczenie jest ponawiane po 1, 2, 4, … sekundach, maksymalnie co 60, a po pięciu próbach staje się martwą wiadomością (dead letter). To wartości domyślne; zobacz [Praca w tle](/pl/reference/configuration/#praca-w-tle).

Wyszukiwanie i ponawianie martwych wiadomości opisuje [Monitorowanie](/pl/operate/monitoring/#nieudane-dostarczenia-zdarzeń).
