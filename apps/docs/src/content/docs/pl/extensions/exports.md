---
title: Konektory eksportu
description: Synchronizuj katalog z marketplace'em lub sklepem internetowym za pomocą połączeń, mapowań eksportu, wykonań uruchamianych zmianami, odrzuceń, wycofań i ponownych synchronizacji.
---

Konektor eksportu wysyła rekordy katalogu do zewnętrznego miejsca docelowego, np. sklepu Shopify, konta sprzedawcy Amazon lub marketplace'u, i utrzymuje je w synchronizacji. Attricat zapewnia konektorom elementy, które trudno dobrze zrobić samodzielnie:

- **Połączenia** przechowują dane uwierzytelniające miejsca docelowego i przeprowadzają przepływ OAuth. Twój komponent nigdy nie widzi sekretu ani tokenu.
- **Mapowania eksportu** zapamiętują dla każdego rekordu i połączenia zewnętrzny identyfikator i to, co wysłano ostatnio. Ich status to stan eksportu rekordu, który użytkownicy widzą w Attricat.
- **Zadania konektorów** decydują, kiedy eksportować: według harmonogramu, po zmianach rekordów albo gdy operator poprosi o ponowną synchronizację lub ponowienie.
- **Typowane wyniki** pozwalają komponentowi odrzucić jeden rekord albo poczekać na miejsce docelowe bez przerywania całego wykonania.

Wszystko na tej stronie wymaga API hosta 1.1 (`"host_api": "^1.1"`).

## Co zadeklarować

```json
{
  "attricat": { "id": "acme.shopify", "host_api": "^1.1" },
  "permissions": ["attricat.read", "exports.mappings", "connections.use", "network.request"],
  "host_permissions": [
    { "id": "shopify-token", "matches": ["https://*.myshopify.com/admin/oauth/*"], "methods": ["POST"] },
    { "id": "shopify-api", "matches": ["https://*.myshopify.com/admin/api/*"], "methods": ["POST"],
      "rate_limit": { "requests": 2, "per_seconds": 1, "burst": 40 } }
  ],
  "artifacts": [{ "id": "server", "kind": "server_wasm", "path": "dist/server.wasm" }],
  "server": {
    "operations": [{ "id": "export", "handler": "export", "request_schema": { "type": "object" } }],
    "connections": [{
      "id": "shop", "title": "Shopify shop", "grant_types": ["client_credentials"],
      "host_permission": "shopify-token", "api_host_permissions": ["shopify-api"],
      "token_header": { "name": "x-shopify-access-token" },
      "parameters_schema": { "type": "object", "required": ["shop_domain"] }
    }],
    "exports": [{ "id": "shop", "title": "Shopify", "connection": "shop", "resync_operation": "export" }]
  }
}
```

