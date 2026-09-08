---
title: Wprowadzenie
description: Zacznij korzystać z Attricat do modelowania, zarządzania i przeglądania katalogu.
---

Attricat to wersjonowany silnik katalogowy. Definiujesz strukturę danych w **blueprintach**, tworzysz z nich **encje** i używasz **kontekstów**, gdy wartość zmienia się zależnie od miejsca, kanału lub innej hierarchii.

## Najważniejsze pojęcia

- **Blueprinty** to wersjonowane definicje typów encji, atrybutów i układów.
- **Encje** to rekordy katalogu. Zachowują dokładną rewizję blueprintu użytego przy ich utworzeniu.
- **Konteksty** pozwalają dziedziczyć wartość domyślną lub zdefiniować świadome lokalne nadpisanie.
- **Relacje** łączą encje bez utraty ich typu i historii.

## Typowy proces

1. Utwórz i opublikuj blueprint, na przykład `product`.
2. Utwórz encje na podstawie opublikowanego blueprintu.
3. Dodaj konteksty, gdy wartość musi różnić się zależnie od rynku, kanału lub lokalizacji.
4. Wyszukuj i utrzymuj rekordy w **Eksploratorze encji**.
5. Przeglądaj zmiany w dzienniku aktywności.

> Dostępne działania zależą od uprawnień w przestrzeni roboczej.

Przejdź do [przeglądania encji](/pl/guides/explore/) lub dowiedz się, jak [tworzyć blueprinty](/pl/builders/blueprints/).
