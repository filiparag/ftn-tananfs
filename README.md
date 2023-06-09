---
title: Primer realizacije _FUSE_ fajlsistema
subtitle: Ispitni rad iz predmeta Operativni sistemi
author: Filip Parag
date: 12. jun 2023.
papersize: A4
output:
  pdf_document:
---


**Student**: Filip Parag, RA 122/2018

**Mentor**: dr Veljko Petrović

# TananFS

_TananFS_ je edukativni fajlsistem napravljen sa ciljem da ima malo metapodataka, a istovremeno velike gornje granice za imena i veličine datoteka.

## Arhitektura

Fajlsistem svoj sadržaj na blok uređaju raspoređuje na sledeći način:

- prvi (_boot_) sektor ostavlja prazan
- u naredna 1024 bajta smešta superblok
- redom bit mapa inoda i mapa blokova, ne manje od po 1024 bita zauzeća
- prazno mesto za poravnanje do sledećeg bloka
- region inoda
- prazno mesto za poravnanje do sledećeg bloka
- region blokova

### Superblok

Superblok je struktura koja sadrži sve podatke neophodne za učitavanje postojećeg fajlsistema sa blok uređaja (u daljem tekstu - disk). U tabeli su prikazana polja superbloka:

| Pozicija | Tip   | Naziv polja         |
| -------- | ----- | ------------------- |
| 0        | `u64` | ukupan broj inoda   |
| 8        | `u64` | slobodnih inoda     |
| 16       | `u64` | ukupan broj blokova |
| 24       | `u64` | slobodnih blokova   |
| 32       | `u32` | veličina bloka      |
| 56       | `u64` | magični broj        |

: Polja strukture Superblok

Veličina bloka je stepen dvojke u opsegu od 512 do 4096 bajta i preporučljivo je da se poklapa sa veličinom sektora diska, jer u suprotnom dolazi smaknutih upisivanja i čitanja koja umanjuju performanse i potencijalno smanjuju životni vek fleš memorije. U ovom tekstu se podrazumeva da su veličina sektora i bloka istovetne.

Magični broj je torka bajtova `0x54616E616E465321` koja služi za otkrivanje postojećeg fajlsistema. Pri pokretanju programa, magični broj se traži za svaku potencijalnu veličinu bloka, i ukoliko biva pronađen, postojeći fajlsistem se učitava, a u suprotnom se kreira novi fajlsistem tako da zauzme ceo disk.

**Računanje kapaciteta**

Kapacitet fajlsistema je broj upotrebljivih bajtova za datoteke, kada se od veličine diska oduzme prostor za metapodatke. Formula za dobijanje kapaciteta:

```
kapacitet = veličina boot sektora +
            veličina superbloka +
            veličina bit mape za inode +
            veličina bit mape za blokove +
            veličina regiona inoda +
            poravnjanje do sledećeg bloka
```

Problem je kako unapred odrediti broj inoda i veličine bit mapa kada oni zavise od kapaciteta. Problem se izbegava tako što se uzme gornja granica veličine, tj. prvo se prostor nakon superbloka podeli sa količinom podataka po inodi, a zatim se preostali prostor podeli sa veličinom bloka. Na taj način se garantuje dovoljan broj inoda i blokova uz minimalne gubitke.

### Bit mapa

Bit mape služe za evidenciju slobodnih polja (inoda i blokova) u što kompaktnijem obliku. Svakom polju je pridružen jedan bit, a bitmapa zauzima stepen dva broja bajtova, a najmanje 1024 radi poravnanja.

Zauzimanje polja ide sekvencijalno, odnosno uvek se traži prvo slobodno polje. U memoriji se bit mape čuvaju kao niz `usize` elemenata, pa se pri pretrazi narednog slobodnog obrađuje po 64 polja po iteraciji u slučaju savremenih računara.

Ovakvo zauzimanje dovodi do neželjenog spoljnog parčanja slobodnog prostora pri čestom brisanju i smanjivanju datoteka, ali to ne predstavlja preveliki problem na poluprovodničkim diskovima koji nisu elektromehaničke ili optičke prirode.

\pagebreak

### Inoda

Inoda je struktura fiksne veličine za čuvanje svih metapodataka datoteke i sadrži sve izuzev imena, koje je proizvoljne dužine:

