---
title: Wersje i migracja
description: Jak działają wersje schematów, dlaczego encje są przypięte do jednej z nich i jak przenieść je do nowszej wersji.
---

## Szkice, opublikowane wersje i przypinanie

Każdy schemat ma ciąg wersji: 1, 2, 3 i tak dalej.

- Wersję w stanie **szkicu** można edytować i usuwać. Żadna encja nie może z niej korzystać.
- Wersja **opublikowana** jest zamrożona. Jej TOML, atrybuty, widoki i schematy walidacji nigdy się już nie zmieniają.
- Wersja **bieżąca** to najwyższa opublikowana wersja. Korzystają z niej nowe encje.

Każda encja jest przypięta do wersji, z którą została utworzona lub do której została ostatnio zmigrowana. Jej wartości są walidowane według reguł tej wersji i wyświetlane w jej układzie. Opublikowanie wersji 3 niczego nie zmienia dla encji w wersji 2, dopóki ktoś jej nie zmigruje.

Dzięki temu wartość zachowuje znaczenie, jakie miała w chwili zapisu, nawet gdy definicja się zmieni.

## Utwórz nową wersję

W aplikacji internetowej otwórz schemat w **Zarządzanie → Schematy** i utwórz nową wersję na podstawie bieżącej. Za pomocą CLI:

```sh
acli blueprint revision <blueprint-id> --file product-v2.toml
acli blueprint publish <blueprint-id> 2
```

`code` musi pozostać taki sam. Wszystko inne może się zmienić.

## Nieaktualne encje

Po opublikowaniu nowszej wersji encje w starszych wersjach stają się **nieaktualne**. Znajdziesz je w kilku miejscach:

- **Przeglądarka encji** domyślnie pokazuje bieżącą wersję i wyświetla komunikat z linkiem do ukrytych starszych encji.
- Wybranie **Wszystkie wersje** w **Przeglądarce encji** i posortowanie według kolumny **Schemat** umieszcza najstarsze wersje na początku.
- Strona encji oznacza nieaktualną encję i prowadzi do jej strony aktualizacji.
- **Zarządzanie → Stan danych** zlicza nieaktualne encje dla każdego schematu.

## Zmigruj jedną encję

Otwórz encję i wybierz jej aktualizację. Attricat porównuje jej bieżące wartości z wersją docelową i zwraca jeden z tych wyników:

| Wynik | Znaczenie | Co zrobić |
| --- | --- | --- |
| `ready` | Każda wartość pasuje do nowej wersji. | Zmigruj. |
| `needs_input` | Nowa wersja wymaga czegoś, czego encja nie ma, np. nowego wymaganego atrybutu. | Uzupełnij brakujące wartości na stronie aktualizacji, a następnie zmigruj. |
| `blocked` | Istniejące wartości nie mogą pasować, np. wartość zmienionego typu lub relacje przekraczające nową liczność. | Popraw lub odrzuć konfliktowe wartości, a następnie zmigruj. |

Celem jest zawsze bieżąca opublikowana wersja. Migracja w jednym kroku kopiuje wartości do nowej wersji, waliduje je, odbudowuje dane wyszukiwania encji i zapisuje migrację. Jeśli cokolwiek się nie powiedzie, nic się nie zmienia.

Za pomocą CLI:

```sh
acli entity migrate <entity-id>
```

## Zmigruj wiele encji

### Z aplikacji internetowej

Gdy nowa wersja może przechować wszystko, co mogła poprzednia, strona schematu udostępnia akcję **Migruj zgodne encje**. Uruchamia ona w tle partię, która sprawdza każdą encję przypiętą do dowolnej starszej wersji schematu. Każda encja, której migracja ma wynik `ready`, zostaje zmigrowana. Pozostałe czekają na przegląd.

Karta **Migracje** schematu pokazuje każdą partię z wersją docelową, postępem i liczbą encji zmigrowanych, wymagających przeglądu i zakończonych błędem. Gdy partia dla bieżącej wersji czeka w kolejce lub jest przetwarzana, akcja jest niedostępna.

| Stan partii | Znaczenie |
| --- | --- |
| `queued` | Czeka na proces roboczy. |
| `running` | Jest przetwarzana. |
| `completed` | Sprawdzono każdą kwalifikującą się encję. Niektóre mogą nadal wymagać przeglądu. |
| `superseded` | Zastąpiona przez nowszą partię. |

Partie konfigurują dwa ustawienia serwera: `BLUEPRINT_MIGRATION_PAGE_SIZE` i `BLUEPRINT_MIGRATION_CONCURRENCY`. Zobacz [dokumentację konfiguracji](/pl/reference/configuration/#zachowanie-i-limity-katalogu).

### Z CLI

`entity migrate-bulk` migruje każdą encję z wynikiem `ready` z jednej wersji źródłowej i raportuje pozostałe:

```sh
acli entity migrate-bulk --blueprint product --from-version 1 --dry-run
acli entity migrate-bulk --blueprint product --from-version 1
```

Podsumowanie grupuje encje w `ready`, `needs_input`, `blocked` i `failed`. Encje wymagające danych lub zablokowane są pomijane; rozwiąż je na ich stronach aktualizacji, a następnie uruchom polecenie ponownie.

## Projektowanie wersji, które migrują bez problemów

- **Dodawaj, nie zmieniaj.** Nowy opcjonalny atrybut pozostawia każdą encję w stanie `ready`.
- **Nie używaj ponownie kodów.** Jeśli `weight` zmienia jednostkę z gramów na kilogramy, dodaj `weight_kg` i wycofaj `weight`. Ponowne użycie kodu sprawia, że stare wartości wyglądają na poprawne, choć są błędne.
- **Najpierw poluzuj, potem zaostrz.** Opublikuj wersję, która dodaje pole, uzupełnij je w całym katalogu, a potem opublikuj wersję, która czyni je wymaganym.
- **Uważaj na liczność.** Zmiana relacji z `many` na `one` blokuje każdą encję, która ma więcej niż jeden cel.
- **Aktualizuj dołączenia świadomie.** Nowa wersja domieszki trafia do schematu dopiero wtedy, gdy opublikujesz wersję schematu, która na nią wskazuje.
