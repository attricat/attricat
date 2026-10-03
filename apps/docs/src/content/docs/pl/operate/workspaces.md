---
title: Administracja obszarem roboczym
description: Zarządzaj członkami, zespołami, rolami, zaproszeniami, nawigacją paska bocznego, tokenami API i dziennikiem audytu.
---

Obszar roboczy to jeden katalog z własnymi członkami, schematami, encjami, kontekstami i rozszerzeniami. Obszary robocze są całkowicie odseparowane: nic nie jest między nimi współdzielone.

Większość zadań administracyjnych wykonasz w **Zarządzanie → Zarządzanie obszarem roboczym**, gdzie jest pięć kart: **Członkowie**, **Zespoły**, **Role**, **Zaproszenia** i **Nawigacja**.

## Logowanie

Każdy obszar roboczy ma **identyfikator logowania**, który wygląda jak nazwa domeny, np. `acme.example` lub `default.local`. Użytkownicy wpisują go na stronie logowania, a potem podają adres e-mail i hasło. Identyfikator służy wyłącznie do kierowania logowania; Attricat nie sprawdza go w DNS.

Sesje trwają osiem godzin. Pięć nieudanych prób logowania dla tego samego obszaru roboczego i adresu e-mail w ciągu piętnastu minut tymczasowo blokuje kolejne próby. **Nie pamiętasz hasła?** wysyła link do resetowania, który działa jednorazowo przez 30 minut. Ze względów bezpieczeństwa formularz resetowania nigdy nie ujawnia, czy dany adres ma konto.

Obecnie jedyną metodą jest logowanie hasłem. Logowanie jednokrotne (SSO), uwierzytelnianie wieloskładnikowe i klucze dostępu nie są jeszcze dostępne.

## Członkowie

Karta **Członkowie** wyświetla wszystkie osoby w obszarze roboczym wraz z przydzielonymi im rolami. Możesz tutaj:

- przydzielać i odbierać role;
- ustawić członka jako **nieaktywnego**, co wylogowuje go i blokuje dostęp bez usuwania jego historii;
- przenieść własność na innego aktywnego członka (tylko właściciele).

Obszar roboczy zawsze ma co najmniej jednego aktywnego właściciela. Zmiany członkostwa i ról wylogowują osobę, której dotyczą, z istniejących sesji.

## Zespoły

