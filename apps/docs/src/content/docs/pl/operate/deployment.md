---
title: Wdrażanie Attricat
description: Uruchom obraz Attricat produkcyjnie, z migracjami, kontrolami stanu, wdrażaniem, wycofywaniem i rotacją sekretów.
---

Attricat jest dostarczany jako jeden obraz kontenera. Ten sam obraz uruchamia trzy role:

| Rola | Uruchamia | Repliki |
| --- | --- | --- |
| `migrate` | Stosuje migracje bazy danych, a następnie kończy działanie. | Jedna, przed każdym wdrożeniem. |
| `api` | Aplikację webową pod `/`, API pod `/api` i wszystkie procesy robocze w tle z wyjątkiem przetwarzania plików. | Jedna lub więcej. |
| `file-worker` | Skanuje, przetwarza i porządkuje przesłane pliki. | Jedna lub więcej. |

Obraz zawiera też klienta CLI `acli` jako czwartą rolę.

Resztę zapewniasz i utrzymujesz samodzielnie:

- **PostgreSQL 18**;
- prywatny zasobnik (bucket) **zgodny z S3**;
- serwer **SMTP** z TLS;
- opcjonalnie kolektor śladów **OTLP** i **Prometheus**;
- opcjonalnie **Redis**, aby kilka replik API współdzieliło bufor.

## Wypróbuj na swoim komputerze

Aby wypróbować Attricat na jednym komputerze z Docker Compose, uruchom:

```sh
curl -fsSL https://docs.attricat.com/install.sh | sh
```

Skrypt tworzy katalog `attricat` w bieżącym katalogu, zapisuje w nim `compose.yml`, plik `.env` i katalog `secrets` z wygenerowanym hasłem właściciela, a następnie uruchamia najnowszy obraz razem z PostgreSQL, RustFS i Mailpit. Gdy wszystkie usługi działają, wypisuje adres logowania, obszar roboczy, e-mail i hasło. Wiadomości wysyłane przez serwer, na przykład resetowanie hasła, trafiają do Mailpit pod <http://localhost:8025>.

Aby zmienić wartość domyślną, przekaż zmienną do `sh` przy pierwszym uruchomieniu, na przykład `curl -fsSL https://docs.attricat.com/install.sh | ATTRICAT_PORT=8080 sh`:

| Zmienna | Domyślnie |
| --- | --- |
| `ATTRICAT_DIR` | `attricat` |
| `ATTRICAT_IMAGE` | `ghcr.io/attricat/attricat:latest` |
| `ATTRICAT_PORT` | `3000` |
| `MAILPIT_UI_PORT` | `8025` |
| `ATTRICAT_OWNER_EMAIL` | `owner@example.com` |
| `ATTRICAT_OWNER_PASSWORD` | 24 losowe litery i cyfry |

Skrypt zapisuje te ustawienia w `.env`, a hasło w `secrets/bootstrap_owner_password`. Uruchom go ponownie, aby pobrać najnowszy obraz; zachowa `.env` i sekrety, a nadpisze `compose.yml`. W katalogu `attricat` polecenie `docker compose down` zatrzymuje Attricat, a `docker compose down -v` dodatkowo usuwa jego dane. Obraz jest budowany tylko dla amd64, więc komputery z Apple Silicon i innymi procesorami arm64 uruchamiają go w emulacji.

Ta konfiguracja nasłuchuje tylko na localhost, używa zwykłego HTTP i stałych wewnętrznych danych dostępowych, więc służy wyłącznie do testów. Dalsza część tej strony dotyczy środowiska produkcyjnego.

## Przypnij obraz

