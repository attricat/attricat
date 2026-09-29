---
title: Wdrażanie Attricat
description: Uruchom obraz Attricat produkcyjnie, z migracjami, kontrolami stanu, wdrażaniem, wycofywaniem i rotacją sekretów.
---

Attricat jest dostarczany jako jeden obraz kontenera. Ten sam obraz uruchamia trzy role:

| Rola | Uruchamia | Repliki |
| --- | --- | --- |
| `migrate` | Stosuje migracje bazy danych, a następnie kończy działanie. | Jedna, przed każdym wdrożeniem. |
| `api` | Aplikację webową pod `/`, API pod `/api` (oraz pod oryginalnymi ścieżkami) i wszystkie procesy robocze w tle z wyjątkiem przetwarzania plików. | Jedna lub więcej. |
| `file-worker` | Skanuje, przetwarza i porządkuje przesłane pliki. | Jedna lub więcej. |

Obraz zawiera też klienta CLI `acli` jako czwartą rolę.

Resztę zapewniasz i utrzymujesz samodzielnie:

- **PostgreSQL 18**;
- prywatny zasobnik (bucket) **zgodny z S3**;
- serwer **SMTP** z TLS;
- opcjonalnie kolektor śladów **OTLP** i **Prometheus**.

## Przypnij obraz

Wdrażaj według skrótu (digest), nigdy według zmieniającego się tagu:

```sh
docker run --rm --env-file attricat.env ghcr.io/attricat/attricat@sha256:… migrate
docker run -d --env-file attricat.env -p 3000:3000 ghcr.io/attricat/attricat@sha256:… api
docker run -d --env-file attricat.env ghcr.io/attricat/attricat@sha256:… file-worker
```

Obraz działa jako uid i gid 10001, działa z systemem plików root tylko do odczytu i nie zawiera kompilatorów ani narzędzi do budowania. API nasłuchuje na porcie 3000; prywatny nasłuch procesu roboczego plików dla kontroli stanu i metryk działa na porcie 3001.

## Docker Compose

Plik `deploy/compose.yml` w repozytorium uruchamia API i proces roboczy plików z jednego obrazu. Skopiuj `deploy/.env.production.example` do `deploy/.env.production`, zastąp każdą wartość zastępczą, przechowuj plik w menedżerze sekretów i uruchom:

```sh
docker compose --env-file deploy/.env.production -f deploy/compose.yml up -d
```

Każde ustawienie jest opisane w [dokumentacji konfiguracji](/pl/reference/configuration/).

## Pierwsze uruchomienie

Przy pierwszym uruchomieniu API tworzy obszar roboczy o nazwie z `CATALOG_BOOTSTRAP_WORKSPACE_NAME` i konto właściciela dla `CATALOG_BOOTSTRAP_OWNER_EMAIL`. Ustaw `CATALOG_BOOTSTRAP_OWNER_PASSWORD` na czas pierwszego uruchomienia, aby właściciel mógł się zalogować, a potem usuń tę zmienną. Nigdy nie zmienia ona istniejącego hasła.

Właściciel loguje się identyfikatorem obszaru roboczego, zwykle `default.local`, a następnie zaprasza pozostałe osoby.

## Kontrole stanu

| Punkt końcowy | Znaczenie | Zastosowanie |
| --- | --- | --- |
| `GET /health/live` (także `/health`) | Proces odpowiada na HTTP. Nie sprawdza zależności. | Sonda żywotności (liveness). |
| `GET /health/ready` | PostgreSQL i zasobnik są osiągalne. W przeciwnym razie zwraca `503 not_ready` bez szczegółów. | Sonda gotowości (readiness) i kierowanie ruchu w load balancerze. |

Proces roboczy plików udostępnia te same dwa punkty końcowe na swoim nasłuchu operacyjnym.

## Wdróż nową wersję

1. Zachowaj skrót aktualnie działającej wersji.
2. Uruchom raz rolę `migrate` nowego obrazu. Środowisko produkcyjne ustawia `CATALOG_AUTO_MIGRATE=false`, więc repliki API nigdy nie migrują samodzielnie.
3. Zastąp repliki API i procesu roboczego plików nowym skrótem.
4. Poczekaj na `/health/ready` i uruchom testy dymne.

Skrypt `scripts/operations.sh` z repozytorium opakowuje te kroki wokół polecenia wdrożenia Twojej platformy:

```sh
export APP_IMAGE=ghcr.io/attricat/attricat@sha256:…
export DATABASE_URL='postgres://…'
scripts/operations.sh migrate

export DEPLOY_COMMAND='./platform deploy --image "$APP_IMAGE"'
export READINESS_URL='https://catalog.example.com/health/ready'
export SMOKE_COMMAND='./platform smoke catalog'
scripts/operations.sh rollout
```

## Wycofaj wdrożenie

Migracje działają tylko do przodu. Wycofanie oznacza ponowne wdrożenie poprzedniego skrótu:

```sh
export ROLLBACK_COMMAND='./platform deploy --image ghcr.io/attricat/attricat@sha256:previous'
scripts/operations.sh rollback
```

Jeśli poprzednia wersja nie może działać na zmigrowanej bazie danych, przywróć bazę danych i zasobnik z kopii zapasowej wykonanej przed wdrożeniem. Zobacz [Kopia zapasowa i przywracanie](/pl/operate/backup/).

## Rotuj sekrety

Wstrzykuj dane uwierzytelniające bazy danych, zasobnika, SMTP, metryk i inicjalizacji z menedżera sekretów, nigdy z obrazu, pliku Compose, argumentu wiersza poleceń ani logu.

Aby przeprowadzić rotację: utwórz nowe dane uwierzytelniające, przekaż je nowej wersji wdrożenia, wdróż role migrate, API i procesu roboczego, poczekaj na gotowość i testy dymne, a następnie unieważnij stare dane uwierzytelniające.

```sh
export ROTATE_SECRETS_COMMAND='./platform rotate-and-roll catalog-secrets'
export READINESS_URL='https://catalog.example.com/health/ready'
scripts/operations.sh rotate-secrets
```

## Lista kontrolna dla produkcji

- `SESSION_COOKIE_SECURE=true` i HTTPS przed API.
- `CATALOG_DEVTOOLS=false`.
- `CATALOG_AUTO_MIGRATE=false`, z rolą `migrate` uruchamianą przed każdym wdrożeniem.
- `SMTP_TLS_MODE=starttls` lub `implicit`.
- Prawdziwy `CATALOG_BOOTSTRAP_OWNER_EMAIL`; `CATALOG_BOOTSTRAP_OWNER_PASSWORD` usunięty po pierwszym uruchomieniu.
- Ustawiony `FILE_WORKER_METRICS_TOKEN`, a `/metrics` osiągalne tylko z sieci monitoringu.
- Prywatny zasobnik z danymi uwierzytelniającymi ograniczonymi do `PutObject`, `GetObject`, `DeleteObject` i `HeadBucket`.
- Nieustawione `CATALOG_E2E_FIXTURE_EMAIL` i `CATALOG_E2E_FIXTURE_PASSWORD`.
- Przetestowana procedura tworzenia kopii zapasowej i przywracania.