Karta **Zespoły** grupuje członków pod wspólną nazwą, na przykład *Jakość* lub *Serwis terenowy*, aby [atrybut użytkownika lub zespołu](/pl/builders/modeling/#przypisz-odpowiedzialność) mógł przypisać pracę całemu zespołowi. Każdy, kto ma `members.manage`, może utworzyć zespół, zmienić jego nazwę i skład albo go usunąć. Kodu zespołu nie można zmienić po utworzeniu.

Rekordy zapisują sam zespół, więc zmiana jego składu nigdy nie zmienia rekordów. Filtry **Przypisane do mnie** w Eksploratorze obejmują rekordy przypisane do Twoich zespołów. Usunięty zespół pozostaje w rekordach, które już go używają, i jest oznaczony jako usunięty, ale nie można go już przypisać. Zespoły nie nadają uprawnień.

## Role

Rola to nazwany zestaw uprawnień. Czterech wbudowanych ról nie można zmieniać:

| Rola | Może |
| --- | --- |
| `owner` | Wszystko, łącznie z przeniesieniem własności. |
| `admin` | Wszystko oprócz własności i cyklu życia obszaru roboczego. |
| `editor` | Odczytywać i zapisywać schematy, encje i konteksty; usuwać encje; odczytywać stan danych. Nie może publikować schematów ani encji ani administrować obszarem roboczym. |
| `viewer` | Odczytywać schematy, encje, konteksty i stan danych. |

Utwórz **role niestandardowe** na karcie **Role**, aby przyznać węższy lub inny zestaw uprawnień. Do roli możesz dodać tylko te uprawnienia, które masz sam. Zduplikuj rolę wbudowaną, aby zacząć od jej uprawnień. Wycofując rolę niestandardową, możesz przenieść jej przydziały na rolę zastępczą.

Pełna lista uprawnień znajduje się w [dokumentacji uprawnień](/pl/reference/permissions/).

### Przydziały z zakresem

Przydział roli obowiązuje w jednym zakresie:

| Zakres | Obejmuje |
| --- | --- |
| **Cały obszar roboczy** | Wszystko. |
| **Rodzina schematów** | Jeden schemat i jego encje we wszystkich wersjach. |
| **Encja** | Jedną encję. |
| **Poddrzewo kontekstu** | Jeden kontekst i wszystko poniżej niego, ale nie jego kontekst nadrzędny ani konteksty równorzędne. |

Przydziały sumują się. Osoba z rolą `viewer` w obszarze roboczym i `editor` w poddrzewie kontekstu `PL` może odczytywać wszystko i edytować wartości w `PL` oraz jego kontekstach podrzędnych.

Rolę właściciela można przydzielić tylko w całym obszarze roboczym.

## Zaproszenia

Na karcie **Zaproszenia** zaproś osobę przez e-mail, podając rolę, zakres i datę wygaśnięcia. Otrzyma ona jednorazowy link. Osoba, która ma już konto, akceptuje zaproszenie i dołącza; nowa osoba najpierw ustawia hasło.

Możesz też utworzyć użytkownika bezpośrednio i wysłać mu link wdrożeniowy. Oczekujące zaproszenie możesz w każdej chwili odwołać.

Wiadomości z zaproszeniami i linkami wdrożeniowymi wymagają [skonfigurowanego SMTP](/pl/reference/configuration/#e-mail).

## Nawigacja

Karta **Nawigacja** określa skróty do schematów na pasku bocznym przeglądarki encji. Przypnij opublikowane schematy encji i opcjonalnie ogranicz każdy skrót do wybranych ról, aby użytkownicy widzieli te części katalogu, nad którymi pracują.

Zmiana nawigacji wymaga uprawnienia `workspace_navigation.manage`.

## Twój profil

**Profil → Konto** pokazuje, jak widzą Cię inne osoby w obszarze roboczym.

- **Zmień nazwę wyświetlaną** ustawia nazwę widoczną na liście członków, w dzienniku audytu i w historii encji. Musi mieć od 2 do 64 znaków, zawierać tylko litery, cyfry i spacje oraz nie może zaczynać się ani kończyć spacją. Nazwa wyświetlana jest wspólna dla wszystkich Twoich obszarów roboczych.
- **Prześlij zdjęcie** ustawia awatar z obrazu PNG lub JPEG o rozmiarze do 10 MB. Obraz jest przycinany do wyśrodkowanego kwadratu, zmniejszany i umieszczany na białym tle, więc pojawia się po chwili. **Zmień zdjęcie** zastępuje go, a **Usuń zdjęcie** przywraca inicjały.

Zdjęcie należy do bieżącego obszaru roboczego: ustaw je w każdym obszarze, z którego korzystasz. Widzą je wszyscy członkowie obszaru, ale udostępniana jest tylko zmniejszona wersja. Przesłany oryginalny plik nigdy nie jest nikomu pokazywany.

## Osobiste tokeny API

Skrypty, CLI i integracje uwierzytelniają się osobistymi tokenami API. Utwórz token w **Profil → Osobiste tokeny API** albo poleceniem `acli token create`.

- Token ma etykietę, opcjonalną datę wygaśnięcia i jawną listę uprawnień. Nigdy nie może zrobić więcej niż jego właściciel: jeśli właściciel straci uprawnienie, token również je traci.
- Sekret tokenu zaczyna się od `cat_pat_` i jest wyświetlany tylko raz. Przechowuj go w menedżerze sekretów.
- Wysyłaj go jako `Authorization: Bearer cat_pat_…`. To token wyznacza obszar roboczy; żaden nagłówek ani parametr go nie wybiera.
- Token może utworzyć inny token tylko z częścią własnych uprawnień. Jeśli token tworzący wygasa, nowy musi wygasnąć nie później.
- Odwołuj tokeny, których już nie potrzebujesz. Odwołanie tokenu nie odwołuje tokenów, które utworzył; odwołaj każdy z osobna. Lista tokenów pokazuje, kiedy każdy z nich był ostatnio użyty, z dokładnością do minuty.

Tworzenie tokenów wymaga uprawnienia `tokens.manage`.

## Dziennik audytu

**Zarządzanie → Aktywność / dziennik audytu** wyświetla każdą udaną zmianę w obszarze roboczym: kto ją wykonał (osoba, token lub agent), co się zmieniło, kiedy oraz identyfikator żądania. Filtruj według czasu, wykonawcy, kategorii działania, typu obiektu lub tego, czy zmianę wprowadziła osoba, czy agent.

Wpisy audytu nigdy nie zawierają haseł, tokenów ani sekretów. Zmiany wprowadzone przez agenta pokazują uruchomienie agenta, narzędzie i osobę, która je zatwierdziła. Nieudane lub odrzucone żądania nie są rejestrowane, ponieważ niczego nie zmieniły.

Odczyt dziennika audytu wymaga uprawnienia `audit.read`.

```sh
acli audit list --executor-type agent --occurred-after 2026-03-01T00:00:00Z
```
