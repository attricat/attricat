---
title: Walidacja
description: Ograniczaj pojedyncze wartości i całe encje za pomocą JSON Schema i dowiedz się, jak walidacja współdziała z kontekstami.
---

Attricat waliduje każdy zapis na serwerze, zanim cokolwiek zostanie zapisane. Walidacja pochodzi z następujących źródeł:

1. **Typ atrybutu.** Atrybut `number` odrzuca `"abc"`; `date` odrzuca `2026-13-01`.
2. **`value_schema`** w atrybucie: JSON Schema dla jednej wartości.
3. **`entity_schema`** w schemacie: JSON Schema dla całej encji.
4. **`unique_keys`** w schemacie: identyfikatory biznesowe, których dwie encje nie mogą współdzielić. Zobacz [Klucze unikalne](#klucze-unikalne).
5. **Kontrole** w `x-attricat-checks` schematu encji: porównania atrybutów i kontrole powiązanych rekordów.
6. **Warunki** przejść statusu oraz **egzekwowane reguły**.

Oba schematy walidacji używają JSON Schema Draft 2020-12. Aplikacja internetowa korzysta z tych samych schematów, aby ostrzegać Cię podczas pisania, ale liczy się odpowiedź serwera.

## Ogranicz jedną wartość

Zapisz schemat jako JSON w łańcuchu znaków TOML. Apostrofy pozwalają uniknąć escapowania.

```toml
[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'

[[attributes]]
code = "sku"
value_type = "string"
value_schema = '{"type":"string","pattern":"^[A-Z]{3}-[0-9]{4}$"}'

[[attributes]]
code = "size"
value_type = "string"
value_schema = '{"enum":["XS","S","M","L","XL"]}'
```

`value_schema` działa na atrybutach skalarnych: łańcuchach znaków, liczbach, liczbach całkowitych, wartościach logicznych, datach, datach z godziną i godzinach. Relacje i pliki nie mogą go mieć. Relacje ograniczaj przez `entity_schema`.

Wartość, która nie przejdzie walidacji, zwraca `422 attribute_value_schema_mismatch`.

## Statusy

Status to atrybut `string`, którego `value_schema` ma `enum` ze stałymi kodami oraz adnotację `x-attricat-status`. Adnotacja nadaje każdemu kodowi etykietę, opcjonalny odcień koloru i opcjonalnie dozwolone przejścia między kodami:

```toml
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["draft", "live", "retired"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "draft", "label": "Draft" },
      { "code": "live", "label": "Live", "tone": "success" },
      { "code": "retired", "label": "Retired" }
    ],
    "transitions": [
      { "from": null, "to": "draft" },
      { "from": "draft", "to": "live" },
      { "from": "live", "to": "retired" }
    ]
  }
}'''
```

- Każdy kod z `enum` musi mieć dokładnie jedną opcję, podaną w kolejności wyświetlania. Kody składają się z liter, cyfr, `_` i `-`. Etykiety to tekst, który można [przetłumaczyć](/pl/builders/translations/#etykiety-statusów) odwołaniami do leksykonu `{{…}}`.
- `tone` przyjmuje wartość `default`, `success`, `warning`, `error` lub `info`. Etykieta jest zawsze widoczna, więc kolor nigdy nie jest jedyną informacją.
- Pomiń `transitions`, aby zezwolić na każdą zmianę. Z `transitions` dozwolone są tylko wymienione zmiany; pusta tablica nie zezwala na żadną. `null` oznacza „brak wartości”: przejście z `null` pozwala ustawić pierwszą wartość (także domyślną), a przejście do `null` pozwala ją wyczyścić. Pozostawienie tej samej wartości jest zawsze dozwolone.

Aplikacja internetowa pokazuje status jako etykietę i edytuje go listą wyboru, w której niedozwolone opcje są wyłączone. Przejścia sprawdza serwer dla każdego zapisu, także z API, CLI, przepływów pracy, przywracania historii i migracji. Porównywane są wartości efektywne, więc wartość odziedziczona z kontekstu nadrzędnego jest punktem wyjścia. Niedozwolona zmiana zwraca `422 attribute_value_schema_mismatch`.

### Warunki przejść

Dozwolone przejście może nadal zależeć od danych. Dodaj `conditions` do krawędzi, aby czegoś wymagać przed zmianą, na przykład zapisania przyczyny źródłowej przed zamknięciem niezgodności:

```toml
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["open", "closed"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "open", "label": "Open" },
      { "code": "closed", "label": "Closed", "tone": "success" }
    ],
    "transitions": [
      { "from": null, "to": "open" },
      { "from": "closed", "to": "open" },
      { "from": "open", "to": "closed", "conditions": [
        { "code": "root-cause", "message": "Record the root cause before closing",
          "predicate": { "type": "required", "attribute_code": "root_cause" } },
        { "code": "actions-closed", "message": "Close every corrective action first",
          "predicate": { "type": "referenced_by", "blueprint_code": "corrective_action",
            "relationship_code": "nonconformance", "max": 0,
            "predicate": { "type": "one_of", "attribute_code": "state", "values": ["open"] } } }
      ] }
    ]
  }
}'''
```

- Warunek ma `code`, opcjonalny `message` (od 1 do 500 znaków) i `predicate`. Predykaty są takie same jak w [regułach](/pl/reference/blueprint/#predykaty), z wyjątkiem `stale`, `unique` i `acyclic`. Krawędź może mieć najwyżej 16 warunków.
- Każdą krawędź `from`/`to` można zadeklarować tylko raz.
- Warunki są sprawdzane na stanie, który powstaje po zapisie, więc uzupełnienie `root_cause` i zamknięcie w jednym zapisie działa.
- Są sprawdzane w każdym kontekście, w którym zmienia się status, dla każdego zapisującego.

Jeśli którykolwiek warunek nie jest spełniony, cały zapis zostaje odrzucony z `422 transition_conditions_unmet` i listą wszystkich niespełnionych warunków. Egzekwowane reguły mogą chronić przejścia w ten sam sposób; zobacz [Egzekwowanie reguły](/pl/builders/rules/#egzekwowanie-reguły).

Aby z wyprzedzeniem sprawdzić, które statusy docelowe są dostępne, wywołaj `GET /v1/entities/{id}/status-transitions?context_id=<uuid>` (bez parametru używany jest kontekst domyślny). Odpowiedź zawiera każde zadeklarowane przejście z zapisanego statusu z polem `allowed`, a dla zablokowanych także `denial_code` i `denial_reason`, oraz `unmet`, czyli niespełnione warunki i egzekwowane reguły. Są one oceniane na zapisanej encji tak, jakby status już się zmienił. Podobnie jak zapis, punkt końcowy sprawdza wybrany kontekst i każdy kontekst, który dziedziczy z niego status, a każdy niespełniony warunek wymienia konteksty, w których nie jest spełniony. Blokada przez warunki ma `denial_code` `transition_conditions_unmet`.

### Kontroluj cykl życia rekordu

W dokumentach kontrolowanych, inspekcjach czy ocenach status może też decydować, kto może wykonać przejście, zamrażać rekord, gdy jest już ostateczny, wiązać zatwierdzenie z dokładnie tą treścią, którą przejrzano, i zachowywać wydane pliki przez okres retencji. Pełny przykład zawiera [Krok 10 tworzenia schematu](/pl/builders/blueprints/#krok-10-kontroluj-cykl-życia-rekordu), a wszystkie klucze wymienia sekcja [Statusy](/pl/reference/blueprint/#statusy).

#### Kto może wykonać przejście

Przejście może określać wymagania. Osoba zapisująca zmianę musi spełnić je wszystkie, oprócz posiadania `entities.write`:

- `permission`: [uprawnienie](/pl/reference/permissions/), które musi mieć dla tej encji, np. `entities.publish`.
- `roles`: musi mieć co najmniej jedną z tych ról (wbudowaną lub [niestandardową](/pl/operate/workspaces/#role)), przydzieloną w całym obszarze roboczym, dla schematu lub dla tej encji.
- `separate_from`: rozdzielenie obowiązków. Nie może to być osoba, która jako ostatnia wykonała w tej encji i w tym kontekście przejście o jednym z podanych kodów `code`. Na przykład osoba, która przesłała dokument do przeglądu, nie może go też zatwierdzić.

Nadaj przejściu `code`, aby wskazywać je w `separate_from` i w historii. Odrzucone przejście zwraca `403 status_transition_forbidden` lub `403 status_separation_of_duties` i nic nie zostaje zapisane. Lista wyboru statusu na stronie encji wyłącza przejścia, których nie możesz wykonać, i wyjaśnia dlaczego.

Te same kontrole obowiązują każdego, kto zapisuje dane: API, CLI, przepływy pracy (jako osobę, której zmiana uruchomiła przepływ), rozszerzenia i agentów (jako osobę, która zatwierdziła zmianę). Zapis bez możliwej do ustalenia osoby, np. z zaplanowanego zadania, nie może wykonać ograniczonego przejścia.

Wymagania zawsze dotyczą osoby zapisującej zmianę. Nie mogą odwoływać się do [atrybutu użytkownika lub zespołu](/pl/builders/modeling/#przypisz-odpowiedzialność) w rekordzie, więc nie da się zadeklarować zasady „zamknąć może tylko osoba przypisana”.

#### Blokuj sfinalizowane rekordy

`lock` w statusie sprawia, że treść jest tylko do odczytu, dopóki rekord ma ten status:

- `"lock": "all"` zamraża wszystkie atrybuty, relacje i pliki oprócz samego statusu.
- `"lock": ["title", "procedure"]` zamraża tylko te atrybuty.

Rekordu nie można usunąć, dopóki którykolwiek z jego kontekstów ma status z blokadą, niezależnie od jej postaci.

Blokady są egzekwowane na serwerze dla każdej drogi zapisu: strony encji, API, CLI, przepływów pracy, rozszerzeń, agentów, przywracania z historii wartości, przesyłania plików i zmiany ich kolejności oraz migracji. Odrzucony zapis zwraca `409 record_locked`. Strona encji pokazuje zablokowane pola jako tylko do odczytu wraz z powodem.

Status, który deklaruje blokadę, wymaga jawnej listy `transitions`, więc wyjście z niego jest zawsze nazwanym, ograniczonym przejściem. Aby poprawić wydany rekord, najpierw wykonaj przejście korygujące, a dopiero potem edytuj. Korekta może zmienić wyłącznie status. Odblokowanie jest zapisywane w dzienniku audytu jako `entity.record.unlock`.

Blokady działają w obrębie kontekstu: rekord wydany na jednym rynku nadal można edytować na innym, gdzie jest szkicem, o ile zmiana nie trafi do zablokowanego rynku przez dziedziczenie.

#### Wiąż zatwierdzenia z przejrzaną treścią

`approval` w statusie rejestruje zatwierdzenie za każdym razem, gdy rekord przechodzi do tego statusu: kto zatwierdził, kiedy, oraz skrót SHA-256 objętej treści. `covers` przyjmuje `"all"` lub listę atrybutów; objęte relacje i pliki wchodzą do skrótu, pliki według ich dokładnej zawartości bajtowej.

Gdy objęta treść się później zmieni, zatwierdzenie zostaje unieważnione w tym samym zapisie, a jeśli rekord wciąż ma status zatwierdzony, przechodzi do `void_to`. Na przykład edycja tytułu zatwierdzonego dokumentu może odesłać go z powrotem do przeglądu. Zmiany atrybutów, których zatwierdzenie nie obejmuje, nie naruszają zatwierdzenia.

Zatwierdzenia i unieważnienia pojawiają się w dzienniku audytu (`entity.approval.record`, `entity.approval.void`) oraz w panelu **Kontrola rekordu** na stronie encji.

#### Zachowuj wydane pliki

`retention_days` w zablokowanym statusie zakłada blokadę retencji na każdy plik, do którego odwołują się zablokowane atrybuty, gdy rekord przechodzi do tego statusu. Zablokowane pliki nigdy nie są usuwane z magazynu przed wygaśnięciem blokady, nawet jeśli późniejsza korekta je odłączy. Blokady są wyświetlane na stronie encji i w dzienniku audytu. Zobacz [Blokady retencji](/pl/operate/workspaces/#blokady-retencji).
## Ogranicz całą encję

`entity_schema` widzi encję jako jeden obiekt JSON. Używaj go dla reguł obejmujących więcej niż jeden atrybut:

```toml
entity_schema = '''
{
  "type": "object",
  "required": ["title", "price"],
  "allOf": [
    {
      "if": { "properties": { "on_sale": { "const": true } }, "required": ["on_sale"] },
      "then": { "required": ["sale_price"] }
    }
  ],
  "dependentRequired": { "discontinued_on": ["replacement"] }
}
'''
```

Obiekt walidowany przez Attricat wygląda tak:

```json
{
  "title": "Linen shirt",
  "price": 49.0,
  "on_sale": true,
  "sale_price": 39.0,
  "categories": ["e8b7a8d3-c954-4c0f-b658-0f686ba466a3"]
}
```

- Wartości skalarne pojawiają się w postaci JSON.
- Relacje są tablicami UUID encji docelowych. Użyj `minItems` i `maxItems`, aby wymagać co najmniej jednej kategorii lub najwyżej trzech tagów.
- Atrybuty bez wartości są pomijane, więc `required` oznacza „ma wartość”.

Najwyższego poziomu `required`, `properties`, `dependentRequired` i `dependentSchemas` mogą wskazywać tylko atrybuty, które ma schemat, w tym atrybuty wybrane z domieszek. Literówka w tych miejscach powoduje błąd kompilacji.

`entity_schema` jest dozwolony tylko w schematach encji. Encja, która nie przejdzie walidacji, zwraca `422 entity_schema_mismatch`.

## Porównuj atrybuty za pomocą kontroli

JSON Schema nie wyrazi warunku „data ważności nie może być wcześniejsza niż data rozpoczęcia”. Zamiast tego dodaj do schematu encji nazwane kontrole w `x-attricat-checks`:

```toml
entity_schema = '''
{
  "type": "object",
  "required": ["title"],
  "x-attricat-checks": [
    { "code": "valid-range", "message": "Valid until must not be before valid from",
      "predicate": { "type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from" } },
    { "code": "tolerance", "message": "The lower limit must not exceed the upper limit",
      "predicate": { "type": "compare", "attribute_code": "lower_limit", "op": "lte", "other_attribute_code": "upper_limit" } },
    { "code": "not-self-superseding",
      "predicate": { "type": "compare", "attribute_code": "supersedes", "op": "disjoint", "other_attribute_code": "superseded_by" } }
  ]
}
'''
```

Każda kontrola ma:

| Klucz | Opis |
| --- | --- |
| `code` | Unikalny w obrębie listy. Podawany w błędach. |
| `message` | Opcjonalny, od 1 do 500 znaków. Wyświetlany, gdy kontrola nie przejdzie; w przeciwnym razie Attricat generuje komunikat. |
| `predicate` | Dowolny [predykat](/pl/reference/blueprint/#predykaty) z wyjątkiem `stale`, `unique` i `acyclic`, których nie da się sprawdzić podczas zapisu. |

Schemat może mieć najwyżej 32 kontrole. Ich typy są sprawdzane podczas kompilacji schematu: nieznany atrybut, porównanie porządkujące na łańcuchu znaków albo porównanie daty z liczbą kończy się błędem `422 invalid_blueprint_definition`.

Porównanie z brakującą wartością przechodzi, więc powyższa kontrola `valid-range` działa dopiero wtedy, gdy obie daty są ustawione. Jeśli atrybuty muszą być wypełnione, oznacz je jako wymagane albo dodaj predykat `required`.

Kontrole działają po JSON Schema, na każdej ścieżce zapisu: w API, CLI, aplikacji internetowej, przepływach pracy, migracjach, przywracaniu historii i przenoszeniu kontekstów. Tak jak schemat, są sprawdzane w każdym kontekście. Zapis, który ich nie przejdzie, zwraca `422 entity_check_failed` i nic nie zostaje zapisane.

## Sprawdzaj powiązane rekordy

Kontrola może sięgnąć o jeden krok wzdłuż relacji. `linked` sprawdza rekordy, które encja wskazuje, a `referenced_by` liczy rekordy, które wskazują ją.

```toml
entity_schema = '''
{
  "type": "object",
  "x-attricat-checks": [
    { "code": "facility-of-supplier",
      "message": "Every facility must belong to the certificate's supplier",
      "predicate": { "type": "linked", "relationship_code": "facilities",
        "predicate": { "type": "compare", "attribute_code": "supplier", "op": "eq", "subject_attribute_code": "supplier" } } },
    { "code": "approved-facilities",
      "message": "Only approved facilities can be certified",
      "predicate": { "type": "linked", "relationship_code": "facilities",
        "predicate": { "type": "one_of", "attribute_code": "approval_status", "values": ["approved"] } } }
  ]
}
'''
```

- Wewnątrz `linked` i `referenced_by` klucz `attribute_code` wskazuje atrybut drugiego rekordu. `subject_attribute_code` wskazuje atrybut zapisywanej encji.
- `quantifier` w `linked` to `all` (domyślnie; spełniony, gdy nie ma powiązań), `any` lub `none`.
- Powiązane rekordy są rozstrzygane w tym samym kontekście co encja. Atrybut, którego powiązany rekord nie ma, jest traktowany jako brakujący. Usunięte rekordy są pomijane.
- Kontrola nie przechodzi, jeśli relacja ma więcej niż 200 powiązanych rekordów albo encję wskazuje więcej niż 1000 rekordów.

### Zmiany w powiązanych rekordach są zgłaszane, a nie odrzucane

Kontrola działa przy zapisie własnej encji. Edycja powiązanego rekordu nie sprawdza ponownie każdej encji, która go wskazuje, i nie jest odrzucana. Jeśli zakład straci zatwierdzenie, certyfikaty, które go wskazują, nie blokują tej zmiany.

Aby wychwycić takie przypadki, dodaj [regułę](/pl/builders/rules/) z tym samym predykatem i wyzwalaczem `event`. Gdy zmieni się rekord powiązany lub wskazujący, reguły wyzwalane zdarzeniem uruchamiają się ponownie dla maksymalnie 100 zależnych od niego encji i zapisują ustalenia. Samego certyfikatu nie da się ponownie zapisać, dopóki problem nie zostanie usunięty, bo przy następnym zapisie jego kontrola nie przejdzie.

## Egzekwowane reguły

[Reguła jakości danych](/pl/builders/rules/#egzekwowanie-reguły) z tabelą `enforcement` działa jak kontrola, którą można włączać i wyłączać bez publikowania nowej wersji schematu. Odrzuca zapisy naruszające regułę lub chronione przejścia statusu z `422 rule_violation`. Kontroli używaj dla ograniczeń, które są częścią modelu, a egzekwowanych reguł dla zasad, które zespół włącza po [przebiegu próbnym](/pl/builders/rules/#przebieg-próbny-przed-włączeniem).

## Błędy i ich naprawa

| Kod | Status | Znaczenie |
| --- | --- | --- |
| `entity_check_failed` | 422 | Kontrola z `x-attricat-checks` nie przechodzi w którymś kontekście. |
| `transition_conditions_unmet` | 422 | Warunki przejścia statusu nie są spełnione. |
| `rule_violation` | 422 | Po zapisie encja naruszałaby egzekwowaną regułę. |
| `publication_checks_failed` | 422 | Kontrole wymagane przez kanał nie przechodzą. Zobacz [Publikowanie](/pl/guides/publishing/#wymagaj-kontroli-przed-publikacją). |
| `status_transition_forbidden` | 403 | Przejście wymaga uprawnienia lub roli, których nie masz. Zobacz [Kto może wykonać przejście](#kto-może-wykonać-przejście). |
| `status_separation_of_duties` | 403 | To przejście musi wykonać ktoś inny. |
| `record_locked` | 409 | Status rekordu blokuje zmienioną treść albo rekordu nie można usunąć. Najpierw wykonaj przejście korygujące. Zobacz [Blokuj sfinalizowane rekordy](#blokuj-sfinalizowane-rekordy). |

Przy zapisie kontrole, warunki i egzekwowane reguły działają w tej kolejności, po JSON Schema, a zgłaszana jest tylko pierwsza grupa, która nie przeszła. Cztery błędy `422` wymieniają do 50 naruszeń w `error.details.violations`, każde z kodem `code`, komunikatem `message`, kontekstami `contexts`, w których wystąpiło, i atrybutami `attributes`, których dotyczy. Wszystkie pola opisuje [dokumentacja API](/pl/reference/api/#szczegóły-błędu).

W aplikacji internetowej strona encji wymienia kontrole, które nie przeszły, wraz z kontekstami, i pokazuje każdy komunikat przy polach wymienionych w `attributes`. Komunikat przy polu znika, gdy je edytujesz. Klienci API otrzymują te same informacje w `attributes`. Aby usunąć problem, zmień te atrybuty w wymienionych kontekstach albo popraw powiązane lub wskazujące rekordy, o których mówi komunikat. Odrzucona zmiana pozostaje w swoim polu i jest wysyłana ponownie z następnym zapisywanym polem. Przy zmianie statusu sprawdź niespełnione warunki przez [punkt końcowy status-transitions](#warunki-przejść).

## Klucze unikalne

Schemat walidacji sprawdza jedną encję naraz, więc nie powstrzyma dwóch encji przed otrzymaniem tego samego numeru części. Zadeklaruj zamiast tego klucz unikalny:

```toml
[[unique_keys]]
code = "part_number"
attributes = ["part_number"]

[[unique_keys]]
code = "document_revision"
attributes = ["document", "revision_label"]
```

Drugi klucz jest złożony: dokument może mieć tylko jedną wersję `B`, ale każdy dokument może mieć własną. Domyślnie tekst jest porównywany bez względu na wielkość liter i nadmiarowe białe znaki, a encja bez którejś wartości klucza nie jest względem niego sprawdzana. Zapis powodujący konflikt zwraca `409 unique_key_conflict` z encją, która już ma tę wartość.

Zasady porównywania, klucze w poszczególnych kontekstach i to, co się dzieje po dodaniu klucza do schematu, który ma już encje, opisuje sekcja [Klucze unikalne](/pl/reference/blueprint/#klucze-unikalne).

## Walidacja a konteksty

Encja ma w każdym kontekście rozstrzygnięty zestaw wartości, zbudowany z jej własnych wartości i wartości odziedziczonych. Po każdej zmianie Attricat waliduje encję w **każdym** kontekście, nie tylko w edytowanym.

Załóżmy na przykład, że `title` jest wymagany i dziedziczy wartość z kontekstu domyślnego. Wyczyszczenie domyślnego tytułu pozostawiłoby każdy kontekst potomny bez tytułu, więc zapis zostaje odrzucony, mimo że edytowano kontekst domyślny.

Przeniesienie kontekstu pod nowego rodzica jest sprawdzane tak samo: przed zapisaniem przeniesienia każda aktywna encja jest walidowana względem nowego łańcucha dziedziczenia.

## Czego nie waliduje się

- Tagi widoczności i `readonly` są wskazówkami prezentacyjnymi. Nie blokują zapisu wartości przez API.
- `system_tags` i `system_metadata` encji znajdują się poza schematem i nie są sprawdzane.
- Schemat walidacji z nowszej wersji schematu nie dotyczy encji pozostających przy starszej wersji, dopóki nie zostaną zmigrowane.