Wdrażaj według skrótu (digest), nigdy według zmieniającego się tagu. Ustawienia pochodzą z pliku środowiskowego, a [sekrety](/pl/reference/configuration/#sekrety) z katalogu zamontowanego w `/run/secrets`:

```sh
secrets="--volume /etc/attricat/secrets:/run/secrets:ro"
docker run --rm --env-file attricat.env $secrets ghcr.io/attricat/attricat@sha256:… migrate
docker run -d --env-file attricat.env $secrets -p 3000:3000 ghcr.io/attricat/attricat@sha256:… api
docker run -d --env-file attricat.env $secrets ghcr.io/attricat/attricat@sha256:… file-worker
```

Każde wydanie ma też tag z numerem wersji, np. `0.3.0`, oraz tag linii minor, np. `0.3`, który przechodzi na najnowsze wydanie poprawkowe. Od wersji 1.0 tag wersji głównej, np. `1`, wskazuje najnowsze wydanie tej wersji głównej. Za pomocą tych tagów znajdź skrót wydania, np. poleceniem `docker buildx imagetools inspect ghcr.io/attricat/attricat:0.3.0`, a potem wdrażaj ten skrót. `latest` wskazuje gałąź main, a nie najnowsze wydanie. Zmiany w każdej wersji opisuje [strona wydań](https://github.com/attricat/attricat/releases).

Obraz działa jako uid i gid 10001, działa z systemem plików root tylko do odczytu i nie zawiera kompilatorów ani narzędzi do budowania. API nasłuchuje na porcie 3000; prywatny nasłuch procesu roboczego plików dla kontroli stanu i metryk działa na porcie 3001.

## Docker Compose

Plik `deploy/compose.yml` w repozytorium uruchamia API i proces roboczy plików z jednego obrazu i przekazuje im każdy sekret jako [sekret Docker Compose](https://docs.docker.com/compose/how-tos/use-secrets/). Skopiuj `deploy/.env.production.example` do `deploy/.env.production` i zastąp każdą wartość zastępczą. Następnie utwórz po jednym pliku na sekret w `deploy/secrets` albo w katalogu wskazanym przez `ATTRICAT_SECRETS_HOST_DIR`. Compose wymaga, aby każdy plik istniał; sekret, którego nie używasz, zostaw pusty. Użytkownik kontenera, uid 10001, musi móc je odczytać:

```sh
mkdir -m 700 deploy/secrets
cd deploy/secrets
printf '%s\n' 'postgres://attricat:…@postgres.example:5432/attricat' > database_url
printf '%s\n' '…' > s3_secret_access_key
printf '%s\n' '…' > smtp_password
printf '%s\n' '…' > file_worker_metrics_token
printf '%s\n' '…' > bootstrap_owner_password
touch llm_api_key redis_url extension_credential_key identity_provider_credential_key operator_api_token
sudo chown 10001:10001 * && sudo chmod 400 *
cd ../..
docker compose --env-file deploy/.env.production -f deploy/compose.yml up -d
```

Pozostaw plik pusty, aby wyłączyć daną funkcję. Logowanie jednokrotne wymaga `identity_provider_credential_key`, połączenia rozszerzeń wymagają `extension_credential_key`, a API operatora wymaga `operator_api_token`. Każde ustawienie jest opisane w [dokumentacji konfiguracji](/pl/reference/configuration/).

## Pierwsze uruchomienie

Przy pierwszym uruchomieniu API nadaje startowemu obszarowi roboczemu nazwę z `ATTRICAT_BOOTSTRAP_WORKSPACE_NAME` i tworzy konto właściciela dla `ATTRICAT_BOOTSTRAP_OWNER_EMAIL`. Podaj sekret `bootstrap_owner_password` na czas pierwszego uruchomienia, aby właściciel mógł się zalogować, a potem opróżnij jego plik. Nigdy nie zmienia on istniejącego hasła.

Właściciel loguje się identyfikatorem obszaru roboczego, zwykle `default.local`, a następnie zaprasza pozostałe osoby. Aby dodać kolejne obszary robocze, ustaw sekret `operator_api_token` i użyj [API operatora](/pl/reference/api/#api-operatora).

## Kontrole stanu

| Punkt końcowy | Znaczenie | Zastosowanie |
| --- | --- | --- |
| `GET /health/live` (także `/health`) | Proces odpowiada na HTTP. Nie sprawdza zależności. | Sonda żywotności (liveness). |
| `GET /health/ready` | PostgreSQL i zasobnik są osiągalne. W przeciwnym razie zwraca `503 not_ready` bez szczegółów. | Sonda gotowości (readiness) i kierowanie ruchu w load balancerze. |

Proces roboczy plików udostępnia te same dwa punkty końcowe na swoim nasłuchu operacyjnym.

## Uruchom kilka replik API

Repliki API mogą współdzielić jedną bazę danych i jeden zasobnik.

**Połączenia z bazą danych.** Poza pulami żądań i zadań każdy proces API może utrzymywać do trzech dodatkowych połączeń z PostgreSQL. Harmonogramy reguł, harmonogramy przepływów pracy i przyjmowanie zadań rozszerzeń działają w danej chwili tylko na jednej replice, która utrzymuje blokadę na własnym połączeniu. Pozostałe repliki co pięć sekund łączą się na chwilę, aby sprawdzić, czy powinny przejąć tę rolę. Uwzględnij te połączenia przy ustalaniu `max_connections` w PostgreSQL. Łącz się z PostgreSQL bezpośrednio albo przez pooler w trybie sesji; tryb transakcyjny PgBouncera nie jest obsługiwany.

**Wspólny bufor (opcjonalnie).** Każda replika buforuje definicje we własnej pamięci, co zawsze jest poprawne. Aby repliki współdzieliły buforowane wartości i limity żądań sieciowych rozszerzeń, uruchom Redis, ustaw `CACHE_BACKEND=redis` i zapisz jego adres w sekrecie `redis_url`:

```sh
printf '%s\n' 'rediss://:haslo@redis.example.com:6380/0' > deploy/secrets/redis_url
```

`rediss://` łączy się przez TLS, a `redis://` bez niego. Redis nigdy nie jest wymagany: gdy przestanie odpowiadać, API działa dalej z pamięci i bazy danych, a połączenie odnawia samo. Kilka wdrożeń Attricat może korzystać z jednego serwera Redis, bo każdy klucz zawiera losowy identyfikator bazy danych danego wdrożenia. Kopia bazy danych zachowuje ten identyfikator, więc wdrożenie działające na kopii bazy innego wdrożenia, na przykład środowisko testowe sklonowane z produkcji, musi używać innego `CACHE_KEY_PREFIX` albo innej bazy Redis. Po przywróceniu kopii zapasowej w miejsce dotychczasowej bazy wyczyść Redis; zobacz [Kopia zapasowa i przywracanie](/pl/operate/backup/).

## Wdróż nową wersję

1. Zachowaj skrót aktualnie działającej wersji.
2. Uruchom raz rolę `migrate` nowego obrazu. Środowisko produkcyjne ustawia `ATTRICAT_AUTO_MIGRATE=false`, więc repliki API nigdy nie migrują samodzielnie.
3. Zastąp repliki API i procesu roboczego plików nowym skrótem.
4. Poczekaj na `/health/ready` i uruchom testy dymne.

Przykład, w którym `./platform` zastępuje polecenia wdrożenia i testów dymnych Twojej platformy:

```sh
export APP_IMAGE=ghcr.io/attricat/attricat@sha256:…
docker run --rm --env-file attricat.env \
  --volume /etc/attricat/secrets:/run/secrets:ro "$APP_IMAGE" migrate

./platform deploy --image "$APP_IMAGE"
curl --fail --silent --retry 60 --retry-delay 2 --retry-all-errors \
  https://attricat.example.com/health/ready
./platform smoke catalog
```

### Aktualizacja z obrazu starszego niż 2026-10-30

Od tej daty definicje są zapisywane w YAML, a definicje zapisane w TOML nie są konwertowane. Odczyt nadal działa, ale tworzenie lub zmiana rekordu w takiej rewizji schematu kończy się błędem `422 invalid_blueprint_definition`. Dla każdego takiego schematu opublikuj nową rewizję w YAML, a następnie przenieś jego rekordy poleceniem `acli record migrate`.

## Wycofaj wdrożenie

Migracje działają tylko do przodu. Wycofanie oznacza ponowne wdrożenie poprzedniego skrótu:

```sh
./platform deploy --image ghcr.io/attricat/attricat@sha256:previous
curl --fail --silent --retry 60 --retry-delay 2 --retry-all-errors \
  https://attricat.example.com/health/ready
```

Jeśli poprzednia wersja nie może działać na zmigrowanej bazie danych, przywróć bazę danych i zasobnik z kopii zapasowej wykonanej przed wdrożeniem. Zobacz [Kopia zapasowa i przywracanie](/pl/operate/backup/).

## Rotuj sekrety

Dane uwierzytelniające bazy danych, zasobnika, SMTP, agentów, Redis, metryk i inicjalizacji to [pliki sekretów](/pl/reference/configuration/#sekrety) montowane w `/run/secrets`, nigdy zmienne środowiskowe. Start odrzuca sekret ustawiony jako zmienna środowiskowa. Zapisuj pliki z menedżera sekretów; nigdy nie umieszczaj danych uwierzytelniających w obrazie, pliku Compose, argumencie wiersza poleceń ani logu.

Sekrety są odczytywane przy starcie. Aby przeprowadzić rotację: utwórz nowe dane uwierzytelniające, podmień plik sekretu, wdróż role migrate, API i procesu roboczego, poczekaj na gotowość i testy dymne, a następnie unieważnij stare dane uwierzytelniające.

## Lista kontrolna dla produkcji

- `SESSION_COOKIE_SECURE=true` i HTTPS przed API.
- `ATTRICAT_DEVTOOLS=false`.
- `ATTRICAT_AUTO_MIGRATE=false`, z rolą `migrate` uruchamianą przed każdym wdrożeniem.
- `SMTP_TLS_MODE=starttls` lub `implicit`.
- Prawdziwy `ATTRICAT_BOOTSTRAP_OWNER_EMAIL`; sekret `bootstrap_owner_password` opróżniony po pierwszym uruchomieniu.
- Pliki sekretów czytelne tylko dla uid 10001 i żadnych sekretów w zmiennych środowiskowych.
- Ustawiony sekret `file_worker_metrics_token`, a `/metrics` osiągalne tylko z sieci monitoringu.
- Prywatny zasobnik z danymi uwierzytelniającymi ograniczonymi do `PutObject`, `GetObject`, `DeleteObject`, `AbortMultipartUpload` i `ListBucket`.
- Nieustawione `ATTRICAT_E2E_FIXTURE_EMAIL`.
- Proxy, które ustawia `Strict-Transport-Security` i ogranicza liczbę żądań do `/api/auth/` dla każdego adresu klienta. Attricat ogranicza próby dla konta i obszaru roboczego, co nie powstrzyma jednego klienta próbującego wielu z nich.
- Co najmniej 2 GiB pamięci dla każdego procesu API. Haszowanie haseł celowo zużywa dużo pamięci, więc przy mniejszym limicie seria logowań może zakończyć proces.
- Przetestowana procedura tworzenia kopii zapasowej i przywracania.
- Przy kilku replikach API: `max_connections` w PostgreSQL z zapasem do trzech dodatkowych połączeń na proces API, brak poolera w trybie transakcyjnym i opcjonalnie `CACHE_BACKEND=redis`.
