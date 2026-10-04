---
title: Instalacja i obsługa pakietów rozwiązań
description: Bezpieczne sprawdzanie, planowanie, stosowanie i obsługa istniejących pakietów rozwiązań.
---

Pakiet rozwiązania to wersjonowane archiwum `.tar.zst` dostarczone przez wydawcę. Przygotowuje nową instalację do konkretnego zastosowania: może dostarczać Schematy, konteksty i kanały publikacji, reguły, przepływy pracy, zapisane wyszukiwania, nawigację, układ rozszerzeń, tłumaczenia etykiet, zasoby graficzne, wskazówki i opcjonalne dane przykładowe.

Po zastosowaniu pakietu użytkownicy pracują z jego zasobami przez zwykłe funkcje Attricat. Nie trzeba utrzymywać działającej usługi pakietu. Pakiet nie jest właścicielem zasobów, nie synchronizuje ich ani nie usuwa ich później.

## Zanim zaczniesz

- Pobierz archiwum od zaufanego wydawcy i przeczytaj informacje o wydaniu. Attricat nie pobiera pakietów ani zawartości repozytoriów.
- Zaloguj się przez CLI do odpowiedniego obszaru roboczego. Obsługa pakietów wymaga `solution_packs.manage`, domyślnie dostępnego właścicielom i administratorom.
- Nieznane pakiety testuj w jednorazowym obszarze roboczym, szczególnie z danymi przykładowymi.
- Pakiet może zainstalować wymagane rozszerzenia z oficjalnego rejestru rozszerzeń Attricat. Zastosowanie pakietu instaluje je, konfiguruje, nadaje im uprawnienia i włącza je bez osobnego zatwierdzania. Zobacz [Rozszerzenia](#rozszerzenia).

Jeśli archiwum zostanie odrzucone, poproś wydawcę o zgodne wydanie zamiast samodzielnie je edytować lub przepakowywać.

## 1. Sprawdź

```sh
acli solution-pack inspect --file pack.tar.zst
```

Serwer sprawdza archiwum i zwraca jego identyfikator, wersję, skrót, podsumowanie zasobów i ostrzeżenia dotyczące danych przykładowych. Podsumowanie `seeds` wymienia pakiety wymagane przez ten pakiet, deklarowane konteksty i kanały, reguły i przepływy pracy wraz z informacją, czy zostaną włączone, oraz zapisane wyszukiwania. Nie tworzy zasobów ani nie zapisuje archiwum. Poprawna walidacja nie gwarantuje, że pakiet jest odpowiedni dla Twojego obszaru roboczego.

## 2. Zaplanuj

```sh
acli solution-pack plan --file pack.tar.zst \
  --prefix example --blueprint-publication publish
acli solution-pack plan show <plan-id>
```

Planowanie zapisuje niezmienny plan próbny bez zmieniania zasobów katalogu. Sprawdź gotowość, akcje, konflikty i wymagania dotyczące rozszerzeń. Wymaganie ze statusem `install` wskazuje oficjalne wydanie, które zostanie zainstalowane, oraz uprawnienia, które otrzyma.

- `--prefix` tworzy kody nowych Schematów, takie jak `example_product`. Użyj od 1 do 32 małych liter, cyfr lub podkreśleń; zacznij literą i nie kończ podkreśleniem.
- `--blueprint-publication` przyjmuje `draft` lub `publish`. Nawigacja i dane przykładowe mogą wymagać opublikowanych Schematów, więc wybór `draft` może zablokować taki pakiet.
- `--include-sample-data` jawnie wybiera fikcyjne encje przykładowe. Pomiń tę opcję, jeśli ich nie potrzebujesz.

| Akcja | Znaczenie |
| --- | --- |
| `create` | Utworzenie Schematu, kontekstu, kanału publikacji, reguły, przepływu pracy, zapisanego wyszukiwania, zasobu graficznego lub wybranej encji przykładowej. |
| `map` | Ponowne użycie jawnie wskazanego zgodnego zasobu lub kontekstu albo zasobu zainstalowanego przez wymagany pakiet. |
| `append` | Dodanie wpisów nawigacji, układu rozszerzeń lub tłumaczeń. |
| `satisfied` | Żądane ustawienie lub kanał publikacji już istnieje w dokładnie tej postaci. |
| `skip` | Pominięcie niedostępnego elementu opcjonalnego. |
| `conflict` | Obecny stan obszaru roboczego uniemożliwia operację. |
| `blocked` | Wymaganie nie jest spełnione lub zmiana nie jest obsługiwana. |

Można zastosować tylko gotowe plany. Plan wygasa po 24 godzinach, jeśli jego stosowanie jeszcze się nie rozpoczęło.

### Reguły, przepływy pracy i zapisane wyszukiwania

- **Reguły** są tworzone dla Schematów pakietu i publikowane. Podsumowanie planu pokazuje, które zostaną włączone; pozostałe są wyłączone, dopóki ich nie włączysz. Reguły wymagają opublikowanych Schematów, więc `--blueprint-publication draft` blokuje regułę dla nowo tworzonego Schematu.
- Reguła, która **odrzuca nieprawidłowe zmiany**, nie jest włączana dla Schematu wskazanego przez Ciebie lub użytego ponownie, ponieważ ten Schemat może już mieć encje. Zostaje zainstalowana jako wyłączona, a podsumowanie planu pokazuje `enable_deferred_reason: enforcing_rule_requires_dry_run`. Uruchom regułę w trybie próbnym, przejrzyj wyniki i włącz ją tak jak każdą inną regułę.
- **Przepływy pracy** są zawsze publikowane. Podsumowanie planu pokazuje, które zostaną włączone; pozostałe są wyłączone, dopóki ich nie włączysz.
- **Zapisane wyszukiwania** są udostępniane całemu obszarowi roboczemu i pojawiają się na liście **Zapisane wyszukiwania** w Przeglądarce encji. Właścicielem wyszukiwań utworzonych przez stosowany plan jesteś Ty.

Kody nowych reguł i przepływów pracy zaczynają się od Twojego prefiksu. Włączone reguły i przepływy pracy reagują na późniejsze zmiany, także na encje przykładowe tworzone przez ten sam plan, dlatego przejrzyj je przed zastosowaniem. Po instalacji są zwykłymi zasobami, którymi zarządzasz jak zwykle.

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

Istniejące zasoby musisz wskazać jawnie; Attricat nie nadpisuje ich po cichu. Jeśli wymaganie dotyczące rozszerzenia jest zablokowane, rozwiąż problem przez standardowe zarządzanie rozszerzeniami i utwórz nowy plan.

### Konteksty i kanały publikacji

Pakiet może tworzyć [konteksty](/pl/guides/contexts/), na przykład dla rynków lub języków, i ustawiać niektóre z nich jako [kanały publikacji](/pl/guides/publishing/). Nowe konteksty otrzymują kody z prefiksem, takie jak `example_pl`. Aby użyć istniejącego kontekstu, wskaż go:

```sh
acli context list
acli solution-pack plan --file pack.tar.zst --prefix example \
  --blueprint-publication publish \
  --map-context contexts/poland=PL
```

Kanał może wymagać spełnienia niektórych reguł pakietu lub poprawności encji, zanim encja zostanie w nim opublikowana; podsumowanie planu je wymienia (`required_rule_codes`, `require_valid_entity`). Kanał jest planowany po tych regułach.

Wskazany kontekst jest używany bez zmian. Jeśli pakiet oczekuje, że będzie kanałem, istniejący kanał o dokładnie takich samych ustawieniach, łącznie z wymaganymi sprawdzeniami, ma status `satisfied`, a brakujący zostanie utworzony. Kanał o innych ustawieniach, na przykład wyłączony, gdy pakiet oczekuje włączonego, powoduje konflikt `publication_channel_mismatch`: zmień kanał samodzielnie albo nie wskazuj tego kontekstu. Reguły, zapisane wyszukiwania i wartości przykładowe przypisane do kontekstu pakietu używają utworzonego lub wskazanego kontekstu.

### Wymagane pakiety

Pakiet może zależeć od innych pakietów, na przykład aby współdzielić Schemat dostawcy. Plan pokazuje każdy z nich jako akcję `prerequisite`:

- `map`: ponownie używana jest ukończona instalacja wymaganego pakietu w akceptowanej wersji.
- `blocked` z `prerequisite_missing` lub `prerequisite_incompatible`: najpierw zainstaluj odpowiednią wersję wymaganego pakietu, a potem utwórz nowy plan.

Schematy współdzielone z wymaganym pakietem są ponownie używane tylko wtedy, gdy nadal są dokładnie zgodne (`prerequisite_blueprint_match`). Jeśli ktoś zmienił lub usunął taki Schemat, plan zgłasza konflikt zamiast tworzyć kopię. Attricat nigdy nie instaluje wymaganych pakietów za Ciebie.

### Rozszerzenia

Gdy wymagane rozszerzenie nie jest zainstalowane, planowanie wybiera najnowsze wydanie z zakresu wersji pakietu z oficjalnego rejestru Attricat. Rejestry dodane w obszarze roboczym nie są używane. Planowanie wymaga dostępu do oficjalnego rejestru, ale niczego nie instaluje.

Zastosowanie planu instaluje dokładnie to wydanie przed pozostałymi krokami. Konfiguruje je zgodnie z pakietem, nadaje uprawnienia wymagane przez wydanie i włącza je. Każda zmiana trafia do historii cyklu życia rozszerzenia i do dziennika audytu. Jeśli wydanie zmieniło się od przejrzenia planu, zastosowanie zostaje przerwane; utwórz nowy plan.

Pakiety nie zmieniają istniejących instalacji. Zgodna instalacja spełnia wymaganie niezależnie od tego, czy jest włączona. Niezgodna wersja, instalacja w kwarantannie lub inna konfiguracja blokuje plan; pakiet nigdy nie aktualizuje ani nie rekonfiguruje zainstalowanego rozszerzenia. Rozszerzenia zainstalowane przez pakiet są zwykłymi instalacjami, którymi zarządzasz, które wyłączasz lub usuwasz jak zwykle.

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

Jeśli stosowanie pakietu zostało przerwane lub zakończyło się błędem, który pozwala na wznowienie, ponów je z tym samym identyfikatorem planu. Serwer sprawdza ukończone kroki i kontynuuje pozostałe bez tworzenia duplikatów; każdy kontekst, kanał, reguła, przepływ pracy i zapisane wyszukiwanie powstaje razem z zapisem swojego kroku. Rozpoczęte stosowanie można wznowić po wygaśnięciu planu, dopóki nie upłynie termin przechowywania jego danych.

Jeśli zmiany w obszarze roboczym unieważniły plan, przejrzyj ukończone kroki i diagnostykę przed utworzeniem kolejnego. Jeśli późniejszy krok zakończy się trwałym błędem, wcześniejsze udane zapisy pozostają w obszarze roboczym; Attricat ich nie wycofuje.

## Zastosuj nowsze wydanie

```sh
acli solution-pack plan --file pack-v2.tar.zst --prefix example \
  --blueprint-publication publish --from-application <application-id>
```

Wskaż jedno ukończone zastosowanie tego samego pakietu w tym samym obszarze roboczym. Wydanie musi być nowsze. `--from-application` nie można łączyć z jawnymi mapowaniami.

Niezmienione, dokładnie zgodne opublikowane Schematy i niezmienione zasoby graficzne mogą być użyte ponownie; nowe zasoby mogą zostać utworzone. Konteksty z wcześniejszej instalacji są używane ponownie, a reguły, przepływy pracy, zapisane wyszukiwania i kanały, które już utworzyła, są pomijane jako `provided_by_prior_application` i nigdy nie są aktualizowane. Zmienione definicje są blokowane jako `update_not_supported`. Usunięte zasoby są raportowane, ale nie kasowane. Brakujące lub zmodyfikowane wcześniejsze zasoby mogą powodować konflikty. Dla zmian, których pakiet nie obsługuje, uzgodnij z wydawcą procedurę migracji.

## Opcjonalne dane przykładowe

Dodaj `--include-sample-data` podczas planowania dopiero po przeczytaniu ostrzeżenia. Encje przykładowe powstają na podstawie opublikowanych Schematów i otrzymują widoczne oznaczenie danych przykładowych. Ich wartości trafiają do kontekstu domyślnego albo, jeśli tak przewiduje pakiet, do jednego z jego kontekstów. Encje przykładowe mogą też mieć dołączone pliki z pakietu, na przykład obrazy lub dokumenty PDF; planowanie przesyła je do zwykłego magazynu plików, a na encjach przykładowych pojawiają się jako zwykłe pliki. Attricat sprawdza typ każdego pliku i reguły plików atrybutu, ale nie potrafi ocenić, czy jego treść jest fikcyjna. Są zwykłymi encjami: ich tworzenie zapisuje audyt i zdarzenia `entity.created.v1`, może uruchamiać aktywne przepływy pracy lub rozszerzenia i powodować skutki w systemach zewnętrznych. Wartości mogą pozostać w historii audytu i zdarzeń po usunięciu tymczasowych danych pakietu.

Pierwszy plan z danymi przykładowymi rezerwuje dokładnie tę kombinację wydania, archiwum i zestawu danych. Drugi plan nie może wybrać tej samej kombinacji, nawet po wygaśnięciu lub porzuceniu pierwszego; zmiana prefiksu nie usuwa rezerwacji. Ponawiaj oryginalny plan. Jeśli wygaśnie przed rozpoczęciem stosowania, uzgodnij nowe wydanie z wydawcą.

Stosowanie pakietu z danymi przykładowymi można wznowić przez 30 dni od rozpoczęcia. Aby trwale je zatrzymać:

```sh
acli solution-pack applications abandon <application-id>
```

Po porzuceniu serwer usuwa tymczasowe dane wejściowe, w tym pliki z pakietu, które nie zostały dołączone. Wcześniej utworzone encje i zwykła historia audytu oraz zdarzeń pozostają bez zmian. Usunięcie encji przykładowej usuwa ją razem z wartościami we wszystkich kontekstach i plikami, jak każdą inną encję; przechowywane pliki podlegają zwykłym zasadom przechowywania plików. Późniejsze wydania nie resetują bieżących wartości ani nie przywracają usuniętych oznaczeń danych przykładowych. Nie ma polecenia resetowania zestawu ani automatycznego sprzątania.

## Tłumaczenia

Pakiet może zawierać [tłumaczenia etykiet](/pl/builders/translations/), widoczne podczas sprawdzania jako ustawienie `workspace/lexicon`, a w planie jako akcja `append`. Zastosowanie pakietu dodaje brakujące wpisy i aktualizuje wpisy dostarczone wcześniej przez pakiet. Wpisy zapisane w obszarze roboczym, przed instalacją lub po niej, nigdy nie są nadpisywane, a edycja wpisu dostarczonego przez pakiet czyni go wpisem obszaru roboczego. Tłumaczenia nigdy nie powodują konfliktów.

## Ograniczenia i usuwanie

Pakiety nie mogą zmieniać istniejących kontekstów ani kanałów, zmieniać członkostwa lub ról, instalować rozszerzeń spoza oficjalnego rejestru, aktualizować zainstalowanych rozszerzeń, uruchamiać wykonywalnych instalatorów, instalować wymaganych pakietów ani aktualizować istniejących Schematów, reguł, przepływów pracy czy zapisanych wyszukiwań.

Nie ma odinstalowania ani wycofania całego pakietu. Administratorzy mogą edytować lub usuwać pojedyncze zasoby zwykłymi operacjami, z uwzględnieniem autoryzacji, zależności, publikacji i zasad przechowywania. Przed usunięciem sprawdź dane biznesowe; zasoby wskazane w historii zastosowania mogą być współdzielone lub zmodyfikowane.
