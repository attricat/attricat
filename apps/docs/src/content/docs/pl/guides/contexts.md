---
title: Konteksty
description: Modeluj wartości zależne od rynku, kanału, lokalizacji lub hierarchii.
---

Kontekst to węzeł w hierarchii przestrzeni roboczej. Używaj kontekstów, gdy encja potrzebuje wartości różniącej się zależnie od rynku, kanału, sklepu lub innego zakresu.

## Dziedziczenie

Każda encja ma wartości w kontekście domyślnym. Kontekst inny niż domyślny może zdefiniować własną wartość albo dziedziczyć wartość domyślną, zależnie od konfiguracji atrybutu w blueprintcie.

Na przykład produkt może mieć opis domyślny i przetłumaczony opis w kontekście rynku. Gdy nie istnieje opis lokalny, Attricat wyświetla skonfigurowaną wartość dziedziczoną.

## Edycja wartości

Wybierz kontekst podczas wyświetlania lub edycji encji. Atrybuty skonfigurowane do edycji wyłącznie w kontekście domyślnym są tylko do odczytu poza nim. Inne atrybuty mogą zostać nadpisane, jeśli pozwalają na to uprawnienia.

Twórz i organizuj konteksty w **Zarządzaj → Konteksty**. Wybierz jasną hierarchię przed dodaniem wielu nadpisań encji: dziedziczenie podąża za tą hierarchią.
