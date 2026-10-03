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
- wyszukiwać encje i odczytywać zapisane wyszukiwania;
- wyświetlać podgląd migracji encji;
- odczytywać stan danych, ustalenia reguł i uruchomienia przepływów pracy;
- oglądać obrazy i odczytywać pliki tekstowe w obszarze roboczym;
- odczytywać uruchomienia operacji rozszerzeń i zadania konektorów (z uprawnieniem `extensions.manage`);
- wyjaśniać przejścia statusów encji, jej zatwierdzenia i blokady retencji.

**Po Twoim zatwierdzeniu** może:

- tworzyć schematy i ich wersje oraz publikować schematy;
- tworzyć, aktualizować, migrować i usuwać encje; ustawiać, usuwać i przywracać wartości; zmieniać relacje; łączyć pliki;
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

## Rekordy kontrolowane

Schematy mogą ograniczać, kto wykonuje przejście statusu, blokować sfinalizowane rekordy i wiązać zatwierdzenia z przejrzaną treścią. Agent przestrzega tych samych zasad co Ty:

- Zmiana odrzucona przez te zasady kończy się błędem z jasnym powodem, np. zablokowanym rekordem lub przejściem, którego nie możesz wykonać. Agent wyjaśnia go zamiast ponawiać próbę i może pokazać, które przejścia możesz wykonać, kto musi działać i które przejście korygujące odblokowuje rekord.
- Przy przejściu, które musi wykonać inna osoba niż autor wcześniejszego przejścia, za wykonującego uznaje się Ciebie, ponieważ zatwierdzona przez Ciebie zmiana działa w Twoim imieniu.
- Jeśli proponowana edycja dotyczy zatwierdzonej treści, to gdy ją zaakceptujesz, zatwierdzenie rekordu zostanie unieważnione, a rekord w tej samej zmianie wróci do wcześniejszego statusu.

## Gdzie widać zmiany

Zmiany wprowadzone przez agenta przechodzą tę samą walidację i audyt co Twoje edycje. Na stronie **Zmiany** encji oraz w **Zarządzanie → Aktywność / dziennik audytu** widać uruchomienie agenta, narzędzie, decyzję o zatwierdzeniu i osobę, która zatwierdziła zmianę.

## Dane wysyłane do dostawcy

Każde żądanie do dostawcy AI zawiera dotychczasową rozmowę i wyniki narzędzi agenta, które mogą zawierać dane katalogu. Wybierz dostawcę, którego warunki przechowywania danych odpowiadają Twojemu katalogowi. Attricat przechowuje rozmowę, ale nie przechowuje surowych odpowiedzi dostawcy ani jego danych uwierzytelniających.

## Uprawnienia

Korzystanie z agentów wymaga uprawnienia `agents.run`, które domyślnie mają role właściciela i administratora. Nadaj je osobom, które powinny móc zlecać zmiany w ten sposób.
