---
title: Przepływy pracy
description: Automatyzuj małe, audytowalne zmiany rekordu lub rekordów, które się do niego odwołują, w odpowiedzi na zdarzenia katalogu, harmonogram lub ręczne wywołanie.
---

Przepływ pracy reaguje na wyzwalacz, stosując krótką listę akcji do jednego rekordu: dodaje lub usuwa tagi systemowe, aktualizuje metadane systemowe albo zapisuje wartość atrybutu. Przepływy pracy to wersjonowany YAML, podobnie jak Schematy, a każda wprowadzana przez nie zmiana przechodzi zwykłą walidację i trafia do dziennika audytu.

Przepływy pracy nie mają skryptów, pętli, zapytań ani wywołań sieciowych. Zmieniają rekord, który je wyzwolił, a jedną ograniczoną akcją także rekordy, które łączą się z nim przez wskazaną relację. Do większych zadań napisz [rozszerzenie](/pl/extensions/build/).

## Pierwszy przepływ pracy

Oznacz produkt do przeglądu przy każdej zmianie jego tytułu:

```yaml
format_version: 1
code: review-title-change
name: Review title changes

triggers:
  - event_type: attribute_value.changed.v1
    attributes: [title]

actions:
  - type: system_tags_add
    tags: [needs-review]
```

Utwórz go, opublikuj i włącz w **Zarządzanie → Przepływy pracy** albo za pomocą CLI:

```sh
acli workflow validate --file review-title-change.yaml
acli workflow create --file review-title-change.yaml
acli workflow publish <workflow-id> 1
acli workflow enable <workflow-id> 1
```

Włączenie zapisuje bieżący punkt w strumieniu zdarzeń. Wykonania uruchamiają tylko zdarzenia po tym punkcie; przepływ pracy nie przetwarza historii.

## Definicja

