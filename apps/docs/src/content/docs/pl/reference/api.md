---
title: Dokumentacja API
description: Jak uwierzytelniać się w HTTP API Attricat, jakie obowiązują konwencje i jakie trasy są dostępne w poszczególnych obszarach.
---

Aplikacja webowa Attricat jest zbudowana na tym samym HTTP API, które możesz wywoływać ze skryptów i integracji. [CLI](/pl/reference/cli/) opakowuje niemal całe API.

## Bazowy adres URL

Gdy API serwuje aplikację webową (tak jak w obrazie kontenera), trasy API są dostępne zarówno pod ich własnymi ścieżkami, jak i pod `/api`. Na przykład `GET https://catalog.example.com/api/blueprints` i `GET https://catalog.example.com/blueprints` to ta sama trasa. Integracje powinny używać prefiksu `/api`.

## Uwierzytelnianie

**Osobisty token API**: wyślij `Authorization: Bearer cat_pat_…`. To token wyznacza obszar roboczy. Tokeny są zalecanym sposobem integracji.

**Sesja przeglądarki**: `POST /auth/login` ustawia plik cookie HttpOnly `catalog_session` oraz czytelny plik cookie `catalog_csrf`. Każde niebezpieczne żądanie (wszystko poza `GET`, `HEAD`, `OPTIONS`) wysłane z plikiem cookie musi także wysłać `X-Catalog-Csrf` z wartością pliku cookie CSRF.

Klienci nigdy nie wysyłają identyfikatora obszaru roboczego; zawsze pochodzi on z tokenu lub sesji.

Trasy publiczne, które nie wymagają danych uwierzytelniających: `/health`, `/health/live`, `/health/ready`, `POST /auth/discover`, `POST /auth/login`, `POST /auth/password-reset`, `POST /auth/password-reset/confirm` i `POST /onboarding/complete`.

## Konwencje

- Żądania i odpowiedzi są w formacie JSON, chyba że trasa stanowi inaczej. Udane puste odpowiedzi mają kod `204`.
- Błędy zwracają status HTTP i treść w postaci `{"error": {"code": "entity_schema_mismatch", "message": "…"}}`. Dopasowuj po `code`, a nie po komunikacie. Niektóre błędy dodają obiekt `error.details` z danymi, na podstawie których możesz działać, np. encją, która już ma dany klucz unikalny.
- `401` oznacza brak ważnych danych uwierzytelniających; `403` oznacza, że dane uwierzytelniające nie mają uprawnienia lub zakresu.
- `422` oznacza, że żądanie zostało zrozumiane, ale jest nieprawidłowe, np. schemat się nie kompiluje albo wartość nie spełnia swojego schematu walidacji.
- `409` oznacza konflikt z bieżącym stanem, np. przekroczenie limitu krotności relacji.
- Każda odpowiedź zawiera `x-request-id`. W tym nagłówku możesz wysłać własny UUID; pojawi się on w dzienniku audytu.
- Każda odpowiedź ma nagłówek `Server-Timing` z łącznym czasem po stronie serwera.

## Typowe kody błędów

| Kod | Status | Znaczenie |
| --- | --- | --- |
| `invalid_blueprint_definition` | 422 | TOML schematu się nie kompiluje. Komunikat podaje przyczynę. |
| `attribute_value_schema_mismatch` | 422 | Wartość nie spełnia `value_schema` swojego atrybutu. |
| `entity_schema_mismatch` | 422 | Encja nie spełnia swojego `entity_schema` w którymś kontekście. |
| `relationship_cardinality_conflict` | 409 | Zapis relacji przekracza `cardinality` lub `target_cardinality` albo nadaje encji w drzewie (`tree`) drugiego rodzica. |
| `relationship_target_type_mismatch` | 422 | Schemat powiązanej encji nie jest dozwolony przez `target_blueprint` ani `target_blueprints`. |
| `relationship_cycle` | 409 | Powiązanie zamknęłoby cykl w relacji `acyclic` lub `tree`. `details.path` wymienia identyfikatory encji wzdłuż cyklu. |
| `unique_key_conflict` | 409 | Inna encja ma już te wartości klucza unikalnego. `details` zawiera `key`, `context`, `values` i `conflicting_entity_id`. |
| `unique_key_duplicates` | 409 | Publikacja nowego klucza unikalnego nie powiodła się, bo istniejące encje współdzielą wartości. `details.duplicates` je wymienia. |
| `relationship_hierarchy_violations` | 409 | Publikacja `acyclic` lub `tree` nie powiodła się, bo istniejące powiązania zawierają cykle lub nadmiarowych rodziców. `details` je wymienia. |
| `stale_entity` | 409 | `expected_updated_at` nie odpowiada już encji. Wczytaj ją ponownie i spróbuj jeszcze raz. |
| `relationship_path_sort_requires_single_result_version` | 422 | Sortowanie według powiązanej wartości w kilku wersjach schematu. |
| `file_processing` | 409 | Plik nie jest jeszcze gotowy do pobrania. |
| `approval_already_decided` | 409 | Wywołanie narzędzia przez agenta zostało już zatwierdzone lub odrzucone. |
| `service_unavailable` | 503 | Dla tras agentów: nie skonfigurowano dostawcy AI. |

