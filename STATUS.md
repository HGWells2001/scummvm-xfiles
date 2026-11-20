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
La suite runtime 44782908237 e PASS: primo eseguibile Windows completo con core Shiloh collegato staticamente. Il pacchetto successivo aggiunge controllo delle DLL senza i percorsi della macchina di compilazione, avvio --list-engines e opzione locale -SmokeTest. Il test di una partita completa resta da eseguire.


## Esito finale verificato
Revisione di codice 8c40dd2caa986fc9fbe3a6acef451b42ff04929b: entrambe le suite completate con SUCCESS.
- 44785250218: test C++/Python/Node, compilazione del motore e anteprima Windows PASS.
- 44785250159: test ABI Rust, raccolta note dipendenze, runtime Windows e controllo autonomo delle DLL/registrazione motore PASS.
Non restano suite di questa revisione in esecuzione. La build integrata e disponibile fra gli artefatti della suite runtime. Nessuna verifica di partita completa o qualita audio/video viene dichiarata.


Artefatto runtime: https://github.com/HGWells2001/scummvm-xfiles/actions/runs/44785250159/artifacts/14653308848
SHA256 ZIP: 80d028ed56209562f10818dc6f6f2dc0ce60aaa62e3bc9e3e3c0dc78808b60b9

## Correzione avviatore del 20 novembre 2026

Revisione 1df4675eb5d4d51289b7c7a1fb7a558e2dd21273: risolto Split-Path su stringa vuota nei parametri predefiniti. Percorsi inizializzati dopo param, con ripiego per la directory dello script e LocalAppData; messaggio dedicato per DLL mancanti.

- Suite 44790119649 SUCCESS: regressione Windows PowerShell 5.1 con cartelle con spazi, parametri impliciti/espliciti e eseguibile assente; test database e compilazioni Linux/Windows.
- Suite 44790119765 SUCCESS: ABI Rust, runtime Windows, dipendenze autonome e registrazione xfiles.
- La prima versione del nuovo test conservava il codice di uscita della prova di errore intenzionale: corretto, poi rieseguito con successo.
- Test C++ e Node locali PASS; esecuzione locale dei file PowerShell bloccata dalla policy, lasciata invariata.
- L'eseguibile preesistente in D:\scummvm-xfiles termina con 0xC0000135; importa WinSparkle.dll assente dalla sua cartella. Non costituisce una verifica del pacchetto runtime integrato. Il download del runtime per la prova locale non e stato completato.

Pacchetto aggiornato: https://github.com/HGWells2001/scummvm-xfiles/actions/runs/44790119765/artifacts/14654934372

SHA256 ZIP: 09c48354069e4a231804b0a02e140115fcc932deb0487eea05021831eac7ea53

Tutte le suite avviate sono terminate. Rimangono da verificare avvio locale con dati reali, audio/video, salvataggi e partita completa.


## Diagnosi schermo nero con audio

Il pacchetto 1df4675 e stato scaricato e il suo SHA256 verificato. Il core e stato eseguito sui dati reali sia di work/xfiles-data sia della cartella preparata accanto al pacchetto dell'utente: 120 tick, view=691, 434481 byte RGB non nulli. Il frame esportato mostra l'ufficio FBI di Seattle. Non e una prova di audio o partita completa.

Una cattura della finestra mostra il logo introduttivo nel renderer predefinito. Con SDL_RENDER_DRIVER=software e SDL_VIDEODRIVER=windows sono visibili anche sfondo e voci del menu dopo aver saltato i quattro filmati. Il nero permanente segnalato non e stato riprodotto con queste impostazioni; non viene attribuito con certezza a un codec o driver.

Trovati font e tema mancanti nel vecchio pacchetto; la vecchia build non includeva FreeType2. I pacchetti aggiornati includono fonts.dat, tema, traduzioni, licenze e FreeType2. Il controllo del pacchetto verifica anche questa funzionalita. La prima verifica aveva rilevato la mancata disponibilita di pkgconf: aggiunto e verificato con la nuova build. Gli avviatori indicano esplicitamente il percorso delle risorse e producono log.

Avvia-XFiles-Video-Compatibile.cmd offre rendering software in finestra e una configurazione ScummVM separata, con log xfiles-video.log. Riutilizza i dati e i salvataggi esistenti. Il suo effetto sul problema segnalato richiede conferma nell'avvio dell'utente.

Revisione 5b8591ad225868e77caff41889a16dd3f9543154: suite 44794217680 e 44794217492 SUCCESS. Verificati ABI Rust, test database e avviatore, compilazioni Linux/Windows, FreeType2, registrazione xfiles e caricamento autonomo delle DLL. Tutte le suite avviate sono terminate.

Pacchetto: https://github.com/HGWells2001/scummvm-xfiles/actions/runs/44794217680/artifacts/14657703400

SHA256 ZIP: a01c346569b1603d8ceaaf0e025f202d8e022ec42ae911250bb9958f557e8a0b

## Audio assente nella prima scena: QDMC

Segnalazione: video e audio regolari con l'avviatore software, ma niente audio davanti all'ufficio FBI. Il filmato XV/64421.XMV usa QDesign Music (QDMC), che il core fissato legge da cache PCM. FFprobe locale conferma QDMC, 44100 Hz e due canali.

Aggiunto tools/prepare_audio.mjs: cerca le tracce QDMC, usa FFmpeg/ffprobe gia installati (-ignore_editlist 1), conserva WAV PCM e metadati compatibili con Shiloh fuori dai dati del gioco. Gli avviatori impostano XF_CACHE e preparano/verificano la cache prima di avviare il runtime. Gli errori sono visibili in preparazione-audio.log. Nessun file originale o decoder viene scaricato, modificato o pubblicato.

Verifica sui dati reali: 15 file QDMC preparati; seconda esecuzione 0 preparati e 15 riutilizzati. Alterando un WAV di prova, viene rigenerato solo quello (1 preparato, 14 riutilizzati). FNV64 verificato contro un'implementazione BigInt indipendente. Test di intestazioni QuickTime, troncamenti e isolamento della cache PASS.

Primi 120 tick della scena: prima 0 campioni non nulli; dopo 175096. Ripetendo con la sola cache XV/64421.XMV si ottiene lo stesso risultato. Il frame RGB mantiene lo stesso SHA256 prima/dopo (5e7f64c5fff523df626b3fa7dd549c0c77aaa7153a2b3746cec0960f865ca7ff). Questa prova verifica l'uscita PCM, non una partita completa o ogni dettaglio della qualita sonora.

Revisione 1fe502c4eb65895e600687106f8291bf653eabd0: suite 44796666310 e 44796666340 SUCCESS; tutte terminate. Test Node/C++/Python, avviatore Windows, ABI Rust, compilazioni Linux/Windows e controllo delle DLL PASS.

Patch audio (6295 byte): https://github.com/HGWells2001/scummvm-xfiles/actions/runs/44796666340/artifacts/14658339270
SHA256: 92002093766e93fe06e4266bb96ecaf82032bf11bf71a6688080f2e9ddfcd5e2

Runtime completo: https://github.com/HGWells2001/scummvm-xfiles/actions/runs/44796666340/artifacts/14659311690
SHA256: 141ca40267dfed5ba29f7ac2a72613fbf2f31bbead5c3c0015639f3ecef40399
