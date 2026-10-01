---
title: Instalacja i obsługa pakietów rozwiązań
description: Bezpieczne sprawdzanie, planowanie, stosowanie i obsługa istniejących pakietów rozwiązań.
---

Pakiet rozwiązania to wersjonowane archiwum `.tar.zst` dostarczone przez wydawcę. Przygotowuje obszar roboczy do konkretnego zastosowania: może dostarczać Schematy, nawigację, układ rozszerzeń, zasoby graficzne, wskazówki i opcjonalne dane przykładowe.

Po zastosowaniu pakietu użytkownicy pracują z jego zasobami przez zwykłe funkcje Attricat. Nie trzeba utrzymywać działającej usługi pakietu. Pakiet nie jest właścicielem zasobów, nie synchronizuje ich ani nie usuwa ich później.

## Zanim zaczniesz

- Pobierz archiwum od zaufanego wydawcy i przeczytaj informacje o wydaniu. Attricat nie pobiera pakietów ani zawartości repozytoriów.
- Zaloguj się przez CLI do odpowiedniego obszaru roboczego. Obsługa pakietów wymaga `solution_packs.manage`, domyślnie dostępnego właścicielom i administratorom.
- Nieznane pakiety testuj w jednorazowym obszarze roboczym, szczególnie z danymi przykładowymi.
- Wymaganymi rozszerzeniami zarządzaj w [zwykły sposób](/pl/builders/extensions/). Pakiet nigdy nie instaluje, nie konfiguruje, nie włącza ani nie usuwa rozszerzeń i nie nadaje im uprawnień.

Jeśli archiwum zostanie odrzucone, poproś wydawcę o zgodne wydanie zamiast samodzielnie je edytować lub przepakowywać.

## 1. Sprawdź

```sh
acli solution-pack inspect --file pack.tar.zst
```

Serwer sprawdza archiwum i zwraca jego identyfikator, wersję, skrót, podsumowanie zasobów i ostrzeżenia dotyczące danych przykładowych. Nie tworzy zasobów ani nie zapisuje archiwum. Poprawna walidacja nie gwarantuje, że pakiet jest odpowiedni dla Twojego obszaru roboczego.

## 2. Zaplanuj

```sh
acli solution-pack plan --file pack.tar.zst \
  --prefix example --blueprint-publication publish
acli solution-pack plan show <plan-id>
```

Planowanie zapisuje niezmienny plan próbny bez zmieniania zasobów katalogu. Sprawdź gotowość, akcje, konflikty i wymagania dotyczące rozszerzeń.

- `--prefix` tworzy kody nowych Schematów, takie jak `example_product`. Użyj od 1 do 32 małych liter, cyfr lub podkreśleń; zacznij literą i nie kończ podkreśleniem.
- `--blueprint-publication` przyjmuje `draft` lub `publish`. Nawigacja i dane przykładowe mogą wymagać opublikowanych Schematów, więc wybór `draft` może zablokować taki pakiet.
- `--include-sample-data` jawnie wybiera fikcyjne encje przykładowe. Pomiń tę opcję, jeśli ich nie potrzebujesz.

| Akcja | Znaczenie |
| --- | --- |
| `create` | Utworzenie Schematu, zasobu graficznego lub wybranej encji przykładowej. |
| `map` | Ponowne użycie jawnie wskazanego zgodnego zasobu. |
| `append` | Dodanie wpisów nawigacji lub układu rozszerzeń. |
| `satisfied` | Żądane ustawienie już istnieje w dokładnie tej postaci. |
| `skip` | Pominięcie niedostępnego elementu opcjonalnego. |
| `conflict` | Obecny stan obszaru roboczego uniemożliwia operację. |
| `blocked` | Wymaganie nie jest spełnione lub zmiana nie jest obsługiwana. |

Można zastosować tylko gotowe plany. Plan wygasa po 24 godzinach, jeśli jego stosowanie jeszcze się nie rozpoczęło.

## 3. Rozwiąż konflikty

W przypadku zajętego kodu wybierz inny prefiks albo jawnie wskaż dokładnie zgodny opublikowany Schemat. Używaj kluczy zasobów z wyniku sprawdzania archiwum i instrukcji instalacji wydawcy:

```sh
acli solution-pack plan --file pack.tar.zst --prefix example \
  --blueprint-publication publish \
  --map blueprints/product=shared_product
```

Aby ponownie użyć dokładnie zgodnego zasobu graficznego, najpierw znajdź jego identyfikator:

```sh
acli presentation-asset list
acli presentation-asset show <asset-id>
acli solution-pack plan --file pack.tar.zst --prefix example \
  --blueprint-publication publish \
  --map-asset assets/brand-logo=<asset-id>
```

Istniejące zasoby musisz wskazać jawnie; Attricat nie nadpisuje ich po cichu. Jeśli wymaganie dotyczące rozszerzenia nie jest spełnione, rozwiąż problem przez standardowe zarządzanie rozszerzeniami i utwórz nowy plan. Zgodna, wyłączona instalacja może spełniać wymaganie; administrator musi osobno zatwierdzić uprawnienia i włączenie.

