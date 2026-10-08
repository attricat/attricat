---
title: Instalowanie rozszerzeń i zarządzanie nimi
description: Znajduj, instaluj, sprawdzaj, przyznawaj uprawnienia, włączaj, uaktualniaj i izoluj rozszerzenia w obszarze roboczym.
---

Rozszerzenia dodają funkcje do Attricat bez zmieniania rdzenia produktu: import i eksport CSV, silnik formuł, panel pokazujący stany magazynowe z innego systemu, niestandardową komórkę tabeli. Rozszerzenie może uruchamiać kod na serwerze w piaskownicy WebAssembly, dodawać interfejs w izolowanych ramkach, reagować na zdarzenia katalogu i wywoływać usługi zewnętrzne przez kontrolowane API sieciowe.

Włączone rozszerzenie jest zaufanym oprogramowaniem obszaru roboczego. Działa w ramach przyznanych mu uprawnień, więc przyznawaj tylko te, których potrzebuje.

Zarządzanie rozszerzeniami wymaga `extensions.manage`. Przeglądanie rejestru wymaga `extensions.read`.

## Skąd pochodzą rozszerzenia

W **Zarządzanie → Rozszerzenia** rozszerzenie można uzyskać z trzech miejsc:

- **Rynek** wyświetla rozszerzenia z zaufanych rejestrów. Każdy obszar roboczy ma oficjalny rejestr Attricat. Administratorzy mogą dodawać kolejne rejestry GitHub. Rejestr to repozytorium GitHub z indeksem `registry.json`; Attricat rozwiązuje tylko repozytoria wymienione w indeksie, któremu ufa.
- **Prześlij archiwum** instaluje pakiet `.tar.zst` z Twojego komputera, do 32 MiB. Używaj tej opcji dla prywatnych rozszerzeń i do testowania własnych.
- **CLI**: `acli extension install`, `acli extension sideload --file extension.tar.zst`.

Przesłane archiwa przechodzą te same kontrole co instalacje z rejestru: limity rozmiaru, bezpieczne ścieżki plików, ścisły manifest i zgodną wersję API hosta.

## Zainstaluj, sprawdź i włącz

Nowa instalacja startuje jako **wyłączona**. Przed jej włączeniem:

1. **Przeczytaj żądane uprawnienia.** Każde z nich wskazuje możliwość, np. `catalog.write` (zmiana rekordów), `events.subscribe` (reagowanie na zmiany), `network.request` (wywoływanie usług zewnętrznych) lub `client.entity_action` (dodanie przycisku na stronach rekordów). W nazwach uprawnień rekordy występują pod nazwą `entity`. Wszystkie wymienia [dokumentacja manifestu](/pl/extensions/manifest/#uprawnienia).
2. **Sprawdź dostęp do sieci.** `network.request` pozwala wyłącznie na wywołania wzorców URL wymienionych jako uprawnienia hosta. Każdy wzorzec pokazuje swoje hosty, metody, limity rozmiaru i limit czasu.
3. **Skonfiguruj** rozszerzenie, jeśli ma ustawienia.
4. **Przyznaj** wymagane uprawnienia. Uprawnienia opcjonalne można pominąć; rozszerzenie musi działać bez nich.
5. **Włącz** je.

Włączenie się nie powiedzie, jeśli wymagane uprawnienie nie zostało przyznane, konfiguracja jest nieprawidłowa albo brakuje zależności od innego rozszerzenia lub jest ona wyłączona.

## Uaktualnienia

Uaktualnienie przełącza instalację na nowsze wydanie. Ponieważ nowe wydanie może żądać innych uprawnień, uaktualnienie usuwa wszystkie przyznane uprawnienia i konfigurację oraz pozostawia rozszerzenie wyłączone. Ponownie je sprawdź, skonfiguruj, przyznaj uprawnienia i włącz.

Trwające operacje pozostają przy wydaniu, na którym się rozpoczęły. Są wstrzymywane, dopóki to dokładnie wydanie nie zostanie ponownie autoryzowane.

## Rozmieść interfejs rozszerzeń

Gdy kilka rozszerzeń dodaje elementy w tym samym miejscu, np. na pasku akcji rekordu, pojawiają się w stałej kolejności według identyfikatora rozszerzenia. **Zarządzanie → Rozszerzenia → Układ rozszerzeń** pozwala zmienić kolejność, ukryć elementy i przenieść strony rozszerzeń do głównej nawigacji.

Schemat może nadpisać układ dla stron swoich rekordów. Zobacz [Widoki i układy](/pl/builders/views/#panele-rozszerzeń-na-stronach-rekordów).

## Sekrety

Rozszerzenia wywołujące usługi zewnętrzne często potrzebują klucza API. Zapisz go jako nazwany sekret rozszerzeń obszaru roboczego przez API:

```http
PUT /workspace/extension-secrets/destination-token
{"value": "…"}
```

`GET /workspace/extension-secrets` zwraca wyłącznie nazwy, a `DELETE` usuwa sekret. Wartości są tylko do zapisu: nikt nie może ich odczytać, a rozszerzenie z `secrets.read` otrzymuje wartość tylko w trakcie działania.

## Gdy coś pójdzie nie tak

- **Wyłącz** rozszerzenie, aby je zatrzymać bez utraty konfiguracji i przyznanych uprawnień.
- **Kwarantanna** oznacza instalację jako niebezpieczną. Attricat automatycznie poddaje rozszerzenie kwarantannie, gdy jego kod serwerowy ulegnie awarii, wyczerpie pamięć lub czas albo zwróci błąd podczas obsługi zdarzenia. Ponowne włączenie jeszcze raz sprawdza wydanie i konfigurację.
- **Usuń** odinstalowuje rozszerzenie. Jego historia pozostaje w dzienniku audytu.
- Przełącznik **trybu rozszerzeń** obszaru roboczego wyłącza naraz wszystkie rozszerzenia w obszarze roboczym, bez zmieniania żadnej instalacji. Włącz go ponownie, aby przywrócić te, które były włączone.

Operatorzy wdrożenia mają dwa dodatkowe mechanizmy: `EXTENSIONS_MODE=disabled` zatrzymuje rozszerzenia w całym wdrożeniu, a `EXTENSION_DENYLIST` blokuje wybrane rozszerzenia lub wydania. Zobacz [dokumentację konfiguracji](/pl/reference/configuration/#rozszerzenia).

Otwarte ramki interfejsu wykrywają te zmiany w ciągu 15 sekund i zamykają się.

## CLI

```sh
acli extension-registry list
acli extension-registry add --source acme/catalog-extensions
acli extension list
acli extension install --owner acme --repository catalog-inventory --release-id <github-release-id>
acli extension sideload --file inventory-1.2.0.tar.zst
acli extension configure acme.inventory --configuration config.json
acli extension grant acme.inventory --grant-kind capability --grant-id catalog.read
acli extension grant acme.inventory --grant-kind host_permission --grant-id inventory-api
acli extension enable acme.inventory
acli extension disable acme.inventory
acli extension workspace-mode --enabled false
```

## Zbuduj własne

Zobacz [Budowanie rozszerzenia](/pl/extensions/build/).
