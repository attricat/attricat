---
title: Dokumentacja konfiguracji
description: Wszystkie zmienne środowiskowe odczytywane przez API Attricat, proces roboczy plików i aplikację webową.
---

Attricat konfiguruje się zmiennymi środowiskowymi. API i proces roboczy plików odczytują je przy starcie; niczego z tej strony nie można zmienić z aplikacji webowej ani przez API.

Oba procesy weryfikują konfigurację, zanim zaczną przyjmować pracę. Nieprawidłowa wartość, brak wymaganej wartości lub nieosiągalny zasobnik (bucket) S3 przerywają start z błędem, zamiast uruchamiać proces ze zgadniętą wartością domyślną.

Plik `deploy/.env.production.example` w repozytorium jest punktem wyjścia dla wdrożenia. Skopiuj go, zastąp każdą wartość `replace-me` i przechowuj wynik w menedżerze sekretów.

## Minimalna konfiguracja produkcyjna

Produkcyjne API potrzebuje co najmniej:

```sh
DATABASE_URL=postgres://attricat:…@postgres.example:5432/attricat
ATTRICAT_AUTO_MIGRATE=false
ATTRICAT_BOOTSTRAP_OWNER_EMAIL=owner@example.com
SESSION_COOKIE_SECURE=true
ATTRICAT_DEVTOOLS=false

S3_ENDPOINT=https://s3.example.com
S3_REGION=us-east-1
S3_BUCKET=attricat
S3_ACCESS_KEY_ID=…
S3_SECRET_ACCESS_KEY=…
S3_FORCE_PATH_STYLE=false
S3_UPLOAD_TIMEOUT_SECONDS=30
S3_DOWNLOAD_TIMEOUT_SECONDS=30

SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_TLS_MODE=starttls
MAIL_FROM="Attricat <no-reply@example.com>"
PASSWORD_RESET_URL=https://attricat.example.com/password-reset/confirm
WORKSPACE_INVITATION_URL=https://attricat.example.com/invitations/accept
WORKSPACE_ONBOARDING_URL=https://attricat.example.com/onboarding
```

Proces roboczy plików potrzebuje tych samych wartości `DATABASE_URL` i `S3_*` oraz `FILE_WORKER_METRICS_TOKEN`, ponieważ obraz kontenera wiąże jego nasłuch operacyjny poza interfejsem loopback.