| Klucz | Opis |
| --- | --- |
| `format_version` | `1` tylko dla wyzwalaczy zdarzeń. `2` dodaje wyzwalacze ręczne i harmonogramowe. |
| `code` | Unikalny [kod](/pl/reference/blueprint/#kody). |
| `name` | Nazwa wyświetlana. |
| `triggers` | Jeden lub więcej wyzwalaczy. |
| `actions` | Jedna lub więcej akcji, stosowanych po kolei. |

## Wyzwalacze zdarzeń

Wyzwalacz zdarzenia wskazuje jedno zdarzenie rekordu i opcjonalnie je zawęża:

```yaml
triggers:
  - event_type: record.updated.v1
    envelope: {source_kind: api}
    facts:
      "facts.0.attribute_code": price
      "facts.0.context_code": PL
```

`event_type` to jedno z: `record.created.v1`, `record.updated.v1`, `record.migrated.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1` lub `relationship.changed.v1`.

`envelope` dokładnie dopasowuje właściwości zdarzenia: `event_type`, `aggregate_kind`, `source_kind`, `source_name` lub `metadata.<key>`.

`facts` dopasowuje fakty przenoszone przez zdarzenie. Każdy fakt opisuje jeden zmieniony atrybut. Jego pola to `attribute_id`, `attribute_code`, `context_id`, `context_code`, `relationship_target_record_id`, `change_kind`, `before_value` i `after_value`. Odwołuj się do nich jako `facts.<index>.<field>`, gdzie indeks `0` oznacza pierwszy fakt.

Akcje przepływu pracy wyzwolonego zdarzeniem dotyczą rekordu, którego dotyczy zdarzenie.

### Reaguj tylko na wybrane schematy

Przepływ pracy należy do całej przestrzeni roboczej. `blueprints` zawiera od 1 do 100 kodów schematów i wtedy wyzwalacz uruchamia przepływ tylko dla rekordów tych schematów:

```yaml
triggers:
  - event_type: record.updated.v1
    blueprints: [product, service]
```

- Każdy kod musi wskazywać schemat istniejący w przestrzeni roboczej w chwili zapisu przepływu, więc literówka jest odrzucana z `422 invalid_workflow_definition`, zamiast po cichu wyłączać przepływ.
- Rekord pasuje według swojego schematu, niezależnie od rewizji, w której jest.
- Gdy każdy wyzwalacz wymienia swoje schematy, każda akcja `attribute_write` na rekordzie wyzwalającym musi wskazywać atrybut, który jeden z tych schematów deklaruje w najnowszej opublikowanej rewizji, albo [atrybut wielokrotnego użytku](/pl/builders/modeling/#wielokrotne-użycie-atrybutów). Edytor zaznacza literówkę w kodzie już podczas pisania, a zapis ją odrzuca, więc nie dochodzi ona do uruchomienia. Zapisy w [`referencing_records_update`](#aktualizuj-rekordy-odwołujące-się-do-rekordu-wyzwalającego) nie są tak sprawdzane, bo rekordy odwołujące się mogą mieć dowolny schemat.
- `blueprints` łączy się z `attributes`, `envelope` i `facts`: wszystkie muszą pasować. Bez niego wyzwalacz pasuje do rekordów każdego schematu.
- Wyzwalacze ręczne i harmonogramowe same wskazują swój rekord, więc nie przyjmują `blueprints`.

### Reaguj tylko na wybrane atrybuty

`attributes` to lista od 1 do 100 kodów atrybutów. Wyzwalacz uruchamia wykonanie tylko wtedy, gdy w zdarzeniu zmienił się co najmniej jeden z tych atrybutów, niezależnie od jego miejsca wśród faktów zdarzenia:

```yaml
triggers:
  - event_type: attribute_value.changed.v1
    attributes: [body, revision_notes, license]

  - event_type: relationship.changed.v1
    attributes: [license]
```

- Zmiana to fakt w zdarzeniu. Zapisanie wartości identycznej z bieżącą nie tworzy faktu, więc nie uruchamia wykonania.
- Jeden zapis może wygenerować `attribute_value.changed.v1`, `relationship.changed.v1` albo `record.updated.v1`, zależnie od tego, co zawiera. Wymień każdy typ zdarzenia, przez który mogą zmienić się dane atrybuty, jak w przykładzie powyżej.
- `record.created.v1` traktuje każdą wartość, z którą powstaje nowy rekord, łącznie z domyślnymi, jako zmienioną.
- **Atrybuty relacji:** dodanie lub usunięcie celu jest zmianą atrybutu relacji. Każdy dodany lub usunięty cel to osobny fakt z `change_kind` równym `relationship_add` lub `relationship_remove`. Zmiany samego powiązanego rekordu są zdarzeniami tamtego rekordu, nie tego.
- **Atrybuty plikowe:** przesłanie, podłączenie, zmiana kolejności lub usunięcie plików trafia do dziennika audytu, ale nie tworzy zdarzenia, więc atrybut plikowy w `attributes` nigdy nie uruchamia wykonania. Śledź zmiany plików atrybutem skalarnym, który zmienia się razem z nimi, np. numerem rewizji.
- `attributes` nie działa z `record.migrated.v1`, które nie zawiera faktów. Łączy się z `envelope` i `facts`: wszystkie muszą pasować.

### Reaguj tylko na dodane lub usunięte cele relacji

`change_kinds` to lista rodzajów zmian, na które reaguje wyzwalacz. Wyzwalacz uruchamia wykonanie tylko wtedy, gdy zdarzenie zawiera fakt jednego z tych rodzajów. Razem z `attributes` ten sam fakt musi też dotyczyć jednego z tych atrybutów:

```yaml
triggers:
  # Do rekordu podłączono licencję
  - event_type: relationship.changed.v1
    attributes: [license]
    change_kinds: [relationship_add]

  # Od rekordu odłączono licencję
  - event_type: relationship.changed.v1
    attributes: [license]
    change_kinds: [relationship_remove]
```

- Rodzaje to `set`, `replace`, `remove`, `restore`, `relationship_add` i `relationship_remove`. Zobacz, [które zmiany tworzą fakty](/pl/reference/events/#które-zmiany-tworzą-fakty).
- Zapis zmieniający relacje razem z innymi wartościami tworzy `record.updated.v1`, a nie `relationship.changed.v1`. Dodaj drugi wyzwalacz z `event_type = "record.updated.v1"` i tymi samymi filtrami, aby wychwycić też takie zapisy.
- `change_kinds` nie działa z `record.migrated.v1`. Łączy się z pozostałymi filtrami: wszystkie muszą pasować.

### Reaguj tylko na dodane lub usunięte tagi

`tags_added` i `tags_removed` to listy od 1 do 100 tagów systemowych. Wyzwalacz uruchamia wykonanie tylko wtedy, gdy zapis dodał lub usunął co najmniej jeden z nich:

```yaml
triggers:
  - event_type: record.updated.v1
    tags_added: [approved]

  - event_type: record.updated.v1
    tags_removed: [approved]
```

- Liczą się tylko tagi, które faktycznie się zmieniły. Zapisanie rekordu z tagami, które już ma, nie uruchamia wykonania.
- `tags_added` działa z `record.created.v1`, gdzie każdy tag, z którym powstaje rekord, liczy się jako dodany, oraz z `record.updated.v1`. `tags_removed` działa tylko z `record.updated.v1`.
- Zmiany tagów wprowadzone przez przepływ pracy nie uruchamiają przepływów, jak każda inna zmiana wprowadzona przez przepływ pracy.
- Oba łączą się z pozostałymi filtrami: wszystkie muszą pasować. Aby reagować na każdą z tych zmian, użyj dwóch wyzwalaczy, jak powyżej.

## Wyzwalacze ręczne i harmonogramowe

Przy `format_version: 2` przepływ pracy można też uruchomić ręcznie lub według harmonogramu.

```yaml
format_version: 2
code: nightly-flag
name: Nightly flag

triggers:
  - type: manual

  - type: schedule
    cron: '0 0 2 * * *'
    timezone: UTC
    target_record_id: '00000000-0000-0000-0000-000000000001'

actions:
  - type: system_metadata_merge
    values: {last_nightly_check: done}
```

- **Ręczny**: uruchom wykonanie dla jednego rekordu ze strony przepływu pracy albo poleceniem `acli workflow run-now <workflow-id> --record-id <uuid> --idempotency-key <key>`. Zawsze używa włączonej wersji.
- **Harmonogram**: sześciopolowy cron w UTC. `timezone` musi mieć wartość `"UTC"`, co zapobiega powtórzonym lub pominiętym wykonaniom przy zmianie czasu. `target_record_id` to rekord, do którego stosowane są akcje. Jeśli serwer nie działał w chwili, gdy wykonanie było zaplanowane, wystąpienia opóźnione o więcej niż pięć minut są pomijane i zapisywane. Wystąpienie jest też pomijane, dopóki poprzednie wykonanie wciąż oczekuje.

Wykonania ręczne i harmonogramowe nie mają zdarzenia, więc ich akcje muszą używać stałych wartości.

## Akcje

| `type` | Klucze | Efekt |
| --- | --- | --- |
| `system_tags_add` | `tags` (od 1 do 100, każdy do 128 bajtów) | Dodaje tagi systemowe. |
| `system_tags_remove` | `tags` | Usuwa tagi systemowe. |
| `system_metadata_merge` | `values` (mapowanie wartości skalarnych) | Ustawia klucze w metadanych systemowych. Klucze mogą być ścieżkami z kropkami. |
| `system_metadata_delete` | `keys` | Usuwa klucze z metadanych systemowych. |
| `attribute_write` | `attribute_code` i dokładnie jeden z `fixed` lub `event_field` | Zapisuje skalarną wartość atrybutu. `fixed` to literał; `event_field` kopiuje wartość ze zdarzenia, np. `facts.0.after_value`. |
| `referencing_records_update` | `relationship_attribute`, opcjonalnie `max_targets` i zagnieżdżone `actions` | Stosuje zagnieżdżone akcje do każdego rekordu, który łączy się z rekordem wyzwalającym. Zobacz niżej. |

`attribute_write` przestrzega Schematu: obowiązują kontrole typów, schematy wartości i [przejścia statusów](/pl/builders/validation/#statusy).

### Aktualizuj rekordy odwołujące się do rekordu wyzwalającego

Gdy zmienia się licencja, specyfikacja lub skład, zależne od nich rekordy często muszą wrócić do przeglądu. `referencing_records_update` znajduje każdy istniejący rekord, którego `relationship_attribute` łączy się obecnie z rekordem wyzwalającym, w dowolnym kontekście, i stosuje do niego zagnieżdżone akcje:

```yaml
format_version: 2
code: license-changed
name: Send licensed products back to review

triggers:
  - event_type: attribute_value.changed.v1
    attributes: [terms]

actions:
  - type: referencing_records_update
    relationship_attribute: license
    max_targets: 200
    actions:
      - type: attribute_write
        attribute_code: status
        fixed: in_review
      - type: system_tags_add
        tags: [needs-review]
```

- `relationship_attribute` to kod atrybutu relacji w rekordach **odwołujących się**, a nie w rekordzie wyzwalającym.
- Akcja ma od 1 do 20 akcji zagnieżdżonych. Mogą one ustawiać statusy i inne atrybuty stałymi wartościami oraz dodawać lub usuwać tagi i metadane systemowe. Nie mogą używać `event_field` ani zawierać kolejnego `referencing_records_update`.
- `max_targets` domyślnie wynosi 100, a maksymalnie 500. Jeśli z rekordem wyzwalającym łączy się więcej rekordów, akcja kończy się błędem, zanim zmieni kolejny rekord. Zwiększ limit lub zawęź relację.
- Każdy rekord jest aktualizowany osobnym zapisem, z tymi samymi kontrolami co każda inna edycja: schematami, sprawdzeniami, kluczami unikalnymi, egzekwującymi regułami i przejściami statusów, łącznie z ich warunkami, [blokadami oraz wymaganymi uprawnieniami lub rolami](/pl/builders/validation/#kontroluj-cykl-życia-rekordu). Wymagania przejść są sprawdzane względem osoby, której zmiana uruchomiła przepływ pracy. Rekord, który nie przejdzie którejś z tych kontroli, na przykład zablokowany lub którego status nie pozwala na dane przejście, kończy się błędem sam; pozostałe rekordy są nadal aktualizowane.
- Jeśli którykolwiek rekord się nie powiedzie, wykonanie jest ponawiane. Ponowienie wraca tylko do rekordów, które się nie powiodły lub do których nie dotarto; już zaktualizowane rekordy nigdy nie są zmieniane dwukrotnie. Rekord, który do tego czasu nie istnieje lub nie łączy się już z rekordem wyzwalającym, jest pomijany.
- Wynik dla każdego rekordu, wraz z ostatnim błędem, zobaczysz poleceniem `acli workflow run-targets <run-id>` albo przez `GET /workflow-runs/{id}/targets`. Lista wykonań pokazuje ogólny błąd wykonania, gdy stanie się ono martwą wiadomością.
- Zmiany tych rekordów nie uruchamiają przepływów pracy, tak jak każda inna zmiana wprowadzona przez przepływ pracy.

## Jak działają wykonania

- Każde wykonanie używa wersji, która była włączona w chwili jego rozpoczęcia, nawet jeśli później zostanie włączona nowsza.
- Dostarczanie odbywa się co najmniej raz. Każda akcja jest zapisywana wraz z wykonaniem, więc ponowienie nigdy nie zastosuje jej dwukrotnie. Akcja `referencing_records_update` zapisuje też każdy aktualizowany rekord.
- Nieudane wykonanie jest ponawiane z rosnącymi opóźnieniami i po pięciu próbach staje się martwą wiadomością (dead letter). Odtwórz je z listy wykonań przepływu pracy albo poleceniem `acli workflow run-replay <run-id>`.
- Wyłączenie przepływu pracy anuluje jego wykonania oczekujące w kolejce i trwające.
- Zmiany wprowadzone przez przepływ pracy domyślnie nie wyzwalają ponownie przepływów pracy, a łańcuchy są ograniczone do głębokości ośmiu.

## Jeszcze niedostępne

- Wyzwalacze `extension_event` są akceptowane w definicjach z `format_version: 2`, ale żadne zdarzenie rozszerzenia nie jest jeszcze dostarczane do przepływów pracy.
- Przepływy pracy nie mogą wywoływać webhooków, wysyłać e-maili ani odczytywać sekretów.

## Uprawnienia

`workflows.read` pozwala wyświetlać przepływy pracy i historię wykonań. `workflows.manage` pozwala je tworzyć, publikować, włączać, wyłączać, uruchamiać i odtwarzać. Oba uprawnienia są domyślnie przyznane rolom właściciela i administratora.
