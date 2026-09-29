---
title: Pakiety rozwiązań
description: Przygotuj obszar roboczy z wersjonowanego archiwum Schematów, nawigacji, układu rozszerzeń, zasobów graficznych i kontroli konfiguracji.
---

Pakiet rozwiązania to archiwum `.tar.zst`, które konfiguruje obszar roboczy pod konkretny przypadek użycia, np. katalog e-commerce. Może zawierać Schematy, skróty nawigacji eksploracji, domyślny układ rozszerzeń, logotypy i ilustracje, wskazówki konfiguracyjne, kontrole i opcjonalne dane przykładowe.

Zastosowanie pakietu to jednorazowy krok konfiguracji. Później wszystko, co utworzył, jest zwykłymi danymi obszaru roboczego, które administratorzy edytują jak zwykle. Pakiet nie jest właścicielem tych zasobów, nie synchronizuje ich i nie usuwa ich później.

## Co pakiet może, a czego nie może

Pakiet może:

- tworzyć nowe Schematy (jako szkice lub opublikowane) albo ponownie używać pasujących opublikowanych Schematów, które wskażesz;
- dodawać wpisy do nawigacji eksploracji i do układu rozszerzeń obszaru roboczego;
- tworzyć logotypy, ikony i ilustracje;
- sprawdzać, czy wymagane rozszerzenia są zainstalowane i skonfigurowane;
- dostarczać README, informacje o wydaniu, listę kontrolną konfiguracji i kontrole informacyjne;
- tworzyć syntetyczne encje przykładowe, jeśli się na to zgodzisz.

Pakiet nie może:

- instalować, konfigurować, przyznawać uprawnień ani włączać rozszerzenia;
- tworzyć ani zmieniać kontekstów ani kanałów eksportu;
- aktualizować istniejącego Schematu ani nadpisywać czegokolwiek, co już jest w obszarze roboczym;
- zmieniać członków, ról ani uprawnień;
- uruchamiać skryptów, SQL ani niczego innego wykonywalnego;
- zawierać sekretów.

Nie ma odinstalowania. Aby wycofać pakiet, usuń utworzone przez niego zasoby jeden po drugim.

## Zastosuj pakiet

Administrowanie pakietami odbywa się przez CLI i wymaga `solution_packs.manage`, które role właściciela i administratora mają domyślnie. Archiwum otrzymujesz od jego wydawcy; Attricat nigdy sam nie pobiera pakietów.

### 1. Sprawdź

```sh
acli solution-pack inspect --file ecommerce-1.2.0.tar.zst
```

Serwer waliduje archiwum i zwraca jego identyfikator, wersję, skrót oraz podsumowanie zawartości. Nic nie jest zapisywane.

### 2. Zaplanuj

```sh
acli solution-pack plan --file ecommerce-1.2.0.tar.zst \
  --prefix ecom --blueprint-publication publish
```

Plan to przebieg próbny zapisany na serwerze. Wymienia każdą akcję, którą wykonałby pakiet, i informuje, czy jest to możliwe.

- `--prefix` jest dodawany przed kodami nowych Schematów, więc `product` z pakietu staje się `ecom_product`. Musi mieć od 1 do 32 znaków: małe litery, cyfry i podkreślenia, zaczynać się literą i nie kończyć podkreśleniem.
- `--blueprint-publication` ma wartość `draft` lub `publish`. Wybierz `draft`, aby przejrzeć Schematy, zanim ktokolwiek będzie mógł tworzyć encje.
- `--include-sample-data` dodaje syntetyczne encje przykładowe z pakietu. Pomiń tę opcję, chyba że ich potrzebujesz.

Każda akcja w planie to jedna z:

| Akcja | Znaczenie |
| --- | --- |
| `create` | Utworzenie nowego Schematu lub zasobu. |
| `map` | Ponowne użycie wybranego istniejącego Schematu lub zasobu. |
| `append` | Dodanie wpisu nawigacji lub układu rozszerzeń. |
| `satisfied` | Wpis już istnieje w dokładnie tej postaci; nic do zrobienia. |
| `skip` | Opcjonalnego elementu nie da się zastosować i zostanie pominięty. |
| `conflict` | Coś w obszarze roboczym stoi na przeszkodzie, np. Schemat o tym samym kodzie. |
| `blocked` | Brakuje wymaganej zależności, np. wymaganego rozszerzenia. |

Planu z konfliktami lub zablokowanymi akcjami nie można zastosować. Plany wygasają po 24 godzinach.

### 3. Rozwiąż konflikty

Jeśli kod Schematu jest zajęty, wybierz inny `--prefix` albo każ planerowi ponownie użyć istniejącego opublikowanego Schematu, którego definicja jest dokładnie taka sama:

```sh
acli solution-pack plan --file ecommerce-1.2.0.tar.zst --prefix ecom \
  --blueprint-publication publish \
  --map blueprints/product=shared_product \
  --map-asset assets/brand-logo=<existing-asset-uuid>
```

Jeśli brakuje wymaganego rozszerzenia, zainstaluj je i skonfiguruj w zwykły [sposób dla rozszerzeń](/pl/builders/extensions/), a następnie zaplanuj ponownie.

### 4. Zastosuj

```sh
acli solution-pack apply <plan-id>
```

Zastosowanie przyjmuje tylko identyfikator planu; między planowaniem a zastosowaniem nic nie może się zmienić. Przed każdym krokiem Attricat sprawdza, czy obszar roboczy nadal odpowiada planowi. Jeśli coś się zmieniło, zastosowanie zatrzymuje się jako nieaktualne i trzeba zaplanować ponownie. Przerwane zastosowanie można uruchomić ponownie; będzie kontynuowane od miejsca, w którym się zatrzymało.

