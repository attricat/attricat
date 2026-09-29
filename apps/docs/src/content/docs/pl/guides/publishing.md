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

## Co cofa publikację

Domyślnie każda zmiana encji cofa wszystkie jej publikacje w kanałach: zmiany wartości, relacji, plików, metadanych systemowych i aktualizacje schematu.

Zmiana kontekstu cofa publikacje w tym kanale, ponieważ może zmienić wynikowe wartości każdej encji w tym kontekście.

## Zachowaj publikację po zaufanych edycjach

Schemat może wskazać role, których edycje zachowują istniejące publikacje:

```toml
[publication]
retain_on_edit_roles = ["admin", "product_owner"]
```

Role muszą istnieć w chwili publikowania wersji schematu. To ustawienie nie pozwala nikomu edytować ani publikować; nadal potrzebne są odpowiednie uprawnienia. Nie dotyczy zmian kontekstu, które zawsze cofają publikację.

## Eksporty

Zadania eksportu konektora zadeklarowane w schemacie uruchamiają się raz dla każdego włączonego kanału i obejmują tylko encje opublikowane w tym kanale w chwili odczytu każdej strony. Encja, której publikację cofnięto w trakcie eksportu, nie trafia do kolejnych stron. Danych już wysłanych do systemu zewnętrznego nie można wycofać. Zobacz [Zadania konektorów](/pl/reference/blueprint/#zadania-konektorów).