- [Rodzaj połączenia](/pl/extensions/manifest/#połączenia) opisuje jeden typ konta w miejscu docelowym. Operatorzy tworzą nazwane połączenia tego rodzaju w **Zarządzanie → Rozszerzenia**.
- [Miejsce docelowe eksportu](/pl/extensions/manifest/#miejsca-docelowe-eksportu) nadaje nazwę i punkty wejścia ekranom stanu eksportu, podglądu, ponawiania i ponownej synchronizacji.
- Wysyłaniem zajmuje się operacja. Zbuduj komponent dla [świata](/pl/extensions/server/#wersja-api-hosta) `outcome-extension`, aby partie mogły [odrzucać rekordy i się odraczać](/pl/extensions/operations/#odraczanie-i-niepowodzenia-partii-11).

Schemat uruchamia następnie operację jako [zadanie konektora](/pl/extensions/operations/#zadania-konektorów-w-schematach) dla jednego połączenia:

```yaml
connector_jobs:
  - code: shopify
    direction: export
    extension_id: acme.shopify
    operation_id: export
    connection: shop-eu
    trigger:
      events: [record.updated.v1, record.published.v1, record.unpublished.v1]
      debounce_seconds: 30
    input: {}
```

## Pętla wysyłania

Każda partia wykonania eksportu przechodzi te same kroki:

1. **Odczytaj zakres.** `api.call("connector.scope.v1", "{}")` podaje [połączenie, kanał i tryb](#zakres-wykonania) wykonania.
2. **Odczytaj stronę.** `attricat.page-v2` zwraca do 16 wierszy opublikowanych rekordów z wartościami rozwiązanymi w kontekście kanału i `content_revision` każdego rekordu. Zobacz [Odczyt rekordów do eksportu](#odczyt-rekordów-do-eksportu).
3. **Zajmij każdy rekord.** Wywołaj `mappings.begin.v1` z `record_id` i `content_revision` wiersza. Pomiń rekord przy `stale` (nie ma nic nowego do wysłania) lub `busy` (wysyła go ktoś inny). Przy `proceed` zachowaj `lease_token`.
4. **Pomiń niezmienioną treść.** Oblicz skrót ładunku, który chcesz wysłać. Jeśli jest równy `content_hash` mapowania, zatwierdź `synced` bez wywoływania miejsca docelowego. Ma to największe znaczenie przy ponownej synchronizacji.
5. **Wyślij.** Wyślij rekord przez `network.request.v2` z `connection` wykonania.
6. **Zapisz wynik** przez `mappings.commit.v1` z tokenem dzierżawy:
   - powodzenie: `status: "synced"`, `external_id` i `content_hash`;
   - miejsce docelowe odrzuca rekord: zwróć [niepowodzenie](/pl/extensions/server/#typowane-wyniki) `rejected` wskazujące `record-id`. Stan eksportu rekordu zmienia się na `rejected` z Twoim kodem i komunikatem, a partia uruchamia się ponownie i zastaje ten rekord jako rozstrzygnięty;
   - limit żądań lub chwilowa niedostępność: zwróć `retry` z `after-ms` albo zakończ partię z `resume-after-ms`;
   - brak odpowiedzi: zatwierdź `uncertain` i [wyszukaj pozycję](#niepewne-wysyłki), zanim wyślesz ją ponownie.
7. **Wycofaj.** Po ostatniej stronie odczytaj [wycofania](#wycofania), usuń te pozycje z miejsca docelowego i zatwierdź `withdrawn`.
8. **Zapisz punkt kontrolny** z kursorem stron i kursorem wycofań, a gdy oba się wyczerpią, zwróć `done`.

Ponieważ partia może wykonać się dwukrotnie z tym samym kluczem partii, każdy krok musi być bezpieczny do powtórzenia. `begin` i `commit` załatwiają większość z tego za Ciebie: rekord już zatwierdzony w swojej rewizji odpowiada `stale`.

## Zakres wykonania

`api.call("connector.scope.v1", "{}")` zwraca zadanie, Schemat, wersję, kontekst, kanał, połączenie i tryb wykonania albo `null` poza wykonaniem konektora. Tryb to `incremental` dla wykonania z harmonogramu lub ręcznego, `triggered` dla wykonania [uruchomionego zmianami](#eksport-po-zmianach) albo `resync`.

Wykonanie konektora jest ograniczone do swojego zakresu. Odczytuje tylko opublikowane rekordy swojego Schematu, zapisuje tylko mapowania własnego połączenia i wysyła tylko token tego połączenia. Wykonanie domyślnego miejsca docelowego, bez `connection` w zadaniu, nie wysyła żadnego tokenu połączenia. Wykonania eksportu nie mogą zapisywać wartości. Mogą zapisywać [adnotacje](/pl/extensions/operations/#adnotacje-rekordów) rekordów Schematu zadania, aby zewnętrzne identyfikatory lub stan synchronizacji były widoczne dla kontroli i w przeglądarce rekordów.

## Odczyt rekordów do eksportu

`attricat.page-v2({"cursor", "limit", "expand"?})` odczytuje od 1 do 16 wierszy Schematu, kontekstu i kanału wykonania. Każdy wiersz ma `record_id`, `content_revision`, `blueprint_version`, `updated_at` oraz:

- `values`: wartości skalarne rozwiązane przez łańcuch kontekstów zastępczych, więc obejmują też wartości odziedziczone z kontekstów nadrzędnych;
- `relationships`: cele relacji według kodu atrybutu;
- `files`: odwołania do plików (`file_id`, `filename`, `mime_type`, `byte_size`, `sha256`) według kodu atrybutu, do użycia z [`open-record-file`](/pl/extensions/operations/#przesyłanie-plików) i [publicznymi linkami](#publiczne-linki-do-plików);
- `related`: dla maksymalnie czterech kodów relacji wymienionych w `expand` wartości, relacje, pliki i rewizję rekordów docelowych oraz informację, czy każdy z nich jest opublikowany w kanale. Przydaje się przy wariantach i ofertach typu rodzic/dziecko.

Z `expand` pole `content_revision` wiersza to najnowsza rewizja rekordu i osadzonych w nim rekordów. Edycja lub usunięcie wariantu sprawia więc, że wiersz rodzica jest nowszy niż ostatnio zatwierdzona dla niego rewizja, i rodzic zostaje wysłany ponownie.

### Rewizje rekordów

Każdy rekord ma `content_revision`: numer sekwencyjny ostatniego zdarzenia, które zmieniło jego wartości, relacje, publikację lub cykl życia. Ta wartość tylko rośnie. Zmiany adnotacji jej nie zwiększają, więc Twoje własne zapisy pomocnicze nigdy nie sprawiają, że rekord wygląda na zmieniony. Zmiany publikacji liczą się tylko dla odczytywanego kanału: wiersz strony konektora uwzględnia zmiany w kanale wykonania, a odczyt bez kanału nie uwzględnia żadnych, więc publikacja w innym kanale nigdy nie sprawia, że eksport wygląda na nieaktualny. Zawierają ją odczyty typowane, wiersze stron i dostarczenia do `event-handler`. Dostarczenie zdarzenia publikacji lub wycofania publikacji zawiera rewizję bez zmian publikacji, tak jak odczyt bez kanału.

## Mapowania eksportu

Z `exports.mappings` Attricat przechowuje jedno mapowanie na instalację, połączenie i rekord. Mapowanie zawiera zewnętrzny identyfikator, nazwane zewnętrzne identyfikatory, skrót treści i rewizję rekordu z ostatniego wysłania, status, ostatni błąd oraz znaczniki czasu. Mapowania są przygotowane na pełne katalogi i mają indeks na zewnętrznym identyfikatorze. Mapowanie jest usuwane razem ze swoim rekordem lub połączeniem. Rekord usunięty miękko zachowuje mapowanie, aby można było wycofać pozycję, a połączenie utworzone ponownie pod tą samą nazwą odzyskuje swoje zewnętrzne identyfikatory.

Status mapowania to stan eksportu rekordu, widoczny na stronie rekordu, w przeglądarce rekordów i na stronie Stan danych. Statusy to `pending`, `synced`, `rejected`, `retrying`, `uncertain` i `withdrawn`.

Poniższe operacje `api.call` działają tak samo w procedurach obsługi zdarzeń i w wykonaniach operacji. `connection` jest wszędzie opcjonalne; bez niego mapowanie należy do domyślnego miejsca docelowego rozszerzenia.

| Operacja | Żądanie | Wynik |
| --- | --- | --- |
| `mappings.get.v1` | `{connection?, record_id}` lub `{connection?, external_id}` | `{mapping}` albo `{mapping: null}` |
| `mappings.list.v1` | `{connection?, status?, after?, limit?}` (od 1 do 100) | `{mappings, next}` |
| `mappings.begin.v1` | `{connection?, record_id, record_revision, lease_ms?, force?}` | `{outcome: "proceed", lease_token, mapping}`, `{outcome: "stale", mapping}` lub `{outcome: "busy", lease_until}` |
| `mappings.commit.v1` | `{connection?, record_id, lease_token?, status, record_revision?, external_id?, external_ids?, content_hash?, error?}` | `{mapping}` |
| `mappings.release.v1` | `{connection?, record_id, lease_token}` | `{released}` |
| `mappings.delete.v1` | `{connection?, record_id}` | `{deleted}` |

W wykonaniu operacji `begin`, `commit`, `release` i `delete` przyjmują tylko rekordy objęte wykonaniem: rekordy Schematu wykonania konektora (także usunięte lub nieopublikowane, które wycofuje) albo rekordy wybrane w wykonaniu interaktywnym. Wykonanie ograniczone do wybranych rekordów zapisuje tylko ich mapowania.

### Begin i commit

Wywołaj `begin` przed wysłaniem rekordu. Chroni ono przed pracą wykonaną w złej kolejności lub dwukrotnie:

- Odpowiada `stale` dla rewizji starszej niż rewizja mapowania, niezależnie od jego statusu, ponieważ zatwierdzenie zostałoby odrzucone po wysłaniu. Odpowiada też `stale` dla usuniętego rekordu, którego mapowanie przyjmuje tylko zatwierdzenie `withdrawn`.
- Dla rewizji samego mapowania odpowiada `stale`, gdy mapowanie ma już status `synced` lub `rejected`, chyba że operator poprosił o ponowienie albo przekazujesz `force`. Rekord `withdrawn` opublikowany ponownie bez zmian wraca z tą samą rewizją i jest wysyłany ponownie.
- W przeciwnym razie dzierżawi rekord na `lease_ms` (domyślnie dwie minuty, najwyżej 15), aby procedura obsługi zdarzeń i wykonanie konektora nigdy nie wysyłały tego samego rekordu jednocześnie. Odpowiada `busy`, dopóki dzierżawę trzyma ktoś inny. Ponowiona partia lub dostarczenie odzyskuje własną wcześniejszą dzierżawę.

`commit` zapisuje wynik i zwalnia dzierżawę. Bez tokenu dzierżawy kończy się błędem, gdy inny proces trzyma ważną dzierżawę, a wynik `synced` nigdy nie zastępuje nowszej zsynchronizowanej rewizji. Jeśli partia się nie powiedzie, zwolnij dzierżawy, które trzyma; dzierżawy, które host zastanie nierozstrzygnięte, gdy wykonanie staje się martwą wiadomością, przechodzą w stan `retrying`.

Duże dane referencyjne miejsca docelowego, takie jak drzewa kategorii czy definicje typów produktów, przechowuj w [magazynie rozszerzenia](/pl/extensions/server/#magazyn), a nie w mapowaniach.

## Połączenia

Połączenie to konto w miejscu docelowym, które operator konfiguruje dla Twojego rozszerzenia. Rodzaje połączeń deklarujesz w [manifeście](/pl/extensions/manifest/#połączenia). Attricat przechowuje dane uwierzytelniające w postaci zaszyfrowanej, przeprowadza przepływ OAuth i odświeża tokeny.

Z `connections.use`:

- `connections.list.v1` z `{}` zwraca `{"connections": [...]}`.
- `connections.get.v1` z `{"connection": "shop-eu"}` zwraca jedno połączenie.

Każde połączenie opisują pola `connection` (jego nazwa), `kind`, `display_name`, `parameters` (niesekretne ustawienia, np. domena sklepu lub identyfikator sprzedawcy, walidowane względem `parameters_schema` rodzaju) oraz `status` (`pending_authorization`, `active`, `failed` lub `revoked`).

Aby wywołać miejsce docelowe, przekaż `connection` do [`network.request.v2`](/pl/extensions/server/#networkrequestv2) albo [przesyłania v2](/pl/extensions/operations/#przesyłanie-plików). Host dodaje token dostępu, jako `Authorization: Bearer` albo w nagłówku `token_header` rodzaju, tylko do żądań pasujących do jednej z reguł `api_host_permissions` tego rodzaju. Odpowiedź 401 usuwa zapamiętany token, więc następne żądanie otrzyma nowy. Jeśli tokenu nie da się odświeżyć z przyczyny przejściowej, żądanie odpowiada `connection_unavailable`; ponów partię później.

## Wycofania

`attricat.withdrawn(cursor, limit)` zwraca od 1 do 100 rekordów Schematu zadania, które wycofano z publikacji w kanale wykonania (i nadal są niepublikowane), w kolejności zdarzeń. Elementy mają postać `{record_id, reason, record_revision, occurred_at}`, wraz z `next_cursor` i `more`.

- Usunięcie rekordu wycofuje go z każdego kanału, w którym był opublikowany, więc jest zgłaszany z powodem `deleted`. Rekord nigdy nieopublikowany w kanale nie jest zgłaszany.
- Pusty kursor zaczyna od najstarszej zmiany. Zapisz kursor, np. w magazynie rozszerzenia dla każdego zadania i kanału, aby następne wykonanie kontynuowało od miejsca, w którym skończyło poprzednie.
- Wykonanie ograniczone do wybranych rekordów dostaje zamiast tego ostatnie wycofanie każdego z nich, strona po stronie z `more`. Jego kursor zachowuje pozycję kanału, więc kolejne wykonania nadal widzą pozostałe wycofania.
- Gdy Attricat jest zajęty zapisywaniem zdarzeń, strona może wrócić bez rekordów, ale z `more: true` i tym samym kursorem. Wywołaj ją ponownie; to nie jest koniec.

Usuń każdą pozycję z miejsca docelowego i zatwierdź jej mapowanie jako `withdrawn`.

## Niepewne wysyłki

Żądanie, które przekroczyło limit czasu, mogło zostać mimo to wykonane. W `network.request.v2` jest to wynik `uncertain`. Miejsca docelowe przyjmujące nagłówek `Idempotency-Key` ułatwiają sprawę: wyślij stały `idempotency_key`, np. identyfikator i rewizję rekordu, i wyślij żądanie ponownie.

Dla miejsc docelowych bez kluczy idempotencji zatwierdź mapowanie jako `uncertain`. Zanim ponownie wyślesz ten rekord, wyszukaj go w miejscu docelowym, np. po SKU, i zapisz, co znalazłeś. `mappings.get.v1` z `external_id` znajduje rekord dla pozycji widocznej w miejscu docelowym.

Kontrola rozbieżności to operacja, zadeklarowana jako `reconcile_operation` miejsca docelowego, która przegląda miejsce docelowe i porównuje je z `mappings.list.v1`. Może znaleźć pozycje edytowane lub usunięte w miejscu docelowym, oferty nieznane Attricat i mapowania wskazujące brakujące pozycje.

## Publiczne linki do plików

Niektóre miejsca docelowe same pobierają obrazy, czasem kilka godzin po zgłoszeniu oferty. Z `attricat.files.public_links`, `attricat.files.read` i `attricat.read` wywołanie `files.link.v1` wydaje publiczny, trudny do odgadnięcia link do pliku, do którego odwołuje się rekord:

```json
{ "file_id": "…", "record_id": "…", "expires_in_seconds": 86400 }
```

Zwraca `{"url": "…", "expires_at": "…"}`. Link jest ważny od 60 sekund do 7 dni. W wykonaniu operacji można linkować tylko rekordy objęte wykonaniem: wybór wykonania konektora (a w eksporcie do kanału opublikowane rekordy) i aktywne rekordy, do których one linkują, albo rekordy wybrane w wykonaniu interaktywnym. Każdy, kto ma link, może pobrać plik, dopóki link nie wygasł, a rozszerzenie jest włączone i ma wszystkie trzy uprawnienia. Usunięcie rekordu lub zastąpienie pliku natychmiast unieważnia link. Przechowywany jest tylko skrót tokenu. Wdrożenie musi ustawić [`EXTENSION_PUBLIC_FILE_BASE_URL`](/pl/reference/configuration/#rozszerzenia); w przeciwnym razie wywołanie kończy się błędem.

Aby zamiast tego samodzielnie przesłać bajty, skopiuj plik do wykonania przez [`transfer.open-record-file`](/pl/extensions/operations/#przesyłanie-plików) i wyślij go przez `transfer.deliver-output-v2`.

## Eksport po zmianach

Zadanie eksportu może też uruchamiać się, gdy jego rekordy się zmieniają:

```yaml
connector_jobs:
  - code: shopify
    direction: export
    extension_id: acme.shopify
    operation_id: export
    connection: shop-eu
    trigger:
      events: [record.updated.v1, attribute_value.changed.v1]
      debounce_seconds: 30
    input: {}
```

Wykonanie rusza, gdy od najnowszego pasującego zdarzenia rekordów Schematu minie `debounce_seconds` sekund, więc seria edycji daje jedno wykonanie. Ciągłe edycje nie mogą go wstrzymywać w nieskończoność: wykonanie rusza także wtedy, gdy najstarsza oczekująca zmiana ma dziesięciokrotność `debounce_seconds`. Jeśli wykonanie nie może wystartować, np. gdy rozszerzenie jest wyłączone, zmiany czekają i zostaną wyeksportowane, gdy będzie to możliwe. Pasujące zdarzenie rekordu, do którego rekord Schematu odwołuje się przez relację, liczy się jako zmiana rekordu odwołującego się, bo `page-v2` może go osadzić. Wykonanie obejmuje tylko zmienione rekordy, do 10 000; przy większej liczbie zmian wykonuje całe zadanie. Dozwolone zdarzenia wymienia sekcja [Zadania konektorów](/pl/reference/blueprint/#zadania-konektorów).

Ponieważ mapowania eksportu przechowują jeden stan na rekord i połączenie, zadanie eksportu rozszerzenia z `exports.mappings` musi wskazać swój kanał jako `context`, gdy obszar roboczy ma kilka włączonych kanałów.

### Eksport z procedury obsługi zdarzeń

Wykonanie konektora odczytuje strony rekordów i sprawdza się przy katalogach dowolnej wielkości. Dla miejsc docelowych, które oczekują jednego wywołania na zmianę, `event-handler` może sam wysłać rekord: opóźnij go przez [`debounce_ms`](/pl/extensions/server/#łączenie-serii-zdarzeń), wywołaj `begin` z `record-revision` dostarczenia, wyślij i zatwierdź. Obie ścieżki korzystają z tych samych mapowań i dzierżaw, więc zadanie i procedura obsługi nigdy nie wysyłają tego samego rekordu jednocześnie.

## Ponowienia

Operatorzy ponawiają rekordy w sekcji **Stan eksportu** rozszerzenia albo na karcie **Eksporty** rekordu. Ponowienie ustawia mapowania rekordów na `pending`, więc `begin` dzierżawi je ponownie nawet w rozstrzygniętej rewizji, i emituje [`export.retry_requested.v1`](/pl/reference/events/) dla każdego rekordu, wyłącznie do Twojego rozszerzenia:

```json
{ "record_id": "…", "extension_id": "acme.shopify", "connection": "shop-eu" }
```

Zasubskrybuj to zdarzenie procedurą obsługi, aby od razu wysłać rekord, albo pozwól, aby następne wykonanie podjęło oczekujące rekordy. Operator może też poprosić, aby ponowienie uruchomiło ponowną synchronizację tylko tych rekordów w każdym Twoim zadaniu eksportu dla tego połączenia.

## Ponowna synchronizacja

Ponowna synchronizacja (resync) eksportuje jeszcze raz każdy opublikowany rekord zadania eksportu albo tylko część z nich:

```http
POST /api/blueprint-connector-jobs/{id}/resync
{"idempotency_key": "resync-2026-03-01", "status": "rejected"}
```

Przekaż `record_ids` dla wybranych rekordów albo `status` dla rekordów z tym stanem eksportu w połączeniu zadania, ale nie oba naraz. Bez żadnego z nich eksportowany jest każdy opublikowany rekord. Ponowna synchronizacja odczytuje te same wiersze co każde wykonanie, ale `begin` dzierżawi rekordy rozstrzygnięte przed rozpoczęciem wykonania, więc eksporter powinien pominąć rekordy, których zapisany skrót treści się nie zmienił, i wysłać pozostałe. Klucz idempotencji obejmuje wybór: ten sam klucz z innymi `record_ids` lub innym `status` uruchamia nową synchronizację.

## Podgląd i edytory mapowań

- `preview_command` miejsca docelowego wskazuje [polecenie serwerowe](/pl/extensions/server/#polecenia-dla-twojego-interfejsu), które buduje ładunek dla jednego rekordu bez wysyłania go. Otrzymuje `{export_id, record_id, connection}` i zwyczajowo zwraca `{payload, issues}`, gdzie `issues` wymienia, co miejsce docelowe by odrzuciło, np. brakujące atrybuty typu produktu. Karta **Eksporty** rekordu pokazuje go jako **Podgląd**.
- Panel [`export_mapping`](/pl/extensions/client/#inne-miejsca) pojawia się na stronie zarządzania Twoim rozszerzeniem raz dla każdego miejsca docelowego eksportu. Używaj go do edycji mapowań zbyt dużych na konfigurację, np. atrybutów na atrybuty typu produktu marketplace'u. W przeciwieństwie do innych paneli może wywoływać Twoje polecenia serwerowe, więc polecenie waliduje mapowanie i zapisuje je w magazynie rozszerzenia, skąd odczytuje je eksporter. Nieprawidłowe dane zgłaszaj w odpowiedzi polecenia: błąd polecenia dociera do panelu tylko jako ogólne niepowodzenie.

## Testuj eksporter lokalnie

Do testowania eksportera nie potrzebujesz prawdziwego sklepu. Na lokalnym serwerze z `ATTRICAT_DEVTOOLS=true` ustawienia [`EXTENSION_DEV_EGRESS_RESOLVE` i `EXTENSION_DEV_EGRESS_CA_FILE`](/pl/reference/configuration/#extensions) kierują żądania do wybranych hostów na lokalną atrapę.

[`examples/marketplace-export`](https://github.com/attricat/attricat/tree/main/examples/marketplace-export) w repozytorium Attricat zawiera działający testowy eksporter dla Shopify, Amazona i Allegro, atrapę ich API, która potrafi symulować ograniczanie i awarie, oraz skrypt end-to-end. Zacznij od niego, pisząc własny eksporter.

## Uwagi projektowe

- **Kanały i konta.** Zadanie eksportuje jeden kanał publikacji, z wartościami rozwiązanymi w kontekście tego kanału. Konto w miejscu docelowym to połączenie: jeden kanał może zasilać kilka połączeń, każde przez osobne zadanie, a jedno połączenie może obejmować kilka marketplace'ów jednego konta, np. identyfikatory marketplace'ów Amazon jako parametry połączenia.
- **Cena i stan magazynowy.** Wysyłaj to, co zawiera katalog. Ceny i stany, które często się zmieniają albo pochodzą z innego systemu, lepiej synchronizować osobnym zadaniem na przechowujących je atrybutach, niż łączyć z eksportem treści. Walutę bierz z parametru połączenia lub wartości w katalogu, nigdy z wartości domyślnej w kodzie.