## Trasy

### Schematy

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `GET` | `/blueprints` | Opublikowane schematy encji. |
| `GET` | `/blueprints/catalogue` | Wszystkie rodziny schematów i wersje. |
| `POST` | `/blueprints` | Tworzy schemat (pierwszy szkic) z `{"definition": "<toml>"}`. |
| `GET`, `POST` | `/blueprints/{id}/versions` | Wyświetla wersje lub tworzy kolejny szkic. |
| `GET` | `/blueprints/{id}` | Bieżąca opublikowana wersja. |
| `GET` | `/blueprints/{id}/versions/{version}` | Konkretna wersja, w tym szkice. |
| `POST` | `/blueprints/{id}/versions/{version}/publish` | Publikuje szkic. |
| `GET` | `/blueprints/by-code/{code}` i `/blueprints/by-code/{code}/versions/{version}` | Wyszukiwanie po kodzie. |
| `GET` | `/blueprints/{id}/migration-batches` | Partie migracji w tle. |
| `GET` | `/blueprints/{id}/connector-jobs` | Zadania konektorów. |
| `POST` | `/blueprint-connector-jobs/{id}/run` | Uruchamia zadanie konektora z `{"idempotency_key": "…"}`. |
| `GET`, `POST` | `/reusable-attributes`; `POST /reusable-attributes/{id}/versions`; `POST /reusable-attribute-revisions/{id}/publish`; `GET`, `POST /reusable-attribute-groups` | Atrybuty wielokrotnego użytku. |