## Serwer i baza danych

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `DATABASE_URL` | Wymagana | Ciąg połączenia PostgreSQL. Attricat jest przeznaczony dla PostgreSQL 18. |
| `DATABASE_REQUEST_POOL_CONNECTIONS` | `10` | Liczba połączeń w puli obsługującej żądania HTTP, współdzielonej przez wszystkie obszary robocze. Liczba całkowita od 1 do 100. |
| `DATABASE_TASK_POOL_CONNECTIONS` | `10` | Liczba połączeń w puli używanej przez procesy robocze w tle, współdzielonej przez wszystkie obszary robocze. Liczba całkowita od 1 do 100. API zapisuje w logu przy starcie sumę połączeń pul żądań, zadań i konserwacji. Każdy proces API może też utrzymywać do trzech połączeń poza pulami dla koordynatorów zadań w tle; wymagają one połączeń w trybie sesji, więc tryb transakcyjny PgBouncera nie jest obsługiwany. Zobacz [Uruchom kilka replik API](/pl/operate/deployment/#uruchom-kilka-replik-api). |
| `BIND_ADDR` | `127.0.0.1:3000` | Adres, na którym nasłuchuje API. Obraz kontenera ustawia `0.0.0.0:3000`. |
| `ATTRICAT_AUTO_MIGRATE` | `true` | Stosuje migracje bazy danych przy starcie API. W produkcji ustaw `false` i uruchom raz rolę `migrate` obrazu przed wdrożeniem replik API. |
| `WEB_DIST_DIR` | Nieustawiona | Katalog ze skompilowaną aplikacją webową. Gdy jest ustawiona, API serwuje aplikację pod każdą ścieżką poza `/api` i sondami stanu. Obraz kontenera ustawia `/srv/attricat/web`. |
| `RUST_LOG` | `info` | Filtr logów, np. `api=debug`. |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | Nieustawiona | Punkt końcowy OTLP/gRPC do eksportu śladów z API i procesu roboczego plików. Pozostaw nieustawioną, aby wyłączyć eksport śladów. |
| `ATTRICAT_DEVTOOLS` | `true` w lokalnym środowisku deweloperskim | Włącza Inspektor w aplikacji webowej oraz wpisy czasu SQL przeglądarki rekordów w nagłówku `Server-Timing`. W produkcji ustaw `false`. W innych kompilacjach, także produkcyjnych, przeglądarka może wczytać Inspektor na żądanie po ustawieniu w local storage `attricat.inspector-enabled` na `true` i przeładowaniu strony; czasy SQL nadal wymagają tego ustawienia w API. Treść zapytań SQL i wartości parametrów nigdy nie są ujawniane. |

## Początkowy obszar roboczy i właściciel

Przy starcie API upewnia się, że istnieje jeden obszar roboczy i jego właściciel. Te wartości odczytuje wyłącznie serwer; klienci nie mogą ich przekazać.

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `ATTRICAT_WORKSPACE_ID` | `00000000-0000-4000-8000-000000000002` | UUID obszaru roboczego tworzonego przy starcie. Nie wybiera obszaru roboczego dla żądań HTTP; robi to sesja zalogowanego użytkownika lub token. |
| `ATTRICAT_BOOTSTRAP_WORKSPACE_NAME` | `Default workspace` | Nazwa wyświetlana używana przy pierwszym utworzeniu obszaru roboczego. |
| `ATTRICAT_BOOTSTRAP_OWNER_EMAIL` | `owner@example.test` | Adres e-mail pierwszego właściciela. Jest przycinany i zamieniany na małe litery. Start tworzy użytkownika, członkostwo i przydział roli właściciela, jeśli nie istnieją. We wdrożeniu zawsze ustaw prawdziwy adres. |
| `ATTRICAT_BOOTSTRAP_OWNER_ID` | Losowy UUID | Opcjonalny stały UUID początkowego właściciela. |
| `ATTRICAT_BOOTSTRAP_OWNER_PASSWORD` | Nieustawiona | Opcjonalne pierwsze hasło nowo utworzonego właściciela. Jest haszowane przed zapisaniem i nigdy nie zmienia istniejącego hasła. Podaj je tylko przy pierwszym uruchomieniu, a następnie usuń. |
| `ATTRICAT_DEMO_MODE` | `false` | Tylko dla publicznych wdrożeń demonstracyjnych. Włącza `ATTRICAT_SAMPLE_ACCOUNTS`, nadaje początkowemu obszarowi roboczemu identyfikator logowania `demo.attricat.com` i domyślną nazwę `Demo`, a strona logowania otwiera go z wybranym kontem edytora. Odwiedzający mogą przełączyć się na konto przeglądającego, administratora lub właściciela. Resetowanie hasła, zmiany członków i przypisań ról, przekazanie własności, zaproszenia i nowi użytkownicy są wyłączone, aby nikt nie zablokował wspólnych kont. Jeśli agenci są skonfigurowani, mogą odczytywać katalog i objaśniać zmiany, ale nie mogą ich wprowadzać. Nigdy nie włączaj tego trybu dla obszaru roboczego z prawdziwymi danymi. |
| `ATTRICAT_SAMPLE_ACCOUNTS` | `false` | Tworzy konta `viewer@`, `editor@` i `admin@` w domenie adresu e-mail właściciela, każde z odpowiednią rolą wbudowaną i hasłem z `ATTRICAT_BOOTSTRAP_OWNER_PASSWORD` (która musi wtedy pozostać ustawiona). Strona logowania pokazuje je do wyboru. Każdy, kto ma dostęp do serwera, może się nimi zalogować, więc używaj tej opcji tylko lokalnie i w wersjach demonstracyjnych. |

Identyfikator logowania początkowego obszaru roboczego to `default.local`.

## Sesje i bezpieczeństwo

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `SESSION_COOKIE_SECURE` | `true` | Oznacza pliki cookie sesji i CSRF jako `Secure`. Ustaw `false` tylko w lokalnym środowisku deweloperskim bez HTTPS. |

Czas trwania sesji (osiem godzin), limit prób logowania (pięć niepowodzeń na obszar roboczy i adres e-mail w ciągu piętnastu minut) oraz czas ważności linku do resetowania hasła (30 minut) są stałe.

## Limity HTTP

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `HTTP_REQUEST_TIMEOUT_SECONDS` | `30` | Limit czasu rzeczywistego dla jednego żądania. Żądanie, które go przekroczy, zwraca `503` z kodem `request_timeout`. Zapis mógł się jednak wykonać, więc sprawdź to przed ponowieniem. |
| `HTTP_MAX_CONCURRENT_REQUESTS` | `256` | Maksymalna liczba jednocześnie obsługiwanych żądań na proces API. Nadmiarowe żądania natychmiast otrzymują `503` zamiast czekać w kolejce. Kontrole stanu i otwarte strumienie zdarzeń mają osobne limity. |
| `HTTP_MAX_EVENT_STREAMS` | `128` | Maksymalna liczba otwartych strumieni zdarzeń na proces API. |
| `HTTP_MAX_EVENT_STREAMS_PER_PRINCIPAL` | `4` | Maksymalna liczba otwartych strumieni zdarzeń jednego użytkownika w obszarze roboczym, łącznie dla wszystkich jego sesji i tokenów. |
| `HTTP_EVENT_STREAM_LIFETIME_SECONDS` | `900` | Najdłuższy czas otwarcia strumienia zdarzeń. Potem API go zamyka, a klient łączy się ponownie z `Last-Event-ID`. |
| `HTTP_DEFAULT_BODY_BYTES` | `2097152` (2 MiB) | Domyślny limit treści żądania. Trasy przesyłania używają zamiast niego limitów plików opisanych poniżej. |

## Zachowanie i limity katalogu

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `PREVIEW_MAX_RELATIONSHIP_DEPTH` | `3` | Największa głębokość zagnieżdżenia relacji, o jaką może prosić podgląd rekordu. |
| `PREVIEW_MAX_RELATIONSHIP_ITEMS` | `10` | Maksymalna liczba powiązanych rekordów pokazywanych bezpośrednio w podglądzie dla jednej relacji. |
| `RECORD_MAX_PAGE_SIZE` | `100` | Największy rozmiar strony przy przeglądaniu celów relacji. |
| `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE` | `50` | Największy rozmiar strony list relacji przychodzących. Ogranicza `page_size` w blokach widoku `incoming_relationship_list`. |
| `RELATIONSHIP_FACET_MAX_NODES` | `100` | Maksymalna liczba węzłów zwracanych na stronę faset relacji w przeglądarce rekordów. |
| `DATA_HEALTH_CACHE_TTL_SECONDS` | `300` | Jak długo buforowane są odpowiedzi stanu danych. `0` wyłącza bufor. Każda zapisana zmiana katalogu odświeża je przy następnym żądaniu. |
| `CACHE_BACKEND` | `memory` | Gdzie przechowywane są buforowane definicje: `memory` (w każdym procesie) lub `redis` (wspólnie dla wszystkich replik, które współdzielą wtedy także limity żądań sieciowych rozszerzeń). Przy obu ustawieniach buforowane dane są zawsze poprawne. Gdy Redis jest niedostępny, API działa dalej z pamięci i samo odnawia połączenie. |
| `REDIS_URL` | Brak | Adres połączenia z Redis. Wymagany, gdy `CACHE_BACKEND` ma wartość `redis`. Dla TLS użyj `rediss://` (lub `valkeys://`). |
| `CACHE_MAX_ENTRIES` | `20000` | Maksymalna liczba wpisów bufora w pamięci jednego procesu. |
| `CACHE_KEY_PREFIX` | `attricat` | Początek każdego klucza w Redis; po nim następuje losowy identyfikator bazy danych. Kopia bazy danych zachowuje ten identyfikator, więc kopia działająca obok źródła (na przykład środowisko testowe sklonowane z produkcji) wymaga innego prefiksu albo innej bazy Redis. Po przywróceniu kopii zapasowej w miejsce dotychczasowej bazy zmień go albo wyczyść Redis. |
| `ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS` | `90` | Liczba dni przechowywania historii wartości atrybutów. Co minutę API przez maksymalnie 10 sekund usuwa starszą historię, najwyżej 1000 wierszy na transakcję. Nieudane czyszczenie trafia do logu i jest ponawiane; nie zatrzymuje API. |
| `BLUEPRINT_MIGRATION_PAGE_SIZE` | `100` | Liczba rekordów odczytywanych na stronę podczas migracji schematu w tle. Od 1 do 1000. |
| `BLUEPRINT_MIGRATION_CONCURRENCY` | `4` | Liczba rekordów migrowanych jednocześnie w ramach jednej partii migracji. Od 1 do 64. |

## Praca w tle

API uruchamia proces roboczy zadań dla uruchomień agentów, reguł i przepływów pracy, dostarczania zdarzeń do rozszerzeń, operacji rozszerzeń oraz migracji schematów. Uruchamia także dyspozytor zdarzeń, który dostarcza wewnętrzne zdarzenia domenowe do ich procedur obsługi.

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `TASK_WORKER_ID` | Losowy dla każdego procesu | Stała nazwa procesu roboczego zadań w tym procesie, od 1 do 128 bajtów. |
| `TASK_WORKER_CONCURRENCY` | `8` | Liczba zadań uruchamianych przez ten proces jednocześnie. |
| `TASK_WORKER_POLL_MILLIS` | `250` | Opóźnienie w stanie bezczynności między odpytaniami o nowe zadania. |
| `TASK_WORKER_SHUTDOWN_GRACE_SECONDS` | `30` | Ile czasu przy zamykaniu mają uruchomione zadania na zakończenie. Niezakończone zadania są podejmowane ponownie po wygaśnięciu ich dzierżawy. |
| `EVENT_DISPATCHER_LEASE_SECONDS` | `30` | Jak długo jedna próba dostarczenia utrzymuje dzierżawę. Krótsza dzierżawa zwiększa ryzyko podwójnego dostarczenia. |
| `EVENT_DISPATCHER_RETRY_INITIAL_SECONDS` | `1` | Opóźnienie pierwszej ponownej próby po nieudanym dostarczeniu. Opóźnienia podwajają się po każdym niepowodzeniu. |
| `EVENT_DISPATCHER_RETRY_MAX_SECONDS` | `60` | Górna granica opóźnienia ponownej próby. |
| `EVENT_DISPATCHER_MAX_ATTEMPTS` | `5` | Liczba prób, po której dostarczenie staje się martwą wiadomością (dead letter). |
| `EVENT_DISPATCHER_POLL_MILLIS` | `250` | Opóźnienie między odpytaniami dyspozytora. |

Wszystkie wartości muszą być dodatnimi liczbami całkowitymi. Zero lub wartość niecałkowita przerywa start.

## Magazyn obiektów

Attricat przechowuje przesłane pliki, wygenerowane warianty obrazów, artefakty rozszerzeń i zasoby prezentacyjne w jednym prywatnym zasobniku zgodnym z S3. Wszystkie zmienne `S3_*` są wymagane zarówno przez API, jak i przez proces roboczy plików.

| Zmienna | Opis |
| --- | --- |
| `S3_ENDPOINT` | Bezwzględny adres URL HTTP(S) usługi zgodnej z S3. |
| `S3_REGION` | Region podpisywania. |
| `S3_BUCKET` | Nazwa istniejącego zasobnika. W produkcji Attricat go nie tworzy. |
| `S3_ACCESS_KEY_ID` | Klucz dostępu. |
| `S3_SECRET_ACCESS_KEY` | Klucz tajny. Przechowuj go w menedżerze sekretów. |
| `S3_FORCE_PATH_STYLE` | `true` lub `false`. Użyj `true` dla usług, które nie obsługują zasobników adresowanych przez host wirtualny, takich jak RustFS czy MinIO. |
| `S3_UPLOAD_TIMEOUT_SECONDS` | Limit czasu przesyłania i usuwania. |
| `S3_DOWNLOAD_TIMEOUT_SECONDS` | Limit czasu pobierania i sprawdzania zasobnika przy starcie. |

Przyznaj danym uwierzytelniającym tylko `PutObject`, `GetObject`, `DeleteObject` i `HeadBucket` na tym zasobniku, utrzymuj zasobnik jako prywatny i używaj TLS. Klienci nigdy nie otrzymują kluczy obiektów ani podpisanych adresów URL; każde pobranie przechodzi przez API.

## Przesyłanie i przetwarzanie plików

| Zmienna | Domyślnie | Używa | Opis |
| --- | --- | --- | --- |
| `FILE_UPLOAD_MAX_BYTES` | `52428800` (50 MiB) | API | Największy pojedynczy przesyłany plik. Klucz `max_bytes` atrybutu plikowego może ustawić niższy limit. |
| `FILE_UPLOAD_MAX_FILES` | `10` | API | Maksymalna liczba plików w jednym żądaniu przesyłania. Atrybut z `cardinality = "one"` przyjmuje dokładnie jeden. |
| `FILE_WORKER_ID` | Losowy dla każdego procesu | Proces roboczy plików | Stały identyfikator zapisywany w zadaniach przejmowanych przez ten proces roboczy. |
| `FILE_WORKER_POLL_MILLISECONDS` | `500` | Proces roboczy plików | Opóźnienie między odpytaniami o zadania. |
| `FILE_WORKER_OPERATIONS_BIND_ADDR` | `127.0.0.1:3001` | Proces roboczy plików | Prywatny nasłuch dla `/health/live`, `/health/ready` i `/metrics`. |
| `FILE_WORKER_METRICS_TOKEN` | Nieustawiona | Proces roboczy plików | Token Bearer wymagany dla `/metrics`. Bez niego start odrzuca nasłuch spoza interfejsu loopback. |
| `FILE_WORKER_MAX_PIXELS` | `40000000` | Proces roboczy plików | Największy zdekodowany obraz, w pikselach, jaki przetworzy proces roboczy. |
| `FILE_WORKER_MAX_ATTEMPTS` | `5` | Proces roboczy plików | Liczba prób, po której zadanie przetwarzania trwale kończy się niepowodzeniem. |
| `FILE_DELETE_GRACE_SECONDS` | `86400` (jeden dzień) | Proces roboczy plików | Czas między oznaczeniem pliku bez odwołań jako usuniętego a trwałym usunięciem jego obiektu. |

Zadanie przetwarzania, które trwale się nie powiodło, można ponownie dodać do kolejki. Licznik prób jest zerowany tylko dla zadania, które trwale się nie powiodło:

```sh
docker run --rm --env-file … ghcr.io/attricat/attricat@sha256:… file-worker --retry <job-uuid>
```

## E-mail

Attricat wysyła przez SMTP wiadomości z resetowaniem hasła, zaproszeniami i linkami wdrożeniowymi.

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `SMTP_HOST` | `127.0.0.1` | Host serwera SMTP. |
| `SMTP_PORT` | `1025` | Port serwera SMTP. Zwykle `587` dla `starttls` i `465` dla `implicit`. |
| `SMTP_TLS_MODE` | `starttls` | `starttls` lub `implicit`. `disabled` jest akceptowane tylko dla lokalnego przekaźnika bez uwierzytelniania, takiego jak Mailpit; start odrzuca dane uwierzytelniające w połączeniu z `disabled`. |
| `SMTP_USERNAME` | Nieustawiona | Nazwa użytkownika SMTP. Ustaw jednocześnie nazwę użytkownika i hasło albo żadne z nich. |
| `SMTP_PASSWORD` | Nieustawiona | Hasło SMTP. |
| `MAIL_FROM` | `Attricat <no-reply@attricat.local>` | Adres nadawcy. |
| `PASSWORD_RESET_URL` | Lokalny URL | Bezwzględny adres URL strony potwierdzenia resetowania hasła w aplikacji webowej, np. `https://attricat.example.com/password-reset/confirm`. |
| `WORKSPACE_INVITATION_URL` | Lokalny URL | Bezwzględny adres URL używany w zaproszeniach dla istniejących użytkowników, np. `https://attricat.example.com/invitations/accept`. |
| `WORKSPACE_ONBOARDING_URL` | Lokalny URL | Bezwzględny adres URL używany w linkach wdrożeniowych dla nowych użytkowników, np. `https://attricat.example.com/onboarding`. |

## Agenci

Agenci są wyłączeni, dopóki nie ustawisz `LLM_API_KEY`. Reszta API uruchamia się bez niej normalnie. Dostawca musi być zgodny z API OpenAI Chat Completions. Jakie dane otrzymuje dostawca, opisuje [Agenci i zatwierdzenia](/pl/guides/agents/).

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `LLM_API_KEY` | Nieustawiona | Klucz API dostawcy. Ustaw go tylko w środowisku procesu API. Nigdy nie jest przechowywany ani zapisywany w logach. |
| `LLM_BASE_URL` | `https://api.openai.com/v1` | Bezwzględny bazowy adres URL API Chat Completions. |
| `LLM_MODEL` | `gpt-4o-mini` | Identyfikator modelu. Zapisywany przy każdym uruchomieniu. |
| `LLM_REASONING_EFFORT` | Nieustawiona | Opcjonalny `reasoning_effort` wysyłany z każdym żądaniem. Niektóre modele odrzucają narzędzia funkcyjne, chyba że ma on wartość `none`. |
| `LLM_REQUEST_TIMEOUT_SECONDS` | `60` | Limit czasu jednego żądania do dostawcy, od 1 do 3600. |
| `LLM_RUN_TIMEOUT_SECONDS` | `300` | Limit czasu całego uruchomienia agenta, od 1 do 3600. |
| `LLM_MAX_TOOL_ROUNDS` | `25` | Ile razy agent może wywołać narzędzia i kontynuować, zanim się zatrzyma; licznik zaczyna się od nowa po każdym zatwierdzeniu. Od 1 do 100. |

Te limity agentów są stałe: 32 KiB na wiadomość użytkownika, 16 załączników na wiadomość, 5 MiB na obraz osadzony w wiadomości, osiem rund wywołań narzędzi na uruchomienie, 64 KiB na zserializowany wynik narzędzia i 32 wywołania narzędzi na odpowiedź dostawcy.

## Rozszerzenia

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `EXTENSION_OFFICIAL_REGISTRY` | `attricat/attricat-extensions` | `owner/repository` w GitHubie używane jako wbudowany rejestr rozszerzeń każdego obszaru roboczego. |
| `EXTENSIONS_MODE` | `enabled` | Wyłącznik awaryjny. Ustaw dokładnie `disabled`, aby zatrzymać w całym wdrożeniu wykonywanie rozszerzeń, dostarczanie artefaktów, polecenia, magazyn, wywołania hosta i dostarczanie zdarzeń. Instalacje i przyznane uprawnienia pozostają bez zmian. Każda inna nierozpoznana wartość jest traktowana jak `disabled`. |
| `EXTENSION_DENYLIST` | Pusta | Rozdzielona przecinkami lista identyfikatorów rozszerzeń (`acme.inventory`) lub konkretnych wydań (`acme.inventory@<release-uuid>`) do zablokowania. Sprawdzana przy każdym wywołaniu w czasie działania. |

## Serwer deweloperski aplikacji webowej

| Zmienna | Domyślnie | Opis |
| --- | --- | --- |
| `ATTRICAT_API_URL` | `http://127.0.0.1:3000/api` | Bazowy adres URL API razem z `/api`. Używa go CLI, a serwer deweloperski Vite przekierowuje `/api` do jego źródła (origin). |
| `WEB_PORT` | `5173` | Port serwera deweloperskiego Vite. |

## Lokalne usługi deweloperskie

Lokalny stos z repozytorium uruchamia w kontenerach PostgreSQL, Mailpit (przechwytywanie poczty), Jaeger (ślady) i RustFS (magazyn zgodny z S3). Te zmienne ustawiają tylko ich porty na hoście i dane uwierzytelniające.

| Zmienna | Domyślnie |
| --- | --- |
| `POSTGRES_DB` | `attricat` |
| `POSTGRES_USER` | `postgres` |
| `POSTGRES_PASSWORD` | `postgres` |
| `POSTGRES_PORT` | `5432` |
| `MAILPIT_SMTP_PORT` | `1025` |
| `MAILPIT_UI_PORT` | `8025` |
| `JAEGER_OTLP_GRPC_PORT` | `4317` |
| `JAEGER_UI_PORT` | `16686` |
| `RUSTFS_PORT` | `9000` |
| `RUSTFS_CONSOLE_PORT` | `9001` |

`ATTRICAT_E2E_FIXTURE_EMAIL` i `ATTRICAT_E2E_FIXTURE_PASSWORD` tworzą użytkownika testowego na potrzeby testów end-to-end. Nigdy nie ustawiaj ich w produkcji.

## CLI

Klient wiersza poleceń `acli` odczytuje własne zmienne. Zobacz [dokumentację CLI](/pl/reference/cli/#konfiguracja).
