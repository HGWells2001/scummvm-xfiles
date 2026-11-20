# Stato del supporto X-Files

Il repository contiene due varianti distinte: un lettore C++ di ricerca e un runtime sperimentale che collega Shiloh nello stesso processo ScummVM. Nessuna partita completa e stata ancora verificata tramite questo host.

## Runtime integrato

- Core Shiloh fissato al commit e3dfde0d098b25135a5243839d44842ccbc5fe91, frontend SDL originale disabilitato.
- Input mouse/tastiera/tastierino, framebuffer RGB 640x480 e audio PCM tramite ScummVM.
- Collegamento alle richieste Nuova partita, Carica, Esci e ai salvataggi del core.
- Salvataggi e preferenze fuori dai dati originali; integrazione con il browser salvataggi ScummVM ancora da fare.
- Pausa ScummVM sincronizzata al clock del core; errori e panic Rust confinati al confine ABI.
- Rust 1.100.0 e Cargo.lock fissati. Avviatore italiano incluso negli artefatti.

## Lettore C++ e dati originali

- Rilevamento della coppia HDB/GAM locale con dimensioni e MD5 dei primi 5000 byte.
- NeoAccess: 39 classi HDB, 4 classi GAM, tutti i conteggi degli indici locali corrispondono.
- 4922 descrittori di file su 6410 risorse; le restanti sono risorse speciali/non interpretate dal lettore C++.
- Tutte le 14247 azioni locali decodificate strutturalmente. Questo lettore non le esegue.
- Browser QuickTime con pausa e introduzione; gioco completo non eseguibile in questa variante.

## Preparazione e verifica

- I sette ISO contengono record associati con gli stessi nomi dei veri file dati. Nella cartella iniziale alcuni erano stati estratti come resource fork da 286 byte.
- Recupero corretto completato in work/xfiles-data, senza modificare la cartella originale o gli ISO su D:.
- Saltati 2061 record associati; riconosciuti 101 duplicati QuickTime equivalenti salvo date e padding. Manifest SHA256 nella cartella dati preparata.
- Test locali C++ di database, risorse e azioni: PASS. Test Node dell'estrazione ISO: PASS.
- Sintassi PowerShell e avviatore -PrepareOnly sui dati reali: PASS.
- Host C++ opzionale: compilazione locale MinGW PASS. Test ABI Rust su Linux: PASS.
- Build Windows del lettore: suite 44779755477 PASS.
- Primo collegamento completo del runtime: mancava synchronization per WaitOnAddress/WakeByAddress. Aggiunta e verificata nella toolchain; esito sulla revisione corrente consultabile in Actions.

## Limiti e ripresa

La compilazione non equivale a una partita completa verificata. Restano prove interattive, audio/video, caricamento/salvataggio e percorrenza completa della storia attraverso il nuovo host.

Sorgente ScummVM fissato a 55c2973c1df5e8790b4248ae36a6b1b37842ce12. La copia di lavoro separata e in work/xfiles-upstream; i dati preparati sono in work/xfiles-data. Gli originali sono in D:\The X-Files e nei sette ISO adiacenti. Usare README.md per compilazione e avvio; nessun file originale del gioco viene pubblicato.