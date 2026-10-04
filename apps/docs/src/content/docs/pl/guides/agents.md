---
title: Agenci i zatwierdzenia
description: Poproś agenta AI o sprawdzenie i zmianę katalogu. Każda zmiana czeka na Twoje zatwierdzenie.
---

Agent Attricat to asystent konwersacyjny, który może odczytywać katalog i proponować w nim zmiany. Bez pytania może wyszukiwać encje, sprawdzać schematy, przeszukiwać katalog i przeglądać stan danych. Każda zmiana, którą chce wprowadzić, zatrzymuje się i czeka na zatwierdzenie przez człowieka.

Agenci są opcjonalni. Zanim się pojawią, administrator musi [skonfigurować dostawcę AI](/pl/reference/configuration/#agenci).

## Rozpocznij rozmowę

Otwórz **Agenci** na pasku bocznym i wybierz **Nowa rozmowa**. Nadaj wątkowi tytuł opisujący zadanie, na przykład *Clean up inactive suppliers*, i opisz, czego potrzebujesz.

Rozmowę możesz też rozpocząć:

- przyciskiem **Zapytaj o tę encję** na stronie encji;
- przyciskiem **Wyślij do rozmowy z agentem** po zaznaczeniu encji w **Przeglądarce encji**.

Dołącz pliki przyciskiem **Dodaj pliki**, maksymalnie 16 na wiadomość. Obrazy do 5 MiB są wysyłane do dostawcy, aby agent mógł je zobaczyć. Pozostałe pliki są opisywane nazwą i typem, a agent może je otworzyć swoimi narzędziami do plików.

## Co może agent

Agent działa w Twoim imieniu. Widzi i zmienia tylko to, na co pozwala Twoja rola.

**Bez zatwierdzenia** może:

- wyświetlać i odczytywać schematy, konteksty, encje i ich historię;
- wyszukiwać użytkowników i zespoły obszaru roboczego, aby wypełnić atrybuty użytkownika lub zespołu;
- wyszukiwać encje i odczytywać zapisane wyszukiwania;
- wyświetlać podgląd migracji encji;
- odczytywać stan danych, ustalenia reguł i uruchomienia przepływów pracy;
- oglądać obrazy i odczytywać pliki tekstowe w obszarze roboczym;
- odczytywać uruchomienia operacji rozszerzeń i zadania konektorów (z uprawnieniem `extensions.manage`);
- wyjaśniać przejścia statusów encji, jej zatwierdzenia i blokady retencji.

**Po Twoim zatwierdzeniu** może:

- tworzyć schematy i ich wersje oraz publikować schematy;
- tworzyć, aktualizować, migrować i usuwać encje; ustawiać, usuwać i przywracać wartości; zmieniać relacje; łączyć pliki;
- wprowadzać kilka zmian encji razem jako jeden wsad;
- aktualizować tagi systemowe i metadane;
- publikować encje i cofać ich publikację;
- tworzyć, aktualizować i usuwać konteksty;
- tworzyć i aktualizować zapisane wyszukiwania.

Nie może zarządzać regułami, przepływami pracy, rozszerzeniami, członkami ani rolami.

## Zatwierdź lub odrzuć

Gdy agent chce coś zmienić, rozmowa pokazuje **Wymagane zatwierdzenie** wraz z narzędziem, którego chce użyć. Rozwiń **Pokaż proponowane dane wejściowe JSON**, aby zobaczyć dokładnie, co zostanie wysłane.

- **Zatwierdź** uruchamia tę jedną zmianę. W chwili jej wykonania Attricat ponownie sprawdza Twoje uprawnienia, więc zatwierdzenie nigdy nie pozwala agentowi zrobić więcej, niż możesz Ty.
- **Odrzuć** zatrzymuje zmianę. Powiedz agentowi, co ma zrobić inaczej.

Decyzja jest ostateczna. Dwukrotne zatwierdzenie nigdy nie uruchamia zmiany dwa razy.

Czytaj propozycje uważnie. Zastąpienie relacji ustawia pełną listę dla danego atrybutu i kontekstu; pusta lista usuwa każde powiązanie.

### Zmiany w kilku encjach

Gdy jedna prośba zmienia kilka encji, np. wydanie nowej wersji i zastąpienie poprzedniej, agent proponuje jeden **wsad** (`apply_entity_batch`). Podsumowanie do zatwierdzenia wymienia wszystkie kroki po kolei. Zatwierdzasz wsad raz i zostaje on zapisany w całości albo wcale: jeśli jeden krok się nie powiedzie, np. dlatego, że encja w międzyczasie się zmieniła, nic nie zostaje zapisane, a agent dowiaduje się, który krok zawiódł.

## Gdy zmiana zostaje odrzucona

Zmiany wprowadzone przez agenta przechodzą tę samą walidację co Twoje edycje, a serwer odrzuca je z tych samych powodów. Gdy zatwierdzona zmiana zostaje odrzucona, nic nie zostaje zapisane. Agent wyjaśnia przyczynę zamiast ponawiać próbę i, jeśli to ma sens, proponuje poprawioną zmianę, która ponownie wymaga Twojego zatwierdzenia.

- **Kontrole i reguły.** Zmianę odrzuca kontrola (`entity_check_failed`), warunek przejścia statusu (`transition_conditions_unmet`), egzekwowana reguła (`rule_violation`) lub kontrole wymagane przez kanał (`publication_checks_failed`), które nie przeszły. Agent wyjaśnia, które kontrole nie przeszły. Aby wyjaśnić, dlaczego opcja statusu jest zablokowana lub encji nie można jeszcze opublikować, może odczytać przejścia statusu i gotowość encji do publikacji.
- **Klucze unikalne.** Jeśli inna encja ma już ten sam numer części lub dokumentu, zmiana zostaje odrzucona, a agent dowiaduje się, która encja go ma. Powinien pokazać Ci tę encję i zapytać, czy ją zaktualizować, czy użyć innej wartości, zamiast ponawiać próbę.
- **Hierarchie.** Powiązanie, które uczyniłoby encję własnym przodkiem, np. lokalizację wewnątrz niej samej, zostaje odrzucone wraz ze ścieżką pętli.
- **Dozwolone cele.** Relacja może wskazywać tylko wymienione w niej schematy.
- **Ograniczenia publikacji.** Publikacja schematu, który dodaje klucz unikalny lub hierarchię, nie powiedzie się, jeśli istniejące encje je naruszają; agent wymienia je, aby można było je najpierw poprawić.
- **Rekordy kontrolowane.** Schematy mogą ograniczać, kto wykonuje przejście statusu, blokować sfinalizowane rekordy i wiązać zatwierdzenia z przejrzaną treścią, a agent przestrzega tych samych zasad co Ty. Zablokowany rekord lub przejście, którego nie możesz wykonać, kończy się odrzuceniem z jasnym powodem, a agent może pokazać, które przejścia możesz wykonać, kto musi działać i które przejście korygujące odblokowuje rekord.
- Przy przejściu, które musi wykonać inna osoba niż autor wcześniejszego przejścia, za wykonującego uznaje się Ciebie, ponieważ zatwierdzona przez Ciebie zmiana działa w Twoim imieniu.
- Jeśli proponowana edycja dotyczy zatwierdzonej treści, to gdy ją zaakceptujesz, zatwierdzenie rekordu zostanie unieważnione, a rekord w tej samej zmianie wróci do wcześniejszego statusu.

## Gdzie widać zmiany

Zmiany wprowadzone przez agenta są audytowane tak samo jak Twoje edycje. Na stronie **Zmiany** encji oraz w **Zarządzanie → Aktywność / dziennik audytu** widać uruchomienie agenta, narzędzie, decyzję o zatwierdzeniu i osobę, która zatwierdziła zmianę.

## Dane wysyłane do dostawcy

Każde żądanie do dostawcy AI zawiera dotychczasową rozmowę i wyniki narzędzi agenta, które mogą zawierać dane katalogu. Wybierz dostawcę, którego warunki przechowywania danych odpowiadają Twojemu katalogowi. Attricat przechowuje rozmowę, ale nie przechowuje surowych odpowiedzi dostawcy ani jego danych uwierzytelniających.

## Uprawnienia

Korzystanie z agentów wymaga uprawnienia `agents.run`, które domyślnie mają role właściciela i administratora. Nadaj je osobom, które powinny móc zlecać zmiany w ten sposób.
