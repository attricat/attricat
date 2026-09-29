---
title: Kopia zapasowa i przywracanie
description: Twórz kopię zapasową bazy danych i magazynu obiektów jednocześnie i ćwicz ich przywracanie.
---

Katalog Attricat przechowywany jest w dwóch miejscach: PostgreSQL zawiera dane i metadane plików, a zasobnik (bucket) S3 zawiera pliki, warianty obrazów, pakiety rozszerzeń, wyniki operacji i zasoby prezentacyjne. Oba miejsca odwołują się do siebie nawzajem, dlatego **twórz ich kopie zapasowe i przywracaj je razem**. Przywrócenie tylko jednego z nich pozostawia rekordy bez plików albo pliki, do których nikt nie może się dostać.

Obraz Attricat nie zawiera `pg_dump`, `pg_restore` ani narzędzi S3. Użyj narzędzi w pasującej wersji z Twojej platformy lub z osobnego kontenera administracyjnego.

## Utwórz kopię zapasową

1. Wstrzymaj zapisy: zatrzymaj ruch do API i proces roboczy plików w momencie, który potrafisz wskazać.
2. Zapisz skrót obrazu Attricat i czas.
3. Zrzuć bazę danych poleceniem `pg_dump --format=custom` zgodnym z PostgreSQL 18.
4. Wykonaj migawkę całego zasobnika, dopóki zapisy są nadal wstrzymane.
5. Zapisz manifest łączący zrzut z migawką, z sumami kontrolnymi i identyfikatorami migawek lub wersji od Twojego dostawcy.
6. Wznów pracę dopiero wtedy, gdy obie części i manifest są bezpiecznie przechowane.

Jeśli Twój dostawca potrafi wykonać migawki bazy danych i zasobnika w tym samym punkcie zatrzymania, użyj ich zamiast tego.

Przechowuj dane uwierzytelniające w menedżerze sekretów, a nie w poleceniach, logach ani manifeście.

## Przećwicz przywracanie

Ćwicz przywracanie co najmniej raz na wydanie, do jednorazowej bazy danych i zasobnika:

1. Utrzymuj środowisko docelowe zatrzymane.
2. Zweryfikuj każdą sumę kontrolną w manifeście.
3. Przywróć bazę danych i zasobnik.
4. Uruchom rolę `migrate` obrazu.
5. Uruchom API i proces roboczy plików na przywróconej parze.
6. Poczekaj na obie kontrole gotowości.
7. Sprawdź znaną encję, znany przesłany plik i znany wynik operacji rozszerzenia.

Jeśli którykolwiek krok się nie powiedzie, pozostaw środowisko docelowe zatrzymane i zbadaj problem. Nigdy nie uruchamiaj produkcyjnie środowiska, w którym przywrócono tylko jedną część.

## Zasady przechowywania, o których warto pamiętać

- **Historia wartości** starsza niż `ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS` (domyślnie 90) jest usuwana przy starcie API.
- **Pliki bez odwołań** są oznaczane jako usunięte, a następnie trwale usuwane po `FILE_DELETE_GRACE_SECONDS` (domyślnie jeden dzień). Aby zachować taki plik, odtwórz odwołanie do niego przed trwałym usunięciem. Nie traktuj okresu karencji jako kopii zapasowej.
- **Wyniki operacji rozszerzeń** są przechowywane przez 30 dni od zakończenia uruchomienia.
- **Zdarzenia i wpisy audytu** nie są automatycznie usuwane. Nie usuwaj ich ręcznie; system zdarzeń od nich zależy.
