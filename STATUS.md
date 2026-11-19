# Stato del progetto

Il gioco completo NON e ancora giocabile. Il motore e un visualizzatore sperimentale di risorse.

## Implementato

- Riconoscimento della coppia HDB/GAM locale con hash e dimensioni.
- Lettore NeoAccess con controlli di limiti, cicli e indici; lettura liste persistenti.
- Parser risorse: varianti, percorsi relativi, intervalli temporali.
- Browser nativo ScummVM con riproduzione QuickTime, pausa e sequenza introduttiva.
- Build anteprima Windows e suite automatiche GitHub.

## Verifiche

- Prima suite GitHub: test parser e compilazione motore PASS (run 44619003926, commit 79780b3).
- Compilatore MSVC locale: errore installazione C1902; nessuna impostazione di sistema alterata.
- Toolchain alternativa MinGW: test C++ eseguiti e lettore verificato sui database originali.
- XFILES.HDB: 39 classi; XFILES.GAM: 4 classi. Tutti i conteggi degli indici coincidono con i descrittori.
- Parser risorse locale: 4922 descrittori letti su 6410; 1488 risorse speciali/non interpretate.
- Il sorgente del nuovo frontend compila con MinGW; esecuzione e audio/video da verificare con la build completa.

## Ripresa

Directory locale: C:\Users\mgian\Documents\Codex\2026-11-19\new-chat\scummvm-xfiles
Dati originali: D:\The X-Files, database sotto MININST.
Sorgente ScummVM di lavoro: work/xfiles-upstream (copia separata; nessuna modifica al vecchio progetto).
ScummVM fissato a 55c2973c1df5e8790b4248ae36a6b1b37842ce12.
Riferimento strutturale Shiloh e3dfde0d098b25135a5243839d44842ccbc5fe91; attribuzione in THIRD_PARTY.md.

Restano interprete eventi/script, gerarchia scene, dialoghi, inventario, PDA, salvataggi e partite complete.