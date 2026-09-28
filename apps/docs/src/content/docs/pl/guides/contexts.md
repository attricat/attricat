---
title: Konteksty
description: Modeluj wartości zależne od rynku, kanału, lokalizacji lub hierarchii.
---

Kontekst to węzeł w hierarchii przestrzeni roboczej. Używaj kontekstów, gdy encja potrzebuje wartości różniącej się zależnie od rynku, kanału, sklepu lub innego zakresu.

## Dziedziczenie

Przestrzeń robocza ma główny kontekst domyślny; encja może przechowywać wartości w nim lub w kontekstach potomnych. Jeśli atrybut dziedziczy wartości, brak lokalnej wartości oznacza użycie wartości z najbliższego przodka, aż do kontekstu domyślnego. Atrybut z `context_fallback = "none"` nie dziedziczy wartości.

Na przykład produkt może mieć opis domyślny i przetłumaczony opis w kontekście rynku. Potomny kontekst kanału bez własnego opisu użyje opisu rynku, jeśli włączono dziedziczenie.

## Edycja wartości

Wybierz kontekst podczas wyświetlania lub edycji encji. Atrybuty skonfigurowane do edycji wyłącznie w kontekście domyślnym są tylko do odczytu poza nim. Inne atrybuty mogą zostać nadpisane, jeśli pozwalają na to uprawnienia.

Twórz i organizuj konteksty w **Zarządzaj → Konteksty**. Wybierz jasną hierarchię przed dodaniem wielu nadpisań encji: dziedziczenie podąża za tą hierarchią.
