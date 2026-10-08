---
title: Monitorowanie
description: Obserwuj stan danych katalogu, kolejki w tle, metryki, ślady i nieudane dostarczenia zdarzeń.
---

## Stan danych

**Zarządzanie → Stan danych** podsumowuje stan katalogu:

- **Karty podsumowania**: nieaktualne rekordy, aktywne rekordy, zaległe rekordy oraz relacje wskazujące na usunięte rekordy.
- **Pamięć masowa**: rozmiar każdej tabeli i łączny.
- **Stan schematów**: dla każdego schematu liczba rekordów, liczba rekordów nieaktualnych lub zaległych oraz najstarsza aktualizacja.
- **Rozkład świeżości**: jak dawno aktualizowano wartości.
- **Kompletność domyślna**: ile rekordów ma w kontekście domyślnym każde pole wymagane przez schemat.
- **Pokrycie kontekstów**: ile rekordów ma własne wartości w każdym kontekście.
- **Integralność relacji**: powiązania, których cel został usunięty.

**Nieaktualne po** określa, co jest uznawane za zaległe, od 1 do 3650 dni (domyślnie 90). Wyniki są domyślnie buforowane przez pięć minut (`DATA_HEALTH_CACHE_TTL_SECONDS`); **Odśwież** czyści bufor.

Rozszerzenia mogą dodawać karty do tej strony.

## Przetwarzanie w tle

**Zarządzanie → Przetwarzanie w tle** pokazuje kolejkę zadań API dla każdego typu zadania: zadania w kolejce, uruchomione, nieudane, wygasłe dzierżawy oraz czas oczekiwania najstarszego zadania gotowego do uruchomienia. Strona odświeża się co 30 sekund.

- **W kolejce** obejmuje ponowne próby zaplanowane na później.
- **Wygasłe dzierżawy** to zadania, których proces roboczy przestał odpowiadać; przejmuje je inny proces roboczy.
- **Nieudane** to martwe wiadomości (dead letters), które wymagają uwagi.

Przetwarzanie plików ma własną kolejkę i nie jest tu pokazywane; obserwuj je przez metryki.

## Metryki

Oba procesy udostępniają metryki Prometheus. Trzymaj je w sieci prywatnej.

| Proces | Punkt końcowy | Dostęp |
| --- | --- | --- |
| API | `GET /metrics` | Sesja lub token z `data_health.read`. |
| Proces roboczy plików | `GET /metrics` na nasłuchu operacyjnym (port 3001) | `Authorization: Bearer $FILE_WORKER_METRICS_TOKEN`. |

Etykiety mają ograniczony zbiór wartości: trasy są raportowane jako szablony, a nazwa pliku, identyfikator ani URL nigdy nie są etykietą. Jedyną serią z etykietą obszaru roboczego jest `catalog_event_delivery_queue_depth` (`workspace_id`).

Przydatne serie:

| Seria | Na co zwracać uwagę |
| --- | --- |
| `catalog_database_ready`, `catalog_object_store_ready` | Zero przez dwa interwały sondy. |
| `catalog_task_queue_depth`, `catalog_task_queue_oldest_age_seconds`, `catalog_task_queue_retries` | Rosnąca głębokość, stare zadania, jakiekolwiek martwe wiadomości. |
| `catalog_file_worker_queue_depth`, `catalog_file_worker_oldest_age_seconds`, `catalog_file_worker_retries` | To samo dla przetwarzania plików. |
| `catalog_file_worker_jobs_failed_total` | Wzrosty. |
| `catalog_event_delivery_queue_depth{workspace_id,consumer,status}` | Rosnące `pending`, jakiekolwiek `dead_letter`. |
| `catalog_event_deliveries_total{outcome}` | Wzrosty `dead_letter`. |
| `catalog_extension_operation_runs`, `catalog_extension_operation_oldest_age_seconds` | Zablokowane operacje rozszerzeń. |
| `catalog_file_uploads_total`, `catalog_file_downloads_total`, `catalog_object_store_operations_total` | Wyniki z błędem. |
| `catalog_value_history_cleanup_total{outcome}` | `failed`. Powtarzające się `budget_exhausted` oznacza, że każdy 10-sekundowy przebieg kończy się, zanim usunie całą starą historię. |
| `catalog_upload_cleanup_total{outcome}` | `failed`. Nieudane usunięcia porzuconych przesłanych plików są ponawiane. |
| `catalog_query_cache_requests_total{namespace,outcome}` | Malejący udział `hit` i `remote_hit` względem `miss`. |
| `catalog_query_cache_redis_connected` | `0` na którejkolwiek replice: utraciła połączenie z Redis i do jego odnowienia buforuje tylko w pamięci. |
| `catalog_query_cache_redis_circuit_opened_total` | Wzrosty: Redis jest połączony, ale stale przekracza limit czasu lub gubi polecenia, więc repliki co jakiś czas pomijają go na kilka sekund. |
| `catalog_db_round_trips_per_operation{scope}` | Rosnąca liczba zapytań do bazy danych na trasę żądania, rodzaj zadania lub pętlę w tle. |
| `catalog_db_round_trips_total{scope}` | Rosnące tempo dla `unscoped` albo pętla w tle, której tempo rośnie, gdy katalog jest bezczynny. |

