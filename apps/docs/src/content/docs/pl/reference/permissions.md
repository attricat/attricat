---
title: Dokumentacja uprawnień
description: Wszystkie uprawnienia obszaru roboczego, na co pozwalają i które role wbudowane je mają.
---

Uprawnienia przyznaje się przez role. Role i zakresy opisuje [Administracja obszarem roboczym](/pl/operate/workspaces/#role).

Uprawnienia do rekordów mają nazwy `entities.*`, tak jak w API.

Brak zalogowania lub wygasła sesja zwraca `401`. Zalogowana osoba bez uprawnienia otrzymuje `403`, a odpowiedź nie ujawnia, czy obiekt docelowy istnieje.

| Uprawnienie | Zakres | owner | admin | editor | viewer |
| --- | --- | :-: | :-: | :-: | :-: |
| `workspace.manage` | Cykl życia obszaru roboczego i przeniesienie własności. | ✓ | | | |
| `members.manage` | Członkowie, zaproszenia, tworzenie użytkowników i zespoły (`/workspace/teams`, `acli team`). | ✓ | ✓ | | |
| `roles.grant` | Przydzielanie i odbieranie ról. Wymagane razem z `members.manage`. | ✓ | ✓ | | |
| `roles.manage` | Role niestandardowe; ponawianie dostarczeń zdarzeń, które stały się martwymi wiadomościami. | ✓ | ✓ | | |
| `tokens.manage` | Tworzenie i odwoływanie własnych osobistych tokenów API. | ✓ | ✓ | | |
| `workspace_navigation.manage` | Skróty na pasku bocznym przeglądarki rekordów. | ✓ | ✓ | | |
| `audit.read` | Dziennik audytu. | ✓ | ✓ | | |
| `blueprints.read` | Schematy i atrybuty wielokrotnego użytku. | ✓ | ✓ | ✓ | ✓ |
| `blueprints.write` | Tworzenie szkiców i wersji schematów; atrybuty wielokrotnego użytku. | ✓ | ✓ | ✓ | |
| `blueprints.publish` | Publikowanie wersji schematów. | ✓ | ✓ | | |
| `entities.read` | Rekordy, wyszukiwanie, zapisane wyszukiwania, pliki i historia; katalog użytkowników i zespołów do przypisań (`GET /directory`, `acli directory`), który pokazuje imiona i nazwiska oraz adresy e-mail członków. | ✓ | ✓ | ✓ | ✓ |
| `entities.write` | Tworzenie i edytowanie rekordów, przesyłanie plików, migrowanie rekordów, dołączanie atrybutów wielokrotnego użytku, uruchamianie poleceń rozszerzeń z interfejsu. | ✓ | ✓ | ✓ | |
| `entities.delete` | Usuwanie rekordów. | ✓ | ✓ | ✓ | |
| `entities.publish` | Publikowanie rekordów w kanałach i wycofywanie ich publikacji. | ✓ | ✓ | | |
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

Status w schemacie może wymagać do przejścia uprawnienia lub roli albo innej osoby niż ta, która wykonała wcześniejsze przejście, dodatkowo do `entities.write` i dla każdego, kto zapisuje dane; zablokowane rekordy odrzucają zmiany niezależnie od uprawnień zapisującego. Zobacz [Kontroluj cykl życia rekordu](/pl/builders/validation/#kontroluj-cykl-życia-rekordu).

## Widoczność atrybutów

Uprawnienia dotyczą całych rekordów. Kto może odczytać rekord, może odczytać każdą wartość jego atrybutów we wszystkich kontekstach, także historię wartości i pokazywane w niej zmiany. Te same wartości widzą wyszukiwanie, filtry, etykiety wyświetlania, narzędzia agenta i rozszerzenia. Nie da się ukryć pojedynczych atrybutów, takich jak wycena czy uwagi o pochodzeniu, przed osobami, które mogą odczytać resztę rekordu.

Aby ukryć poufne dane przed częścią osób, zapisz je w osobnym schemacie powiązanym z rekordem i nadaj `entities.read` dla tego schematu tylko osobom, które ich potrzebują. Pamiętaj, że:

- Przydział dla pojedynczego schematu nie obejmuje wyszukiwania, filtrów ani zapisanych wyszukiwań. Wymagają one przydziału na cały obszar roboczy, a taki przydział pozwala czytać wszystkie schematy.
- Listy i podglądy relacji w dostępnym rekordzie mogą pokazywać etykietę i wartości powiązanego rekordu. Nie umieszczaj pól poufnego schematu w widokach dostępnego schematu ani w jego etykiecie `dropdown_option`.
- Rozszerzenia, przepływy pracy i eksporty przez konektory odczytują wszystkie dane.

## Osobiste tokeny API

Token ma własną listę uprawnień. Każde żądanie jest dozwolone tylko wtedy, gdy pozwalają na nie zarówno token, jak i bieżące role jego właściciela. Przydzielanie lub odbieranie ról za pomocą tokenu wymaga, aby token miał zarówno `members.manage`, jak i `roles.grant`.

## Agenci

Agent działa z uprawnieniami osoby, która rozpoczęła rozmowę; są one sprawdzane ponownie przy wykonywaniu każdej zatwierdzonej zmiany. Zobacz [Agenci i zatwierdzenia](/pl/guides/agents/).
