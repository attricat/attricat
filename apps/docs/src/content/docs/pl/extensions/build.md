---
title: Budowanie rozszerzenia
description: Z czego składa się rozszerzenie Attricat, jak je spakować i jak przetestować je w lokalnym obszarze roboczym.
---

Rozszerzenie to archiwum `.tar.zst` z plikiem `manifest.json` w katalogu głównym i plikami zadeklarowanymi w manifeście. Może zawierać:

- **Komponenty serwerowe**: komponenty WebAssembly, które obsługują zdarzenia katalogu, odpowiadają na polecenia z interfejsu rozszerzenia i wykonują długie operacje, takie jak importy i eksporty. Zobacz [Środowisko wykonawcze serwera](/pl/extensions/server/).
- **Interfejs kliencki** w stałych miejscach aplikacji webowej albo jako pełne strony, na jednym z trzech poziomów opisanych niżej.
- **Deklaracje**, na podstawie których działa host bez uruchamiania Twojego kodu: typy atrybutów, komórki tabeli, schematy konfiguracji i kontrakty zdarzeń.

## Trzy sposoby rysowania interfejsu

| | Deklaratywny | Natywny interfejs | Interfejs w ramce |
| --- | --- | --- | --- |
| Kto rysuje | Attricat | Attricat, z komponentów zestawu UI składanych przez Twój kod | Twój kod, własnym HTML |
| Gdzie działa Twój kod | Nigdzie w przeglądarce | W izolowanym Workerze | W izolowanej ramce |
| Manifest | `cell`, `action`, notatki pól | Artefakt `client_native` | Artefakt `client_frame` |
| Typowe zastosowania | Komórki tabeli, akcje w menu, odznaki przy polach | Karty rekordu, panele, okna, ustawienia, edytory atrybutów | Pełne strony i narzędzia, których nie da się zbudować z zestawu UI |

Wybieraj je w tej kolejności. Kontrybucje **deklaratywne** tylko opisują, co pokazać, więc nic nie kosztują na rekord i zawsze wyglądają jak Attricat. **Natywny interfejs** pozwala Twojej logice sterować ekranem, a Attricat rysuje go własnymi komponentami: interfejs podąża za motywem, językiem i obsługą klawiatury reszty aplikacji i zachowuje swój stan przy zmianie kontekstu lub motywu. Zobacz [Natywny interfejs](/pl/extensions/native/). **Interfejs w ramce** daje Ci całą stronę do narysowania po swojemu, dla warsztatów i edytorów, których nie da się wyrazić zestawem UI. Zobacz [Kontrybucje klienckie](/pl/extensions/client/).

