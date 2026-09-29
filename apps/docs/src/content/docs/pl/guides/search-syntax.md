---
title: Składnia wyszukiwania
description: Język zapytań Przeglądarki, od zwykłych terminów po kwalifikowane ścieżki relacji i symbole wieloznaczne.
---

Pole wyszukiwania Przeglądarki i pole `query` w `POST /v1/entities/search` korzystają z tego samego języka zapytań. Zapytania są sprawdzane względem schematu przed uruchomieniem. Zapytanie, które odwołuje się do nieistniejącego atrybutu lub relacji, kończy się czytelnym błędem, zamiast zwracać pusty wynik.

## Terminy

Zapytanie składa się z jednego lub kilku terminów oddzielonych spacjami. Encja musi pasować do **każdego** terminu.

```text
linen shirt
```

dopasowuje encje, które zawierają zarówno „linen”, jak i „shirt”.

Nie ma operatorów `OR` i `NOT` ani grupowania.

## Zwykłe terminy

Termin bez dwukropka przeszukuje tekst własnych atrybutów skalarnych wybranego schematu. Nie podąża za relacjami.

```text
linen
```

## Symbole wieloznaczne

Końcowy `*` dopasowuje dowolne zakończenie:

```text
sku:ABC-12*
```

`*` jest dozwolony tylko na końcu terminu.

## Terminy kwalifikowane

`selector:term` zawęża miejsce, w którym dopasowywany jest termin. Selektor to ścieżka z kropkami.

| Postać | Przykład | Dopasowuje |
| --- | --- | --- |
| `attribute:term` | `sku:ABC*` | Własny atrybut `sku` schematu. |
| `blueprint:term` | `product:linen` | Dowolny własny atrybut wybranego schematu. Schemat można wskazać kodem lub nazwą wyświetlaną, więc `Produkt:lniana` również działa. |
| `blueprint.attribute:term` | `product.sku:ABC*` | To samo co `sku:ABC*`, zapisane jawnie. |
| `relationship:term` | `colors:red` | Dowolny atrybut encji powiązanej przez `colors`. |
| `relationship.attribute:term` | `colors.name:red` | `name` encji powiązanej przez `colors`. |
| `rel.rel.attribute:term` | `category.parent.name:summer` | Przechodzi przez maksymalnie trzy relacje, a następnie dopasowuje jeden atrybut. |

Nazwy wyświetlane schematów są dopasowywane bez rozróżniania wielkości liter. Jeśli dwa schematy mają tę samą nazwę wyświetlaną, użyj kodu.

Gdy jednoczłonowy selektor może oznaczać zarówno nazwę schematu, jak i jedną z jego relacji, pierwszeństwo ma schemat.

## Wyszukiwanie globalne

`*:term` przeszukuje wszystkie schematy, a następnie cofa się wzdłuż relacji, maksymalnie o trzy kroki, aby znaleźć encje wybranego schematu powiązane z dopasowaniem.

```text
*:red
```

znajduje produkty, które same są czerwone, produkty powiązane z kolorem o nazwie red oraz produkty powiązane z rodziną powiązaną z czymś czerwonym.

Wyszukiwanie globalne to najbardziej kosztowna postać zapytania. Ma limity po stronie serwera dotyczące liczby sprawdzanych wartości, encji i powiązań oraz budżet czasu 250 ms. Jeśli zapytanie je przekroczy, żądanie kończy się błędem `422`, zamiast zwracać częściowe wyniki. W takim przypadku zawęź zapytanie kwalifikowanym selektorem.

## Dlaczego to pasuje?

Każdy wynik zawiera wyjaśnienie dopasowania dla każdego terminu: encję i atrybut, które pasowały, oraz ścieżkę relacji od wyniku do nich. W Przeglądarce otwórz **Informacje o wyszukiwaniu** przy wyniku.

## Łączenie z filtrami i fasetami

Zapytanie, [filtry atrybutów](/pl/guides/explore/#filtry), [fasety relacji](/pl/guides/explore/#fasety-relacji), zakres wersji i filtry tagów systemowych zawężają ten sam zbiór wyników. Liczniki faset są obliczane z tych samych dopasowań, więc licznik nigdy nie jest sprzeczny z listą wyników.
