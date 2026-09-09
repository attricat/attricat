---
title: Przeglądanie encji
description: Wyszukuj, sprawdzaj i utrzymuj rekordy katalogu.
---

Otwórz **Eksplorator encji**, aby przeglądać encje dostępne w przestrzeni roboczej.

## Wyszukiwanie rekordów

Wybierz blueprint, aby rozpocząć przeglądanie jego encji. Zwykły termin wyszukuje wartości skalarne tylko w tym schemacie. Użyj `*:red`, aby jawnie przeszukać powiązane rekordy przez maksymalnie trzy krawędzie relacji. Zapytanie może wskazać pole, na przykład `sku:123*`, albo jawnie przeszukać powiązaną encję przez jedną relację, na przykład `color.name:red`.

Użyj wyboru kontekstu, aby zobaczyć wartości rozwiązywane dla określonego kontekstu. Wartość może być zdefiniowana bezpośrednio w kontekście albo odziedziczona z wartości domyślnej.

## Praca z encją

Otwórz wynik, aby sprawdzić jego wartości, rewizję schematu, pliki, relacje i historię zmian. Jeżeli możesz edytować encję, zapisz zmiany na jej stronie szczegółów. Pola tylko do odczytu nadal są widoczne, ale zarządza nimi integracja systemowa, a nie przeglądarka.

Gdy blueprint encji ma nowszą opublikowaną rewizję, Attricat oznacza rekord jako nieaktualny. Przed aktualizacją sprawdź migrację, aby zmiany pól lub walidacji były świadome.

Zobacz [Konteksty](/pl/guides/contexts/), aby dowiedzieć się, jak działają wartości dziedziczone.
