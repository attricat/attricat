---
title: Zgodność
description: Co Attricat zachowuje bez zmian między wydaniami, co może się zmienić i jak ogłaszane jest usuwanie funkcji.
---

Od wersji 1.0 Attricat stosuje [wersjonowanie semantyczne](https://semver.org/lang/pl/). Wydanie poprawkowe ani pomniejsze nie psuje niczego, co jest tu wymienione jako stabilne. Zmiana niezgodna wymaga nowej wersji głównej.

## Stabilne

- **API HTTP:** trasy, pola żądań i pola odpowiedzi opisane w [dokumentacji API](/pl/reference/api/). Nowe opcjonalne pola żądań i nowe pola odpowiedzi mogą pojawić się w każdym wydaniu, więc klienci muszą ignorować pola, których nie rozpoznają.
- **Kody błędów:** pole `code` błędu API. Angielska treść `message` może się zmienić, więc dopasowuj kod.
- **Format schematu:** schemat poprawny dzisiaj pozostaje poprawny.
- **CLI:** polecenia i opcje z [dokumentacji CLI](/pl/reference/cli/) oraz pola ich wyniku JSON.
- **Konfiguracja:** [ustawienia i sekrety](/pl/reference/configuration/) ustawiane przez operatorów.
- **Zdarzenia:** [koperta zdarzenia](/pl/reference/events/).
- **ABI hosta rozszerzeń:** wydane wersje `attricat:host` tylko się rozszerzają. Zobacz [wersje ABI hosta](/pl/extensions/server/).

## Niestabilne

- Schemat bazy danych. Czytaj i zapisuj przez API lub `acli`. Rób kopie zapasowe bazy, ale nie opieraj się na jej tabelach.
- Układ, teksty i wewnętrzne żądania aplikacji webowej.
- Nazwy i etykiety metryk oraz komunikaty w logach.
- Treść odpowiedzi oznaczonych jako wewnętrzne oraz tras, których nie ma w dokumentacji API.

## Aktualizacje

Uruchom migracje przed wdrożeniem nowej wersji i najpierw wykonaj kopię zapasową. Nie ma gwarancji, że poprzednia wersja zadziała na zmigrowanej bazie, więc cofnięcie może wymagać przywrócenia tej kopii. Zobacz [Wdrożenie](/pl/operate/deployment/).

Wydanie, które wymaga działań operatora, informuje o tym w opisie.

## Wycofywanie

Stabilny element jest oznaczany jako wycofywany w wydaniu pomniejszym i działa do następnej wersji głównej. Opis wydania wskazuje zamiennik.

Poprawka bezpieczeństwa może zmienić stabilne zachowanie w wydaniu pomniejszym lub poprawkowym, gdy nie da się go bezpiecznie zachować. Opis wydania mówi, co się zmieniło i dlaczego.
