---
title: Przeglądanie encji
description: Wyszukuj, filtruj i utrzymuj rekordy katalogu.
---

Otwórz **Eksplorator encji**, aby przeglądać encje dostępne w przestrzeni roboczej.

## Wyszukiwanie rekordów

Wybierz blueprint. Domyślnie zobaczysz encje z bieżącej opublikowanej rewizji; możesz wybrać starszą opublikowaną rewizję lub **Wszystkie wersje**. Gdy wyświetlana jest bieżąca rewizja, komunikat wskazuje ukryte starsze encje i, jeśli masz uprawnienia, stan migracji.

Zwykły termin przeszukuje wartości skalarne tylko na wybranym blueprintcie. Użyj `*:red`, aby jawnie przeszukać powiązane rekordy przez maksymalnie trzy krawędzie relacji. Możesz wskazać pole, np. `sku:123*`, albo powiązany atrybut, np. `family.product_type.name:red`.

Wybierz kontekst, aby zobaczyć wartości rozstrzygnięte dla niego. Wartość może być lokalna lub odziedziczona z kontekstu nadrzędnego zgodnie z konfiguracją atrybutu.

## Filtrowanie rekordów

Filtry relacji ograniczają wyniki na podstawie powiązanych rekordów i hierarchii. Filtry atrybutów działają na wartościach skalarnych w kontekście domyślnym. Łańcuchy znaków obsługują równość, zawieranie i prefiks; liczby, daty i godziny obsługują równość i zakres, a wartości logiczne równość. Wszystkie aktywne filtry i zapytanie tekstowe muszą pasować.

Sortowanie według powiązanej wartości jest dostępne w jednej rewizji, jeśli każdy krok relacji ma pojedynczą wartość. W widoku **Wszystkie wersje** działa tylko wtedy, gdy pełny zestaw pasujących wyników korzysta z jednej rewizji źródłowej.

Aktywne filtry pojawiają się jako usuwalne etykiety pod polem wyszukiwania. Ich stan jest zachowywany w adresie strony. **Zapisz wyszukiwanie** tworzy nazwany, prywatny lub dostępny w przestrzeni roboczej widok pod `?savedView=<id>`. Po edycji zapisanego wyszukiwania użyj **Zapisz zmiany**, aby je nadpisać, albo **Zapisz wyszukiwanie**, aby utworzyć kopię. **Kopiuj link** używa adresu ze stanem w URL, a dla długich zapytań zapisuje migawkę pod `?viewState=<id>`. Link nie nadaje uprawnień: odbiorca nadal potrzebuje dostępu do przestrzeni roboczej i odczytu encji.

## Praca z encją

Otwórz wynik, aby sprawdzić wartości, rewizję schematu, pliki, relacje i historię zmian. Jeżeli możesz edytować encję, zapisz zmiany na jej stronie szczegółów. Pola tylko do odczytu pozostają widoczne, ale nie można ich zmieniać w przeglądarce. Użytkownik z uprawnieniem `entities.delete` może usunąć encję po potwierdzeniu; znika ona ze zwykłych wyników, lecz jej historia zostaje zachowana.

Gdy blueprint ma nowszą opublikowaną rewizję, Attricat oznacza rekord jako nieaktualny. Przed aktualizacją sprawdź migrację.

Zobacz [Konteksty](/pl/guides/contexts/), aby dowiedzieć się, jak działają wartości dziedziczone.
