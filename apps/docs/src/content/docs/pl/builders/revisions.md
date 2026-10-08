---
title: Wersje i migracja
description: Jak działają wersje schematów, dlaczego rekordy są przypięte do jednej z nich i jak przenieść je do nowszej wersji.
---

## Szkice, opublikowane wersje i przypinanie

Każdy schemat ma ciąg wersji: 1, 2, 3 i tak dalej.

- Wersję w stanie **szkicu** można edytować i usuwać. Żaden rekord nie może z niej korzystać.
- Wersja **opublikowana** jest zamrożona. Jej TOML, atrybuty, widoki i schematy walidacji nigdy się już nie zmieniają.
- Wersja **bieżąca** to najwyższa opublikowana wersja. Korzystają z niej nowe rekordy.

Każdy rekord jest przypięty do wersji, z którą został utworzony lub do której został ostatnio zmigrowany. Jej wartości są walidowane według reguł tej wersji i wyświetlane w jej układzie. Opublikowanie wersji 3 niczego nie zmienia dla rekordu w wersji 2, dopóki ktoś go nie zmigruje.

Dzięki temu wartość zachowuje znaczenie, jakie miała w chwili zapisu, nawet gdy definicja się zmieni.

## Utwórz nową wersję

W aplikacji internetowej otwórz schemat w **Zarządzanie → Schematy** i utwórz nową wersję na podstawie bieżącej. Za pomocą CLI:

```sh
acli blueprint revision <blueprint-id> --file product-v2.toml
acli blueprint publish <blueprint-id> 2
```

`code` musi pozostać taki sam. Wszystko inne może się zmienić.

## Nieaktualne rekordy

Po opublikowaniu nowszej wersji rekordy w starszych wersjach stają się **nieaktualne**. Znajdziesz je w kilku miejscach:

- **Przeglądarka rekordów** domyślnie pokazuje bieżącą wersję i wyświetla komunikat z linkiem do ukrytych starszych rekordów.
- Wybranie **Wszystkie wersje** w **Przeglądarce rekordów** i posortowanie według kolumny **Schemat** umieszcza najstarsze wersje na początku.
- Strona rekordu oznacza nieaktualny rekord i prowadzi do jej strony aktualizacji.
- **Zarządzanie → Stan danych** zlicza nieaktualne rekordy dla każdego schematu.

## Zmigruj jeden rekord

Otwórz rekord i wybierz jego aktualizację. Attricat porównuje jego bieżące wartości z wersją docelową i zwraca jeden z tych wyników:

| Wynik | Znaczenie | Co zrobić |
| --- | --- | --- |
| `ready` | Każda wartość pasuje do nowej wersji. | Zmigruj. |
| `needs_input` | Nowa wersja wymaga czegoś, czego rekord nie ma, np. nowego wymaganego atrybutu. | Uzupełnij brakujące wartości na stronie aktualizacji, a następnie zmigruj. |
| `blocked` | Istniejące wartości nie mogą pasować, np. wartość zmienionego typu lub relacje przekraczające nową liczność. | Popraw lub odrzuć konfliktowe wartości, a następnie zmigruj. |

Celem jest zawsze bieżąca opublikowana wersja. Migracja w jednym kroku kopiuje wartości do nowej wersji, waliduje je, odbudowuje dane wyszukiwania rekordu i zapisuje migrację. Jeśli cokolwiek się nie powiedzie, nic się nie zmienia.

Za pomocą CLI, w którym rekordy występują pod nazwą `entity`:

```sh
acli entity migrate <entity-id>
```

## Zmigruj wiele rekordów

### Z aplikacji internetowej

Gdy nowa wersja może przechować wszystko, co mogła poprzednia, strona schematu udostępnia akcję **Migruj zgodne rekordy**. Uruchamia ona w tle partię, która sprawdza każdy rekord przypięty do dowolnej starszej wersji schematu. Każdy rekord, którego migracja ma wynik `ready`, zostaje zmigrowany. Pozostałe czekają na przegląd.

Karta **Migracje** schematu pokazuje każdą partię z wersją docelową, postępem i liczbą rekordów zmigrowanych, wymagających przeglądu i zakończonych błędem. Gdy partia dla bieżącej wersji czeka w kolejce lub jest przetwarzana, akcja jest niedostępna.

| Stan partii | Znaczenie |
| --- | --- |
| `queued` | Czeka na proces roboczy. |
| `running` | Jest przetwarzana. |
| `completed` | Sprawdzono każdy kwalifikujący się rekord. Niektóre mogą nadal wymagać przeglądu. |
| `superseded` | Zastąpiona przez nowszą partię. |

Partie konfigurują dwa ustawienia serwera: `BLUEPRINT_MIGRATION_PAGE_SIZE` i `BLUEPRINT_MIGRATION_CONCURRENCY`. Zobacz [dokumentację konfiguracji](/pl/reference/configuration/#zachowanie-i-limity-katalogu).

### Z CLI

`entity migrate-bulk` migruje każdy rekord z wynikiem `ready` z jednej wersji źródłowej i raportuje pozostałe:

```sh
acli entity migrate-bulk --blueprint product --from-version 1 --dry-run
acli entity migrate-bulk --blueprint product --from-version 1
```

Podsumowanie grupuje rekordy w `ready`, `needs_input`, `blocked` i `failed`. Rekordy wymagające danych lub zablokowane są pomijane; rozwiąż je na ich stronach aktualizacji, a następnie uruchom polecenie ponownie.

## Projektowanie wersji, które migrują bez problemów

- **Dodawaj, nie zmieniaj.** Nowy opcjonalny atrybut pozostawia każdy rekord w stanie `ready`.
- **Nie używaj ponownie kodów.** Jeśli `weight` zmienia jednostkę z gramów na kilogramy, dodaj `weight_kg` i wycofaj `weight`. Ponowne użycie kodu sprawia, że stare wartości wyglądają na poprawne, choć są błędne.
- **Najpierw poluzuj, potem zaostrz.** Opublikuj wersję, która dodaje pole, uzupełnij je w całym katalogu, a potem opublikuj wersję, która czyni je wymaganym.
- **Uważaj na liczność.** Zmiana relacji z `many` na `one` blokuje każdy rekord, który ma więcej niż jeden cel.
- **Aktualizuj include świadomie.** Nowa wersja mixinu trafia do schematu dopiero wtedy, gdy opublikujesz wersję schematu, która na nią wskazuje.
