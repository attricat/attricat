---
title: Instalowanie rozszerzeń i zarządzanie nimi
description: Znajduj, instaluj, sprawdzaj, przyznawaj uprawnienia, włączaj, uaktualniaj i izoluj rozszerzenia w obszarze roboczym.
---

Rozszerzenia dodają funkcje do Attricat bez zmieniania rdzenia produktu: import i eksport CSV, silnik formuł, panel pokazujący stany magazynowe z innego systemu, niestandardową komórkę tabeli. Rozszerzenie może uruchamiać kod na serwerze w piaskownicy WebAssembly, dodawać interfejs w izolowanych ramkach, reagować na zdarzenia katalogu i wywoływać usługi zewnętrzne przez kontrolowane API sieciowe.

Włączone rozszerzenie jest zaufanym oprogramowaniem obszaru roboczego. Działa w ramach przyznanych mu uprawnień, więc przyznawaj tylko te, których potrzebuje.

Zarządzanie rozszerzeniami wymaga `extensions.manage`. Przeglądanie rejestru wymaga `extensions.read`.

## Skąd pochodzą rozszerzenia

W **Zarządzanie → Rozszerzenia** rozszerzenie można uzyskać z trzech miejsc:

- **Rynek** wyświetla rozszerzenia z zaufanych rejestrów. Każdy obszar roboczy ma oficjalny rejestr Attricat. Administratorzy mogą dodawać kolejne rejestry GitHub. Rejestr to repozytorium GitHub z indeksem `registry.json`; Attricat rozwiązuje tylko repozytoria wymienione w indeksie, któremu ufa, instaluje tylko wersje wymienione w tym indeksie, a przy dodawaniu rejestru sprawdza, czy ma on prawidłowy indeks.
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

Rozszerzenie zainstalowane z rejestru pokazuje **Dostępna aktualizacja**, gdy jego rejestr wymienia nowszą wersję, którą ten serwer może uruchomić. Uaktualnienie rozpoczniesz przyciskiem **Aktualizuj** na liście **Zainstalowane** albo na jego stronie na rynku, albo przyciskiem **Uaktualnij do v…** na stronie rozszerzenia; Attricat najpierw prosi o potwierdzenie. Rozszerzenie zainstalowane z archiwum uaktualnisz przyciskiem **Uaktualnij z archiwum** na jego stronie.

Uaktualnienie przełącza instalację na nowsze wydanie. Przyznane uprawnienia i konfiguracja zostają, jeśli nadal obowiązują, a włączone rozszerzenie pozostaje włączone, gdy nie ma nic nowego do sprawdzenia.

Rozszerzenie zostaje wyłączone do sprawdzenia, gdy nowe wydanie wymaga nieprzyznanego uprawnienia, zmienia regułę uprawnienia hosta, wymaga konfiguracji, której obecne wartości nie spełniają, albo inaczej zależy od innych rozszerzeń. Zmieniona reguła traci przyznane uprawnienie, a niepasująca konfiguracja jest czyszczona.

Uaktualnienie anuluje wykonania pozostające na starym wydaniu, ponieważ nie mogą przejść na nowy kod. Harmonogramy przechodzą na nowe wydanie, chyba że ich operacja, dane wejściowe lub konfiguracja już nie pasują; takie pozostają wstrzymane, dopóki nie utworzysz ich ponownie. Jeśli korzystasz z eksportu wyzwalanego zmianami, po uaktualnieniu uruchom ponowną synchronizację, aby wysłać zmiany z anulowanego wykonania.

Strona rozszerzenia proponuje nowsze wydania z jego rejestru. Opcja **Uaktualnij z archiwum** na tej stronie albo `acli extension upgrade <id> --file` uaktualnia z przesłanego archiwum, z takim samym skutkiem. W obu przypadkach wersja musi być wyższa od zainstalowanej.

