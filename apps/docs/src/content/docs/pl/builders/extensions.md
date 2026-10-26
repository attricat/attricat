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

1. **Przeczytaj żądane uprawnienia.** Każde z nich wskazuje możliwość, np. `attricat.write` (zmiana rekordów), `events.subscribe` (reagowanie na zmiany), `network.request` (wywoływanie usług zewnętrznych) lub `client.record_action` (dodanie przycisku na stronach rekordów). Wszystkie wymienia [dokumentacja manifestu](/pl/extensions/manifest/#uprawnienia).
2. **Sprawdź dostęp do sieci.** `network.request` pozwala wyłącznie na wywołania wzorców URL wymienionych jako uprawnienia hosta. Każdy wzorzec pokazuje swoje hosty, metody, limity rozmiaru i limit czasu oraz to, czy może wysyłać ciągi zapytania.
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

## Połączenia

Rozszerzenia eksportujące do marketplace'ów i sklepów internetowych łączą się z miejscem docelowym przez **połączenia**: po jednym na konto, np. sklep Shopify albo konto sprzedawcy Amazon w jednym regionie. Rozszerzenie może mieć kilka połączeń każdego zadeklarowanego rodzaju. Zarządzasz nimi w sekcji **Połączenia** na stronie rozszerzenia w **Zarządzanie → Rozszerzenia**.

Attricat przechowuje sekrety klienta i tokeny w postaci zaszyfrowanej, sam pobiera i odświeża tokeny dostępu i dodaje je wyłącznie do żądań kierowanych do API miejsca docelowego. Rozszerzenie nigdy ich nie widzi i nikt nie może ich odczytać. Zanim można będzie tworzyć połączenia, wdrożenie musi mieć sekret [`extension_credential_key`](/pl/reference/configuration/#sekrety).

Aby dodać połączenie, wybierz **Dodaj połączenie** i wypełnij:

- **Rodzaj**, spośród rodzajów zadeklarowanych przez rozszerzenie;
- **Nazwa**, krótki identyfikator, którego [zadania konektorów](/pl/reference/blueprint/#zadania-konektorów) używają w `connection = "…"`, oraz **Nazwa wyświetlana**;
- **Identyfikator klienta** i, jeśli miejsce docelowe tego wymaga, **Sekret klienta**;
- **Autoryzacja**, czyli przepływ OAuth używany przez miejsce docelowe:
  - **Dane klienta** wymagają sekretu klienta. Połączenie działa od razu.
  - **Token odświeżania** wymaga tokenu odświeżania uzyskanego z miejsca docelowego.
  - **Kod urządzenia** wymaga adresu URL autoryzacji urządzenia. Po utworzeniu połączenia wybierz **Autoryzuj**, wpisz wyświetlony kod na stronie miejsca docelowego i zatwierdź dostęp, a następnie wybierz **Sprawdź autoryzację**.
  - **Kod autoryzacji**: wpisz **Adres URL autoryzacji** miejsca docelowego i zarejestruj w swojej aplikacji w miejscu docelowym adres URI przekierowania pokazany przy połączeniu. Potem wybierz **Autoryzuj w przeglądarce** i zatwierdź aplikację w miejscu docelowym; wrócisz na stronę rozszerzenia z aktywnym połączeniem. Dokończ ją w ciągu dziesięciu minut, w tej samej przeglądarce, w której masz otwartą sesję Attricat: autoryzacja kończy się tylko dla sesji, która ją rozpoczęła. Jeśli wdrożenie nie ma ustawionego [`EXTENSION_OAUTH_CALLBACK_URL`](/pl/reference/configuration/#rozszerzenia), skopiuj zamiast tego kod z przekierowania miejsca docelowego, wpisz go wraz z adresem URI przekierowania i wybierz **Połącz**.
- **Adres URL tokenu** i **Zakres**, jeśli miejsce docelowe go wymaga. Adresy tokenu i autoryzacji urządzenia muszą pasować do uprawnienia hosta, które rozszerzenie deklaruje dla tego rodzaju.
- **Parametry**: obiekt JSON z niesekretnymi ustawieniami odczytywanymi przez rozszerzenie, np. domeną sklepu lub identyfikatorem marketplace'u.

**Testuj** pobiera token dostępu, aby sprawdzić połączenie. Aby zmienić dane uwierzytelniające, wpisz **Nowy sekret klienta** lub **Nowy token odświeżania** i wybierz **Zastąp**. Połączenie, którego autoryzację miejsce docelowe odrzuci, ma stan **Błąd**, dopóki go nie naprawisz; błędy przejściowe nie zmieniają jego stanu. **Usuń** usuwa połączenie razem z zapisanymi danymi uwierzytelniającymi i stanem eksportu.

## Limity żądań

Uprawnienie hosta może deklarować, ile żądań rozszerzenie może wysłać, np. pięć na sekundę. Sekcja **Limity żądań** pokazuje zadeklarowany limit każdego uprawnienia hosta. **Obniż** ustawia mniejszy limit, np. aby zostawić zapas dla innych systemów korzystających z tego samego konta. Limitu nie można podnieść ponad wartość zadeklarowaną przez rozszerzenie. **Przywróć** wraca do zadeklarowanego limitu. Uprawnienia hosta bez zadeklarowanego limitu dzielą 60 żądań na minutę.

## Stan eksportu

Dla rozszerzenia, które deklaruje miejsca docelowe eksportu, sekcja **Stan eksportu** pokazuje liczbę rekordów według połączenia i stanu. Wybierz stan, aby zobaczyć jego rekordy wraz z błędem z miejsca docelowego, zaznacz rekordy i wybierz **Ponów** albo ponów wszystkie rekordy w tym stanie. Ponowienie oznacza rekordy jako oczekujące i prosi rozszerzenie o ich ponowny eksport. Sekcja pokazuje też dostarczenia zdarzeń, które rozszerzenie odrzuciło lub których zaniechało w ciągu ostatnich 30 dni, oraz edytor mapowań, jeśli rozszerzenie udostępnia go dla danego miejsca docelowego.

Stan każdego rekordu widać też na jego [karcie Eksporty](/pl/guides/records/#eksporty), a strona Stan danych zlicza problemy we wszystkich rozszerzeniach.

## Gdy coś pójdzie nie tak

- **Wyłącz** rozszerzenie, aby je zatrzymać bez utraty konfiguracji i przyznanych uprawnień.
- **Kwarantanna** oznacza instalację jako niebezpieczną. Attricat automatycznie poddaje rozszerzenie kwarantannie, gdy jego kod serwerowy ulegnie awarii albo wyczerpie pamięć lub czas. Starsze rozszerzenia trafiają do kwarantanny także wtedy, gdy zwrócą błąd podczas obsługi zdarzenia; rozszerzenia zbudowane dla API hosta 1.1 zgłaszają zamiast tego odrzucony rekord lub błąd przejściowy, który jest ponawiany albo widoczny w stanie eksportu. Ponowne włączenie jeszcze raz sprawdza wydanie i konfigurację.
- **Usuń** odinstalowuje rozszerzenie. Jego historia pozostaje w dzienniku audytu.
- Przełącznik **trybu rozszerzeń** obszaru roboczego wyłącza naraz wszystkie rozszerzenia w obszarze roboczym, bez zmieniania żadnej instalacji. Włącz go ponownie, aby przywrócić te, które były włączone.

Operatorzy wdrożenia mają dwa dodatkowe mechanizmy: `EXTENSIONS_MODE=disabled` zatrzymuje rozszerzenia w całym wdrożeniu, a `EXTENSION_DENYLIST` blokuje wybrane rozszerzenia lub wydania. Zobacz [dokumentację konfiguracji](/pl/reference/configuration/#rozszerzenia).

Otwarte ramki interfejsu wykrywają te zmiany w ciągu 15 sekund i zamykają się.

## CLI

```sh
acli extension registry list
acli extension registry add --source acme/attricat-extensions
acli extension list
acli extension install --owner acme --repository attricat-inventory --release-id <github-release-id>
acli extension sideload --file inventory-1.2.0.tar.zst
acli extension configure acme.inventory --configuration config.json
acli extension grant acme.inventory --grant-kind capability --grant-id attricat.read
acli extension grant acme.inventory --grant-kind host_permission --grant-id inventory-api
acli extension enable acme.inventory
acli extension disable acme.inventory
acli extension workspace-mode --enabled false
```

## Zbuduj własne

Zobacz [Budowanie rozszerzenia](/pl/extensions/build/).
