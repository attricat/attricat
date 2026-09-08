---
title: Blueprinty
description: Definiuj wersjonowane schematy encji w katalogu.
---

Blueprint definiuje typ encji albo wielokrotnie używany mixin. Opisuje atrybuty, walidację, relacje, pliki i widoki. Po opublikowaniu każda rewizja jest niezmienna.

## Utwórz blueprint

Otwórz **Zarządzaj → Blueprinty** i utwórz nowy szkic. Nadaj mu stabilny kod i nazwę, a następnie zdefiniuj atrybuty w TOML.

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[attributes]]
code = "name"
value_type = "string"
```

Blueprint encji może tworzyć encje. Mixin dodaje atrybuty wielokrotnego użycia do innych blueprintów.

## Publikuj świadomie

Publikacja udostępnia rewizję blueprintu dla nowych encji. Istniejące encje pozostają przy rewizji, z którą zostały utworzone; zachowuje to znaczenie oraz zasady walidacji obowiązujące w tamtym czasie.

Gdy publikujesz nowszą rewizję, przejrzyj kandydatów do migracji przed aktualizacją istniejących encji. Nie używaj ponownie kodów dla niezgodnego znaczenia.

## Zachowanie atrybutów

Atrybuty mogą być wartościami skalarnymi, relacjami lub plikami. Konfiguracja blueprintu steruje także walidacją, wartościami domyślnymi, dziedziczeniem kontekstowym oraz możliwością edycji pola w przeglądarce.

Dokumentacja blueprintów będzie rozbudowywana o kolejne przykłady atrybutów i szczegóły walidacji.
