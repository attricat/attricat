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

## Ponowna akceptacja publikacji

Domyślnie każda edycja opublikowanej encji cofa jej publikację w kanałach i wymaga ponownej akceptacji. Rewizja blueprintu może wskazać zaufane role obszaru roboczego, których edycje zachowują istniejącą publikację encji:

```toml
[publication]
retain_on_edit_roles = ["catalog_manager", "product_owner"]
```

Są to kody ról obszaru roboczego. Role muszą istnieć podczas publikowania rewizji blueprintu. To ustawienie nie nadaje uprawnień do edycji ani publikacji; użytkownicy nadal potrzebują zwykłych uprawnień obszaru roboczego. Dotyczy zmian wartości, relacji, metadanych, plików i aktualizacji encji. Zmiany kontekstu zawsze cofają publikację kanału, ponieważ mogą zmienić wynikowe dane wielu encji.

## Zachowanie atrybutów

Atrybuty mogą być wartościami skalarnymi, relacjami lub plikami. Konfiguracja blueprintu steruje także walidacją, wartościami domyślnymi, dziedziczeniem kontekstowym oraz możliwością edycji pola w przeglądarce.

Pełna składnia atrybutów i reguły walidacji znajdują się w [dokumentacji blueprintów w repozytorium (po angielsku)](https://github.com/attricat/attricat/blob/main/docs/blueprints.md).
