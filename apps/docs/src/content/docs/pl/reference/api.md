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
- Błędy zwracają status HTTP i treść w postaci `{"error": {"code": "entity_schema_mismatch", "message": "…"}}`. Dopasowuj po `code`, a nie po komunikacie.
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
| `relationship_cardinality_conflict` | 409 | Zapis relacji przekracza `cardinality` lub `target_cardinality`. |
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
| `GET` | `/metrics` | Metryki Prometheus. |
| `GET` | `/data-health/summary`, `/blueprints`, `/freshness`, `/completeness`, `/contexts`, `/relationships`, `/storage`, `/background-processing` | Stan danych. |
| `POST` | `/data-health/refresh` | Czyści bufor stanu danych. |
| `GET` | `/event-deliveries/dead-letters` | Nieudane dostarczenia zdarzeń. |
| `POST` | `/event-deliveries/{consumer_id}/{event_id}/replay` | Ponawia jedno dostarczenie. |

Uprawnienia wymagane w poszczególnych obszarach opisuje [dokumentacja uprawnień](/pl/reference/permissions/).