| Pozicija | Tip        | Naziv polja                         |
| -------- | ---------- | ----------------------------------- |
| 0        | `u64`      | redni broj                          |
| 8        | `u16`      | režim pristupa                      |
| 10       | `u8`       | tip datoteke                        |
| 18       | `u64`      | veličina u bajtima                  |
| 26       | `u32`      | identifikator korisnika             |
| 30       | `u32`      | identifikator grupe                 |
| 34       | `u64`      | vreme poslednjeg pristupa           |
| 42       | `u64`      | vreme poslednje izmene metapodataka |
| 50       | `u64`      | vreme poslednje izmene podataka     |
| 58       | `u64`      | vreme brisanja                      |
| 66       | `u64`      | broj blokova                        |
| 74       | `[u64; 5]` | niz proizvoljnih metapodataka       |
| 112      | `u64`      | redni broj prvog bloka              |
| 120      | `u64`      | redni broj poslednjeg bloka         |

: Polja strukture Inoda

### Blok

Blok je komad proizvoljnih bajtova unapred poznate veličine i služi za čuvanje podataka i metapodataka promenljive veličine.

Broj inoda je manji ili jednak broju blokova, jer se po jednoj datoteci zauzima tačno jedna inoda, a broj blokova je promenljiv. Nezavisno od veličine bloka, za svaka 4 kibibajta se na disku zauzima po jedna inoda, pod pretpostavkom da većina datoteka ne zauzima manje od te veličine.

\pagebreak

### Datoteka bajta

Datoteka bajta je apstrakcija nad blokovima koja služi kao most između niza bajtova proizvoljne dužine i njihovog skladištenja na disku, raspoređivanjem u blokove. Pomoću ove strukture se naredni delovi arhitekture fajlsistema znatno pojednostavljuju; logika algoritama je olakšana tako što nije potrebno voditi računa o organizaciji po blokovima, već se datoteke mogu posmatrati kao jedan neprekidan niz bajtova čija se veličina menja po potrebi. U daljem tekstu su opisane metode koje pruža ova struktura.

**Nova datoteka**

Nova datoteka bajta se može napraviti prazna - sa veličinom 0 bajta i zauzećem od 0 blokova, ili sa proizvoljnom veličinom bajta; u tom slučaju se zauzimaju blokovi i njihov sadržaj se postavlja na nule.

**Učitavanje datoteke**

Postojeća datoteka na disku se učitava ovom funkcijom, tako što joj se prosledi inoda koja opisuje datoteku. Datoteka bajta predstavlja povezanu listu blokova gde svaki blok u svojih prvih 8 bajta čuva adresu narednog, izuzev poslednjeg koji pokazuje na uzdržanu nepostojeću adresu `0xFFFFFFFFFFFFFF`. U inodi se čuva adresa prvog i poslednjeg bloka, kao i njihov ukupan broj.

Učitana datoteka ne učitava blokove samostalno, samo prepisuje podatke iz inode i postavlja kursor na početak, koji će detaljnije biti opisan kroz ostale funkcije. Teorijska gornja granica veličine ovakve datoteke je deset zetabajta, ograničena pre svega najvećim adresabilnim brojem od strane kursora - `0xFFFFFFFFFFFFFE`.

**Pomeranje kursora**

Kursor datoteke služi za premeštanje mesta na kome se trenutno vrše izmene bajtova. On se može postaviti na apsolutnu poziciju u odnosu na početak i kraj datoteke, kao i na relativnu poziciju u odnosu na trenutnu. Kako svaki blok na svom početku sadrži adresu narednog, kursor omogućava pristup stvarnom i logičkom mestu pojedinih bajtova - za unutrašnje potrebe datoteke bajta se koristi stvarna, a za potrebe svih apstrakcija logička pozicija.

Zbog neporavnatosti stvarnih i logičkih mesta bajtova, programi koji sarađuju sa ovim fajlsistemom mogu imati netačne pretpostavke o najboljem komadanju datoteka pri čitanju i pisanju, što dovodi do istih problema kao pri neslaganju veličine sektora i bloka.

**Čitanje iz datoteke**

Funkciji za čitanje iz datoteke se prosleđuje promenljiva referenca niza bajtova proizvoljne veličine. Ukoliko je od trenutnog mesta kursora do kraja datoteke ostalo manje bajta od veličine niza, vraća se greška, a u suprotnom se sa diska redom učitavaju blokovi i njihov sadržaj se prepisuje u niz. Kursor se tokom čitanja pomera, tako da dva uzastopna poziva ove funkcije vraćaju uzastopne delove datoteke.