## 4. Zastosuj i zweryfikuj

```sh
acli solution-pack apply <plan-id>
acli solution-pack applications list
acli solution-pack applications show <application-id>
acli solution-pack checks list <application-id>
acli solution-pack checks show <application-id> <run-id>
acli solution-pack checks rerun <application-id>
```

Zastosowanie przyjmuje tylko identyfikator przejrzanego planu. Serwer ponownie sprawdza stan obszaru roboczego przed każdym krokiem. Wykonaj kroki z listy kontrolnej konfiguracji i wypróbuj scenariusz biznesowy pakietu.

Kontrole są informacyjne. Negatywny wynik nie cofa zasobów ani nie blokuje ukończonego zastosowania. Historia zawiera informacje o utworzonych lub ponownie użytych zasobach; nie śledzi późniejszych zmian użytkowników.

## Ponawianie i odzyskiwanie

Jeśli stosowanie pakietu zostało przerwane lub zakończyło się błędem, który pozwala na wznowienie, ponów je z tym samym identyfikatorem planu. Serwer sprawdza ukończone kroki i kontynuuje pozostałe bez tworzenia duplikatów. Rozpoczęte stosowanie można wznowić po wygaśnięciu planu, dopóki nie upłynie termin przechowywania jego danych.

Jeśli zmiany w obszarze roboczym unieważniły plan, przejrzyj ukończone kroki i diagnostykę przed utworzeniem kolejnego. Jeśli późniejszy krok zakończy się trwałym błędem, wcześniejsze udane zapisy pozostają w obszarze roboczym; Attricat ich nie wycofuje.

## Zastosuj nowsze wydanie

```sh
acli solution-pack plan --file pack-v2.tar.zst --prefix example \
  --blueprint-publication publish --from-application <application-id>
```

Wskaż jedno ukończone zastosowanie tego samego pakietu w tym samym obszarze roboczym. Wydanie musi być nowsze. `--from-application` nie można łączyć z jawnymi mapowaniami.

Niezmienione, dokładnie zgodne opublikowane Schematy i niezmienione zasoby graficzne mogą być użyte ponownie; nowe zasoby mogą zostać utworzone. Zmienione definicje są blokowane jako `update_not_supported`. Usunięte zasoby są raportowane, ale nie kasowane. Brakujące lub zmodyfikowane wcześniejsze zasoby mogą powodować konflikty. Dla zmian, których pakiet nie obsługuje, uzgodnij z wydawcą procedurę migracji.

## Opcjonalne dane przykładowe

Dodaj `--include-sample-data` podczas planowania dopiero po przeczytaniu ostrzeżenia. Encje przykładowe powstają w kontekście domyślnym na podstawie opublikowanych Schematów i otrzymują widoczne oznaczenie danych przykładowych. Są zwykłymi encjami: ich tworzenie zapisuje audyt i zdarzenia `entity.created.v1`, może uruchamiać aktywne przepływy pracy lub rozszerzenia i powodować skutki w systemach zewnętrznych. Wartości mogą pozostać w historii audytu i zdarzeń po usunięciu tymczasowych danych pakietu.

Pierwszy plan z danymi przykładowymi rezerwuje dokładnie tę kombinację wydania, archiwum i zestawu danych. Drugi plan nie może wybrać tej samej kombinacji, nawet po wygaśnięciu lub porzuceniu pierwszego; zmiana prefiksu nie usuwa rezerwacji. Ponawiaj oryginalny plan. Jeśli wygaśnie przed rozpoczęciem stosowania, uzgodnij nowe wydanie z wydawcą.

Stosowanie pakietu z danymi przykładowymi można wznowić przez 30 dni od rozpoczęcia. Aby trwale je zatrzymać:

```sh
acli solution-pack applications abandon <application-id>
```

Po porzuceniu serwer usuwa tymczasowe dane wejściowe. Wcześniej utworzone encje i zwykła historia audytu oraz zdarzeń pozostają bez zmian. Późniejsze wydania nie resetują bieżących wartości ani nie przywracają usuniętych oznaczeń danych przykładowych. Nie ma polecenia resetowania zestawu ani automatycznego sprzątania.

## Ograniczenia i usuwanie

Pakiety nie mogą tworzyć kontekstów ani kanałów eksportu, zmieniać członkostwa lub uprawnień, uruchamiać wykonywalnych instalatorów, automatycznie rozwiązywać zależności od innych pakietów ani aktualizować istniejących Schematów.

Nie ma odinstalowania ani wycofania całego pakietu. Administratorzy mogą edytować lub usuwać pojedyncze zasoby zwykłymi operacjami, z uwzględnieniem autoryzacji, zależności, publikacji i zasad przechowywania. Przed usunięciem sprawdź dane biznesowe; zasoby wskazane w historii zastosowania mogą być współdzielone lub zmodyfikowane.
