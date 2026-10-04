---
title: Publikowanie
description: Zatwierdzaj encje do eksportu w poszczególnych kanałach i dowiedz się, kiedy zatwierdzenie zostaje cofnięte.
---

Publikacja zapisuje, że ktoś zatwierdził encję dla kanału. System zewnętrzny, taki jak eksport przez konektor lub integracja ze sklepem, może wtedy wysyłać tylko zatwierdzone encje.

Publikacja zatwierdza encję w jej obecnym stanie; Attricat nie przechowuje kopii. Jeśli encja się zmieni, zatwierdzenie zostaje cofnięte, dopóki ktoś nie opublikuje jej ponownie. Dzięki temu nic nie trafia do kanału, zanim ktoś nie sprawdzi aktualnego stanu encji.

## Włącz kanały

Kanałem może być dowolny kontekst. Otwórz **Zarządzanie → Eksporty** i włącz **Kanał eksportu** dla kontekstów, do których publikujesz, na przykład `PL-web` i `DE-web`.

Włączenie kanału wymaga uprawnienia `contexts.write`.

## Opublikuj encję

Na stronie encji sekcja **Publikacja** pokazuje każdy włączony kanał ze stanem **Opublikowano** lub **Nieopublikowano** oraz informacją, kto i kiedy opublikował encję.

- **Opublikuj** zatwierdza encję dla wybranego kanału.
- **Opublikuj we wszystkich kanałach** zatwierdza ją jednocześnie dla każdego włączonego kanału.
- **Cofnij publikację** wycofuje zatwierdzenie dla kanału.
- **Opublikuj ponownie** zatwierdza encję jeszcze raz, gdy edycja cofnęła zatwierdzenie.

W **Przeglądarce encji** wybierz kanał jako kontekst, aby zobaczyć kolumnę **Publikacja**. Posortuj ją, aby przenieść nieopublikowane encje na górę.

Publikowanie wymaga uprawnienia `entities.publish`. Mają je role właściciela i administratora; edytorzy go nie mają.

Za pomocą CLI:

```sh
acli entity publication list <entity-id>
acli entity publication publish <entity-id> --context-id <channel-context-id>
acli entity publication publish-all <entity-id>
acli blueprint publish-entities-all <blueprint-id> <version>
```

## Wymagaj kontroli przed publikacją

Kanał może odrzucać encje, które nie są gotowe, na przykład portal dostawców, do którego nigdy nie może trafić produkt bez SKU lub z wygasłym certyfikatem. W **Zarządzanie → Eksporty** ustaw **Kontrole publikacji** kanału: **Wymagane reguły** i **Wymagaj poprawnej encji**. Przez API:

```http
PUT /publication-channels/{context_id}
{"enabled": true, "required_rule_codes": ["has-sku", "certificate-valid"], "require_valid_entity": true}
```

- `required_rule_codes` zawiera do 32 kodów [reguł jakości danych](/pl/builders/rules/). Wymieniona reguła dotyczy encji, gdy reguła o tym kodzie jest włączona dla wersji schematu encji i nie jest powiązana z innym kontekstem. Reguły, które nie mają zastosowania, są pomijane. Działa każdy predykat, także `unique` i `stale`. Każdy kod musi wskazywać regułę istniejącą w obszarze roboczym, dlatego literówka kończy się błędem `422 invalid_input`, zamiast po cichu wyłączyć kontrolę.
- `require_valid_entity` ponownie sprawdza schemat encji i jego [kontrole](/pl/builders/validation/#porównuj-atrybuty-za-pomocą-kontroli) w kontekście kanału. Wychwytuje to problemy, które pojawiają się bez edycji, np. kontrolę `relative_date` daty wygaśnięcia.
- Oba pola są opcjonalne. Pominięcie pola zachowuje jego bieżące ustawienie. `GET /publication-channels` je pokazuje.

Kontrole są oceniane na bieżąco, w kontekście kanału, w chwili publikacji. Nie korzystają z zapisanych ustaleń, więc poprawka liczy się od razu.

Jeśli kontrola nie przejdzie, nic nie zostaje opublikowane, a żądanie zwraca `422 publication_checks_failed`. `error.details.context` to kod kanału, a `error.details.violations` wymienia reguły i kontrole, które nie przeszły, w tym samym formacie co [błędy walidacji](/pl/builders/validation/#błędy-i-ich-naprawa). Dotyczy to działań **Opublikuj**, **Opublikuj we wszystkich kanałach** oraz publikowania wszystkich encji schematu. Przy publikacji zbiorczej jedna encja, która nie przejdzie kontroli, powoduje odrzucenie całego żądania, a `evidence.entity_id` każdego naruszenia wskazuje encję.

Sekcja **Publikacja** na stronie encji oznacza etykietą **Niegotowe** kanały, w których encji nie można jeszcze opublikować. Aby sprawdzić gotowość przez API bez publikowania, wywołaj `GET /v1/entities/{id}/publications/readiness`. Zwraca każdy włączony kanał z polami `ready` i `violations`. Popraw wskazane atrybuty lub powiązane rekordy, o których mówią komunikaty, i opublikuj ponownie.

## Co cofa publikację

Domyślnie każda zmiana encji cofa wszystkie jej publikacje w kanałach: zmiany wartości, relacji, plików, metadanych systemowych i aktualizacje schematu.

Zmiana lub usunięcie kontekstu cofa publikacje w tym kanale, ponieważ może zmienić wynikowe wartości każdej encji w tym kontekście.

Usunięcie encji cofa wszystkie jej publikacje.

## Zachowaj publikację po zaufanych edycjach

Schemat może wskazać role, których edycje zachowują istniejące publikacje:

```toml
[publication]
retain_on_edit_roles = ["admin", "product_owner"]
```

Role muszą istnieć w chwili publikowania wersji schematu. To ustawienie nie pozwala nikomu edytować ani publikować; nadal potrzebne są odpowiednie uprawnienia. Nie dotyczy zmian ani usuwania kontekstu, które zawsze cofają publikację.

Zaufana edycja nadal musi przejść [kontrole publikacji](#wymagaj-kontroli-przed-publikacją) każdego kanału. Po edycji Attricat ponownie uruchamia kontrole wszystkich kanałów, w których encja jest opublikowana, i cofa publikacje tam, gdzie kontrole już nie przechodzą. Pozostałe publikacje zostają. Na przykład usunięcie SKU zachowuje publikację w kanale bez kontroli, ale cofa ją w kanale, który wymaga reguły `has-sku`.

## Eksporty

Zadania eksportu konektora zadeklarowane w schemacie uruchamiają się raz dla każdego włączonego kanału i obejmują tylko encje opublikowane w tym kanale w chwili odczytu każdej strony. Encja, której publikację cofnięto w trakcie eksportu, nie trafia do kolejnych stron. Danych już wysłanych do systemu zewnętrznego nie można wycofać. Zobacz [Zadania konektorów](/pl/reference/blueprint/#zadania-konektorów).