### Encje

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `POST` | `/v1/entities` | Tworzy encję z wartościami oraz opcjonalnymi `system_tags` i `system_metadata`. |
| `POST` | `/v1/entities/batch` | Tworzy, aktualizuje i usuwa kilka encji naraz: zapisują się wszystkie zmiany albo żadna. Zobacz [Zmiany wsadowe](#zmiany-wsadowe). |
| `GET`, `PUT` | `/v1/entities/{id}` | Odczytuje lub aktualizuje formularz encji: wartości, relacje, usunięcia, adnotacje. |
| `GET`, `DELETE` | `/entities/{id}` | Odczytuje lub usuwa encję. |
| `POST` | `/v1/entities/search` | Wyszukiwanie. Zobacz poniżej. |
| `POST` | `/v1/entities/facets/relationship-tree/children` | Jedna strona elementów podrzędnych fasety relacji, z liczebnościami. |
| `GET` | `/entities/{id}/preview` | Wartości w poszczególnych kontekstach, z powiązanymi encjami osadzonymi w odpowiedzi. |
| `GET` | `/entities/{id}/resolved-preview?context_id=…` | Wartości rozstrzygnięte w jednym kontekście, wraz z kontekstem, z którego pochodzi każda z nich. |
| `GET` | `/entities/{id}/hierarchy` | Przodkowie wzdłuż relacji. |
| `POST` | `/v1/entities/{id}/incoming-relationships` | Encje wskazujące na tę encję. |
| `GET` | `/entities` | Przeglądanie celów relacji. |
| `GET` | `/entities/{id}/values/current` | Bieżące wartości bezpośrednie i powiązania. |
| `POST` | `/entities/{id}/values` | Dołącza wartości. |
| `POST` | `/entities/{id}/relationships/replace`, `/remove` | Zastępuje lub usuwa cele relacji. |
| `GET` | `/entities/{id}/changes` | Historia zmian. Dodaj `limit` (od 1 do 50) i `offset`, aby stronicować. |
| `GET` | `/entities/{id}/values/history` | Historia wartości. Stronicowanie jak wyżej. |
| `POST` | `/entities/{id}/values/history/{history_id}/restore` | Przywraca wcześniejszą wartość. |
| `POST` | `/v1/entities/{id}/blueprint-migration/preview` | Sprawdza migrację do bieżącej wersji. |
| `POST` | `/v1/entities/{id}/blueprint-migration` | Migruje. |
| `POST` | `/v1/entities/{id}/reusable-attributes`, `/v1/entities/{id}/reusable-attribute-groups/{group_id}` | Dołącza atrybut lub grupę atrybutów wielokrotnego użytku. |

### Zmiany wsadowe

Niektóre zmiany mają sens tylko razem: wydanie nowej wersji dokumentu i oznaczenie poprzedniej jako zastąpionej albo zarejestrowanie przemieszczenia i aktualizacja bieżącej lokalizacji obiektu. Wyślij je jako jeden wsad, aby błąd nie zostawił zapisanej tylko połowy zmiany.

```json
POST /api/v1/entities/batch
{
  "operations": [
    {
      "op": "create",
      "entity_id": "5b0b8c55-0c55-4cc5-9a0f-4a4c3d1a2b10",
      "blueprint": { "code": "document_revision" },
      "values": [
        { "kind": "scalar", "attribute_code": "label", "context_id": null, "value": "B" },
        { "kind": "scalar", "attribute_code": "status", "context_id": null, "value": "released" },
        { "kind": "relationship", "attribute_code": "previous", "context_id": null,
          "target_entity_id": "1f7e2d9a-6a3e-4a8a-9d0c-2f8d4f7f9e11" }
      ]
    },
    {
      "op": "update",
      "entity_id": "1f7e2d9a-6a3e-4a8a-9d0c-2f8d4f7f9e11",
      "expected_updated_at": "2026-10-01T09:30:00Z",
      "values": [
        { "kind": "scalar", "attribute_code": "status", "context_id": null, "value": "superseded" }
      ]
    }
  ]
}
```

- `op` to `create`, `update` lub `delete`. `create` przyjmuje te same pola co `POST /v1/entities` oraz opcjonalny wybrany przez Ciebie `entity_id`, aby kolejne operacje mogły powiązać się z nową encją. `update` przyjmuje te same pola co `PUT /v1/entities/{id}`. `delete` przyjmuje `entity_id`.
- `expected_updated_at` w `update` i `delete` jest warunkiem wstępnym: jeśli encja zmieniła się od odczytu, wsad kończy się błędem `409 stale_entity`. Zmiany statusu go wymagają, tak jak pojedyncza aktualizacja.
- Operacje są wykonywane po kolei, a każda jest sprawdzana jak odpowiednie pojedyncze żądanie w chwili wykonania: wartości, schematy walidacji, przejścia statusów, reguły relacji i klucze unikalne. Ułóż je tak, aby każda była poprawna w swojej kolejności.
- Wsad ma od 1 do 50 operacji i do 1000 wartości, powiązań i usunięć. Encja może wystąpić tylko w jednej operacji.
- Każda operacja wymaga własnego uprawnienia: `entities.write` dla encji, aby ją zaktualizować, `entities.delete`, aby ją usunąć, oraz `entities.write` w całym obszarze roboczym, aby utworzyć encję. Jeśli któregoś brakuje, nic nie jest wykonywane, a odpowiedź to `403`.

Udany wsad zwraca `200` z jednym wynikiem na operację, np. `{"op": "update", "entity": {…}}` lub `{"op": "delete", "entity_id": "…"}`. Każda operacja jest zapisywana w dzienniku audytu i emituje swoje zwykłe zdarzenie, ale dopiero po zapisaniu całego wsadu.

Jeśli operacja się nie powiedzie, nic nie zostaje zapisane. Odpowiedź ma status i kod błędu tej operacji, komunikat zaczyna się od `operation <index>:`, a `error.details` zawiera `operation_index` i `entity_id`. Popraw tę operację i wyślij cały wsad ponownie. Odpowiednikiem w CLI jest `acli entity batch --operations <plik>`.

### Wyszukiwanie

```json
POST /api/v1/entities/search
{
  "blueprint": { "code": "product", "version": 3 },
  "query": "colors.name:red linen",
  "filters": [{ "field": "price", "operator": "lt", "value": 50 }],
  "relationship_tree_facets": [{
    "source_relationship_field": "categories",
    "hierarchy_field": "parent",
    "context_id": "00000000-0000-4000-8000-000000000001",
    "selected_target_ids": ["e8b7a8d3-c954-4c0f-b658-0f686ba466a3"]
  }],
  "system_tags": ["needs-review"],
  "sort": { "field": "title", "direction": "asc" },
  "page": { "size": 50, "cursor": null }
}
```

- Pomiń `blueprint.version`, aby przeszukać wszystkie opublikowane wersje.
- `query` używa [składni wyszukiwania](/pl/guides/search-syntax/).
- Operatory filtrów to `eq`, `contains`, `starts_with`, `gt`, `gte`, `lt` i `lte`. `field` może być ścieżką relacji o maksymalnie trzech krokach.
- `sort.field` musi być kolumną skalarną w widoku tabeli schematu, `blueprint_version` lub `publication_status` (z `context_code` wskazującym kanał).
- Odpowiedzi zawierają `items`, `next_cursor`, `result_version_scope`, a dla każdego elementu `table_values` i `match_explanations`. Przekaż `next_cursor` z powrotem jako `page.cursor` przy tym samym sortowaniu.
- `include_total` zwraca łączną liczbę wyników dla pierwszej strony, ograniczoną do 500.

### Pliki

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `POST` | `/entities/{entity_id}/file-attributes/{attribute_code}/uploads` | Przesyłanie z `multipart/form-data`: jedna lub więcej części `files` i opcjonalna część `context_id`. Zwraca `201`. |
| `GET` | `/files/{file_id}` | Metadane i stan przetwarzania. |
| `GET` | `/files/{file_id}/download` | Oryginalny plik. Obsługuje jeden zakres `Range`. |
| `GET` | `/files/{file_id}/variants/{kind}/download` | Wariant `thumbnail` lub `display`. |

Pobieranie zwraca `409 file_processing`, dopóki plik nie ma stanu `ready`.

### Konteksty i publikacja

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `GET`, `POST` | `/contexts` | Wyświetla lub tworzy konteksty. |
| `GET` | `/contexts/{code}` | Odczyt po kodzie. |
| `PUT`, `DELETE` | `/contexts/id/{id}` | Aktualizuje lub usuwa. |
| `GET` | `/publication-channels` | Konteksty będące kanałami. |
| `PUT` | `/publication-channels/{context_id}` | `{"enabled": true}`, aby uczynić kontekst kanałem. |
| `GET`, `POST` | `/v1/entities/{id}/publications` | Stan publikacji lub publikacja z `{"context_id": "…"}`. |
| `POST` | `/v1/entities/{id}/publications/unpublish` | Wycofuje publikację z jednego kanału. |
| `POST` | `/v1/entities/{id}/publications/publish-all` | Publikuje we wszystkich kanałach. |
| `POST` | `/blueprints/{id}/versions/{version}/entity-publications`, `…/publish-all` | Publikuje wszystkie encje danej wersji. |

### Zapisane wyszukiwania

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `GET`, `POST` | `/saved-views` | Wyświetla (opcjonalnie z `q`) lub tworzy nazwane wyszukiwania. |
| `GET`, `PUT`, `DELETE` | `/saved-views/{id}` | Odczytuje, aktualizuje lub usuwa własne wyszukiwania. |
| `POST`, `GET` | `/view-state-links`, `/view-state-links/{id}` | Tworzy lub odczytuje migawkę do udostępnienia. |

Stan ma maksymalnie 32 KiB i używa kluczy adresu URL przeglądarki encji: `blueprint`, `version`, `allVersions`, `query`, `context`, `locked`, `sort`, `attributeFilters` i `relationshipFacets`.

### Tłumaczenia

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `GET` | `/lexicon/entries` | Lista wpisów, opcjonalnie dla jednego języka `language` (`entities.read`). |
| `PUT` | `/lexicon/entries` | Utworzenie lub zastąpienie wpisu: `key`, opcjonalny `context`, `language`, opcjonalna `plural_category` (domyślnie `other`) i `text` (`blueprints.write`). |
| `DELETE` | `/lexicon/entries` | Usunięcie wpisu wskazanego parametrami `key`, `context`, `language` i `plural_category` (`blueprints.write`). |
| `GET` | `/lexicon/export` | Eksport jednego języka `language` jako pliku importu (`blueprints.read`). |
| `POST` | `/lexicon/import` | Import pliku dla jednego języka; `mode=replace` usuwa też wpisy, których nie ma w pliku (`blueprints.write`). |
| `GET` | `/lexicon/report` | Nieprzetłumaczone odwołania, brakujące kategorie liczby mnogiej i osierocone wpisy dla języków `languages` rozdzielonych przecinkami (`blueprints.read`). |

Zobacz [Tłumaczenie etykiet](/pl/builders/translations/).

### Reguły i przepływy pracy

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `POST` | `/rules/validate` | Weryfikuje TOML reguły. |
| `GET`, `POST` | `/rules` | Wyświetla lub tworzy reguły. |
| `GET` | `/rules/{id}` | Odczytuje regułę. |
| `POST` | `/rules/{id}/versions/{version}/publish`, `/enable`; `/rules/{id}/disable` | Cykl życia. |
| `POST` | `/rules/{id}/run-now` | `{"entity_id": null, "dry_run": false, "idempotency_key": "…"}` |
| `GET` | `/rule-runs`, `/rule-findings` | Uruchomienia i ustalenia. |
| `POST` | `/rule-runs/{id}/replay`, `/rule-findings/{id}/acknowledge` | Ponawia martwą wiadomość; potwierdza ustalenie. |
| `POST` | `/workflows/validate` | Weryfikuje TOML przepływu pracy. |
| `GET`, `POST` | `/workflows`, `/workflows/{id}/versions` | Wyświetla lub tworzy przepływy pracy i ich wersje. |
| `POST` | `/workflows/{id}/versions/{version}/publish`, `/enable`; `/workflows/{id}/disable` | Cykl życia. |
| `POST` | `/workflows/{id}/run-now` | Uruchomienie ręczne. |
| `GET` | `/workflow-runs` | Historia uruchomień. |
| `GET` | `/workflow-runs/{id}/targets` | Wyniki akcji `referencing_entities_update` dla poszczególnych rekordów. |
| `POST` | `/workflow-runs/{id}/replay` | Ponawia martwą wiadomość. |

### Agenci

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `GET`, `POST` | `/agent/conversations` | Wyświetla lub tworzy rozmowy. |
| `GET`, `PATCH`, `DELETE` | `/agent/conversations/{id}` | Odczytuje, zmienia nazwę lub archiwizuje. |
| `GET`, `POST` | `/agent/conversations/{id}/messages` | Wiadomości. Wysłanie wiadomości rozpoczyna uruchomienie i zwraca `202`. |
| `POST` | `/agent/conversations/{id}/uploads` | Przesyła załącznik. |
| `GET` | `/agent/conversations/{id}/runs` | Uruchomienia. |
| `GET` | `/agent/runs/{run_id}/events` | Zdarzenia wysyłane przez serwer (SSE). Połącz ponownie z `Last-Event-ID`. |
| `GET` | `/agent/approvals` | Oczekujące zatwierdzenia. |
| `POST` | `/agent/tool-calls/{id}/approve`, `/reject` | Decyzja. |

### Obszar roboczy i dostęp

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `POST` | `/auth/discover`, `/auth/login`, `/auth/renew`, `/auth/logout` | Sesje. |
| `GET` | `/auth/session` | Bieżąca tożsamość i możliwości. |
| `GET`, `POST`, `DELETE` | `/personal-access-tokens[/{id}]` | Osobiste tokeny API. Sekret jest zwracany tylko raz. |
| `GET` | `/workspace/members`; `PUT /workspace/members/{id}` | Członkowie i ich stan. |
| `POST`, `DELETE` | `/workspace/members/{id}/grants[/{grant_id}]` | Przydziały ról. |
| `POST` | `/workspace/members/{id}/transfer-ownership` | Przeniesienie własności. |
| `GET`, `POST`, `PUT` | `/workspace/roles[/{id}]`, `…/duplicate`, `…/retire` | Role. |
| `GET` | `/workspace/permissions`, `/workspace/assignable-roles`, `/workspace/token-permissions`, `/workspace/grant-targets/{scope}` | Dane pomocnicze dla ekranów administracyjnych. |
| `GET`, `POST`, `DELETE` | `/workspace/invitations[/{id}]`; `POST /workspace/invitations/accept` | Zaproszenia. |
| `POST` | `/workspace/users` | Tworzy użytkownika. |
| `GET`, `PUT` | `/workspace/navigation`; `GET /workspace/navigation/sidebar` | Skróty na pasku bocznym. |
| `GET` | `/directory` | Użytkownicy i zespoły, do których mogą odwoływać się atrybuty użytkownika lub zespołu (`entities.read`). |
| `GET`, `POST`, `PATCH`, `DELETE` | `/workspace/teams[/{id}]` | Zespoły (`members.manage`). `PATCH` przyjmuje `name` lub `member_user_ids`, które zastępuje cały skład. |
| `GET` | `/audit-events` | Dziennik audytu. |

### Rozszerzenia

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `GET`, `POST`, `DELETE` | `/extension-registries[/{id}]`; `GET /extension-registries/discover`; `GET /extension-registries/extensions/{owner}/{repo}` | Rejestry i wykrywanie. |
| `GET`, `POST` | `/extensions` | Wyświetla lub instaluje. |
| `POST` | `/extensions/sideload` | Instaluje przesłane archiwum `application/zstd`. |
| `GET`, `DELETE` | `/extensions/{id}` | Odczytuje lub usuwa. |
| `POST` | `/extensions/{id}/upgrade`, `/enable`, `/disable`, `/quarantine` | Cykl życia. |
| `PUT` | `/extensions/{id}/configure` | Konfiguracja. |
| `POST`, `DELETE` | `/extensions/{id}/grants[/{kind}/{grant_id}]` | Przyznane uprawnienia. |
| `GET` | `/extensions/runtime` | Włączone kontrybucje klienta z rozszerzeń. |
| `PUT` | `/workspace/extensions-mode` | `{"enabled": false}` wyłącza wszystkie rozszerzenia w obszarze roboczym. |
| `GET`, `PUT` | `/workspace/extension-layout` | Układ kontrybucji. |
| `GET`, `PUT`, `DELETE` | `/workspace/extension-secrets[/{name}]` | Sekrety (przy odczycie tylko nazwy). |
| `POST` | `/extensions/{id}/operations` | Rozpoczyna operację. |
| `GET` | `/extension-operation-runs[/{id}]`, `…/artifacts`, `…/deliveries` | Uruchomienia, wyniki, dostarczenia. |
| `GET` | `/extension-operation-runs/{id}/artifacts/{artifact_id}/download` | Pobiera wynik. |
| `POST`, `GET`, `PATCH` | `/extensions/{id}/operation-schedules`, `/extension-operation-schedules[/{id}]` | Harmonogramy. |

### Pakiety rozwiązań

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `POST` | `/solution-packs/inspect` | Sprawdza archiwum `application/zstd`. |
| `POST` | `/solution-packs/plans?prefix=…&blueprint_publication=draft\|publish` | Tworzy plan. |
| `GET` | `/solution-packs/plans/{id}` | Odczytuje plan. |
| `POST` | `/solution-packs/plans/{id}/apply` | Stosuje plan. |
| `GET` | `/solution-packs/applications[/{id}]` | Historia zastosowań. |
| `GET`, `POST` | `/solution-packs/applications/{id}/checks` | Uruchomienia kontroli. |
| `GET` | `/presentation-assets[/{id}[/content]]` | Zasoby prezentacyjne. |

### Stan i operacje

| Metoda | Ścieżka | Opis |
| --- | --- | --- |
| `GET` | `/health/live`, `/health/ready` | Sondy. |
| `GET` | `/system/health` | Uruchomiona wersja API, gałąź i commit źródła (`data_health.read`). |
| `GET` | `/metrics` | Metryki Prometheus. |
| `GET` | `/data-health/summary`, `/blueprints`, `/freshness`, `/completeness`, `/contexts`, `/relationships`, `/storage`, `/background-processing` | Stan danych. |
| `POST` | `/data-health/refresh` | Czyści bufor stanu danych. |
| `GET` | `/event-deliveries/dead-letters` | Nieudane dostarczenia zdarzeń. |
| `POST` | `/event-deliveries/{consumer_id}/{event_id}/replay` | Ponawia jedno dostarczenie. |

Uprawnienia wymagane w poszczególnych obszarach opisuje [dokumentacja uprawnień](/pl/reference/permissions/).
