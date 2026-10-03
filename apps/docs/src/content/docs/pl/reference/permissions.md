---
title: Dokumentacja uprawnień
description: Wszystkie uprawnienia obszaru roboczego, na co pozwalają i które role wbudowane je mają.
---

Uprawnienia przyznaje się przez role. Role i zakresy opisuje [Administracja obszarem roboczym](/pl/operate/workspaces/#role).

Brak zalogowania lub wygasła sesja zwraca `401`. Zalogowana osoba bez uprawnienia otrzymuje `403`, a odpowiedź nie ujawnia, czy obiekt docelowy istnieje.

| Uprawnienie | Zakres | owner | admin | editor | viewer |
| --- | --- | :-: | :-: | :-: | :-: |
| `workspace.manage` | Cykl życia obszaru roboczego i przeniesienie własności. | ✓ | | | |
| `members.manage` | Członkowie, zaproszenia i tworzenie użytkowników. | ✓ | ✓ | | |
| `roles.grant` | Przydzielanie i odbieranie ról. Wymagane razem z `members.manage`. | ✓ | ✓ | | |
| `roles.manage` | Role niestandardowe; ponawianie dostarczeń zdarzeń, które stały się martwymi wiadomościami. | ✓ | ✓ | | |
| `tokens.manage` | Tworzenie i odwoływanie własnych osobistych tokenów API. | ✓ | ✓ | | |
| `workspace_navigation.manage` | Skróty na pasku bocznym przeglądarki encji. | ✓ | ✓ | | |
| `audit.read` | Dziennik audytu. | ✓ | ✓ | | |
| `blueprints.read` | Schematy i atrybuty wielokrotnego użytku. | ✓ | ✓ | ✓ | ✓ |
| `blueprints.write` | Tworzenie szkiców i wersji schematów; atrybuty wielokrotnego użytku. | ✓ | ✓ | ✓ | |
| `blueprints.publish` | Publikowanie wersji schematów. | ✓ | ✓ | | |
| `entities.read` | Encje, wyszukiwanie, zapisane wyszukiwania, pliki i historia. | ✓ | ✓ | ✓ | ✓ |
| `entities.write` | Tworzenie i edytowanie encji, przesyłanie plików, migrowanie encji, dołączanie atrybutów wielokrotnego użytku, uruchamianie poleceń rozszerzeń z interfejsu. | ✓ | ✓ | ✓ | |
| `entities.delete` | Usuwanie encji. | ✓ | ✓ | ✓ | |
| `entities.publish` | Publikowanie encji w kanałach i wycofywanie ich publikacji. | ✓ | ✓ | | |
| `contexts.read` | Konteksty i kanały eksportu. | ✓ | ✓ | ✓ | ✓ |
| `contexts.write` | Tworzenie, zmienianie i usuwanie kontekstów; włączanie kanałów eksportu. | ✓ | ✓ | ✓ | |
| `data_health.read` | Stan danych, przetwarzanie w tle, metryki i listy martwych wiadomości zdarzeń. | ✓ | ✓ | ✓ | ✓ |
| `agents.run` | Rozmowy z agentami i zatwierdzenia. | ✓ | ✓ | | |
| `rules.read` | Reguły, uruchomienia i ustalenia. | ✓ | ✓ | | |
| `rules.manage` | Tworzenie, publikowanie, włączanie i uruchamianie reguł; potwierdzanie ustaleń. | ✓ | ✓ | | |
| `workflows.read` | Przepływy pracy i historia uruchomień. | ✓ | ✓ | | |
| `workflows.manage` | Tworzenie, publikowanie, włączanie, uruchamianie i ponawianie przepływów pracy. | ✓ | ✓ | | |
| `extensions.read` | Przeglądanie rejestrów rozszerzeń i zainstalowanych rozszerzeń. | ✓ | ✓ | | |
| `extensions.manage` | Instalowanie, konfigurowanie, przyznawanie uprawnień, włączanie i usuwanie rozszerzeń; rejestry, układ, sekrety, operacje i zadania konektorów. | ✓ | ✓ | | |
| `solution_packs.manage` | Sprawdzanie, planowanie i stosowanie pakietów rozwiązań; zasoby prezentacyjne. | ✓ | ✓ | | |
| `files.hold` | Zakładanie i zwalnianie jawnych blokad retencji plików. | ✓ | ✓ | | |

## Przejścia statusów

Status w schemacie może wymagać uprawnienia lub roli dla pojedynczego przejścia oraz tego, by wykonała je inna osoba niż ta, która wykonała wcześniejsze przejście. Te kontrole obowiązują dodatkowo do `entities.write` i dotyczą każdego, kto zapisuje dane, także przepływów pracy, rozszerzeń i agentów. Odrzucone przejście zwraca `403` z kodem `status_transition_forbidden` lub `status_separation_of_duties`. Zobacz [Kontroluj cykl życia rekordu](/pl/builders/blueprints/#krok-10-kontroluj-cykl-życia-rekordu).

Rekordy w zablokowanym statusie odrzucają zmiany zablokowanej treści z `409 record_locked` niezależnie od uprawnień zapisującego. Odblokowanie wymaga jawnego, dozwolonego przejścia korygującego, które jest zapisywane w dzienniku audytu.

## Osobiste tokeny API

Token ma własną listę uprawnień. Każde żądanie jest dozwolone tylko wtedy, gdy pozwalają na nie zarówno token, jak i bieżące role jego właściciela. Przydzielanie lub odbieranie ról za pomocą tokenu wymaga, aby token miał zarówno `members.manage`, jak i `roles.grant`.

## Agenci

Agent działa z uprawnieniami osoby, która rozpoczęła rozmowę; są one sprawdzane ponownie przy wykonywaniu każdej zatwierdzonej zmiany. Zobacz [Agenci i zatwierdzenia](/pl/guides/agents/).
