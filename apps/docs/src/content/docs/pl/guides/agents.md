---
title: Agenci i zatwierdzenia
description: Poproś agenta AI o sprawdzenie i zmianę katalogu i zatwierdzaj proponowane przez niego zmiany.
---

Agent Attricat to asystent konwersacyjny, który może odczytywać katalog i proponować w nim zmiany. Bez pytania może wyszukiwać rekordy, sprawdzać schematy, przeszukiwać katalog i przeglądać stan danych. Każda zmiana, którą chce wprowadzić, zatrzymuje się i czeka na zatwierdzenie przez człowieka, chyba że pozwolisz mu [zmieniać rekord bez pytania](#automatyczne-zatwierdzanie-zmian-rekordu).

Agenci są opcjonalni. Zanim się pojawią, administrator musi [skonfigurować dostawcę AI](/pl/reference/configuration/#agenci).

W publicznym demo agent działa tylko w trybie odczytu. Może robić wszystko, co opisano w części *Bez zatwierdzenia*, a gdy poprosisz o zmianę, objaśnia, co by zrobił, zamiast ją proponować. Rozmowa pokazuje komunikat **Tylko odczyt**.

## Rozpocznij rozmowę

Otwórz **Agenci** na pasku bocznym, wybierz **Nowa rozmowa** i opisz, czego potrzebujesz. Rozmowa otrzymuje nazwę od Twojej pierwszej wiadomości, a wkrótce potem Attricat może zastąpić ją wygenerowanym tytułem. Aby go zmienić, wybierz **Zmień tytuł rozmowy**.

Rozmowę możesz też rozpocząć:

- przyciskiem **Zapytaj o ten rekord** na stronie rekordu. Takie rozmowy są wyświetlane na karcie **Agenci** rekordu; włącz **Pokaż rozmowy o rekordach**, aby uwzględnić je na liście **Agenci**;
- przyciskiem **Wyślij do rozmowy z agentem** po zaznaczeniu rekordów w **Przeglądarce rekordów**.

Dołącz pliki przyciskiem **Dodaj pliki**, maksymalnie 16 na wiadomość. Obrazy do 5 MiB są wysyłane do dostawcy, aby agent mógł je zobaczyć. Agent odczytuje też tekst plików PDF, Word (`.docx`) i Excel (`.xlsx`): krótki dokument trafia do agenta razem z wiadomością, a dłuższy agent czyta strona po stronie lub arkusz po arkuszu. Pozostałe pliki są opisywane nazwą i typem, a agent może je otworzyć swoimi narzędziami do plików.

Agent czyta tekst, a nie obrazy tekstu. Nie odczyta zeskanowanego pliku PDF bez warstwy tekstowej ani pliku chronionego hasłem i poinformuje o tym. Z arkusza odczytuje wartości ostatnio obliczone przez Excel, a nie formuły. Bardzo duże dokumenty są czytane do limitu rozmiaru.

## Co może agent

Agent działa w Twoim imieniu. Widzi i zmienia tylko to, na co pozwala Twoja rola.

**Bez zatwierdzenia** może:

- wyświetlać i odczytywać schematy, konteksty, rekordy i ich historię;
- wyszukiwać użytkowników i zespoły obszaru roboczego, aby wypełnić atrybuty użytkownika lub zespołu;
- wyszukiwać rekordy i odczytywać zapisane wyszukiwania;
- wyświetlać podgląd migracji rekordu;
- odczytywać stan danych, ustalenia reguł i uruchomienia przepływów pracy;
- oglądać obrazy i odczytywać pliki tekstowe, PDF, Word i Excel w obszarze roboczym;
- odczytywać uruchomienia operacji rozszerzeń i zadania konektorów (z uprawnieniem `extensions.manage`);
- wyjaśniać przejścia statusów encji, jej zatwierdzenia i blokady retencji;
- odczytywać komentarze rekordu;
- odczytywać Twoje [powiadomienia](/pl/guides/inbox/).

**Po Twoim zatwierdzeniu** może:

- tworzyć schematy i ich wersje oraz publikować schematy;
- tworzyć, aktualizować, migrować i usuwać rekordy; ustawiać, usuwać i przywracać wartości; zmieniać relacje; łączyć pliki;
- wprowadzać kilka zmian rekordów razem jako jeden wsad;
- ustawiać te same wartości w wielu rekordach po jednym zatwierdzeniu;
- aktualizować tagi systemowe i metadane;
- publikować rekordy i cofać ich publikację;
- tworzyć, aktualizować i usuwać konteksty;
- duplikować rekordy;
- dodawać komentarze do rekordów i wspominać w nich osoby, co je powiadamia;
- potwierdzać ustalenia reguł (z uprawnieniem `rules.manage`);
- tworzyć i aktualizować zapisane wyszukiwania;
- oznaczać Twoje powiadomienia jako przeczytane lub nieprzeczytane i je usuwać.

Nie może tworzyć ani zmieniać reguł. Nie zarządza też przepływami pracy, rozszerzeniami, członkami ani rolami.

## Zatwierdź lub odrzuć

Gdy agent chce coś zmienić, rozmowa pokazuje **Wymagane zatwierdzenie** wraz z narzędziem, którego chce użyć. Rozwiń **Pokaż proponowane dane wejściowe JSON**, aby zobaczyć dokładnie, co zostanie wysłane.

- **Zatwierdź** uruchamia tę jedną zmianę. W chwili jej wykonania Attricat ponownie sprawdza Twoje uprawnienia, więc zatwierdzenie nigdy nie pozwala agentowi zrobić więcej, niż możesz Ty.
- **Odrzuć** zatrzymuje zmianę. Powiedz agentowi, co ma zrobić inaczej.

Decyzja jest ostateczna. Dwukrotne zatwierdzenie nigdy nie uruchamia zmiany dwa razy.

Czytaj propozycje uważnie. Zastąpienie relacji ustawia pełną listę dla danego atrybutu i kontekstu; pusta lista usuwa każde powiązanie.

### Automatyczne zatwierdzanie zmian rekordu

W rozmowie o rekordzie możesz włączyć **Automatycznie zatwierdzaj zmiany w tej rozmowie**. W wiadomościach, które wysyłasz, agent zmienia wtedy pola, relacje, tagi, pliki i komentarze tego rekordu bez pytania. Usunięcie lub publikacja rekordu oraz zmiany innych rekordów nadal wymagają Twojego zatwierdzenia.

Włączenie wymaga uprawnienia do edycji rekordu. Ustawienie obejmuje wiadomości jednej osoby naraz: jeśli włączył je ktoś inny, włączenie przenosi je na Twoje wiadomości. Każdy, kto może odczytać rozmowę, może je wyłączyć.

### Zmiany w kilku rekordach

Gdy jedna prośba zmienia kilka rekordów, np. wydanie nowej wersji i zastąpienie poprzedniej, agent proponuje jeden **wsad** (`apply_record_batch`). Podsumowanie do zatwierdzenia wymienia wszystkie kroki po kolei. Zatwierdzasz wsad raz i zostaje on zapisany w całości albo wcale: jeśli jeden krok się nie powiedzie, np. dlatego, że rekord w międzyczasie się zmienił, nic nie zostaje zapisane, a agent dowiaduje się, który krok zawiódł.

### Ta sama zmiana w wielu rekordach

Gdy prosisz o tę samą zmianę wartości w wielu rekordach, np. *ustaw sezon Wiosna we wszystkich produktach Acme*, agent proponuje jedną **edycję zbiorczą** (`bulk_set_record_values`). Wybiera rekordy, wymieniając je albo za pomocą filtra wyszukiwania.

W chwili propozycji filtr zamienia się w stałą listę rekordów i to ją zatwierdzasz. Karta zatwierdzenia pokazuje podsumowanie, np. *Set Brand, Season on 412 Product records*, oraz tabelę z nazwami rekordów, obecną i proponowaną wartością: najpierw 20 rekordów, a po kliknięciu **Pokaż wszystkie rekordy** pozostałe. Rekord, który ktoś zapisze po propozycji, nie zostanie nadpisany; jego zmiana nie powiedzie się, bo rekord zmienił się w międzyczasie.

W przeciwieństwie do wsadu edycja zbiorcza nie działa na zasadzie „wszystko albo nic”:

- Po zatwierdzeniu rekordy są zmieniane po kolei, a rozmowa pokazuje postęp.
- Przy każdym rekordzie Attricat sprawdza Twoje uprawnienia, a zmiana trafia do dziennika audytu tak jak Twoja własna edycja.
- Edycja **zatrzymuje się na pierwszym rekordzie, którego nie udało się zmienić**. Wcześniejsze rekordy zachowują zmiany, a kolejne pozostają niezmienione. Attricat dodaje do tego rekordu komentarz podpisany **Attricat** z oznaczeniem **System**, z opisem błędu i linkiem do rozmowy.
- Wynik wymienia zmienione rekordy, rekord, którego nie udało się zmienić, wraz z przyczyną, oraz pominięte rekordy, z linkami do każdego z nich.

Edycja zbiorcza nie zmienia atrybutów statusu, relacji ani plików. Propozycja obejmująca więcej rekordów, niż pozwala obszar roboczy (domyślnie 500), nie trafia do zatwierdzenia; agent zawęża ją albo dzieli.

Wybierz wsad, gdy zmiany muszą się udać albo nie udać razem, a edycję zbiorczą, gdy chcesz wprowadzić tę samą zmianę w wielu rekordach i możesz później poprawić rekord, którego nie udało się zmienić.

## Gdy zmiana zostaje odrzucona

Zmiany wprowadzone przez agenta przechodzą tę samą walidację co Twoje edycje, a serwer odrzuca je z tych samych powodów. Gdy zatwierdzona zmiana zostaje odrzucona, nic nie zostaje zapisane. Agent wyjaśnia przyczynę zamiast ponawiać próbę i, jeśli to ma sens, proponuje poprawioną zmianę, która ponownie wymaga Twojego zatwierdzenia.

- **Kontrole i reguły.** Zmianę odrzuca kontrola (`record_check_failed`), warunek przejścia statusu (`transition_conditions_unmet`), egzekwowana reguła (`rule_violation`) lub kontrole wymagane przez kanał (`publication_checks_failed`), które nie przeszły. Agent wyjaśnia, które kontrole nie przeszły. Aby wyjaśnić, dlaczego opcja statusu jest zablokowana lub rekordu nie można jeszcze opublikować, może odczytać przejścia statusu i gotowość rekordu do publikacji.
- **Klucze unikalne.** Jeśli inny rekord ma już ten sam numer części lub dokumentu, zmiana zostaje odrzucona, a agent dowiaduje się, który rekord go ma. Powinien pokazać Ci ten rekord i zapytać, czy go zaktualizować, czy użyć innej wartości, zamiast ponawiać próbę.
- **Hierarchie.** Powiązanie, które uczyniłoby rekord własnym przodkiem, np. lokalizację wewnątrz niej samej, zostaje odrzucone wraz ze ścieżką pętli.
- **Dozwolone cele.** Relacja może wskazywać tylko wymienione w niej schematy.
- **Ograniczenia publikacji.** Publikacja schematu, który dodaje klucz unikalny lub hierarchię, nie powiedzie się, jeśli istniejące rekordy je naruszają; agent wymienia je, aby można było je najpierw poprawić.
- **Rekordy kontrolowane.** Schematy mogą ograniczać, kto wykonuje przejście statusu, blokować sfinalizowane rekordy i wiązać zatwierdzenia z przejrzaną treścią, a agent przestrzega tych samych zasad co Ty. Zablokowany rekord lub przejście, którego nie możesz wykonać, kończy się odrzuceniem z jasnym powodem, a agent może pokazać, które przejścia możesz wykonać, kto musi działać i które przejście korygujące odblokowuje rekord. Agent nie może sam odblokować rekordu. Gdy żadne przejście nie jest dla Ciebie dostępne, wskazuje właściciela lub administratora, który może użyć **Odblokuj…** na stronie rekordu.
- Przy przejściu, które musi wykonać inna osoba niż autor wcześniejszego przejścia, za wykonującego uznaje się Ciebie, ponieważ zatwierdzona przez Ciebie zmiana działa w Twoim imieniu.
- Jeśli proponowana edycja dotyczy zatwierdzonej treści, to gdy ją zaakceptujesz, zatwierdzenie rekordu zostanie unieważnione, a rekord w tej samej zmianie wróci do wcześniejszego statusu.

## Gdzie widać zmiany

Zmiany wprowadzone przez agenta są audytowane tak samo jak Twoje edycje. Na stronie **Zmiany** rekordu oraz w **Zarządzanie → Aktywność / dziennik audytu** widać uruchomienie agenta, narzędzie, decyzję o zatwierdzeniu i osobę, która zatwierdziła zmianę.

## Dane wysyłane do dostawcy

Każde żądanie do dostawcy AI zawiera dotychczasową rozmowę i wyniki narzędzi agenta, które mogą zawierać dane katalogu. Wybierz dostawcę, którego warunki przechowywania danych odpowiadają Twojemu katalogowi. Attricat przechowuje rozmowę, ale nie przechowuje surowych odpowiedzi dostawcy ani jego danych uwierzytelniających.

## Uprawnienia

Korzystanie z agentów wymaga uprawnienia `agents.run`, które domyślnie mają role właściciela i administratora. Nadaj je osobom, które powinny móc zlecać zmiany w ten sposób.

### Zatwierdzanie edycji zbiorczych

Osoby z uprawnieniem `agents.manage` (domyślnie role właściciela i administratora) wybierają sposób zatwierdzania edycji zbiorczych w **Zarządzanie → Obszar roboczy → Agent**, poleceniem `acli workspace agent set` lub przez `PUT /workspace/agent-settings`:

- **Jedno zatwierdzenie na propozycję** (domyślnie): jedno zatwierdzenie obejmuje wszystkie rekordy z edycji zbiorczej.
- **Zatwierdzanie każdego rekordu**: agent nie może proponować edycji zbiorczych, więc proponuje zmianę każdego rekordu osobno i każda wymaga osobnego zatwierdzenia.

**Rekordy w jednej edycji zbiorczej** ogranicza liczbę rekordów, które może zmienić jedna propozycja: od 1 do 5000 (domyślnie 500).
