---
title: Rozszerzenia
description: Dodawaj zaufane integracje i komponenty klienckie do Attricat.
---

Rozszerzenia dodają możliwości do Attricat bez zmieniania rdzenia katalogu. Mogą dostarczać konfigurację dla przestrzeni roboczej, działanie w tle, zdarzenia i interfejs w przeglądarce.

## Instaluj z zaufanego źródła

Otwórz **Zarządzaj → Rozszerzenia**, aby przeglądać wpisy rejestru lub przesłać lokalny pakiet rozszerzenia, jeśli masz wymagane uprawnienie. Attricat waliduje pakiety przed instalacją i instaluje je jako wyłączone.

## Sprawdź przed włączeniem

Rozszerzenie żąda uprawnień do funkcji hosta, zdarzeń, magazynu i wkładów klienckich, których potrzebuje. Przejrzyj żądania, skonfiguruj rozszerzenie, przyznaj wyłącznie wymagane uprawnienia, a następnie je włącz.

Włączone rozszerzenie jest zaufanym oprogramowaniem przestrzeni roboczej. Ogranicz jego uprawnienia i korzystaj z zaufanego źródła.

## Twórz rozszerzenie

Autorzy rozszerzeń pakują ścisły manifest i korzystają z obsługiwanego kontraktu API hosta Catalog. Przetestuj rozszerzenie w lokalnej przestrzeni roboczej Attricat przed publikacją wydania.

Szczegóły pakietu, uprawnień i środowiska wykonawczego opisuje [kontrakt rozszerzeń w repozytorium (po angielsku)](https://github.com/attricat/attricat/blob/main/docs/extensions.md). Dostarczanie webhooków i część zadeklarowanych miejsc interfejsu nie są jeszcze zaimplementowane; przed użyciem danej funkcji sprawdź ograniczenia kontraktu.
