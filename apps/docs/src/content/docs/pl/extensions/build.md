---
title: Budowanie rozszerzenia
description: Z czego składa się rozszerzenie Attricat, jak je spakować i jak przetestować je w lokalnym obszarze roboczym.
---

Rozszerzenie to archiwum `.tar.zst` z plikiem `manifest.json` w katalogu głównym i plikami zadeklarowanymi w manifeście. Może zawierać:

- **Komponenty serwerowe**: komponenty WebAssembly, które obsługują zdarzenia katalogu, odpowiadają na polecenia z interfejsu rozszerzenia i wykonują długie operacje, takie jak importy i eksporty. Zobacz [Środowisko wykonawcze serwera](/pl/extensions/server/).
- **Komponenty klienckie**: moduły JavaScript renderowane w izolowanych ramkach w stałych miejscach aplikacji webowej albo jako pełne strony. Zobacz [Kontrybucje klienckie](/pl/extensions/client/).
- **Deklaracje**, na podstawie których działa host bez uruchamiania Twojego kodu: typy atrybutów, renderery komórek tabeli, schematy konfiguracji i kontrakty zdarzeń.

Rozszerzenie referencyjne `attricat-extension-example` pokazuje, jak wszystkie te elementy działają razem: oblicza numeryczne atrybuty z formuł i ma stronę warsztatu, akcję rekordu, komórkę tabeli oraz serwerową obsługę zdarzeń.

## Od czego zacząć

