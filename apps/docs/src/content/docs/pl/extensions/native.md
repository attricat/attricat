---
title: Natywny interfejs
description: Buduj karty, panele, okna, ustawienia i edytory atrybutów z komponentów zestawu UI Attricat, z logiką w izolowanym Workerze.
---

Natywny interfejs to środkowy z [trzech sposobów rysowania interfejsu](/pl/extensions/build/#trzy-sposoby-rysowania-interfejsu). Twój kod działa w izolowanym Workerze i składa komponenty z **zestawu UI Attricat**. Attricat sprawdza, co renderujesz, i rysuje to własnymi komponentami, więc interfejs wygląda i zachowuje się jak reszta aplikacji: podąża za motywem i językiem, ma tę samą obsługę klawiatury i fokusu i nadal działa, gdy zmienia się wygląd Attricat.

Używaj natywnego interfejsu do kart rekordu, paneli, okien akcji, ustawień atrybutów i edytorów atrybutów. [Interfejs w ramce](/pl/extensions/client/) zostaw na pełne strony i narzędzia, których nie da się zbudować z zestawu UI, na przykład płótno albo własny edytor.

## Pierwsze kroki

Utwórz rozszerzenie za pomocą CLI:

```sh
acli extension new inventory --id acme.inventory --name "Inventory"
cd inventory
pnpm install
pnpm test
pnpm package
```

Polecenie tworzy manifest z jednym artefaktem `client_native`, panelem schematu i edytorem atrybutu, konfigurację Vite, komponent dla każdej kontrybucji oraz test. Wgraj archiwum z katalogu `dist/` przez **Zarządzanie → Rozszerzenia → Wgraj archiwum**, przyznaj uprawnienia i włącz rozszerzenie.

### Manifest

Zadeklaruj jeden artefakt `client_native` i wskaż go w kontrybucjach:

```json
"artifacts": [
  { "id": "ui", "kind": "client_native", "path": "dist/ui.js", "ui_kit": "^1.0" }
],
"ui": [
  { "id": "stock", "version": 1, "kind": "tab", "artifact": "ui", "outlet": "record_tab", "title": "Stock" }
]
```

`ui_kit` to zakres wersji zestawu UI, z którymi działa Twój pakiet. Wszystko inne w kontrybucji — rodzaj, miejsce osadzenia, tytuł, tłumaczenia, `when` i uprawnienie wymagane przez miejsce osadzenia — działa tak samo jak w interfejsie w ramce. Natywny interfejs jest dostępny w `record_tab`, `blueprint_attribute_configuration`, `blueprint_detail_panel`, `blueprint_panel`, `data_health_card`, `export_mapping`, `action_dialog` i `attribute_editor`. Wszystkie natywne kontrybucje wydania dzielą jeden Worker. Zobacz [dokumentację manifestu](/pl/extensions/manifest/#artefakty).

### Komponenty

Zarejestruj jeden komponent Preact dla każdej natywnej kontrybucji, pod jej `id`:

```tsx
import { register, useAttricat, useOutletContext } from '@attricat/extension';
import { Button, Stack, Table, Text } from '@attricat/ui-kit';
import { useEffect, useState } from 'preact/hooks';

type RecordContext = { record_id: string };
type Row = { id: string; cells: { warehouse: string; quantity: number } };

const Stock = () => {
  const { record_id } = useOutletContext<RecordContext>();
  const attricat = useAttricat();
  const [rows, setRows] = useState<Row[]>([]);
  const load = () =>
    attricat.command({ command_id: 'stock', payload: { record_id } }).then((answer) => setRows(answer.rows));
  useEffect(() => void load(), [record_id]);
  return (
    <Stack gap="sm">
      <Text tone="muted">Stan magazynów</Text>
      <Table
        columns={[
          { key: 'warehouse', label: 'Magazyn', sortable: true },
          { key: 'quantity', label: 'Ilość', align: 'end', sortable: true },
        ]}
        label="Stan"
        rows={rows}
      />
      <Button icon="refresh" onPress={load}>Odśwież</Button>
    </Stack>
  );
};

register('stock', Stock);
```

Hooki dają każdej zamontowanej kontrybucji to, czego potrzebuje:

| Hook | Zwraca |
| --- | --- |
| `useOutletContext<T>()` | Kontekst miejsca osadzenia, np. rekord lub wersję schematu. Aktualizuje się na miejscu, gdy kontekst się zmienia. |
| `useLocale()` | Język osoby, np. `en` lub `pl`. Użyj go do tłumaczenia własnych tekstów. |
| `useTheme()` | `{ color_mode: 'light' \| 'dark' }`. Rzadko jest potrzebny: Attricat i tak rysuje Twoje komponenty w bieżącym trybie. |
| `useConfiguration()` | Konfigurację instalacji, gdy rozszerzenie ma `configuration.read`. |
| `useCapabilities()` | Uprawnienia przyznane rozszerzeniu. |
| `useAttricat()` | Te same wywołania co [API `attricat` interfejsu w ramce](/pl/extensions/client/): `request`, `command`, `storage`, `operations`, `navigate`, `notify`, `refresh` i `dialog.close`, każde sprawdzane względem Twoich uprawnień. |
| `useEditor()` | W edytorze atrybutu: wartość pola, atrybut, właściwości, wymagalność, błędy oraz `propose(value)`. |

Gdy zmienia się kontekst, język lub tryb kolorów, Attricat przekazuje nowe wartości zamontowanemu komponentowi zamiast uruchamiać go od nowa, więc jego stan się zachowuje.

### Budowanie, testy i rozwój

Preset Vite z SDK buduje jeden samodzielny pakiet Workera, pilnuje limitu 1 MiB i zapisuje obok pakietu wersję zestawu UI, z którą go zbudowano:

```ts
import { defineConfig } from 'vite';
import { attricatExtension } from '@attricat/extension/vite';

export default defineConfig(attricatExtension({ entry: 'src/main.tsx' }));
```

`attricat-pack` pakuje `manifest.json`, pakiet i zapis kompilacji do archiwum, które instaluje Attricat.

Testuj kontrybucję w Vitest za pomocą narzędzia testowego z SDK, które uruchamia komponent tak jak Attricat i pozwala sprawdzić, co rysuje i jakie wywołania wykonuje:

```ts
import { renderContribution } from '@attricat/extension/testing';
import { Stock } from './Stock';

const view = await renderContribution(Stock, {
  context: { record_id: '…' },
  broker: { command: async () => ({ rows: [] }) },
});
```

Podczas pracy z lokalnym Attricat uruchomionym z narzędziami deweloperskimi polecenie `acli extension dev acme.inventory --build "pnpm build" --watch src` przebudowuje rozszerzenie po każdej zmianie i podmienia Workera na działającym serwerze, bez wgrywania nowego wydania i ponownego przyznawania uprawnień. Otwarte strony pobierają nową wersję w ciągu 15 sekund.

## Zestaw UI

[Dokumentacja zestawu UI](/pl/extensions/ui-kit/) opisuje każdy komponent z jego właściwościami, zdarzeniami i limitami. W skrócie:

- **Układ:** `Stack`, `Grid`, `Divider`.
- **Tekst:** `Text`, `Heading`, `Badge`, `Link`.
- **Akcje:** `Button`, `IconButton`.
- **Pola:** `TextField`, `NumberField`, `Select`, `Checkbox`, `Switch`.
- **Dane:** `Table`, `EmptyState`.

Wygląd nie jest dowolny. Odstępy, rozmiar, ton i wyróżnienie to stałe wybory, które Attricat odwzorowuje na swój projekt; tony stanu to `info`, `success`, `warning` i `error`, a ikony to [ikony hosta](/pl/extensions/ui-kit/#ikony-hosta). Pola wymagają `label`, tak samo przyciski z ikoną: Attricat używa jej jako nazwy dostępnej. Duże treści są danymi, a nie elementami podrzędnymi: przekaż `rows` tabeli, `options` listy wyboru i `suggestions` pola tekstowego, a Attricat je sortuje, stronicuje i wyświetla.

## Edytory atrybutów

Edytor atrybutu zastępuje domyślną kontrolkę pola schematu. Zadeklaruj go w manifeście (zobacz [Edytory atrybutów](/pl/extensions/manifest/#edytory-atrybutów)), a schemat wskaże go na polu swojego widoku `detail`.

Attricat zachowuje etykietę pola, tekst pomocy, błędy i zapisywanie. Twój edytor tylko proponuje wartości:

```tsx
import { register, useEditor } from '@attricat/extension';
import { TextField } from '@attricat/ui-kit';

const FormulaEditor = () => {
  const editor = useEditor();
  return (
    <TextField
      hideLabel
      label={editor.attribute.name}
      monospace
      onChange={({ value }) => editor.propose(value)}
      value={typeof editor.value === 'string' ? editor.value : ''}
    />
  );
};

register('formula', FormulaEditor);
```

Attricat sprawdza każdą propozycję względem typu i ograniczeń atrybutu i zapisuje ją jak każde inne pole, z uprawnieniami tej osoby: po opuszczeniu pola lub naciśnięciu Enter. Escape przywraca zapisaną wartość. Gdy Twoje rozszerzenie jest wyłączone, nie ma uprawnienia, uległo awarii lub dopiero się uruchamia, pole pokazuje domyślną kontrolkę, więc schemat nigdy nie zależy od dostępności rozszerzenia. Edytory działają tylko na stronie rekordu; tabele Przeglądarki rekordów nadal używają [komórek tabeli](/pl/extensions/manifest/#komórki-tabeli).

## Ograniczenia

- **Bez DOM i CSS.** Twoje komponenty renderują tylko elementy zestawu UI. Nie ma `document`, elementów HTML, arkuszy stylów, SVG ani płótna; wszystko inne jest pomijane.
- **Zdarzenia są asynchroniczne.** Zdarzenie dociera do Workera po działaniu osoby, a to, co w odpowiedzi narysujesz, pojawia się chwilę później. Pola należą do Attricat i wysyłają zmiany, gdy pisanie ustaje, oraz ponownie, gdy pole traci fokus.
- **Rysuj tylko w swoim obszarze.** Natywny interfejs nie może otwierać nakładek, okienek ani okien. Okna otwiera tylko Attricat, przez akcję otwierającą Twoją kontrybucję `dialog`.
- **Bez sieci.** Worker nie ma dostępu do sieci i nie może wczytywać kodu. Z serwerem komunikuj się przez `command`, a z Attricat przez `request`.
- **Limity.** Pakiet może mieć najwyżej 1 MiB. Attricat ogranicza liczbę rysowanych elementów, częstotliwość ich zmian i rozmiar każdej wiadomości oraz zatrzymuje Workera, który przestał odpowiadać. Zatrzymany Worker pokazuje komunikat błędu Attricat w każdej swojej kontrybucji; inne rozszerzenia działają dalej.

## Model bezpieczeństwa

Natywny kod każdego rozszerzenia działa w osobnym Workerze, w ukrytej ramce, którą Attricat tworzy z nieprzezroczystym pochodzeniem (opaque origin) i zasadami Content Security Policy blokującymi dostęp do sieci. Worker nie ma dostępu do strony Attricat, ciasteczek, magazynu ani poświadczeń i nigdy nie dzieli środowiska z innym rozszerzeniem. Komunikuje się z Attricat tylko przez własny kanał wiadomości, który identyfikuje rozszerzenie: każde wywołanie jest sprawdzane względem uprawnień tego rozszerzenia i kontekstu kontrybucji, nigdy względem identyfikatorów w wiadomości. Attricat sprawdza każdy element, właściwość i zdarzenie względem zestawu UI, zanim cokolwiek narysuje.
