---
title: Wprowadzenie
description: Czym jest Attricat, do czego służy i jak łączą się jego elementy.
---

Attricat to katalog ustrukturyzowanych danych produktowych i referencyjnych. Opisujesz rekordy za pomocą wersjonowanych **schematów**, przechowujesz wartości różniące się zależnie od rynku, języka lub kanału w **kontekstach**, łączysz rekordy typowanymi **relacjami** i zatwierdzasz to, co trafia do każdego kanału, **publikując** to.

Każda zmiana jest walidowana na serwerze, zapisywana w dzienniku audytu i zachowywana w historii. Integracje, przepływy pracy i agenci AI zmieniają dane tą samą ścieżką co ludzie, więc reguły obowiązują niezależnie od tego, kto wprowadza zmianę.

## W czym sprawdza się najlepiej

- **Katalogi, których struktura się zmienia.** Wersja schematu nigdy się nie zmienia po opublikowaniu, a każdy rekord pozostaje przy wersji, w której został zapisany, dopóki go nie zmigrujesz. Możesz rozwijać model bez psucia tego, co już istnieje.
- **Wartości różniące się zależnie od miejsca.** Drzewo kontekstów (na przykład rynek, a pod nim kanał) pozwala ustawić każdą wartość raz i ją dziedziczyć, z nadpisaniami tylko tam, gdzie są potrzebne.
- **Klasyfikacje i taksonomie.** Kategorie, marki i materiały są osobnymi rekordami połączonymi relacjami, więc można zmieniać ich nazwy, tłumaczyć je i układać w hierarchie.
- **Kontrolowane dane wyjściowe.** Publikacja zatwierdza rekord dla kanału. Każda późniejsza edycja wycofuje to zatwierdzenie, dopóki ktoś nie opublikuje rekordu ponownie.
- **Bezpieczne rozszerzanie.** Rozszerzenia działają w piaskownicach i mogą robić tylko to, na co pozwoli im administrator.

## Jak łączą się elementy

| Element | Czym jest |
| --- | --- |
| **Obszar roboczy** | Jeden katalog z własnymi członkami, danymi i ustawieniami. |
| **Schemat** | Wersjonowana definicja typu rekordu w TOML: atrybuty, walidacja i układ. |
| **Rekord** | Jeden element katalogu, na przykład produkt lub kategoria, przypięty do wersji schematu. |
| **Atrybut** | Pole rekordu: tekst, liczba, data, relacja, plik i inne. |
| **Kontekst** | Miejsce, w którym wartości mogą się różnić, ułożone w drzewo pod `default`. |
| **Relacja** | Typowane powiązanie jednego rekordu z innymi. |
| **Kanał publikacji** | Kontekst włączony do eksportu, w którym rekordy są zatwierdzane pojedynczo. |
| **Reguła** | Kontrola jakości danych, która zapisuje ustalenia. |
| **Przepływ pracy** | Niewielka automatyzacja, która taguje lub aktualizuje rekord po zdarzeniu. |
| **Rozszerzenie** | Pakiet dodający logikę serwera, interfejs, typy atrybutów lub integracje. |
| **Pakiet rozwiązania** | Archiwum, które przygotowuje obszar roboczy do konkretnego zastosowania. |
| **Agent** | Asystent AI, który odczytuje katalog i proponuje zmiany do zatwierdzenia. |

[Podstawowe pojęcia](/pl/start/concepts/) omawiają każdy z nich dokładniej.

## Co dalej

- Nie znasz jeszcze Attricat? Przejdź przez [Szybki start](/pl/start/quickstart/), aby zbudować mały katalog.
- Projektujesz katalog? Przeczytaj [Modelowanie katalogu](/pl/builders/modeling/) i [Tworzenie schematu](/pl/builders/blueprints/).
- Uruchamiasz Attricat? Zacznij od [Wdrażania Attricat](/pl/operate/deployment/) i [dokumentacji konfiguracji](/pl/reference/configuration/).
- Budujesz integrację? Zobacz [dokumentację API](/pl/reference/api/), [CLI](/pl/reference/cli/) i [Tworzenie rozszerzenia](/pl/extensions/build/).