Rozszerzenie referencyjne [`attricat-extension-example`](https://github.com/attricat/attricat-extension-example) pokazuje, jak wszystkie te elementy działają razem: oblicza numeryczne atrybuty z formuł i generuje dokumenty PDF, a do tego ma stronę warsztatu, akcję rekordu, komórkę tabeli, serwerową obsługę zdarzeń i operacje interaktywne. W jego kodzie najłatwiej zobaczyć, jak używać poszczególnych wywołań.

## Utwórz projekt

Najszybciej uzyskasz działające rozszerzenie, generując je:

```sh
acli extension new inventory --id acme.inventory --name "Inventory" --with-server
cd inventory
pnpm install
pnpm test
pnpm package
acli extension validate dist/acme.inventory-0.1.0.tar.zst
```

Polecenie tworzy [natywny interfejs](/pl/extensions/native/) z panelem, edytorem atrybutu i testem, a z `--with-server` także [komponent serwerowy](/pl/extensions/server/) w Rust, który odpowiada na jedno polecenie. Pomiń `--with-server`, jeśli rozszerzenie ma tylko interfejs. Do zbudowania komponentu serwerowego potrzebne są cel Rust `wasm32-unknown-unknown` i `wasm-tools`.

`acli extension validate` wykonuje te same kontrole co przesłanie, ale bez serwera. Wskaż mu katalog po zbudowaniu albo archiwum.

## Od czego zacząć

| Chcesz | Przeczytaj |
| --- | --- |
| Pokazać coś na stronie rekordu lub w przeglądarce rekordów | [Minimalny manifest](#minimalny-manifest), a potem [Natywny interfejs](/pl/extensions/native/) lub [Kontrybucje klienckie](/pl/extensions/client/) |
| Dać polom schematu własną kontrolkę | [Edytory atrybutów](/pl/extensions/native/#edytory-atrybutów) |
| Reagować na zmiany w katalogu albo obsłużyć swój interfejs logiką serwerową | [Zbuduj komponent serwerowy](#zbuduj-komponent-serwerowy), a potem [Środowisko wykonawcze serwera](/pl/extensions/server/) |
| Importować lub eksportować pliki albo przetwarzać zaznaczone rekordy | [Operacje](/pl/extensions/operations/) |
| Synchronizować katalog z marketplace'em lub sklepem internetowym | [Konektory eksportu](/pl/extensions/exports/) |

## Model bezpieczeństwa

Kod rozszerzenia nigdy nie działa we własnym procesie ani na stronie Attricat.

- Komponenty serwerowe działają w piaskownicy WebAssembly bez systemu plików, środowiska, gniazd i zegara systemowego. Wszystko, co robią, przechodzi przez wywołania hosta, a każde wywołanie jest sprawdzane pod kątem uprawnień przyznanych przez administratora.
- Natywny interfejs działa w osobnym Workerze dla każdego rozszerzenia, wewnątrz ukrytej izolowanej ramki z nieprzezroczystym pochodzeniem (opaque origin) i bez dostępu do sieci. Attricat sprawdza każdy element, który ma narysować, względem zestawu UI i rysuje go własnymi komponentami; żaden kod HTML rozszerzenia nie trafia na stronę Attricat.
- Interfejs w ramce działa w ramkach `<iframe sandbox="allow-scripts">` z nieprzezroczystym pochodzeniem i zasadami Content Security Policy blokującymi dostęp do sieci.
- Oba komunikują się z Attricat przez kanał wiadomości, który udostępnia tylko przyznane im operacje i identyfikuje swoje rozszerzenie.
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
  "permissions": ["attricat.read", "client.record_tab"],
  "artifacts": [
    { "id": "panel", "kind": "client_frame", "path": "dist/panel.js" }
  ],
  "ui": [
    {
      "id": "summary",
      "version": 1,
      "kind": "tab",
      "artifact": "panel",
      "outlet": "record_tab",
      "title": "Stany"
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

1. Skopiuj plik `wit/attricat-host-1.1.0.wit` z repozytorium [attricat-extension-sdk](https://github.com/attricat/attricat-extension-sdk) do katalogu `wit/` swojego pakietu. Repozytorium zawiera każdą wydaną wersję ABI; nigdy nie używaj niewydanej kopii.
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

W trakcie pracy nie przesyłaj rozszerzenia ponownie. Na lokalnym serwerze z `ATTRICAT_DEVTOOLS=true` polecenie [`acli extension dev`](#pracuj-bez-przesyłania) podmienia zbudowany kod w zainstalowanym wydaniu i zachowuje przyznane uprawnienia. Drugie przesłanie tego samego rozszerzenia kończy się błędem `extension_already_installed`.

Sprawdź, czy Twój kod obsługuje:

- **Zduplikowane zdarzenia i zdarzenia w innej kolejności.** Dostarczanie odbywa się co najmniej raz. Używaj identyfikatora zdarzenia jako klucza idempotencji.
- **Własne zdarzenia.** Zapis wykonany przez Twoją obsługę zdarzeń tworzy nowe zdarzenie, które ta obsługa również otrzyma. W `event-handler` z wersji 1.1 pomijaj dostarczenia, których `source-name` to `extension:<identyfikator-twojego-rozszerzenia>`. `handler` z wersji 1.0 nie widzi źródła, więc niech jego zapisy prowadzą do stanu stałego: odczytaj bieżącą wartość i zapisuj tylko wtedy, gdy jest inna. W przeciwnym razie procedura obsługi będzie wyzwalać samą siebie bez końca.
- **Błędy powodujące kwarantannę.** W eksporcie `handler` z wersji 1.0 każdy błąd zwrócony z `handle-event` obejmuje instalację kwarantanną. Dla błędnych danych, których ponowienie nie naprawi, zwracaj `Ok` i zapisz problem w magazynie lub dzienniku. W API hosta 1.1 zgłoś go jako [typowany wynik](/pl/extensions/server/#typowane-wyniki).
- **Brakujące uprawnienia.** Uprawnienia opcjonalne mogą nie zostać przyznane; zadbaj o łagodną degradację.
- **Tryb jasny i ciemny.** W komponentach klienckich odczytuj `attricat.theme`.

### Pracuj bez przesyłania

```sh
acli extension dev acme.inventory --build "pnpm build" --watch src
acli extension dev acme.inventory --component dist/server.wasm --build "pnpm build:server" --watch server/src
```

Każda zmiana przebudowuje i podmienia pakiet Workera albo komponent serwerowy. Aby podmienić oba, podaj `--bundle` i `--component`. Manifest pozostaje taki jak przy instalacji, więc zmiana `manifest.json` wymaga uaktualnienia.

### Uaktualnij do nowej wersji

Gdy zmienia się manifest, podnieś `version`, spakuj rozszerzenie i uaktualnij je:

```sh
acli extension upgrade acme.inventory --file dist/acme.inventory-1.0.1.tar.zst
```

Wersja musi być wyższa od zainstalowanej. Uaktualnienie usuwa przyznane uprawnienia i konfigurację oraz pozostawia rozszerzenie wyłączone, więc przyznaj uprawnienia i włącz je ponownie. Pamięć rozszerzenia, połączenia i mapowania eksportu zostają.

### Debugowanie

- `api.log(level, message)` z `logging.write` zapisuje wpis w dzienniku serwera Attricat, oznaczony identyfikatorem rozszerzenia. Poziomy to `trace`, `debug`, `info`, `warn` i `error`. Wpisy są ograniczone do 16 KiB.
- Każde wywołanie hosta zwraca `Err(message)`, gdy zostanie odrzucone, np. `missing capability 'storage.extension'`. Przekazuj te komunikaty do swoich dzienników; wskazują brakujące uprawnienie.
- Instalacja objęta kwarantanną pokazuje swój stan na swojej stronie w **Zarządzanie → Rozszerzenia**. Usuń przyczynę, w razie błędu w kodzie przesyłając poprawione wydanie, i włącz rozszerzenie ponownie.
- Dla operacji `acli extension operation get <run-id>` pokazuje stan, postęp i błąd wykonania.
- W ramce klienckiej narzędzia deweloperskie przeglądarki pokazują wyjście konsoli i błędy. Wybierz ramkę rozszerzenia w menu kontekstu konsoli.

### Częste problemy

| Co widzisz | Dlaczego | Co zrobić |
| --- | --- | --- |
| `unknown field` z `validate` lub przy przesyłaniu | Manifesty są ścisłe, więc pole z literówką lub w złym miejscu jest odrzucane. | Porównaj je z [dokumentacją manifestu](/pl/extensions/manifest/). |
| `extension_already_installed` | Rozszerzenie o tym identyfikatorze jest już zainstalowane. | Użyj `acli extension dev` lub `acli extension upgrade --file`. |
| `upgrade version must be greater than the installed release` | Uaktualnienia działają tylko w przód. | Podnieś `version` w manifeście. |
| `missing capability '…'` z wywołania hosta | Uprawnienie nie jest zadeklarowane lub nie zostało przyznane. | Zadeklaruj je, przyznaj i włącz rozszerzenie ponownie. |
| Instalacja jest w kwarantannie | Komponent uległ awarii, zwrócił błąd krytyczny lub osiągnął [limit](/pl/extensions/server/#limity). | Sprawdź dziennik serwera, rób mniej w jednym wywołaniu i włącz rozszerzenie ponownie. |
| Komponentu nie da się uruchomić | Został zbudowany dla celu WASI. | Buduj dla `wasm32-unknown-unknown` i sprawdź wynik poleceniem `wasm-tools component wit`. |
| `forbidden`, gdy panel wywołuje polecenie | Panele są tylko do odczytu, z wyjątkiem `export_mapping`. | Wywołaj je z akcji, karty rekordu, okna dialogowego lub strony. |
| `extension command failed` w interfejsie | Polecenie zwróciło błąd, który nie dociera do klienta. | To, co ma zobaczyć użytkownik, przekaż w odpowiedzi, a resztę zapisz w dzienniku. |

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

Administratorzy obszaru roboczego dodają Twój rejestr poleceniem `acli extension registry add --source acme/attricat-extensions`. Oficjalny rejestr to [`attricat/attricat-extensions`](https://github.com/attricat/attricat-extensions); aby zaproponować w nim wpis, otwórz pull request dodający go do pliku `registry.json`.

### Wydaj nową wersję

- Stosuj SemVer dla `version` i nigdy nie używaj ponownie wydanej wersji.
- Zachowuj niezmienne identyfikatory kontrybucji, procedur obsługi i reguł uprawnień, bo odwołują się do nich układy i przyznane uprawnienia.
- Uaktualnienia usuwają przyznane uprawnienia i konfigurację, więc w informacjach o wydaniu napisz, które uprawnienia lub ustawienia się zmieniły.
- Pamięć rozszerzenia zostaje, a uaktualnienie nie uruchamia żadnego kodu. Zapisuj wersję formatu razem z danymi, a Twój kod niech odczytuje starsze formaty.
- W CI uruchom testy, spakuj rozszerzenie i wykonaj `acli extension validate` na archiwum, zanim dołączysz je do wydania.