Jeśli po uaktualnieniu Attricat manifest rozszerzenia nie daje się odczytać w tej wersji, informuje o tym strona rozszerzenia. To rozszerzenie przestaje działać, a pozostałe działają dalej. Uaktualnij je do nowszego wydania albo usuń je i zainstaluj ponownie. W obu przypadkach trzeba ponownie przyznać jego uprawnienia, a rozszerzenie pozostaje wyłączone, dopóki go nie włączysz.

Trwające operacje pozostają przy wydaniu, na którym się rozpoczęły. Są wstrzymywane, dopóki to dokładnie wydanie nie zostanie ponownie autoryzowane.

## Rozmieść interfejs rozszerzeń

Karty rekordu, akcje rekordu, asystenci formularza i akcje Przeglądarki rekordów pojawiają się tylko w schematach, które je wymieniają, w kolejności, w jakiej je wymieniają. Zobacz [Widoki i układy](/pl/builders/views/#rozszerzenia-na-stronach-rekordów-i-w-przeglądarce-rekordów).

W miejscach wspólnych dla wszystkich schematów, takich jak nawigacja i panele, rozszerzenia pojawiają się w stałej kolejności według identyfikatora rozszerzenia. **Zarządzanie → Rozszerzenia → Układ rozszerzeń** pozwala zmienić tę kolejność, ukryć elementy i przenieść strony rozszerzeń do głównej nawigacji.

## Sekrety

Rozszerzenia wywołujące usługi zewnętrzne często potrzebują klucza API albo klucza do podpisów. Każdy sekret należy do jednego rozszerzenia: otwórz rozszerzenie w **Zarządzanie → Rozszerzenia** i jego kartę **Sekrety**, widoczną dla rozszerzeń z uprawnieniem `secrets.read`. Żadne inne rozszerzenie nie odczyta ustawionego tam sekretu, nawet pod tą samą nazwą.

Karta pokazuje sekrety deklarowane przez rozszerzenie wraz z ich opisami i oznacza wymagane, które nie są jeszcze ustawione; ostrzega o nich także przegląd rozszerzenia. **Ustaw wartość** zapisuje sekret. **Dodaj sekret** zapisuje sekret pod nazwą, której rozszerzenie nie deklaruje, na przykład taką, o którą jego dokumentacja prosi dla każdego punktu końcowego webhooka. Nazwy składają się z liter, cyfr, kropek, łączników i podkreśleń i mają maksymalnie 128 znaków.

Wartości są tylko do zapisu: nikt nie może ich odczytać, a rozszerzenie otrzymuje wartość tylko w trakcie działania. Attricat przechowuje je zaszyfrowane kluczem [`extension_credential_key`](/pl/reference/configuration/#sekrety) wdrożenia, więc bez niego nie można ustawiać sekretów. Lista pokazuje, kiedy ostatnio ustawiono każdą wartość. **Zastąp wartość** zmienia sekret bez zmiany nazwy, a po usunięciu sekretu rozszerzenie nie zadziała tam, gdzie go potrzebuje, dopóki nie ustawisz go ponownie. Usunięcie rozszerzenia usuwa jego sekrety.

Automatyzacja może zrobić to samo przez API, z tokenem uprawnionym do zarządzania rozszerzeniami:

```http
PUT /extensions/acme.shop/secrets/destination-token
{"value": "…"}
```

`GET /extensions/acme.shop/secrets` zwraca deklarowane i ustawione sekrety, wyłącznie z nazwami, a `DELETE` usuwa sekret.

[CLI](/pl/reference/cli/#rozszerzenia) ma te same polecenia. `set` odczytuje wartość ze standardowego wejścia, więc nie trafia ona do historii powłoki:

```sh
acli extension secret list acme.shop
printf '%s' "$VALUE" | acli extension secret set acme.shop destination-token --value-stdin
acli extension secret delete acme.shop destination-token
```

## Połączenia

Rozszerzenia eksportujące do marketplace'ów i sklepów internetowych łączą się z miejscem docelowym przez **połączenia**: po jednym na konto, np. sklep Shopify albo konto sprzedawcy Amazon w jednym regionie. Rozszerzenie może mieć kilka połączeń każdego zadeklarowanego rodzaju. Zarządzasz nimi w sekcji **Połączenia** na stronie rozszerzenia w **Zarządzanie → Rozszerzenia**.

Attricat przechowuje sekrety klienta i tokeny w postaci zaszyfrowanej, sam pobiera i odświeża tokeny dostępu i dodaje je wyłącznie do żądań kierowanych do API miejsca docelowego. Rozszerzenie nigdy ich nie widzi i nikt nie może ich odczytać. Zanim można będzie tworzyć połączenia, wdrożenie musi mieć sekret [`extension_credential_key`](/pl/reference/configuration/#sekrety).

Aby dodać połączenie, wybierz **Dodaj połączenie** i wypełnij:

- **Rodzaj**, spośród rodzajów zadeklarowanych przez rozszerzenie;
- **Nazwa**, krótki identyfikator, którego [zadania konektorów](/pl/reference/blueprint/#zadania-konektorów) używają w `connection: …`, oraz **Nazwa wyświetlana**;
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

## Webhooki

Niektóre rozszerzenia odbierają webhooki: żądania, które system zewnętrzny, np. marketplace, wysyła, gdy coś się u niego wydarzy. Dla rozszerzenia, które deklaruje webhooki, sekcja **Webhooki** na jego stronie pokazuje punkty końcowe wraz z ich ostatnim żądaniem i odpowiedzią.

Wybierz **Dodaj punkt końcowy**, wskaż webhook i wpisz nazwę, np. konta nadawcy (litery, cyfry, `.`, `_` lub `-`). Attricat pokazuje adres URL punktu końcowego tylko raz: skopiuj go do ustawień webhooków systemu wysyłającego, zanim zamkniesz okno, bo przechowywany jest tylko jego skrót. Jeśli wdrożenie nie ma ustawionego [`EXTENSION_WEBHOOK_BASE_URL`](/pl/reference/configuration/#rozszerzenia), dostajesz zamiast niego ścieżkę; poprzedź ją publicznym adresem swojego wdrożenia Attricat. Daj każdemu systemowi wysyłającemu osobny punkt końcowy, aby móc zmienić lub usunąć jego adres URL bez wpływu na pozostałe. Rozszerzenie może mieć do 64 punktów końcowych.

Attricat nie sprawdza, kto wysyła żądanie: robi to rozszerzenie, zwykle weryfikując podpis sekretem. Jego dokumentacja określa, jaki [sekret](#sekrety) zapisać i co skonfigurować u nadawcy.

- **Zmień adres URL** wydaje nowy adres URL. Stary natychmiast przestaje działać, więc zaktualizuj nadawcę.
- **Usuń** usuwa punkt końcowy i jego adres URL. Żądania, które już odebrał, są nadal przetwarzane.

Rozszerzenie musi być włączone i mieć przyznane `webhooks.receive`; w przeciwnym razie nadawcy dostają `503` i ponawiają później, o ile w ogóle ponawiają. Rozszerzenie przetwarza odebrane dane później, jako zdarzenia: żądanie, którego nie udało mu się przetworzyć, pojawia się wśród nieudanych dostarczeń jego zdarzeń. Ładunki są przechowywane przez 7 dni.

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
acli extension upgrade acme.inventory --file inventory-1.3.0.tar.zst
acli extension configure acme.inventory --configuration config.json
acli extension grant acme.inventory --grant-kind capability --grant-id attricat.read
acli extension grant acme.inventory --grant-kind host_permission --grant-id inventory-api
acli extension enable acme.inventory
acli extension disable acme.inventory
acli extension webhook create acme.orders --webhook orders --endpoint shop-eu
acli extension workspace-mode --enabled false
```

## Zbuduj własne

Zobacz [Budowanie rozszerzenia](/pl/extensions/build/).
