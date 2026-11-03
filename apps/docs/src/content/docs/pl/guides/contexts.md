---
title: Konteksty
description: Przechowuj wartości różniące się zależnie od rynku, języka, kanału lub lokalizacji i kontroluj sposób ich dziedziczenia.
---

Kontekst to miejsce, w którym wartości rekordu mogą się różnić. Typowe konteksty to rynki, języki, kanały sprzedaży i sklepy. Konteksty tworzą drzewo pod kontekstem głównym o nazwie `default`.

## Jak działa dziedziczenie

Każdy rekord może mieć wartości w dowolnym kontekście. Gdy kontekst nie ma wartości dla atrybutu, Attricat sprawdza jego kontekst nadrzędny, następnie nadrzędny kontekstu nadrzędnego, aż do `default`, i pokazuje pierwszą znalezioną wartość.

```text
default            description = "Linen shirt"
└── PL             description = "Lniana koszula"
    ├── PL-web     (no value → shows "Lniana koszula")
    └── PL-shop    description = "Koszula z lnu, krój regularny"
```

Każdy atrybut może zmienić to zachowanie:

- `context_fallback: none` wyłącza dziedziczenie. Kontekst bez własnej wartości nie pokazuje niczego. Używaj tego dla wartości, które nie mogą przechodzić do kontekstów podrzędnych, na przykład promocji dla jednego kanału.
- `context_editable: default` pozwala zapisywać wartość tylko w `default`. Inne konteksty pokazują ją tylko do odczytu. Używaj tego dla faktów globalnych, takich jak SKU.

Wartości relacji są dziedziczone w ten sam sposób.

## Tworzenie kontekstów

Otwórz **Zarządzanie → Konteksty** i wybierz **Utwórz kontekst**.

- **Kod**: litery, cyfry, łączniki i podkreślenia. Za pomocą kodu ludzie i integracje odwołują się do kontekstu.
- **Kontekst nadrzędny**: wymagany; miejsce kontekstu w drzewie. Wybierz `default`, aby utworzyć kontekst najwyższego poziomu.
- **Metadane**: opcjonalny obiekt JSON opisujący kontekst, na przykład `{"language": "pl"}`.

Za pomocą CLI:

```sh
acli context create --code PL --data '{"language":"pl"}'
acli context create --code PL-web --data '{}' --parent-id <PL-context-id>
```

Kontekst `default` zawsze ma identyfikator `00000000-0000-4000-8000-000000000001`.

## Zmiana drzewa

Możesz przenieść kontekst pod inny kontekst nadrzędny za pomocą `acli context update <context-id> --parent-id <parent-id> --data <metadata-json>` lub [API](/pl/reference/api/); strona **Konteksty** tylko tworzy i usuwa konteksty. Zanim przeniesienie zostanie zapisane, Attricat sprawdza każdy rekord względem jego nowego łańcucha dziedziczenia. Jeśli którykolwiek rekord stałby się nieprawidłowy, na przykład przez utratę wymaganej wartości, przeniesienie zostaje odrzucone.

Nie można usunąć kontekstu, który ma konteksty podrzędne, wartości lub uruchomienia rozszerzeń, ani kontekstu `default`.

## Praca w kontekście

- W **Przeglądarce** selektor **Kontekst** w **Zakresie wyszukiwania** pokazuje, filtruje i sortuje wartości rozstrzygnięte w danym kontekście.
- Na stronie rekordu karty kontekstów w sekcji **Dane** przełączają wartości, które widzisz i edytujesz.
- W panelu **Przeglądaj według** w Przeglądarce rekordów powiązania relacji są rozwiązywane w kontekście wybranym w Przeglądarce.

## Najpierw zaplanuj drzewo

Dziedziczenie podąża za drzewem, więc jego kształt decyduje o tym, ile musisz wpisywać. Umieść wymiar, który współdzieli najwięcej wartości, blisko korzenia. Typowy układ to rynek, a pod nim kanał:

```text
default
├── PL
│   ├── PL-web
│   └── PL-marketplace
└── DE
    └── DE-web
```

Konteksty domyślnie nie są językami. Jeśli Twoje konteksty reprezentują ustawienia regionalne, zapisz to w ich metadanych i uzgodnij tę konwencję w zespole. Zobacz [Modelowanie katalogu](/pl/builders/modeling/#konteksty).

## Konteksty jako kanały publikacji

Każdy kontekst można włączyć jako kanał eksportu w **Zarządzanie → Eksporty**. Rekordy są wtedy publikowane w nim osobno. Zobacz [Publikowanie](/pl/guides/publishing/).

## Konteksty z pakietów rozwiązań

[Pakiet rozwiązania](/pl/builders/solution-packs/#konteksty-i-kanały-publikacji) może podczas stosowania utworzyć potrzebne mu konteksty i kanały publikacji, z kodami zaczynającymi się od prefiksu pakietu. Podczas planowania możesz zamiast tego wskazać istniejący kontekst za pomocą `--map-context`; pakiet używa go wtedy bez zmian i nigdy nie zmienia jego metadanych, kontekstu nadrzędnego ani kanału. Konteksty utworzone przez pakiet są zwykłymi kontekstami, które możesz zmieniać, przenosić i usuwać jak każde inne.

## Uprawnienia

Uprawnienie `contexts.read` jest potrzebne do wyświetlania kontekstów, a `contexts.write` do ich tworzenia, zmieniania lub usuwania oraz do włączania kanałów publikacji. Przyznanie roli można ograniczyć do poddrzewa kontekstów: obowiązuje ono wtedy w tym kontekście i we wszystkim pod nim, ale nie w kontekście nadrzędnym ani w kontekstach równorzędnych. Przeniesienie kontekstu wymaga też `contexts.write` w jego nowym kontekście nadrzędnym.