### 5. Przejrzyj

```sh
acli solution-pack applications list
acli solution-pack applications show <application-id>
acli solution-pack checks list <application-id>
acli solution-pack checks rerun <application-id>
```

Rekord zastosowania przechowuje na potrzeby audytu informację o tym, co zostało utworzone lub ponownie użyte. Kontrole raportują stan konfiguracji, np. „Schemat produktu jest opublikowany” albo „rozszerzenie X jest włączone”. Mają charakter informacyjny: niespełniona kontrola nigdy niczego nie blokuje ani nie cofa.

## Uaktualnij do nowszego wydania pakietu

Aby zastosować nowsze wydanie pakietu, który był już zastosowany, wskaż wcześniejsze zastosowanie:

```sh
acli solution-pack plan --file ecommerce-1.3.0.tar.zst --prefix ecom \
  --blueprint-publication publish --from-application <application-id>
```

Niezmienione Schematy są używane ponownie. Nowe Schematy są tworzone. Schematy zmienione w nowym wydaniu są blokowane z `update_not_supported`, ponieważ pakiet nigdy nie aktualizuje istniejącego Schematu. Zaktualizuj je samodzielnie, tworząc nową wersję. Schematy usunięte w nowym wydaniu są raportowane i pozostawiane bez zmian.

## Dane przykładowe

Gdy planujesz z `--include-sample-data`, syntetyczne encje z pakietu są tworzone w kontekście domyślnym i oznaczane jako przykładowe, aby łatwo było je znaleźć i usunąć. Ich utworzenie uruchamia ten sam audyt i tę samą automatyzację co każda inna nowa encja, w tym przepływy pracy nasłuchujące `entity.created.v1`.

## Zbuduj pakiet

Pakiet to archiwum `.tar.zst` z plikiem `solution-pack.json` w katalogu głównym. Manifest wymienia każdy plik wraz z jego skrótem SHA-256:

```json
{
  "manifest_version": 1,
  "id": "acme.ecommerce",
  "name": "Ecommerce Catalog",
  "version": "1.2.0",
  "description": "Product and category blueprints.",
  "catalog": { "host_api": ">=1.0.0 <2.0.0" },
  "documentation": {
    "readme": { "path": "README.md", "sha256": "…" },
    "setup_checklist": { "path": "setup/checklist.json", "sha256": "…" }
  },
  "checks": { "path": "checks/checks.json", "sha256": "…" },
  "resources": {
    "blueprints": [
      { "key": "blueprints/product", "path": "blueprints/product.toml", "required": true, "sha256": "…" }
    ],
    "workspace_settings": [
      { "key": "workspace/explore-navigation", "path": "workspace/explore-navigation.json", "required": false, "sha256": "…" }
    ],
    "presentation_assets": [
      { "key": "assets/brand-logo", "path": "assets/brand-logo.svg", "required": true,
        "purpose": "logo", "media_type": "image/svg+xml", "sha256": "…" }
    ]
  },
  "extensions": [
    { "key": "extensions/shopify", "id": "acme.shopify", "version": ">=2.1.0 <3.0.0", "required": false }
  ]
}
```

Schematy JSON dla manifestu, kontroli, listy kontrolnej konfiguracji, nawigacji eksploracji, układu rozszerzeń i plików danych przykładowych są opublikowane w repozytorium jako `contracts/solution-pack-*-v1.schema.json`. Nieznane pola są odrzucane.

Wskazówki dla autorów pakietów:

- **Używaj kluczy logicznych, a nie kodów ani UUID.** Schematy pakietu odwołują się do siebie po kluczu; planer zamienia klucze na rzeczywiste kody z użyciem prefiksu administratora.
- **Klucze są trwałe.** Zmiana nazwy klucza w późniejszym wydaniu wygląda jak usunięcie jednego zasobu i dodanie innego.
- **Zachowaj deklaratywność pakietów.** Schematy pakietu nie mogą używać zasad ról `[publication]` ani rendererów komórek z rozszerzeń.
- **Bez sekretów.** Szablony konfiguracji rozszerzeń są publiczne. Klucze o nazwach w rodzaju `password`, `secret`, `token`, `api_key`, `private_key`, `credential` lub `authorization` są odrzucane.
- **Zasoby graficzne**: PNG, WebP i SVG do dowolnego celu; JPEG tylko do ilustracji. Do 2 MiB każdy, łącznie 16 MiB, maksymalnie 4096×4096 pikseli. SVG jest ograniczony do 256 KiB i bezpiecznego podzbioru bez skryptów, stylów, czcionek, animacji i odwołań zewnętrznych.
- **Dokumentacja**: README do 64 KiB, informacje o wydaniu do 32 KiB. Markdown jest wyświetlany z wyłączonym HTML; dozwolone są tylko linki w obrębie tego samego dokumentu.
- **Kontrole** używają jednego z: `blueprint_published`, `extension_installed`, `extension_enabled`, `extension_configuration_matches`, `explore_navigation_entry_present` lub `workspace_extension_layout_placement_present`.

```json
{
  "format_version": 1,
  "checks": [{
    "key": "checks/product-published",
    "title": "Product is published",
    "predicate": { "type": "blueprint_published", "blueprint": "blueprints/product" }
  }]
}
```

Plik nawigacji eksploracji przypina Schematy i opcjonalnie ogranicza ich widoczność do kodów ról:

```json
{
  "format_version": 1,
  "kind": "explore_navigation",
  "entries": [{ "blueprint": "blueprints/product", "visible_to_role_codes": ["editor"] }]
}
```