| Chcesz | Przeczytaj |
| --- | --- |
| Pokazać coś na stronie rekordu lub w przeglądarce rekordów | [Minimalny manifest](#minimalny-manifest), a potem [Kontrybucje klienckie](/pl/extensions/client/) |
| Reagować na zmiany w katalogu albo obsłużyć swój interfejs logiką serwerową | [Zbuduj komponent serwerowy](#zbuduj-komponent-serwerowy), a potem [Środowisko wykonawcze serwera](/pl/extensions/server/) |
| Importować lub eksportować pliki albo przetwarzać zaznaczone rekordy | [Operacje](/pl/extensions/operations/) |
| Synchronizować katalog z marketplace'em lub sklepem internetowym | [Konektory eksportu](/pl/extensions/exports/) |

## Model bezpieczeństwa

Kod rozszerzenia nigdy nie działa we własnym procesie ani na stronie Attricat.

- Komponenty serwerowe działają w piaskownicy WebAssembly bez systemu plików, środowiska, gniazd i zegara systemowego. Wszystko, co robią, przechodzi przez wywołania hosta, a każde wywołanie jest sprawdzane pod kątem uprawnień przyznanych przez administratora.
- Komponenty klienckie działają w ramkach `<iframe sandbox="allow-scripts">` z nieprzezroczystym pochodzeniem (opaque origin) i zasadami Content Security Policy blokującymi dostęp do sieci. Komunikują się z Attricat przez kanał wiadomości, który udostępnia tylko przyznane im operacje.
- Zmiany w katalogu wprowadzone przez rozszerzenie przechodzą przez tę samą walidację, dziennik audytu i strumień zdarzeń co edycje wykonane przez osobę.

Projektuj z myślą o tym: Twoje rozszerzenie prosi o uprawnienia, a administrator decyduje, które przyznać.

## Minimalny manifest

```json
{
  "manifest_version": 1,
  "name": "Inventory panel",
  "version": "1.0.0",
  "description": "Shows warehouse stock on product pages.",
  "icons": { "48": "assets/icon-48.svg" },
  "attricat": {
    "id": "acme.inventory",
    "host_api": ">=1.0.0, <2.0.0"
  },
  "permissions": ["attricat.read"],
  "artifacts": [
    { "id": "panel", "kind": "client_component", "path": "dist/panel.js" }
  ],
  "ui": [
    {
      "id": "summary",
      "version": 1,
      "kind": "embedded",
      "artifact": "panel",
      "outlet": "record_preview_panel"
    }
  ]
}
```

```js
// dist/panel.js
export const mount = async (root, attricat) => {
  const form = await attricat.request(`/api/v1/records/${attricat.context.record_id}`);
  root.textContent = `${form.blueprint.blueprint.name} v${form.record.blueprint_version}`;
  return () => root.replaceChildren();
};
```

Pełny format manifestu opisuje [dokumentacja manifestu](/pl/extensions/manifest/).

## Wersje

Trzy numery wersji są od siebie niezależne:

| Pole | Znaczenie |
| --- | --- |
| `manifest_version` | Format manifestu. Obecnie `1`. |
| `version` | Twoje wydanie, w formacie SemVer. |
| `attricat.host_api` | Zakres SemVer wersji API hosta Attricat, z którymi działa Twój kod, np. `>=1.0.0, <2.0.0`. |

Attricat nigdy nie instaluje wydania, którego zakres `host_api` nie obejmuje działającej wersji API hosta.

## Zbuduj komponent serwerowy

Artefakt serwerowy to komponent WebAssembly, który importuje wyłącznie interfejsy `attricat:host`. Nadaje się każdy język, który potrafi tworzyć komponenty; poniższe przykłady używają Rusta i [`wit-bindgen`](https://github.com/bytecodealliance/wit-bindgen).

1. Skopiuj plik `crates/extension-runtime/wit-host/attricat-extension.wit` z repozytorium Attricat do katalogu `wit/` swojego pakietu.
2. Wygeneruj powiązania dla [świata](/pl/extensions/server/#wersja-api-hosta) i zaimplementuj jego eksporty:

   ```rust
   // Cargo.toml: crate-type = ["cdylib"]; zależności wit-bindgen, serde_json
   wit_bindgen::generate!({ path: "wit", world: "handler-extension" });

   use attricat::host::api::{self, Event};
   use exports::attricat::host::handler::{CommandRequest, CommandResponse, Guest};

   struct Extension;

   impl Guest for Extension {
       fn handle_event(event: Event) -> Result<(), String> {
           if event.event_type != "record.updated.v1" {
               return Ok(());
           }
           // Wymaga logging.write. Ignoruj nieudany wpis: błąd tutaj objąłby rozszerzenie kwarantanną.
           let _ = api::log("info", &format!("record changed: {}", event.aggregate_id));
           Ok(())
       }

       fn handle_command(request: CommandRequest) -> Result<CommandResponse, String> {
           match request.handler.as_str() {
               "ping" => Ok(CommandResponse { payload: r#"{"ok":true}"#.into() }),
               other => Err(format!("unknown command {other}")),
           }
       }
   }

   export!(Extension);
   ```

3. Zbuduj moduł dla `wasm32-unknown-unknown` i opakuj go jako komponent:

   ```sh
   cargo build --release --target wasm32-unknown-unknown
   wasm-tools component new target/wasm32-unknown-unknown/release/acme_inventory.wasm \
     -o dist/server.wasm
   ```

Nie buduj dla `wasm32-wasip1` ani `wasm32-wasip2`: te cele importują interfejsy WASI, których Attricat nigdy nie udostępnia, więc nie da się utworzyć instancji komponentu. Aby to sprawdzić, uruchom `wasm-tools component wit dist/server.wasm`. Każdy wymieniony `import` powinien być interfejsem `attricat:host/…`.

## Spakuj

Zbuduj artefakty, a następnie utwórz archiwum tar skompresowane zstd, którego katalog główny zawiera `manifest.json`, każdą zadeklarowaną ścieżkę artefaktu i ikony:

```text
manifest.json
assets/icon-48.svg
dist/server.wasm
dist/panel.js
```

```sh
tar -cf - manifest.json assets dist | zstd -19 -o dist/acme.inventory-1.0.0.tar.zst
```

Pakiety są ograniczone do 32 MiB po kompresji. Ścieżki muszą być względne, bez `..`, dowiązań i urządzeń.

## Testuj lokalnie

1. Uruchom lokalny obszar roboczy Attricat.
2. Prześlij archiwum w **Zarządzanie → Rozszerzenia → Prześlij archiwum** albo poleceniem `acli extension sideload --file dist/acme.inventory-1.0.0.tar.zst`.
3. Przyznaj wszystkie potrzebne uprawnienia i włącz rozszerzenie.
4. Przetestuj każdą kontrybucję: otwórz strony, na których pojawia się Twój interfejs, wyzwól zdarzenia, na które nasłuchują Twoje procedury obsługi, uruchom swoje polecenia i operacje.

Każde przesłanie to nowe wydanie, więc po każdym trzeba ponownie przyznać uprawnienia i włączyć rozszerzenie.

Sprawdź, czy Twój kod obsługuje:

- **Zduplikowane zdarzenia i zdarzenia w innej kolejności.** Dostarczanie odbywa się co najmniej raz. Używaj identyfikatora zdarzenia jako klucza idempotencji.
- **Własne zdarzenia.** Zapis wykonany przez Twoją obsługę zdarzeń tworzy nowe zdarzenie, które ta obsługa również otrzyma. W `event-handler` z wersji 1.1 pomijaj dostarczenia, których `source-name` to `extension:<identyfikator-twojego-rozszerzenia>`. `handler` z wersji 1.0 nie widzi źródła, więc niech jego zapisy prowadzą do stanu stałego: odczytaj bieżącą wartość i zapisuj tylko wtedy, gdy jest inna. W przeciwnym razie procedura obsługi będzie wyzwalać samą siebie bez końca.
- **Błędy powodujące kwarantannę.** W eksporcie `handler` z wersji 1.0 każdy błąd zwrócony z `handle-event` obejmuje instalację kwarantanną. Dla błędnych danych, których ponowienie nie naprawi, zwracaj `Ok` i zapisz problem w magazynie lub dzienniku. W API hosta 1.1 zgłoś go jako [typowany wynik](/pl/extensions/server/#typowane-wyniki).
- **Brakujące uprawnienia.** Uprawnienia opcjonalne mogą nie zostać przyznane; zadbaj o łagodną degradację.
- **Tryb jasny i ciemny.** W komponentach klienckich odczytuj `attricat.theme`.

### Debugowanie

- `api.log(level, message)` z `logging.write` zapisuje wpis w dzienniku serwera Attricat, oznaczony identyfikatorem rozszerzenia. Poziomy to `trace`, `debug`, `info`, `warn` i `error`. Wpisy są ograniczone do 16 KiB.
- Każde wywołanie hosta zwraca `Err(message)`, gdy zostanie odrzucone, np. `missing capability 'storage.extension'`. Przekazuj te komunikaty do swoich dzienników; wskazują brakujące uprawnienie.
- Instalacja objęta kwarantanną pokazuje swój stan na swojej stronie w **Zarządzanie → Rozszerzenia**. Usuń przyczynę, w razie błędu w kodzie przesyłając poprawione wydanie, i włącz rozszerzenie ponownie.
- Dla operacji `acli extension-operation show <run-id>` pokazuje stan, postęp i błąd przebiegu.
- W ramce klienckiej narzędzia deweloperskie przeglądarki pokazują wyjście konsoli i błędy. Wybierz ramkę rozszerzenia w menu kontekstu konsoli.

## Opublikuj

Aby dystrybuować rozszerzenie przez rejestr, opublikuj archiwum jako zasób GitHub Release (nie szkic ani wersję przedpremierową) w repozytorium rozszerzenia i dodaj repozytorium do pliku `registry.json` rejestru:

```json
{
  "registry_version": 1,
  "extensions": [{
    "id": "acme.inventory",
    "name": "Inventory panel",
    "description": "Shows warehouse stock on product pages.",
    "icon": "icon.svg",
    "repository": "acme/attricat-inventory"
  }]
}
```

Administratorzy obszaru roboczego dodają Twój rejestr poleceniem `acli extension-registry add --source acme/attricat-extensions`. Oficjalny rejestr to `attricat/attricat-extensions`.