\pagebreak

**Pisanje u datoteku**

Pisanje u datoteku radi na sličan način kao i čitanje, osim što se u ovoj funkciji iz niza podaci prepisuju u blokove, koji se zatim skladište na disk. Kada je niz duži od preostalog mesta u datoteci, na njen kraj se dodaju novi blokovi dok se svi podaci ne prepišu.

**Proširenje i smanjenje datoteke**

Datoteka se proširuje nulama na sličan način kao što se u nju piše, a smanjuje se tako što se sa njenog kraja odseče višak, tj. suvišni blokovi se oslobode, a u poslednjem bloku se adresa narednog postavi na nepostojeću.

**Brisanje datoteke**

Pri brisanju datoteke se svi blokovi oslobode, ali kao što je slučaj i kod smanjivanja, njihov sadržaj se ne briše, već biva ostavljen do narednog zauzeća i pisanja u blok. Ovo znači da se ažurira samo bit mapa blokova, a ako je potrebno prebrisati blokove, to se mora uraditi funkcijom za pisanje.

### Direktorijum

Direktorijum je par inode i datoteke bajta posebnog tipa, jer se u njenim blokovima ne čuvaju podaci korisnika, već metapodaci o sadržaju datoteke. U prvom proizvoljnom polju za metapodatke u inodi se čuva broj inode roditelja direktorijuma, u drugom broj potomaka, a u trećem dužina imena datoteke.

Pridružena datoteka bajta započinje imenom direktorijuma, a zatim se redom upisuju njeni potomci: za svakog potomka se čuva broj inode (8 bajta), dužina imena (2 bajta) i ime kao niza bajta proizvoljne dužine. Gornja granica dužine imena je 65536 bajta Unicode karaktera.

Primer prvog bloka `root` direktorijuma sa datotekama `primer1.txt` i `prezentacija.pdf`:
```
FF FF FF FF FF FF FF FF  r  o  o  t  ·  ·  ·  ·
 ·  ·  · 02  · 0B  p  r  i  m  e  r  1  .  t  x
 t  .  .  .  .  .  .  . 03  · 10  p  r  e  z  e
 n  t  a  c  i  j  a  .  p  d  f  ·  ·  ·  ·  ·

```

Prvih osam bajta ima vrednost `FF` jer je ovo zadnji blok, nakon čega je `02` broj inode, `0B` dužina imena i `primer1.txt` ime prvog potomka, a `03`, `10` i `prezentacija.pdf` drugog. Prikazan je samo početak bloka veličine 512 bajta sa 16 bajta po redu, gde tačke van imena predstavljaju `00` radi čitljivosti.


Dok je direktorijum učitan, kopija podataka o potomcima se čuva u radnoj memoriji kako bi pretraga bila brža.

### Datoteka

Datoteka je takođe par inode i datoteke bajtova, ali je njen sadržaj potpuno proizvoljan. Svaka datoteka može pripadati jednom direktorijumu, čiji broj čuva u prvom polju inode za metapodatke. Metapodaci o vremenu pristupa, izrade i izmene se i kod direktorijuma i kod datoteke ažuriraju pri svakoj od ovih radnji.

\pagebreak

### Fajlsistem

Fajlsistem je struktura koja povezuje različite nivoe apstrakcije - s jedne strane vodi zapisnik o zauzeću i stanju svakog bajta, a s druge strane korisnicima daje organizaciju podataka u datoteke i direktorijume koji su izmišljeni za lakši rad na računaru, ali sami po sebi ne postoje na disku.

### Radna memorija i keš

Rad sa diskovima spada u jedan od sporijih načina na koji procesor može da barata podacima. U hijerarhiji memorije na vrhu po brzini stoje procesorski registri i keš, a na dnu su mehanički i optički diskovi i mreža. Do sada opisane strukture fajlsistema vrlo često zahtevaju pisanje i čitanje istih delova diska, pa je smislen način da se oni ubrzaju da se deo tih podataka privremeno čuva u radnoj memoriji.

Zbog ovoga se uvodi LRU (eng. _least recently used_) keš, odnosno privremeno skladište najčešće korišćenih blokova i inoda. Svaki put kada bilo koja struktura zatraži pristup inodi ili bloku, fajlsistem prvo proveri da li već postoji u kešu. Ako ne postoji - učitava se sa diska, skladišti se u keš i kopija se izdaje zahtevaocu. Pored vrednosti strukture se čuva i vreme kada joj je pristupano, kao i da li je izmenjena u odnosu na original na disku.

