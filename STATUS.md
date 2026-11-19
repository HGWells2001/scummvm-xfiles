# Stato del progetto

## Completato

- Repository creato prima dell'implementazione.
- Rilevamento della coppia HDB/GAM locale tramite hash e dimensioni.
- Lettore C++ NeoAccess, inspector, test sintetici e integrazione diagnostica ScummVM.
- Nessun dato originale del gioco pubblicato.

## Limiti

**Il gioco completo non e ancora giocabile.** Il motore apre i database e mostra una diagnostica; non esegue scene o script. Restano dialoghi, inventario, media, salvataggi e verifica di una partita completa.

La compilazione locale si e fermata prima dei test per un errore C1902 dell'installazione Visual Studio. La verifica automatica su GitHub compila il parser, esegue CTest e compila il motore con ScummVM fissato al commit 55c2973c1df5e8790b4248ae36a6b1b37842ce12. Consultare Actions per l'esito corrente.

## Ripresa

Directory locale: `C:\Users\mgian\Documents\Codex\2026-11-19\new-chat\scummvm-xfiles`.
Dati del gioco: `D:\The X-Files`, database in `MININST`.
Riferimento strutturale: Shiloh e3dfde0d098b25135a5243839d44842ccbc5fe91, attribuzione in THIRD_PARTY.md.
