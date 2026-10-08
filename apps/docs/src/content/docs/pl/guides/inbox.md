---
title: Powiadomienia
description: Sprawdź, co zmieniło się dla Ciebie w obszarze roboczym, oznaczaj powiadomienia jako przeczytane i usuwaj niepotrzebne.
---

Twoja skrzynka zbiera powiadomienia o sprawach, które dotyczą Cię w bieżącym obszarze roboczym. Każdy obszar roboczy ma osobną skrzynkę i nikt poza Tobą jej nie widzi.

## O czym dostajesz powiadomienia

| Powiadomienie | Kiedy |
| --- | --- |
| **Przypisanie do rekordu** | Ktoś przypisuje Ciebie lub Twój zespół do rekordu przez atrybut użytkownika lub zespołu. Zapisanie rekordu bez zmiany przypisania nie wysyła kolejnego powiadomienia. |
| **Nowy komentarz** | Ktoś komentuje rekord, do którego masz przypisanie (bezpośrednio lub przez zespół) albo pod którym jest już Twój komentarz. Powiadomienie pokazuje początek komentarza. |
| **Agent czeka na zatwierdzenie** | Uruchomienie agenta rozpoczęte przez Ciebie czeka na zatwierdzenie zmiany. |
| **Błąd pracy agenta** | Uruchomienie agenta rozpoczęte przez Ciebie zakończyło się błędem lub zostało przerwane. |
| **Agent zakończył pracę** | Zaplanowane przez Ciebie uruchomienie agenta zostało zakończone. Wynik uruchomień rozpoczętych w rozmowie widać w samej rozmowie. |
| **Dodanie do zespołu** | Ktoś dodaje Cię do zespołu. |
| **Przyjęte zaproszenie** | Osoba zaproszona przez Ciebie dołącza do obszaru roboczego. |

Nie dostajesz powiadomień o własnych działaniach. Powiadomienia o rekordzie trafiają do Ciebie tylko wtedy, gdy możesz ten rekord otworzyć.

## Otwórz powiadomienia

Wybierz **Powiadomienia** na pasku bocznym. Liczba na ikonie pokazuje liczbę nieprzeczytanych powiadomień i odświeża się co 30 sekund.

Powiadomienia są posortowane od najnowszych. Nieprzeczytane są pogrubione i oznaczone kropką. Karta **Nieprzeczytane** pokazuje tylko je.

Wybierz powiadomienie o rekordzie lub rozmowie z agentem, aby je otworzyć. Otwarcie oznacza powiadomienie jako przeczytane. Powiadomienia takie jak *Dodanie do zespołu* są tylko wiadomościami i niczego nie otwierają.

## Oznacz jako przeczytane lub nieprzeczytane

- **Oznacz jako przeczytane** (otwarta koperta) oznacza powiadomienie bez otwierania go.
- **Oznacz jako nieprzeczytane** (zamknięta koperta) zostawia je jako przypomnienie.
- **Oznacz wszystkie jako przeczytane** u góry oznacza wszystkie nieprzeczytane powiadomienia. Powiadomienia, które przyjdą po otwarciu skrzynki, pozostają nieprzeczytane.

## Usuń powiadomienia

**Usuń trwale** (kosz) usuwa powiadomienie. Usunięcia nie można cofnąć.

## Powiadomienia w CLI, API i u agenta

- [CLI](/pl/reference/cli/#powiadomienia) ma polecenia `acli notification`.
- [API](/pl/reference/api/#powiadomienia) udostępnia ścieżki `/notifications`.
- [Agent](/pl/guides/agents/) może odczytać Twoje powiadomienia, a po Twoim zatwierdzeniu oznaczyć je jako przeczytane lub usunąć.