Wskaźnik dostarczania zdarzeń jest odświeżany co pięć sekund. Dla każdego odbiorcy, który ma dostarczenia w obszarze roboczym, raportowane są wszystkie cztery statusy, a statusy bez dostarczeń mają wartość `0`.

Każda trasa żądania, rodzaj zadania i pętla w tle to osobny `scope` liczby zapytań. Aby zapisywać tę liczbę w logu dla każdej operacji, ustaw `RUST_LOG=catalog_repository::round_trips=debug`.

Sugerowane alerty: powiadamiaj dyżurnego, gdy wskaźnik gotowości wynosi zero przez dwa interwały, gdy liczba martwych wiadomości lub nieudanych zadań jest większa od zera albo gdy rosną liczniki niepowodzeń. Ostrzegaj, gdy najstarszy element w kolejce jest starszy niż pięć minut przez dziesięć minut lub gdy kolejka rośnie przez piętnaście minut. Alarmuj, jeśli metryki znikną na dwa interwały pobierania. Dostosuj progi do wolumenu importów i eksportów.

## Czas obsługi żądań

Każda odpowiedź API ma nagłówek `Server-Timing` z łącznym czasem obsługi (`app;dur=…`). Narzędzia deweloperskie przeglądarki pokazują go w szczegółach czasu żądania. Odpowiedzi stanu danych informują też, czy użyto bufora.

## Logi i ślady

API i proces roboczy plików zapisują ustrukturyzowane zdarzenia sterowane przez `RUST_LOG`. Każde żądanie ma `x-request-id`, zwracany w odpowiedzi i zapisywany w dzienniku audytu, dzięki czemu możesz znaleźć wiersze logu dla zmiany zgłoszonej przez użytkownika.

Ustaw `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, aby wysyłać ślady do kolektora OTLP/gRPC.

Logi i ślady nigdy nie zawierają kluczy obiektów, nazw plików, haseł ani tokenów.

## Nieudane dostarczenia zdarzeń

Zmiany w katalogu tworzą wewnętrzne zdarzenia, które konsumują przepływy pracy, reguły i rozszerzenia. Dostarczenie, które stale się nie udaje, domyślnie po pięciu próbach staje się **martwą wiadomością (dead letter)**.

1. Wyświetl martwe wiadomości (wymaga `data_health.read`):

   ```sh
   acli event dead-letters
   ```

   Każdy wpis zawiera identyfikatory konsumenta i zdarzenia, nazwę konsumenta, typ zdarzenia, liczbę prób, czas niepowodzenia i ostatni błąd.

2. Usuń przyczynę: w rozszerzeniu, w zewnętrznej zależności lub w danych.
3. Upewnij się, że ponowne uruchomienie procedury obsługi jest bezpieczne.
4. Ponów dostarczenie (wymaga `roles.manage`):

   ```sh
   acli event replay <consumer-id> <event-id>
   ```

Ponowienie nie zeruje licznika prób, więc dostarczenie, które znów się nie powiedzie, od razu wraca do martwych wiadomości. Nie ponawiaj wielokrotnie tego samego dostarczenia bez usunięcia przyczyny.

Uruchomienia reguł i przepływów pracy oraz operacje rozszerzeń mają własne martwe wiadomości i polecenia ponawiania; zobacz [Reguły](/pl/builders/rules/), [Przepływy pracy](/pl/builders/workflows/) i [Operacje](/pl/extensions/operations/).

## Ponawianie przetwarzania plików

Plik, którego przetwarzanie trwale się nie powiodło, można ponownie dodać do kolejki:

```sh
docker run --rm --env-file attricat.env ghcr.io/attricat/attricat@sha256:… file-worker --retry <job-uuid>
```