Pisanje struktura na disk takođe prolazi kroz keš - svaki put kada se zahteva upis, fajlsistem proveri da li je od poslednjeg pisanja prošao određeni period, i ako jeste, izmenjene blokove i inode čuva na disk, nakon čega raspoređuje sve keširane podatke po vremenu pristupa i bira da zadrži samo one kojima se nedavno pristupalo.

Redosled pisanja na disk ne poštuje princip lokalnosti - donekle je nasumičan jer zavisi od rasporeda po vremenu pristupa, što dovodi do usporenja ako je disk povezan na magistralu kojoj godi da zaredom dobija susedne podatke, ili je u pitanju uređaj koji ima fizička ograničenja brzine skokova poput hard diska.

Vreme između dva pisanja na disk i broj čuvanih kopija su podesivi parametri fajlsistema i njihove vrednosti zavise od prioriteta korisnika: ako zauzeće radne memorije nije problem, broj keširanih stavki može biti velik, a ako gubitak podataka pri havariji nije presudan, vreme između dva pisanja isto može biti veliko. Podrazumevan period čekanja između dva upisa je jedan sekund, a broj stavki 131072. Jedini izuzetak, kada se pri zahtevu na disk piše sigurno je pri zatvaranju fajlsistema.

\pagebreak

## Sučelje sa operativnim sistemom

Fajlsistem je ostvaren kao _FUSE_ drajver koji živi u korisničkom prostoru i biva pozvan od strane kernela svaki put kada korisnik zatraži. Ovakav pristup nije najperformantniji, ali pruža mnogo lakšu izradu drajvera, što je za fajlsistem edukativnog tipa zadovoljavajuć ustupak. U nastavku će ukratko biti opisano kako _TananFS_ odgovara na sistemske pozive.

### Inicijalizacija i statistika

Pri pokretanju drajvera za fajlsistem se za dati blok uređaj vrši autodetekcija postojećeg fajlsistema traženjem magičnog broja za sve dozvoljene veličine bloka. Ako fajlsistem nije pronađen, pravi se novi, sa posebnim korenim direktorijumom, čiji je vlasnik korisnik `root`.

Kako je zauzimanje i oslobađanje blokova i inoda posao strukture fajlsistema, u svakom trenutku je moguće lako izračunati zauzeće resursa na osnovu polja superbloka, koje se dobija sistemskim pozivom `statfs`.

### Metapodaci i dozvola pristupa

Vlasništvo i dozvole pristupa kod ovog fajlsistema se beleže ali ne i sprovode u sistemskom pozivu `access`, tako da je moguće pristupiti svim podacima od strane svih korisnika.

Funkcije za rukovanje metapodacima su `getattr` i `setattr`. Podržani metapodaci su režim datoteke, vlasnički korisnik i vlasnička grupa.

### Upravljanje direktorijumom

Datoteke se mogu izraditi putem poziva `mkdir`, obrisati ako nemaju potomke sa `rmdir` i izlistati sa `readdir`. Drške datoteka se pri `open` i `opendir` izdaju kao nulte, jer ih fajlsistem ne koristi u svom radu, već se oslanja na LRU keš blokova i inoda.

### Upravljanje datotekom

Nova prazna datoteka se pravi sistemskim pozivom `mknod`. On roditeljskom direktorijumu pridružuje novu datoteku ako ime već nije zauzeto. Promena veličine datoteke se vrši pozivom `fallocate` koji u dodati prostor upisuje nule. Upisivanje na zadati pomeraj radi poziv `write`, a čitanje `read`.

Pri svakom od do sada navedenih poziva se koriste privremene drške datoteka koje se uklanjaju odmah pri izvršetku sistemskog poziva. Kod nasumičnog pristupanja datotekama ovo može predstavljati problem jer je pretraga blokova linearne vremenske složenosti, ali ako se pristupa početku ili kraju adresa bloka je poznata iz inode.

Brisanje datoteke radi poziv `unlink`, koji oslobodi sve resurse vezane za datu datoteku i ukloni je iz roditeljskog direktorijuma.

Pozivi `flush` i `fsync` zatražuju od fajlsistema da sinhronizuje ceo keš sa diskom, jer je evidencija blokova vezanih za datoteku bez dugovečnih drški kvadratne vremenske složenosti.
