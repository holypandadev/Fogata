// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Italian (`it`).
class L10nIt extends L10n {
  L10nIt([String locale = 'it']) : super(locale);

  @override
  String get commonCancel => 'Annulla';

  @override
  String get commonSave => 'Salva';

  @override
  String get commonClose => 'Chiudi';

  @override
  String commonNewMessageFrom(String name, String text) {
    return 'Nuovo messaggio da $name: $text';
  }

  @override
  String commonNewMessagesFrom(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n nuovi messaggi da $name',
      one: '1 nuovo messaggio da $name',
    );
    return '$_temp0';
  }

  @override
  String commonNewMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n nuovi messaggi',
      one: '1 nuovo messaggio',
    );
    return '$_temp0';
  }

  @override
  String get commonAccept => 'Accetta';

  @override
  String get commonDecline => 'Rifiuta';

  @override
  String get commonSend => 'Invia';

  @override
  String get commonTryAgain => 'Riprova';

  @override
  String get commonCopy => 'Copia';

  @override
  String get commonCopied => 'Copiato';

  @override
  String get commonBack => 'Indietro';

  @override
  String get commonDiscard => 'Scarta';

  @override
  String get commonKeepEditing => 'Continua a modificare';

  @override
  String get commonNotNow => 'Non ora';

  @override
  String get commonDownload => 'Scarica';

  @override
  String get commonSettings => 'Impostazioni';

  @override
  String get commonHome => 'Home';

  @override
  String get commonFriends => 'Amici';

  @override
  String get commonContinue => 'Continua';

  @override
  String get commonDone => 'Fatto';

  @override
  String get commonRemove => 'Rimuovi';

  @override
  String get commonYou => 'Tu';

  @override
  String timeYesterdayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'ieri, $time',
      'other': 'ieri, $time',
    });
    return '$_temp0';
  }

  @override
  String timeDayAt(String date, String time) {
    return '$date, $time';
  }

  @override
  String get timeNow => 'adesso';

  @override
  String timeMinutes(int n) {
    return '$n min';
  }

  @override
  String timeHours(int n) {
    return '$n h';
  }

  @override
  String timeHoursMinutes(int hours, int minutes) {
    return '$hours h $minutes min';
  }

  @override
  String timeDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n giorni',
      one: '1 giorno',
    );
    return '$_temp0';
  }

  @override
  String spanMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minuti',
      one: '1 minuto',
    );
    return '$_temp0';
  }

  @override
  String spanHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n ore',
      one: '1 ora',
    );
    return '$_temp0';
  }

  @override
  String spanDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n giorni',
      one: '1 giorno',
    );
    return '$_temp0';
  }

  @override
  String spanWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n settimane',
      one: '1 settimana',
    );
    return '$_temp0';
  }

  @override
  String get agoMoment => 'poco fa';

  @override
  String agoMinutes(int n) {
    return '$n min fa';
  }

  @override
  String agoHours(int n) {
    return '$n h fa';
  }

  @override
  String get agoYesterday => 'ieri';

  @override
  String agoDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n giorni fa',
    );
    return '$_temp0';
  }

  @override
  String agoWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n settimane fa',
      one: '1 settimana fa',
    );
    return '$_temp0';
  }

  @override
  String agoMonths(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n mesi fa',
      one: '1 mese fa',
    );
    return '$_temp0';
  }

  @override
  String get agoOverYear => 'più di un anno fa';

  @override
  String sizeBytes(int n) {
    return '$n B';
  }

  @override
  String sizeKB(String n) {
    return '$n KB';
  }

  @override
  String sizeMB(String n) {
    return '$n MB';
  }

  @override
  String sizeGB(String n) {
    return '$n GB';
  }

  @override
  String sizeTB(String n) {
    return '$n TB';
  }

  @override
  String listTwo(String first, String second) {
    return '$first e $second';
  }

  @override
  String listMore(String rest, String last) {
    return '$rest e $last';
  }

  @override
  String get listSeparator => ', ';

  @override
  String ordinal(int n) {
    return '$nº';
  }

  @override
  String get errLinkExpired => 'Il link è scaduto. Chiedine uno nuovo.';

  @override
  String get errGroupInvite => 'Questo è un invito a un gruppo.';

  @override
  String get errDeviceNotFound =>
      'L’altro dispositivo non è stato trovato. Controlla che siate entrambi online.';

  @override
  String get errNotFriendCode => 'Non sembra un link o un codice di Fogata.';

  @override
  String get errAlreadyInGroup => 'Sei già in quel gruppo.';

  @override
  String get errJoinRoomFirst => 'Prima entra nella stanza.';

  @override
  String get errNewerVersion =>
      'Il tuo profilo è stato salvato da una versione più recente di Fogata. Aggiorna Fogata per aprirlo.';

  @override
  String get errKeychainUnreachable =>
      'Fogata non è riuscito a ottenere la chiave del tuo profilo dal portachiavi di questo computer. Sblocca il portachiavi o consenti a Fogata di usarlo, poi riprova.';

  @override
  String get errKeychainNoKey =>
      'Il portachiavi di questo computer non contiene più la chiave del tuo profilo, quindi Fogata non può aprirlo.';

  @override
  String get errKeychainWrongKey =>
      'La chiave nel portachiavi di questo computer non apre il tuo profilo.';

  @override
  String get errNoGroupKey =>
      'Questo gruppo non è ancora pronto su questo computer. Riprova tra un attimo.';

  @override
  String get errAlreadyRunning =>
      'Fogata è già aperto con questo profilo. Chiudi prima l’altro.';

  @override
  String get errFileGone => 'Quel file non è più nella stanza.';

  @override
  String get errNobodyHasFile => 'Al momento nessuno online ha quel file.';

  @override
  String get errStillStarting => 'Fogata si sta ancora avviando.';

  @override
  String get errIdentityExists =>
      'Questo computer ha già un’identità; la sua chiave non può cambiare.';

  @override
  String get errNameRule => 'Scrivi un nome da 1 a 40 caratteri visibili.';

  @override
  String get errBioRule =>
      'La bio può avere al massimo 190 caratteri su 4 righe, senza caratteri nascosti.';

  @override
  String get errPictureRule =>
      'L’immagine deve essere un PNG o un JPEG di 64 KB al massimo.';

  @override
  String get errNotFriendLink => 'Questo non è un link di amicizia di Fogata.';

  @override
  String get errLinkDamaged => 'Il link è incompleto o danneggiato.';

  @override
  String get errLinkNeedsNewer =>
      'Quel link richiede una versione più recente di Fogata.';

  @override
  String get errCodeShape => 'Il codice ha il formato FGT-XXXXX-XXXXX.';

  @override
  String get errCodeWrong => 'Il codice è sbagliato o non funziona più.';

  @override
  String get errNotProven =>
      'L’altro dispositivo non è riuscito a dimostrare chi è.';

  @override
  String get errCallGone => 'Quella chiamata non sta più squillando.';

  @override
  String get errAlreadyInCall => 'Sei già in una chiamata. Prima riaggancia.';

  @override
  String get errAlreadyFriends => 'Siete già amici.';

  @override
  String get errMemberNotConnected =>
      'Questo membro non è connesso ora. Riprova quando sarà online.';

  @override
  String get errMemberTooOld =>
      'La versione di Fogata di questo membro è troppo vecchia per ricevere richieste di amicizia da un gruppo. Mandagli invece un link di amicizia.';

  @override
  String get errMemberAskLost =>
      'La richiesta di amicizia non ha raggiunto questo membro. Riprova.';

  @override
  String get errOwnLink =>
      'Questo è il tuo link: invialo all’amico che vuoi aggiungere.';

  @override
  String get errLinkUsed =>
      'Quel link è già stato usato o è scaduto. Chiedine uno nuovo.';

  @override
  String get errLinkExpiredUnreached =>
      'Il link è scaduto prima che il suo dispositivo fosse raggiunto. Chiedine uno nuovo.';

  @override
  String get errCodeUnanswered =>
      'Nessun dispositivo ha risposto al codice. Controllalo e verifica che l’altro dispositivo lo mostri ancora.';

  @override
  String get errRefused =>
      'L’altro dispositivo ha rifiutato. Forse il codice è già stato usato.';

  @override
  String get errNobodyLetIn =>
      'Nessuno ti ha fatto entrare. Riprova quando qualcuno del gruppo è online.';

  @override
  String get errNotBelow => 'Puoi farlo solo a chi sta sotto di te nel gruppo.';

  @override
  String get errNotManager => 'Solo chi gestisce il gruppo può farlo.';

  @override
  String get errLastManager => 'Non puoi toglierti dalla gestione del gruppo.';

  @override
  String get errRoomNotEmpty =>
      'Ci sono persone in quella stanza vocale: eliminala quando è vuota.';

  @override
  String get errTooManyFiles =>
      'Stai già condividendo il numero massimo di file consentito per persona in una stanza. Smetti prima di condividerne qualcuno.';

  @override
  String get errDiskFull => 'Il disco è pieno. Libera spazio e riprova.';

  @override
  String errDataFolder(String folder) {
    return 'Fogata non riesce a scrivere nella sua cartella su questo computer ($folder). Controlla di poterci scrivere e che il disco non sia pieno, poi riapri Fogata.';
  }

  @override
  String get errTooManyChanges =>
      'Questo dispositivo ha fatto troppe modifiche al gruppo che nessun altro membro ha ancora ricevuto. Riprova dopo che qualcun altro del gruppo avrà fatto una modifica.';

  @override
  String get errSomething => 'Qualcosa è andato storto.';

  @override
  String errRoomFull(int n) {
    return 'Quella stanza vocale è piena: ospita al massimo $n persone.';
  }

  @override
  String get findingOffline =>
      'Nessun peer della rete pubblica ha risposto: questo computer non sembra raggiungere internet, oppure un firewall blocca Fogata.';

  @override
  String get findingUdpWorks =>
      'UDP funziona. Serve per le chiamate e per attraversare i router.';

  @override
  String get findingUdpBlocked =>
      'Questa rete o questo computer sembra bloccare UDP (peer che hanno risposto via TCP non l’hanno fatto via QUIC). La chat può ancora passare tramite altri membri, ma le connessioni dirette attraverso i router e le chiamate falliranno quasi sempre. Controlla il firewall o le regole della rete.';

  @override
  String get findingCarrierNat =>
      'Questo computer ha un indirizzo NAT di tipo carrier-grade (100.64.0.0/10): il provider condivide un unico indirizzo pubblico tra molti clienti.';

  @override
  String get findingPublicIpv4 =>
      'Questo computer ha un indirizzo IPv4 pubblico: gli altri possono raggiungerlo direttamente.';

  @override
  String get findingHardNat =>
      'Dietro un router che assegna una porta pubblica diversa per ogni destinazione (un NAT difficile).';

  @override
  String get findingEasyNat =>
      'Dietro un router che mantiene la stessa porta pubblica per ogni destinazione (un NAT facile): si può aprire da entrambi i lati contemporaneamente.';

  @override
  String get findingUnknownRouter =>
      'Troppo pochi peer hanno visto l’indirizzo pubblico di questo computer per capire che tipo di router c’è.';

  @override
  String get findingNoIpv6 =>
      'Nessun indirizzo IPv6 pubblico. Fogata non ne ha bisogno, ma rende più facili le connessioni dirette.';

  @override
  String get findingIpv6Works =>
      'L’IPv6 pubblico funziona: gli amici con IPv6 si connettono senza router di mezzo.';

  @override
  String get findingIpv6Unreached =>
      'Questo computer ha un indirizzo IPv6 pubblico, ma nessun peer lo ha ancora raggiunto tramite IPv6.';

  @override
  String get findingSummaryNoUdp =>
      'In breve: aspettati problemi con le connessioni dirette finché UDP non è consentito.';

  @override
  String get findingSummaryPublic =>
      'In breve: i tuoi amici dovrebbero riuscire a raggiungere questo computer direttamente.';

  @override
  String get findingSummaryEasyNat =>
      'In breve: le connessioni dirette dovrebbero funzionare con la maggior parte dei tuoi amici, aprendo entrambi i router contemporaneamente; con un amico dietro un NAT difficile serve un membro del gruppo nel mezzo.';

  @override
  String get findingSummaryHardNatWithIpv6 =>
      'In breve: gli amici con IPv6 o con un indirizzo pubblico si connettono direttamente; con gli altri serve un membro del gruppo nel mezzo.';

  @override
  String get findingSummaryHardNat =>
      'In breve: solo gli amici con un indirizzo pubblico si connettono direttamente; con gli altri serve un membro del gruppo nel mezzo.';

  @override
  String get findingSummaryUnknown =>
      'In breve: informazioni insufficienti; ricontrolla tra un minuto.';

  @override
  String findingInternet(int n) {
    return 'Internet: questo computer raggiunge la rete pubblica ($n peer conosciuti).';
  }

  @override
  String findingDns(String detail) {
    return 'DNS: non è stato possibile leggere per intero le impostazioni di sistema ($detail)';
  }

  @override
  String findingLanDevices(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n dispositivi Fogata',
      one: '1 dispositivo Fogata',
    );
    return '$_temp0 su questa rete locale: gli amici qui si connettono senza passare da internet.';
  }

  @override
  String findingFirewall(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'in PowerShell come amministratore',
      'macos': 'nel Terminale',
      'other': 'in un terminale',
    });
    return 'Il firewall di questo computer non lascia entrare nulla per Fogata, quindi un amico nello stesso posto dietro un altro router (due router in casa, per esempio) non può raggiungerlo. Esegui questo comando una volta $_temp0 per far entrare Fogata:';
  }

  @override
  String get settingsLanguage => 'Lingua';

  @override
  String get settingsLanguageNote =>
      'La lingua dei testi di Fogata. I messaggi restano come sono stati scritti.';

  @override
  String get settingsLanguageChoose => 'Scegli la lingua';

  @override
  String get settingsTime => 'Ora';

  @override
  String get settingsTimeNote =>
      'Come appaiono gli orari nella chat, nelle chiamate e negli elenchi.';

  @override
  String settingsTime24(String example) {
    return '24 ore · $example';
  }

  @override
  String settingsTime12(String example) {
    return '12 ore · $example';
  }

  @override
  String firewallNoticeText(String system, String command) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'in PowerShell come amministratore',
      'macos': 'nel Terminale',
      'other': 'in un terminale',
    });
    return 'Un amico nello stesso posto dietro un altro router non può raggiungere questo computer. Esegui una volta $_temp0: $command';
  }

  @override
  String firewallCopied(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'in PowerShell come amministratore',
      'macos': 'nel Terminale',
      'other': 'in un terminale',
    });
    return 'Copiato. Incollalo $_temp0 ed eseguilo.';
  }

  @override
  String get appRestarted => 'Fogata ha smesso di funzionare e si è riavviato.';

  @override
  String appVoiceRoomInGroup(String room, String group) {
    return '$room in $group';
  }

  @override
  String appBothIn(String room) {
    return 'Siete entrambi in $room.';
  }

  @override
  String appHungUp(String name) {
    return '$name ha riagganciato';
  }

  @override
  String get appYourFriend => 'L’altra persona';

  @override
  String get appCallEndedAlone =>
      'Nessuno è tornato, quindi la chiamata è terminata';

  @override
  String get appAnsweredElsewhere => 'Chiamata accettata su un altro computer';

  @override
  String appVoiceRoomDeleted(String room) {
    return '“$room” è stata eliminata.';
  }

  @override
  String get appSomeone => 'Qualcuno';

  @override
  String get appAGroup => 'un gruppo';

  @override
  String appFriendRequest(String name) {
    return '$name vuole aggiungerti agli amici.';
  }

  @override
  String appRequestDelivered(String name) {
    return 'Richiesta consegnata a $name. Attende la sua risposta.';
  }

  @override
  String appFriendsNow(String name) {
    return 'Ora tu e $name siete amici.';
  }

  @override
  String appJoinedGroup(String group) {
    return 'Ora sei in “$group”.';
  }

  @override
  String appRemovedFromGroup(String group) {
    return 'Non sei più in “$group”.';
  }

  @override
  String appAddedToGroup(String name, String group) {
    return '$name ti ha aggiunto a “$group”. Unirti o no: decidi tu.';
  }

  @override
  String appAsksToJoin(String name, String group) {
    return '$name chiede di unirsi a “$group”.';
  }

  @override
  String appJoinWaiting(String group) {
    return 'In attesa che qualcuno in “$group” ti faccia entrare.';
  }

  @override
  String appJoinDeclined(String group) {
    return 'Qualcuno in “$group” ha rifiutato. Quell’invito non funziona più.';
  }

  @override
  String appDownloadedTo(String path) {
    return 'Scaricato in $path';
  }

  @override
  String appDownloadFailed(String why) {
    return 'Impossibile scaricare: $why';
  }

  @override
  String get appOtherSession =>
      'Questa identità è aperta anche su un altro computer. Chiudi Fogata lì: un’identità funziona su un solo computer alla volta.';

  @override
  String get appFriendsRestored =>
      'Il tuo elenco di amici si era danneggiato su questo computer ed è stato ripristinato dalla copia fatta prima dell’ultima modifica. Un amico aggiunto o rimosso poco prima potrebbe dover essere aggiunto o rimosso di nuovo.';

  @override
  String appFilesRefused(String folder) {
    return 'Alcuni file di Fogata su questo computer erano non cifrati dove vengono conservati cifrati, quindi non sono stati usati. Non è stato eliminato nulla: sono conservati in $folder.';
  }

  @override
  String appInvitationRanOut(String group) {
    return 'L’invito a “$group” è scaduto prima che qualcuno del gruppo ti facesse entrare. Chiedine uno nuovo.';
  }

  @override
  String appTakenOut(String who, String room) {
    return '$who ti ha fatto uscire da $room.';
  }

  @override
  String appRoomForSome(String room) {
    return '$room ora è solo per alcuni membri.';
  }

  @override
  String appTakenOutUntilLetBack(String who, String room) {
    return '$who ti ha fatto uscire da $room finché qualcuno sopra di te non ti farà rientrare.';
  }

  @override
  String appTakenOutFor(String who, String room, String span) {
    return '$who ti ha fatto uscire da $room per $span.';
  }

  @override
  String appTakenOutWhile(String who, String room) {
    return '$who ti ha fatto uscire da $room per un po’.';
  }

  @override
  String appComeBackIn(String room, String wait) {
    return 'Potrai tornare in $room tra $wait.';
  }

  @override
  String appRoomNotForYou(String room) {
    return '$room ora non è per te.';
  }

  @override
  String appFoldersNotShared(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Non si possono condividere cartelle, solo file: $n cartelle sono state escluse.',
      one: 'Non si possono condividere cartelle, solo file: la cartella è stata esclusa.',
    );
    return '$_temp0';
  }

  @override
  String appSharingOne(String file) {
    return 'Stai condividendo $file da questo computer. Non sarà più disponibile quando esci dalla stanza.';
  }

  @override
  String appSharingMany(int n) {
    return 'Stai condividendo $n file da questo computer. Non saranno più disponibili quando esci dalla stanza.';
  }

  @override
  String appSavedTo(String path) {
    return 'Salvato in $path';
  }

  @override
  String appAskingToJoin(String group) {
    return 'Richiesta di unirti a “$group” inviata. Qualcuno del gruppo ti farà entrare quando la vedrà.';
  }

  @override
  String get appLookingForDevice =>
      'Ricerca dell’altro dispositivo per inviare la tua richiesta…';

  @override
  String callStateNoAnswer(String name) {
    return '$name non ha risposto';
  }

  @override
  String callStateDeclined(String name) {
    return '$name non può parlare ora';
  }

  @override
  String callStateBusy(String name) {
    return '$name è in un’altra chiamata';
  }

  @override
  String callStateOutdated(String name) {
    return '$name deve aggiornare Fogata per partecipare';
  }

  @override
  String callStateUnreached(String name) {
    return 'Impossibile raggiungere $name';
  }

  @override
  String get callStateReturn => 'Torna alla chiamata';

  @override
  String get callStateCallBack => 'Richiama';

  @override
  String get callStateCall => 'Chiama';

  @override
  String noticesDeclined(String name, String group) {
    return '$name ha rifiutato l’invito a “$group”';
  }

  @override
  String noticesDeclinedToast(String name, String group) {
    return '$name ha rifiutato l’invito a “$group”.';
  }

  @override
  String noticesMissedInCall(String name) {
    return '$name ha chiamato mentre eri in una chiamata';
  }

  @override
  String noticesMissedOffline(String name) {
    return '$name ha chiamato mentre apparivi offline';
  }

  @override
  String noticesMissedCall(String name) {
    return 'Chiamata persa da $name';
  }

  @override
  String get contactsGeneral => 'Generale';

  @override
  String get directStateTimerOff => 'Off';

  @override
  String get directStateTimer24h => '24 ore';

  @override
  String directStateTimerDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n giorni',
      one: '1 giorno',
    );
    return '$_temp0';
  }

  @override
  String directStateDisappearsMinutes(int n) {
    return 'Scompare tra $n min';
  }

  @override
  String directStateDisappearsHours(int n) {
    return 'Scompare tra $n h';
  }

  @override
  String directStateDisappearsDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Scompare tra $n giorni',
    );
    return '$_temp0';
  }

  @override
  String get directStateNoLimit => 'Nessun limite';

  @override
  String get directStateMessageGone => 'Quel messaggio non c’è più.';

  @override
  String directStateFoldersNotSent(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Non si possono inviare cartelle, solo file: $n cartelle sono state escluse.',
      one: 'Non si possono inviare cartelle, solo file: la cartella è stata esclusa.',
    );
    return '$_temp0';
  }

  @override
  String directStateFileFailed(String why) {
    return 'Impossibile ottenere il file: $why';
  }

  @override
  String movingTooLarge(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'I banner animati possono misurare al massimo $most pixel; questo misura $size.',
      'other':
          'Le immagini animate possono misurare al massimo $most pixel; questa misura $size.',
    });
    return '$_temp0';
  }

  @override
  String movingTooLong(String what, String most, String count) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'I banner animati possono avere al massimo $most fotogrammi; questo ne ha $count.',
      'other':
          'Le immagini animate possono avere al massimo $most fotogrammi; questa ne ha $count.',
    });
    return '$_temp0';
  }

  @override
  String movingTooHeavy(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'I banner animati possono pesare al massimo $most; questo pesa $size.',
      'other':
          'Le immagini animate possono pesare al massimo $most; questa pesa $size.',
    });
    return '$_temp0';
  }

  @override
  String get namesHidden =>
      'Questo nome contiene caratteri nascosti. Riscrivilo senza.';

  @override
  String get namesBlank =>
      'In questo nome non si vede nulla. Usa lettere, numeri o emoji.';

  @override
  String get namesDefaultBio => 'Ciao! Sono su Fogata.';

  @override
  String get namesBioHidden =>
      'Questa bio contiene caratteri nascosti. Riscrivila senza.';

  @override
  String namesBioLines(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'La bio può avere al massimo $n righe.',
      one: 'La bio può avere al massimo 1 riga.',
    );
    return '$_temp0';
  }

  @override
  String namesBioChars(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'La bio può avere al massimo $n caratteri.',
      one: 'La bio può avere al massimo 1 carattere.',
    );
    return '$_temp0';
  }

  @override
  String get restartFailed =>
      'Fogata non è riuscito a riavviarsi. Chiudilo e riaprilo tu.';

  @override
  String typingOne(String name) {
    return '$name sta scrivendo…';
  }

  @override
  String typingTwo(String first, String second) {
    return '$first e $second stanno scrivendo…';
  }

  @override
  String get typingSeveral => 'Diverse persone stanno scrivendo…';

  @override
  String get outgoingTheRoom => 'la stanza';

  @override
  String outgoingDropTitle(String group) {
    return 'Rilascia per condividere in $group';
  }

  @override
  String get outgoingDropNote =>
      'Puoi aggiungere un messaggio e controllarli prima che partano. Gli altri li scaricano da questo computer mentre sei nella stanza.';

  @override
  String outgoingPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n file nascosti o privati sono stati lasciati fuori: file come questi non si aggiungono trascinandoli, perché una pagina web può avviare un trascinamento. Per condividerli, sceglili con la graffetta.',
      one: 'Un file nascosto o privato è stato lasciato fuori: file come questo non si aggiungono trascinandoli, perché una pagina web può avviare un trascinamento. Per condividerlo, sceglilo con la graffetta.',
    );
    return '$_temp0';
  }

  @override
  String outgoingToShare(int n, String count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$count file da condividere · $size',
      one: '$count file da condividere · $size',
    );
    return '$_temp0';
  }

  @override
  String get outgoingRemoveAll => 'Rimuovi tutti';

  @override
  String outgoingRemoveFile(String file) {
    return 'Rimuovi $file';
  }

  @override
  String get windowTrayOpen => 'Apri Fogata';

  @override
  String get windowTrayQuit => 'Esci da Fogata';

  @override
  String get windowMinimize => 'Riduci a icona';

  @override
  String get windowRestore => 'Ripristina';

  @override
  String get windowMaximize => 'Ingrandisci';

  @override
  String get autostartReason => 'Apri Fogata quando accedi.';

  @override
  String get devicesMicrophone => 'Microfono';

  @override
  String get devicesSpeaker => 'Altoparlante';

  @override
  String get devicesCamera => 'Videocamera';

  @override
  String get devicesChooseMicrophone => 'Scegli il microfono';

  @override
  String get devicesChooseSpeaker => 'Scegli l’altoparlante';

  @override
  String get devicesChooseCamera => 'Scegli la videocamera';

  @override
  String get devicesNoCamera => 'Nessuna videocamera collegata.';

  @override
  String get devicesCameraInCall =>
      'La videocamera è accesa nella tua chiamata.';

  @override
  String devicesCameraFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Impossibile usare la videocamera. Consenti l’accesso in Impostazioni di Sistema › Privacy e sicurezza.',
      'other': 'Impossibile usare la videocamera.',
    });
    return '$_temp0';
  }

  @override
  String get devicesCameraDenied =>
      'Il sistema non consente a Fogata di usare la videocamera. Consentilo nelle impostazioni di sistema, poi riprova.';

  @override
  String get devicesCameraUnavailable =>
      'Impossibile raggiungere la videocamera: questo sistema non ha un portale della videocamera.';

  @override
  String get devicesPreviewOnlyYou => 'Solo tu vedi l’anteprima.';

  @override
  String get devicesStopVideo => 'Ferma il video';

  @override
  String get devicesTestVideo => 'Prova il video';

  @override
  String get devicesNoiseOn => 'Riduzione del rumore attiva';

  @override
  String get devicesNoiseOff => 'Riduzione del rumore disattivata';

  @override
  String get devicesNoise => 'Riduzione del rumore';

  @override
  String get devicesNoiseNote =>
      'Abbassa il rumore di fondo costante, come una ventola o un ronzio, in ciò che sentono gli altri.';

  @override
  String get devicesNoiseShort => 'Abbassa ventole o ronzii costanti.';

  @override
  String get devicesMirror => 'Specchia la mia videocamera';

  @override
  String get devicesMirrorNote =>
      'Cambia solo come ti vedi tu qui. Gli altri vedono sempre la tua videocamera così com’è.';

  @override
  String get devicesMirrorOn => 'Videocamera specchiata attiva';

  @override
  String get devicesMirrorOff => 'Videocamera specchiata disattivata';

  @override
  String get devicesTestMic => 'Prova il microfono';

  @override
  String get devicesStopMicTest => 'Ferma la prova';

  @override
  String get devicesMicLevel => 'Livello del microfono';

  @override
  String get devicesHearMyself => 'Ascolta la tua voce';

  @override
  String get devicesHearMyselfNote =>
      'Riproduce il tuo microfono dall’altoparlante. Usa le cuffie, altrimenti l’altoparlante può rientrare nel microfono.';

  @override
  String get devicesHearMyselfOn => 'Ascolta la tua voce: attivo';

  @override
  String get devicesHearMyselfOff => 'Ascolta la tua voce: disattivato';

  @override
  String get devicesMicTestOnlyYou => 'La prova non viene inviata a nessuno.';

  @override
  String get devicesMicInCall => 'Il microfono è in uso nella tua chiamata.';

  @override
  String get devicesNoMic => 'Nessun microfono collegato.';

  @override
  String get devicesSystemDefault => 'Predefinito di sistema';

  @override
  String get devicesNotConnected => 'Non collegato: si usa quello predefinito';

  @override
  String get devicesNoCameraShort => 'Nessuna videocamera collegata';

  @override
  String get devicesMicOptions => 'Opzioni del microfono';

  @override
  String get devicesSpeakerOptions => 'Opzioni dell’altoparlante';

  @override
  String get devicesCameraOptions => 'Opzioni della videocamera';

  @override
  String get devicesInputDevice => 'Dispositivo di ingresso';

  @override
  String get devicesOutputDevice => 'Dispositivo di uscita';

  @override
  String get devicesChosenMissing =>
      'Quello scelto non è collegato: si usa quello predefinito.';

  @override
  String get devicesVoiceSettings => 'Impostazioni voce e video';

  @override
  String get photoImages => 'Immagini';

  @override
  String photoTooLarge(int n) {
    return 'Questa immagine è troppo grande: al massimo $n megapixel.';
  }

  @override
  String get photoUnreadable => 'Impossibile leggere questa immagine.';

  @override
  String get callSource => 'Originale';

  @override
  String callQuality(String resolution, int fps) {
    return '$resolution · $fps fps';
  }

  @override
  String get callSetupFailed => 'Impossibile avviare la chiamata.';

  @override
  String get callNoSoundSystem =>
      'Impossibile raggiungere il sistema audio di questo computer, quindi la chiamata non può iniziare.';

  @override
  String callMicGone(String name) {
    return 'Il tuo microfono è stato scollegato: ora si usa $name.';
  }

  @override
  String get callMicBack => 'Il tuo microfono è di nuovo collegato.';

  @override
  String callMicSilent(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Impossibile usare il microfono. Controlla che sia collegato e consentito in Impostazioni di Sistema › Privacy e sicurezza.',
      'windows': 'Impossibile usare il microfono. Controlla che sia collegato e consentito in Impostazioni › Privacy e sicurezza › Microfono.',
      'other': 'Impossibile usare il microfono. Controlla che sia collegato.',
    });
    return '$_temp0';
  }

  @override
  String get callNoMicrophone => 'Nessun microfono collegato.';

  @override
  String callMicFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Impossibile usare il microfono. Consenti l’accesso in Impostazioni di Sistema › Privacy e sicurezza.',
      'windows': 'Impossibile usare il microfono. Consenti l’accesso in Impostazioni › Privacy e sicurezza › Microfono.',
      'other': 'Impossibile usare il microfono.',
    });
    return '$_temp0';
  }

  @override
  String callScreenFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Impossibile condividere lo schermo. Consenti l’accesso in Impostazioni di Sistema › Privacy e sicurezza.',
      'other': 'Impossibile condividere lo schermo.',
    });
    return '$_temp0';
  }

  @override
  String get callShareLost =>
      'Condivisione interrotta: ciò che stavi condividendo non può più essere acquisito.';

  @override
  String callKeptOut(String name) {
    return 'Connessione con $name persa.';
  }

  @override
  String get qualityGood => 'Buona';

  @override
  String get qualityOkay => 'Discreta';

  @override
  String get qualityPoor => 'Scarsa';

  @override
  String get qualityLost => 'Persa';

  @override
  String qualityDelay(int ms) {
    return '$ms ms';
  }

  @override
  String qualityPercent(String value) {
    return '$value%';
  }

  @override
  String get qualityAnotherMember => 'un altro membro';

  @override
  String qualityLostHint(String name, int seconds) {
    return 'Non arriva nulla da $name da $seconds s. Fogata continua a cercare la connessione, che tornerà da sola appena la sua rete funzionerà di nuovo.';
  }

  @override
  String qualityBothPoor(String name) {
    return 'Entrambe le reti sono scarse in questo momento: tu e $name potreste sentirvi a scatti.';
  }

  @override
  String qualityTheirSide(String name) {
    return 'Il problema è dalla parte di $name: tutti sentono la sua voce a scatti. La sua connessione Wi-Fi o mobile ora è scarsa.';
  }

  @override
  String qualityYourSideWith(String name) {
    return 'Il problema è dalla tua parte: $name e gli altri potrebbero sentirti a scatti. La tua connessione Wi-Fi o mobile ora è scarsa.';
  }

  @override
  String qualityTheirsSlow(String name) {
    return 'La rete di $name ora è un po’ lenta: tutti potrebbero sentire la sua voce a scatti ogni tanto.';
  }

  @override
  String qualityAllGoodWith(String name) {
    return 'Tu e $name vi sentite senza problemi.';
  }

  @override
  String qualityMineSlowWith(String name) {
    return 'La tua rete ora è un po’ lenta: $name potrebbe sentirti a scatti ogni tanto.';
  }

  @override
  String qualityWayThrough(String name, String carrier) {
    return 'La rete di $name va bene; il collegamento tra voi due è più lento perché passa da $carrier. Potresti sentire la sua voce a scatti ogni tanto; gli altri la sentono bene.';
  }

  @override
  String qualityWaySlowFine(String name) {
    return 'La rete di $name va bene; il collegamento tra voi due ora è lento. Potresti sentire la sua voce a scatti ogni tanto; gli altri la sentono bene.';
  }

  @override
  String qualityWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'Il collegamento tra te e $name ora è un po’ lento; può dipendere da una delle due reti. Potreste sentirvi a scatti ogni tanto.',
      'other':
          'Il collegamento tra te e $name ora è scarso; può dipendere da una delle due reti. Potreste sentirvi a scatti.',
    });
    return '$_temp0';
  }

  @override
  String qualityWayVia(String level, String name, String carrier) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'Il collegamento tra te e $name ora è un po’ lento; può dipendere da una delle due reti. Potreste sentirvi a scatti ogni tanto. Passa da $carrier.',
      'other':
          'Il collegamento tra te e $name ora è scarso; può dipendere da una delle due reti. Potreste sentirvi a scatti. Passa da $carrier.',
    });
    return '$_temp0';
  }

  @override
  String get qualityYourSide =>
      'Il problema è dalla tua parte: gli altri potrebbero sentirti a scatti. La tua connessione Wi-Fi o mobile ora è scarsa.';

  @override
  String get qualityMineSlow =>
      'La tua rete ora è un po’ lenta: gli altri potrebbero sentirti a scatti ogni tanto.';

  @override
  String qualityLostShort(String name, int seconds) {
    return 'Non arriva nulla da $name da $seconds s. Fogata continua a cercare la connessione.';
  }

  @override
  String qualityAllWellWith(String name) {
    return 'Tutto bene: tu e $name vi sentite senza problemi.';
  }

  @override
  String qualityNoteDropped(String name) {
    return 'la connessione con $name è caduta e Fogata la sta ricercando';
  }

  @override
  String qualityNoteTheirsPoor(String name) {
    return 'la rete di $name è scarsa, quindi tutti sentono la sua voce a scatti';
  }

  @override
  String qualityNoteTheirsSlow(String name) {
    return 'la rete di $name è un po’ lenta';
  }

  @override
  String qualityNoteThrough(String name, String carrier) {
    return '$name ti arriva tramite $carrier';
  }

  @override
  String qualityNoteWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': 'il collegamento con $name ora è un po’ lento',
      'other': 'il collegamento con $name ora è scarso',
    });
    return '$_temp0';
  }

  @override
  String get qualityNoteSeparator => '; ';

  @override
  String get qualityAllWell => 'Tutto bene: senti tutti e tutti ti sentono.';

  @override
  String qualityAllWellMine(String notes) {
    return 'Dalla tua parte va tutto bene. $notes.';
  }

  @override
  String qualityYourDetails(String level) {
    return 'La tua connessione: $level. Mostra i dettagli';
  }

  @override
  String qualityTheirDetails(String name, String level) {
    return 'Connessione di $name: $level. Mostra i dettagli';
  }

  @override
  String get qualityReconnecting => 'Riconnessione…';

  @override
  String get qualityReconnectingLabel => 'Riconnessione';

  @override
  String qualityLevelConnection(String level) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'good': 'Connessione buona',
      'okay': 'Connessione discreta',
      'weak': 'Connessione scarsa',
      'other': 'Connessione persa',
    });
    return '$_temp0';
  }

  @override
  String get qualityPoorConnection => 'Connessione scarsa';

  @override
  String get qualityMinePoorBanner =>
      'La tua rete è scarsa: gli altri potrebbero sentirti a scatti';

  @override
  String get qualityYourNetwork => 'La tua rete';

  @override
  String qualityTo(String name) {
    return 'Verso $name';
  }

  @override
  String qualityBetween(String name) {
    return 'Tra te e $name';
  }

  @override
  String qualityTheirNetwork(String name) {
    return 'Rete di $name';
  }

  @override
  String get qualityLastHeard => 'Ultimo segnale';

  @override
  String qualitySecondsAgo(int n) {
    return '$n s fa';
  }

  @override
  String get qualityPath => 'Percorso';

  @override
  String get qualityLookingAgain => 'Nuova ricerca in corso';

  @override
  String get qualityDelayLabel => 'Latenza';

  @override
  String get qualityLostPackets => 'Pacchetti persi';

  @override
  String get qualityYourConnection => 'La tua connessione';

  @override
  String qualityTheirConnection(String name) {
    return 'Connessione di $name';
  }

  @override
  String roomCountFullLabel(int count, int max) {
    return 'Piena, $count di $max persone';
  }

  @override
  String roomCountLabel(int count, int max) {
    return '$count di $max persone';
  }

  @override
  String get roomCountFullPrefix => 'Piena · ';

  @override
  String get roomCountFullWord => 'Piena ';

  @override
  String get volumeVoice => 'Volume della voce';

  @override
  String get volumeOnlyYouHear => 'Solo tu senti queste modifiche';

  @override
  String get volumeScreenSound => 'Audio dello schermo';

  @override
  String volumePercent(int n) {
    return '$n%';
  }

  @override
  String get volumeMuteVoiceForMe => 'Silenzia la voce per me';

  @override
  String get volumeMuteForMe => 'Silenzia per me';

  @override
  String get volumeMuteScreenForMe => 'Silenzia l’audio dello schermo per me';

  @override
  String get volumeBackTo100 => 'Torna al 100%';

  @override
  String get volumeBelowYou => 'SOTTO DI TE';

  @override
  String volumeRemoveFrom(String room) {
    return 'Fai uscire da $room';
  }

  @override
  String volumeRemoveTitle(String name, String room) {
    return 'Far uscire $name da $room?';
  }

  @override
  String volumeRemoveText(String name, String room) {
    return '$name esce subito da $room e può rientrare quando vuole. Una sospensione tiene fuori qualcuno per un po’.';
  }

  @override
  String volumeTimeOutOf(String room) {
    return 'Sospendi da $room';
  }

  @override
  String volumeTimeOutText(String name, String room) {
    return '$name esce da $room e non può rientrare per:';
  }

  @override
  String get volumeUntilLetBack =>
      'Finché qualcuno più in alto non revoca la sospensione';

  @override
  String get pipYourScreen => 'Il tuo schermo';

  @override
  String pipScreenOf(String name) {
    return 'Schermo di $name';
  }

  @override
  String get pipYourCamera => 'La tua videocamera';

  @override
  String get pipBackToCall => 'Torna alla chiamata';

  @override
  String get pipBackToRoom => 'Torna alla stanza';

  @override
  String get pipStopWatching => 'Smetti di guardare';

  @override
  String get addToCallTitle => 'Aggiungi alla chiamata';

  @override
  String addToCallNote(int n) {
    return 'Amici online. Fino a $n persone in una chiamata.';
  }

  @override
  String get addToCallSearch => 'Cerca amici';

  @override
  String get addToCallNoFriends => 'Ancora nessun amico.';

  @override
  String get addToCallNoMatch => 'Nessun amico con quel nome.';

  @override
  String get addToCallInCall => 'In chiamata';

  @override
  String get addToCallCalling => 'Squilla…';

  @override
  String get addToCallOffline => 'Offline';

  @override
  String get addToCallCall => 'Chiama';

  @override
  String get streamQualityTitle => 'Qualità della trasmissione';

  @override
  String get streamQualityNoteSharing =>
      'Come il tuo schermo arriva agli altri. Le modifiche valgono subito.';

  @override
  String get streamQualityNote =>
      'Come il tuo schermo arriva agli altri quando lo condividi.';

  @override
  String get streamQualityResolution => 'Risoluzione';

  @override
  String streamQualitySourceLines(int lines) {
    return 'Originale (${lines}p)';
  }

  @override
  String get streamQualitySource => 'Originale';

  @override
  String get streamQualityFrameRate => 'Frequenza fotogrammi';

  @override
  String streamQualityFps(int n) {
    return '$n fps';
  }

  @override
  String get streamQualityHigherNote =>
      'Una qualità più alta consuma più upload e più risorse del computer.';

  @override
  String get streamQualitySomeone => 'Qualcuno';

  @override
  String streamQualityLoweredOne(String name, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Gli altri ricevono quello che hai scelto.',
      'other': '',
    });
    return 'La connessione di $name ora non regge di più, quindi riceve un’immagine più piccola.$_temp0';
  }

  @override
  String streamQualityLoweredMany(String names, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Gli altri ricevono quello che hai scelto.',
      'other': '',
    });
    return 'Alcune connessioni ora non reggono di più: $names ricevono un’immagine più piccola.$_temp0';
  }

  @override
  String get streamQualitySpared =>
      'Per non sovraccaricare il computer, ora tutti quelli che guardano ricevono un’immagine più piccola o meno fluida.';

  @override
  String get streamQualityGoingOut => 'IN USCITA ORA';

  @override
  String get streamQualityNobody => 'Nessuno sta ancora guardando.';

  @override
  String streamQualityNow(int height, int fps) {
    return '${height}p · $fps fps';
  }

  @override
  String callLayersIsCalling(String name) {
    return '$name ti sta chiamando';
  }

  @override
  String get callLayersGoToCall => 'F6 porta alla chiamata';

  @override
  String callLayersWith(String names) {
    return 'Con $names';
  }

  @override
  String get callLayersDirectCall => 'Chiamata diretta · ';

  @override
  String get callLayersAnswerMicOff => 'Rispondi con il microfono disattivato';

  @override
  String callLayersAnsweringLeaves(String room) {
    return 'Rispondendo esci da $room';
  }

  @override
  String get callLayersLeaveAndAnswer => 'Esci e rispondi';

  @override
  String callLayersBothIn(String room) {
    return 'Siete entrambi in $room.';
  }

  @override
  String get callLayersDismiss => 'Ignora';

  @override
  String callLayersCallWith(String names) {
    return 'Chiamata con $names';
  }

  @override
  String get callLayersRoom => 'Stanza';

  @override
  String callLayersBackTo(String title) {
    return 'Torna a $title';
  }

  @override
  String callLayersRinging(String time) {
    return 'Squilla · $time';
  }

  @override
  String callLayersInCall(String time) {
    return 'In chiamata · $time';
  }

  @override
  String callLayersOnlyYou(String time) {
    return 'Solo tu · $time';
  }

  @override
  String callLayersInRoom(int n, String time) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n nella stanza',
    );
    return '$_temp0 · $time';
  }

  @override
  String get callLayersMute => 'Disattiva il microfono';

  @override
  String get callLayersUnmute => 'Attiva il microfono';

  @override
  String get callLayersHangUp => 'Riaggancia';

  @override
  String get callLayersLeaveRoom => 'Esci dalla stanza';

  @override
  String roomDeclineCallTitle(String name) {
    return 'Rifiutare la chiamata di $name?';
  }

  @override
  String roomDeclineCallText(String caller, String callee) {
    return '$caller ti sta chiamando. Puoi essere in una sola chiamata alla volta: se chiami $callee, la rifiuti.';
  }

  @override
  String roomDeclineCallTextSomeone(String caller) {
    return '$caller ti sta chiamando. Puoi essere in una sola chiamata alla volta: se chiami qualcun altro, la rifiuti.';
  }

  @override
  String get roomDeclineAndCall => 'Rifiuta e chiama';

  @override
  String roomHangUpTitle(String name) {
    return 'Terminare la chiamata con $name?';
  }

  @override
  String get roomHangUpText =>
      'Puoi essere in una sola chiamata o stanza alla volta: continuando, la chiamata termina.';

  @override
  String get roomHangUp => 'Riaggancia';

  @override
  String get roomCloseTitle => 'Uscire e chiudere la stanza?';

  @override
  String get roomCloseText =>
      'Sei l’ultima persona qui. Quando esci, la stanza si chiude e ciò che è stato detto sparisce da questo computer; ciò che gli altri hanno scaricato resta sui loro.';

  @override
  String get roomCloseChatMessages => 'Messaggi della chat';

  @override
  String get roomCloseFilesShared => 'File che hai condiviso';

  @override
  String get roomStay => 'Resta';

  @override
  String get roomLeaveAndClose => 'Esci e chiudi';

  @override
  String get roomLivePrefix => 'In diretta · ';

  @override
  String get roomPillConnecting => 'Connessione…';

  @override
  String get roomPillOnlyYou => 'Solo tu';

  @override
  String get roomPillError => 'Errore';

  @override
  String get roomLastOne => 'Sei l’ultima persona nella stanza.';

  @override
  String get roomOnlyYouIn => 'Nella stanza ci sei solo tu.';

  @override
  String get roomAloneWarning =>
      ' Quando esci, si chiude e tutto ciò che contiene sparisce.';

  @override
  String get roomEarlierPeople => 'Persone precedenti';

  @override
  String get roomMorePeople => 'Altre persone';

  @override
  String roomTileYou(String name) {
    return '$name (tu)';
  }

  @override
  String roomTileNoSound(String name) {
    return '$name · senza audio';
  }

  @override
  String roomTileConnecting(String name) {
    return '$name · in connessione';
  }

  @override
  String roomTileAway(String name) {
    return '$name · senza connessione';
  }

  @override
  String roomTileMicOff(String name) {
    return '$name, microfono disattivato';
  }

  @override
  String roomTileMutedByYou(String said) {
    return '$said, silenziato per te';
  }

  @override
  String roomTileVolume(String said, String percent) {
    return '$said, volume $percent';
  }

  @override
  String roomTileLeft(String name) {
    return '$name ha lasciato la chiamata · può tornare';
  }

  @override
  String roomIsSharing(String name) {
    return '$name sta condividendo lo schermo';
  }

  @override
  String get roomNothingSentUntilWatch =>
      'Non ti arriva nulla finché non guardi';

  @override
  String get roomWatch => 'Guarda';

  @override
  String get roomLive => 'LIVE';

  @override
  String get roomSharingYourScreen => 'Stai condividendo il tuo schermo';

  @override
  String get roomShowPreview => 'Mostra anteprima';

  @override
  String get roomPreviewMirror =>
      'Mostrarla mentre condividi Fogata stesso crea uno specchio infinito';

  @override
  String get roomNobodyWatching => 'Nessuno sta guardando';

  @override
  String roomPeopleWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n persone stanno guardando',
      one: '1 persona sta guardando',
    );
    return '$_temp0';
  }

  @override
  String get roomExitFullScreen => 'Esci da schermo intero';

  @override
  String get roomFullScreen => 'Schermo intero';

  @override
  String get roomPlayScreenSound => 'Riproduci l’audio dello schermo';

  @override
  String get roomMuteScreenSound => 'Silenzia l’audio dello schermo';

  @override
  String get roomQualityPoor => 'Scarsa';

  @override
  String get roomQualityOkay => 'Discreta';

  @override
  String get roomQualityGood => 'Buona';

  @override
  String get roomQualityPoorTitle => 'Connessione scarsa';

  @override
  String get roomQualityOkayTitle => 'Connessione discreta';

  @override
  String get roomQualityGoodTitle => 'Connessione buona';

  @override
  String get roomQualityMeasuring => 'Misurazione';

  @override
  String get roomQualityDelay => 'Latenza';

  @override
  String roomQualityMs(String n) {
    return '$n ms';
  }

  @override
  String get roomQualityLost => 'Pacchetti persi';

  @override
  String roomQualityPercent(String n) {
    return '$n%';
  }

  @override
  String get roomQualityPicture => 'Immagine';

  @override
  String get roomQualityPath => 'Percorso';

  @override
  String get roomQualityMayFreeze =>
      'L’immagine potrebbe bloccarsi. Va meglio se meno persone condividono contemporaneamente.';

  @override
  String get roomQualityMayShrink =>
      'L’immagine potrebbe rimpicciolirsi per restare fluida.';

  @override
  String roomCallingName(String name) {
    return 'Sto chiamando $name…';
  }

  @override
  String get roomRingsFor30 =>
      'Squilla sul suo computer per un massimo di 30 secondi.';

  @override
  String get roomReturnToCall => 'Torna alla chiamata';

  @override
  String get roomCallAgain => 'Richiama';

  @override
  String get roomAloneHint => 'Quando entra qualcuno del gruppo, appare qui.';

  @override
  String roomAlreadyLeft(String names) {
    return 'Già fuori dalla stanza: $names.';
  }

  @override
  String get roomGroupFound => 'Gruppo trovato sulla rete';

  @override
  String get roomLookingForRoute => 'ricerca del percorso migliore…';

  @override
  String get roomConnectingTitle => 'Connessione alla stanza';

  @override
  String roomSeconds(String n) {
    return '$n s';
  }

  @override
  String roomReachedOf(int reached, int total, String time) {
    return '$reached di $total · $time';
  }

  @override
  String get roomPathLan => 'rete locale, cifrata';

  @override
  String get roomPathDirect => 'diretta, cifrata';

  @override
  String get roomPathThroughMember => 'tramite un membro, cifrata';

  @override
  String roomPathThrough(String name) {
    return 'tramite $name, cifrata';
  }

  @override
  String get roomCouldNotConnect => 'Impossibile connettersi alla stanza';

  @override
  String get roomCouldNotConnectText =>
      'Nessuno ha risposto in tempo. La tua rete potrebbe bloccare le connessioni dirette tra dispositivi.';

  @override
  String get roomNoAnswer => 'nessuna risposta';

  @override
  String get roomSeeNetworkCheck => 'Vedi la verifica della rete →';

  @override
  String get roomGoesNothing => 'La stanza si chiude per tutti';

  @override
  String roomGoesMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n messaggi spariscono',
      one: '1 messaggio sparisce',
    );
    return '$_temp0';
  }

  @override
  String roomGoesFiles(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n file che hai condiviso spariscono',
      one: '1 file che hai condiviso sparisce',
    );
    return '$_temp0';
  }

  @override
  String roomGoesBoth(int messages, int files) {
    String _temp0 = intl.Intl.pluralLogic(
      messages,
      locale: localeName,
      other: '$messages messaggi',
      one: '1 messaggio',
    );
    String _temp1 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files file',
      one: '1 file',
    );
    return '$_temp0 e $_temp1 spariscono';
  }

  @override
  String get roomMuteMicrophone => 'Disattiva il microfono';

  @override
  String get roomUnmuteMicrophone => 'Attiva il microfono';

  @override
  String get roomStopSharingScreen =>
      'Interrompi la condivisione dello schermo';

  @override
  String get roomShareScreen => 'Condividi lo schermo';

  @override
  String get roomUndeafen => 'Senti di nuovo gli altri';

  @override
  String get roomDeafen => 'Disattiva l’audio';

  @override
  String get roomCameraOff => 'Spegni la videocamera';

  @override
  String get roomCameraOn => 'Accendi la videocamera';

  @override
  String get roomLeave => 'Esci';

  @override
  String get roomSoundWaitsForMic =>
      'L’audio parte solo con il microfono attivo';

  @override
  String get roomStopSharingSound => 'Interrompi la condivisione dell’audio';

  @override
  String get roomShareSoundToo => 'Condividi anche l’audio';

  @override
  String get roomSoundNeedsPipeWire =>
      'Da questo computer non si può condividere l’audio: serve PipeWire, che qui non è in esecuzione. Viene condivisa solo l’immagine.';

  @override
  String get roomAddFriendToCall => 'Aggiungi un amico alla chiamata';

  @override
  String get roomHideChat => 'Nascondi la chat';

  @override
  String get roomShowChat => 'Mostra la chat';

  @override
  String get roomTabChat => 'Chat';

  @override
  String get roomTabFiles => 'File';

  @override
  String roomNoteLeftCall(String name) {
    return '$name ha lasciato la chiamata · ';
  }

  @override
  String roomNoteJoinedCall(String name) {
    return '$name ha raggiunto la chiamata · ';
  }

  @override
  String roomNoteLeftRoom(String name) {
    return '$name ha lasciato la stanza · ';
  }

  @override
  String roomNoteEnteredRoom(String name) {
    return '$name ha raggiunto la stanza · ';
  }

  @override
  String get roomSearchMessages => 'Cerca nei messaggi';

  @override
  String get roomClearSearch => 'Cancella la ricerca';

  @override
  String get roomChatGoneCall =>
      'Questa chat sparisce quando la chiamata finisce';

  @override
  String get roomChatGoneRoom =>
      'Questa chat sparisce quando la stanza si chiude';

  @override
  String roomNoMatches(String text) {
    return 'Nessun messaggio con “$text”';
  }

  @override
  String roomMatches(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n risultati',
      one: '1 risultato',
    );
    return '$_temp0';
  }

  @override
  String get roomReceivingMessages => 'Ricezione dei messaggi della stanza…';

  @override
  String roomReceivingMessagesFrom(String name) {
    return 'Ricezione dei messaggi della stanza dal computer di $name…';
  }

  @override
  String get roomSomeone => 'Qualcuno';

  @override
  String get roomShareFiles => 'Condividi file';

  @override
  String get roomAddMoreFiles => 'Aggiungi altri file';

  @override
  String roomMessageTo(String name) {
    return 'Scrivi a $name';
  }

  @override
  String get roomMessageTheRoom => 'Scrivi nella stanza';

  @override
  String get roomMessageTheCall => 'Scrivi nella chiamata';

  @override
  String get roomAddMessageOptional => 'Aggiungi un messaggio (facoltativo)';

  @override
  String get roomMessageLabel => 'Messaggio';

  @override
  String roomEnterToSend(String key) {
    return '$key per inviare';
  }

  @override
  String roomEscToRemove(String key) {
    return '$key per rimuoverli';
  }

  @override
  String roomOrTwo(String first, String second) {
    return '$first o $second';
  }

  @override
  String roomOrMore(String rest, String last) {
    return '$rest o $last';
  }

  @override
  String roomFileNotAvailable(String name) {
    return 'Non disponibile · $name non c’è più';
  }

  @override
  String get roomSharingFromHere => 'Condiviso da questo computer';

  @override
  String roomAvailableWhile(String names) {
    return 'Disponibile finché $names resta nella stanza';
  }

  @override
  String roomSizeDownloads(String size, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n download',
      one: '1 download',
    );
    return '$size · $_temp0';
  }

  @override
  String roomDownloadFile(String file) {
    return 'Scarica $file';
  }

  @override
  String roomShowInFolder(String file) {
    return 'Mostra $file nella sua cartella';
  }

  @override
  String roomGettingFile(String file) {
    return 'Ricezione di $file…';
  }

  @override
  String get roomSaveToDownloads => 'Salva in Download';

  @override
  String roomPictureKept(String name) {
    return '$name non c’è più · l’immagine resta qui finché non esci';
  }

  @override
  String get roomSaveFile => 'Salva';

  @override
  String get roomClosePicture => 'Chiudi l’immagine';

  @override
  String get roomFilesNote =>
      'I file non vengono caricati da nessuna parte: viaggiano direttamente dal computer di chi li condivide, e solo finché resta nella stanza.';

  @override
  String roomFilesAvailable(int n) {
    return 'Disponibili · $n';
  }

  @override
  String roomFilesGone(int n) {
    return 'Non più disponibili · $n';
  }

  @override
  String roomPersonLeft(String name) {
    return '$name ha lasciato la stanza';
  }

  @override
  String get roomPickOrDrop => 'Sceglili qui o trascinali sulla stanza';

  @override
  String get roomFileYours => 'Tuo';

  @override
  String roomFileFrom(String name) {
    return 'Da $name';
  }

  @override
  String roomDownloads(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n download',
      one: '1 download',
    );
    return '$_temp0';
  }

  @override
  String get roomStopSharingFile => 'Interrompi';

  @override
  String get roomWhatToShare => 'Cosa vuoi condividere?';

  @override
  String roomScreenSource(String name) {
    return 'Schermo: $name';
  }

  @override
  String roomPlayVideo(String file) {
    return 'Riproduci $file';
  }

  @override
  String get roomCloseVideo => 'Chiudi il video';

  @override
  String get roomVideoGone => 'Questo video non è più nella stanza.';

  @override
  String get roomVideoNotFetched => 'Impossibile ottenere questo video.';

  @override
  String get roomVideoCannotPlay =>
      'Questo computer non può riprodurre questo video.';

  @override
  String roomGettingVideoPercent(int n) {
    return 'Ricezione del video · $n%';
  }

  @override
  String get roomGettingVideo => 'Ricezione del video…';

  @override
  String get roomPause => 'Pausa';

  @override
  String get roomPlay => 'Riproduci';

  @override
  String get roomSoundOn => 'Attiva l’audio';

  @override
  String get roomMuteVideo => 'Disattiva l’audio';

  @override
  String get roomLeaveFullScreen => 'Esci da schermo intero';

  @override
  String roomCallWaits(String time) {
    return 'Quando resta una sola persona, la chiamata aspetta $time che gli altri tornino';
  }

  @override
  String roomCallWaitsAlone(String time) {
    return 'Sei l’unica persona rimasta. La chiamata aspetta $time che gli altri tornino';
  }

  @override
  String roomOpenFile(String file) {
    return 'Apri $file';
  }

  @override
  String activityCreated(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai creato',
      'other': '$by ha creato',
    });
    return '$_temp0 il gruppo “$name”';
  }

  @override
  String activityRenamed(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai rinominato',
      'other': '$by ha rinominato',
    });
    return '$_temp0 il gruppo in “$name”';
  }

  @override
  String activityIcon(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai cambiato',
      'other': '$by ha cambiato',
    });
    return '$_temp0 l’icona del gruppo';
  }

  @override
  String activityPictureRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai tolto',
      'other': '$by ha tolto',
    });
    return '$_temp0 l’immagine del gruppo';
  }

  @override
  String activityPicture(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai cambiato',
      'other': '$by ha cambiato',
    });
    return '$_temp0 l’immagine del gruppo';
  }

  @override
  String activityBannerRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai tolto',
      'other': '$by ha tolto',
    });
    return '$_temp0 il banner del gruppo';
  }

  @override
  String activityBanner(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai cambiato',
      'other': '$by ha cambiato',
    });
    return '$_temp0 il banner del gruppo';
  }

  @override
  String activityAdded(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by ti ha aggiunto',
      'other': '$by ha aggiunto $member',
    });
    String _temp1 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai aggiunto $member',
      'other': '$_temp0',
    });
    return '$_temp1';
  }

  @override
  String activityJoined(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai usato',
      'other': '$by ha usato',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'tuo',
      'other': 'di $member',
    });
    return '$_temp0 un link d’invito $_temp1 per entrare';
  }

  @override
  String activityLetIn(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by ti ha fatto entrare',
      'other': '$by ha fatto entrare $member',
    });
    String _temp1 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai fatto entrare $member',
      'other': '$_temp0',
    });
    return '$_temp1';
  }

  @override
  String activityRemoved(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by ti ha rimosso',
      'other': '$by ha rimosso $member',
    });
    String _temp1 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai rimosso $member',
      'other': '$_temp0',
    });
    return '$_temp1';
  }

  @override
  String activityLeft(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai lasciato',
      'other': '$by ha lasciato',
    });
    return '$_temp0 il gruppo';
  }

  @override
  String activityMovedUp(
    String by,
    String byMe,
    String member,
    String memberMe,
    String place,
  ) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by ti ha portato al $place posto',
      'other': '$by ha portato $member al $place posto',
    });
    String _temp1 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai portato $member al $place posto',
      'other': '$_temp0',
    });
    return '$_temp1';
  }

  @override
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by ti ha spostato in fondo',
      'other': '$by ha spostato $member in fondo',
    });
    String _temp1 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai spostato $member in fondo',
      'other': '$_temp0',
    });
    return '$_temp1';
  }

  @override
  String activityManagersAll(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai permesso',
      'other': '$by ha permesso',
    });
    return '$_temp0 a tutti di gestire il gruppo';
  }

  @override
  String activityManagersFirst(String by, String byMe, int n) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai affidato',
      'other': '$by ha affidato',
    });
    String _temp1 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'ai primi $n',
      one: 'solo al primo',
    );
    return '$_temp0 la gestione del gruppo $_temp1';
  }

  @override
  String activityLimits(String by, String byMe, int people, int perRoom) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai impostato',
      'other': '$by ha impostato',
    });
    return '$_temp0 il gruppo a $people persone e $perRoom per stanza vocale';
  }

  @override
  String activityRoomOpened(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai aperto',
      'other': '$by ha aperto',
    });
    return '$_temp0 la stanza vocale “$room”';
  }

  @override
  String activityRoomRenamed(String by, String byMe, String from, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai rinominato',
      'other': '$by ha rinominato',
    });
    return '$_temp0 la stanza vocale “$from” in “$room”';
  }

  @override
  String activityRoomClosed(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai eliminato',
      'other': '$by ha eliminato',
    });
    return '$_temp0 la stanza vocale “$room”';
  }

  @override
  String activityInvited(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai creato',
      'other': '$by ha creato',
    });
    return '$_temp0 un link d’invito';
  }

  @override
  String activityRevoked(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai annullato',
      'other': '$by ha annullato',
    });
    return '$_temp0 un link d’invito';
  }

  @override
  String activityRoomOpenToAll(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai aperto',
      'other': '$by ha aperto',
    });
    return '$_temp0 “$room” a tutti';
  }

  @override
  String activityRoomForAbove(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai riservato',
      'other': '$by ha riservato',
    });
    return '$_temp0 “$room” a chi sta sopra';
  }

  @override
  String activityRoomOnly(String by, String byMe, String people, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai riservato',
      'other': '$by ha riservato',
    });
    return '$_temp0 “$room” solo a $people';
  }

  @override
  String activityKeptOut(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by ti ha escluso da “$room” a tempo indeterminato',
      'other': '$by ha escluso $member da “$room” a tempo indeterminato',
    });
    String _temp1 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai escluso $member da “$room” a tempo indeterminato',
      'other': '$_temp0',
    });
    return '$_temp1';
  }

  @override
  String activityTimedOut(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
    String until,
    String untilKind,
  ) {
    String _temp0 = intl.Intl.selectLogic(untilKind, {
      'today': 'fino alle $until',
      'todayAtOne': 'fino all’$until',
      'yesterday': 'fino a $until',
      'other': 'fino al giorno $until',
    });
    String _temp1 = intl.Intl.selectLogic(untilKind, {
      'today': 'fino alle $until',
      'todayAtOne': 'fino all’$until',
      'yesterday': 'fino a $until',
      'other': 'fino al giorno $until',
    });
    String _temp2 = intl.Intl.selectLogic(untilKind, {
      'today': 'fino alle $until',
      'todayAtOne': 'fino all’$until',
      'yesterday': 'fino a $until',
      'other': 'fino al giorno $until',
    });
    String _temp3 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by ti ha sospeso da “$room” $_temp1',
      'other': '$by ha sospeso $member da “$room” $_temp2',
    });
    String _temp4 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai sospeso $member da “$room” $_temp0',
      'other': '$_temp3',
    });
    return '$_temp4';
  }

  @override
  String activityLetBack(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by ti ha fatto rientrare in “$room”',
      'other': '$by ha fatto rientrare $member in “$room”',
    });
    String _temp1 = intl.Intl.selectLogic(byMe, {
      'yes': 'Hai fatto rientrare $member in “$room”',
      'other': '$_temp0',
    });
    return '$_temp1';
  }

  @override
  String get activityDefaultRoom => 'Generale';

  @override
  String get activityYouInList => 'te';

  @override
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'la tua rimozione',
      'other': 'la rimozione di $author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' da parte tua',
      'other': ' da parte di $by',
      'none': '',
    });
    return 'Annullato: $_temp0$_temp1 è avvenuta nello stesso momento';
  }

  @override
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'il tuo spostamento in fondo',
      'other': 'lo spostamento in fondo di $author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' da parte tua',
      'other': ' da parte di $by',
      'none': '',
    });
    return 'Annullato: $_temp0$_temp1 è avvenuto nello stesso momento';
  }

  @override
  String activityUndoneLeft(String author, String authorMe) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'avevi',
      'other': '$author aveva',
    });
    return 'Annullato: $_temp0 lasciato il gruppo nello stesso momento';
  }

  @override
  String get activityAboutList =>
      'Tutti nel gruppo vedono lo stesso elenco, preso dal registro del gruppo. Le modifiche a immagine e banner sono registrate a parte: sono quelle viste da questo dispositivo.';

  @override
  String get activityAboutOrder =>
      'Nell’ordine del gruppo; gli orari sono quelli dell’orologio di ciascun autore.';

  @override
  String get activityNothingYet => 'Ancora niente.';

  @override
  String get activityLoading => 'Caricamento…';

  @override
  String get activityShowOlder => 'Mostra i precedenti';

  @override
  String activitySeenHere(String when) {
    return '$when · visto su questo dispositivo';
  }

  @override
  String get bellNotifications => 'Notifiche';

  @override
  String bellNewNotifications(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n nuove notifiche',
      one: '1 nuova notifica',
    );
    return '$_temp0';
  }

  @override
  String get bellClearAll => 'Cancella tutto';

  @override
  String get bellNothingNew => 'Niente di nuovo';

  @override
  String get bellFooter =>
      'Le ultime tre restano qui. Le chiamate perse e le richieste compaiono anche in Amici.';

  @override
  String bellBothInRoom(String room) {
    return 'Siete entrambi in $room.';
  }

  @override
  String get bellDismiss => 'Ignora';

  @override
  String bellFriendRequest(String name) {
    return '$name vuole aggiungerti agli amici';
  }

  @override
  String bellCheckFingerprint(String fingerprint) {
    return 'Controlla che l’impronta corrisponda: $fingerprint';
  }

  @override
  String bellJoinAsk(String name, String group) {
    return '$name chiede di unirsi a “$group”';
  }

  @override
  String bellJoinAskNote(String fingerprint) {
    return 'Con uno dei link d’invito del tuo gruppo. Impronta: $fingerprint';
  }

  @override
  String get bellLetIn => 'Fai entrare';

  @override
  String get bellSayNo => 'Rifiuta';

  @override
  String bellGroupInvite(String name, String group) {
    return '$name ti ha aggiunto a “$group”';
  }

  @override
  String bellMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n membri',
      one: '1 membro',
    );
    return '$_temp0';
  }

  @override
  String get bellJoin => 'Unisciti';

  @override
  String bellNewMessages(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n nuovi messaggi da $name',
      one: 'Nuovo messaggio da $name',
    );
    return '$_temp0';
  }

  @override
  String get bellOpen => 'Apri';

  @override
  String get bellMarkRead => 'Segna come letti';

  @override
  String get bellFirewallTitle => 'Il tuo firewall tiene fuori alcuni amici';

  @override
  String get bellCopyCommand => 'Copia il comando';

  @override
  String get bellOtherSessionTitle => 'Aperto anche su un altro computer';

  @override
  String get bellSettingsResetTitle =>
      'Alcune impostazioni sono state ripristinate';

  @override
  String get bellSettingsResetText =>
      'Non è stato possibile leggere alcune impostazioni, che sono tornate ai valori predefiniti: controllale in Impostazioni. Il vecchio file è stato conservato accanto al nuovo, come app.json.bad.';

  @override
  String get friendCardOnline => 'Online';

  @override
  String get friendCardAway => 'Assente';

  @override
  String get friendCardOffline => 'Offline';

  @override
  String get friendCardCallsOnlyOnline =>
      'Le chiamate squillano solo se l’altra persona è online.';

  @override
  String friendCardHangUpFirst(String name) {
    return 'Riaggancia prima di chiamare $name.';
  }

  @override
  String friendCardBothInRoom(String room) {
    return 'Siete entrambi in $room.';
  }

  @override
  String get friendCardMessage => 'Scrivi';

  @override
  String friendCardMutualFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n amici in comune',
      one: '1 amico in comune',
    );
    return '$_temp0';
  }

  @override
  String get memberCardSendRequest => 'Invia richiesta di amicizia';

  @override
  String get memberCardRequestSent => 'Richiesta di amicizia inviata';

  @override
  String get memberCardAccept => 'Accetta richiesta di amicizia';

  @override
  String memberCardAsksYou(String name) {
    return '$name vuole aggiungerti agli amici.';
  }

  @override
  String memberCardInGroup(String group) {
    return 'Membro di $group';
  }

  @override
  String memberCardOnlyOnline(String name) {
    return 'Le richieste di amicizia partono solo mentre $name è online.';
  }

  @override
  String get memberCardShowOwn => 'Mostra la tua scheda';

  @override
  String get userPanelEditProfile => 'Modifica profilo';

  @override
  String get userPanelStatus => 'Stato';

  @override
  String get userPanelOnline => 'Online';

  @override
  String get userPanelAway => 'Assente';

  @override
  String get userPanelOffline => 'Offline';

  @override
  String get userPanelOfflineNote =>
      'I tuoi amici e i membri dei tuoi gruppi ti vedono offline. Puoi comunque stare nelle stanze.';

  @override
  String get userPanelOnlineNote =>
      'I tuoi amici e i membri dei tuoi gruppi ti vedono online, o assente dopo 10 min senza usare questo computer.';

  @override
  String get userPanelBio => 'BIO';

  @override
  String get userPanelAppearingOffline => 'Appari offline';

  @override
  String get userPanelCopyLinkLabel => 'Copia un link di amicizia';

  @override
  String userPanelLinkExpiresIn(String time) {
    return 'Scade tra $time';
  }

  @override
  String get userPanelLinkTerms => '1 persona · 10 min';

  @override
  String get userPanelCopyLink => 'Copia link di amicizia';

  @override
  String get switcherChatEnded => 'Chat diretta · non più amici';

  @override
  String get switcherChatOnline => 'Chat diretta · online';

  @override
  String get switcherChatAway => 'Chat diretta · assente';

  @override
  String get switcherChat => 'Chat diretta';

  @override
  String switcherFriend(String presence) {
    return 'Amico · $presence';
  }

  @override
  String switcherVoiceRoom(String group) {
    return 'Stanza vocale in $group';
  }

  @override
  String switcherVoiceRoomPeople(String group, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Stanza vocale in $group · $n persone',
      one: 'Stanza vocale in $group · $n persona',
    );
    return '$_temp0';
  }

  @override
  String switcherGroup(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Gruppo · $n membri',
      one: 'Gruppo · 1 membro',
    );
    return '$_temp0';
  }

  @override
  String get switcherPlaceholder =>
      'Vai a una chat, un gruppo o una stanza vocale';

  @override
  String get switcherGoTo => 'Vai a';

  @override
  String get switcherEmpty => 'Ancora nessun gruppo, amico o chat.';

  @override
  String switcherNoMatch(String query) {
    return 'Nessun risultato per “$query”.';
  }

  @override
  String get switcherHintChoose => 'per scegliere';

  @override
  String get switcherHintGo => 'per andare';

  @override
  String get switcherHintClose => 'per chiudere';

  @override
  String switcherTooltip(String shortcut) {
    return 'Vai a una chat, un gruppo o una stanza vocale ($shortcut)';
  }

  @override
  String get friendsIntro =>
      'Ogni amicizia esiste solo sui due dispositivi. Nessun elenco pubblico, nessun server.';

  @override
  String get friendsTabAll => 'Tutti';

  @override
  String get friendsTabOnline => 'Online';

  @override
  String get friendsTabRequests => 'Richieste';

  @override
  String get friendsSearch => 'Cerca amici';

  @override
  String get friendsNone =>
      'Ancora nessun amico. Condividi il tuo QR, link o codice, oppure usa quello di qualcun altro.';

  @override
  String get friendsNoneOnline => 'Nessun amico è online in questo momento.';

  @override
  String get friendsNoMatch => 'Nessuno corrisponde alla ricerca.';

  @override
  String get friendsInARoom => 'In una stanza';

  @override
  String friendsInRoomOf(String group) {
    return 'In stanza · $group';
  }

  @override
  String get friendsOnline => 'Online';

  @override
  String get friendsAway => 'Assente';

  @override
  String get friendsNotSeenYet => 'Mai online';

  @override
  String friendsSeen(String when) {
    return 'Ultimo accesso $when';
  }

  @override
  String friendsShowCard(String name) {
    return 'Mostra la scheda di $name';
  }

  @override
  String friendsMissedCall(String when) {
    return 'Chiamata persa · $when';
  }

  @override
  String get friendsMessage => 'Scrivi';

  @override
  String friendsOfflineNoCall(String name) {
    return '$name è offline: le chiamate squillano solo se l’altra persona è online';
  }

  @override
  String friendsHangUpFirst(String name) {
    return 'Riaggancia prima di chiamare $name';
  }

  @override
  String friendsBothInRoom(String room) {
    return 'Siete entrambi in $room.';
  }

  @override
  String friendsOptions(String name) {
    return 'Opzioni per $name';
  }

  @override
  String friendsRemoveTitle(String name) {
    return 'Rimuovere $name dagli amici?';
  }

  @override
  String get friendsRemoveText =>
      'Ognuno sparisce dagli amici dell’altro, senza alcun messaggio. Restate insieme nei gruppi che avete in comune. Per tornare amici serve un nuovo invito.';

  @override
  String get friendsRemove => 'Rimuovi';

  @override
  String friendsAddToGroup(String group) {
    return 'Aggiungi a “$group”';
  }

  @override
  String get friendsRemoveMenu => 'Rimuovi dagli amici…';

  @override
  String get friendsNoRequests => 'Nessuna richiesta in sospeso.';

  @override
  String friendsRequestIncoming(String when) {
    return 'Vuole aggiungerti agli amici · $when';
  }

  @override
  String friendsRequestOutgoing(String when) {
    return 'Inviata $when · in attesa di risposta';
  }

  @override
  String get friendsPendingRequest => 'Richiesta in sospeso';

  @override
  String friendsCheckFingerprint(String name) {
    return 'Controlla che l’impronta corrisponda a quella che $name vede sul suo schermo.';
  }

  @override
  String get friendsAddTitle => 'Aggiungi un amico';

  @override
  String get friendsAddNote =>
      'I vostri dispositivi si scambiano le chiavi direttamente.';

  @override
  String get friendsTabCode => 'Codice';

  @override
  String get friendsTabLink => 'Link';

  @override
  String get friendsTabQr => 'QR';

  @override
  String get friendsMakeInvitation => 'Crea un invito';

  @override
  String get friendsInvitationExpired => 'Questo invito è scaduto.';

  @override
  String get friendsMakeAnother => 'Creane un altro';

  @override
  String friendsHintQr(String time) {
    return 'Scansionalo da Fogata · scade tra $time';
  }

  @override
  String friendsHintLink(String time) {
    return 'Invialo a una persona · scade tra $time';
  }

  @override
  String friendsHintCode(String time) {
    return 'Dettalo a una persona · scade tra $time';
  }

  @override
  String get friendsHiddenCode => 'Nascosto nel caso tu condivida lo schermo.';

  @override
  String get friendsHiddenLink =>
      'Nascosto nel caso tu condivida lo schermo. Contiene gli indirizzi di rete di questo computer: condividilo solo in privato.';

  @override
  String get friendsGotLink =>
      'Hai un link di amicizia o di gruppo, o un codice?';

  @override
  String get friendsPastePlaceholder => 'Incolla un link o un codice';

  @override
  String get friendsLinkOrCode => 'Link o codice';

  @override
  String get friendsAdd => 'Aggiungi';

  @override
  String get friendsQrLabel => 'Codice QR del tuo invito';

  @override
  String friendsAddedToGroup(String name) {
    return '$name riceverà il gruppo la prossima volta che sarete entrambi online.';
  }

  @override
  String get directToday => 'Oggi';

  @override
  String get directYesterday => 'Ieri';

  @override
  String directTimerOff(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Hai disattivato',
      'other': '$name ha disattivato',
    });
    return '$_temp0 i messaggi effimeri';
  }

  @override
  String directTimerFirst(String time) {
    return 'Messaggi effimeri attivi: $time';
  }

  @override
  String directTimerOn(String name, String mine, String time) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Hai attivato',
      'other': '$name ha attivato',
    });
    return '$_temp0 i messaggi effimeri: $time';
  }

  @override
  String directPinnedLine(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Hai fissato',
      'other': '$name ha fissato',
    });
    return '$_temp0 un messaggio';
  }

  @override
  String get directCallFewSeconds => 'pochi secondi';

  @override
  String directCallMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minuti',
      one: '1 minuto',
    );
    return '$_temp0';
  }

  @override
  String directCallHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n ore',
      one: '1 ora',
    );
    return '$_temp0';
  }

  @override
  String directCallHoursMinutes(int hours, int minutes) {
    String _temp0 = intl.Intl.pluralLogic(
      hours,
      locale: localeName,
      other: '$hours ore',
      one: '1 ora',
    );
    String _temp1 = intl.Intl.pluralLogic(
      minutes,
      locale: localeName,
      other: '$minutes minuti',
      one: '1 minuto',
    );
    return '$_temp0 e $_temp1';
  }

  @override
  String directCallLine(String name, String mine, String length) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Hai avviato',
      'other': '$name ha avviato',
    });
    return '$_temp0 una chiamata durata $length';
  }

  @override
  String get directBlockedOffline =>
      'Appari offline. Torna online per scrivere.';

  @override
  String directBlockedFriendOffline(String name) {
    return '$name è offline. Potrai scrivere quando sarete entrambi online.';
  }

  @override
  String get directOnline => 'Online';

  @override
  String get directAway => 'Assente';

  @override
  String get directOffline => 'Offline';

  @override
  String directOfflineSeen(String when) {
    return 'Offline · ultimo accesso $when';
  }

  @override
  String get directHeading => 'MESSAGGI DIRETTI';

  @override
  String directPreviewMine(String text) {
    return 'Tu: $text';
  }

  @override
  String directOpenChat(String name) {
    return 'Apri la chat con $name';
  }

  @override
  String directShowCard(String name) {
    return 'Mostra la scheda di $name';
  }

  @override
  String get directNoLongerFriend => 'Non siete più amici';

  @override
  String directCallOffline(String name) {
    return '$name è offline: le chiamate squillano solo se l’altra persona è online';
  }

  @override
  String directHangUpFirst(String name) {
    return 'Riaggancia prima di chiamare $name';
  }

  @override
  String directPinnedCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n messaggi fissati',
      one: '1 messaggio fissato',
    );
    return '$_temp0';
  }

  @override
  String get directPinnedTitle => 'Messaggi fissati';

  @override
  String directPinnedNote(int max) {
    return 'Fissati per entrambi, e non scompaiono. Massimo $max.';
  }

  @override
  String get directNothingPinned => 'Ancora nessun messaggio fissato';

  @override
  String get directNothingPinnedNote =>
      'Passa il mouse su un messaggio e scegli la puntina per tenerlo qui.';

  @override
  String directDayTime(String day, String time) {
    return '$day, $time';
  }

  @override
  String directFilesCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n file',
    );
    return '$_temp0';
  }

  @override
  String directShowMessageFrom(String name) {
    return 'Mostra il messaggio di $name';
  }

  @override
  String get directUnpin => 'Togli dai fissati';

  @override
  String get directPin => 'Fissa';

  @override
  String get directDeleteMessage => 'Elimina per me';

  @override
  String get directDeleteMessageTitle => 'Eliminare questo messaggio?';

  @override
  String directDeleteMessageNote(String name) {
    return 'Viene eliminato da questo computer, con le sue immagini e i suoi file. $name conserva la sua copia.';
  }

  @override
  String get directDeleteMessageAction => 'Elimina';

  @override
  String get directDeleteForBoth => 'Elimina per entrambi';

  @override
  String directDeleteOwnNote(String name) {
    return 'Elimina per entrambi lo toglie anche dalla chat di $name, appena il suo Fogata lo riceve. Elimina per me lo lascia lì.';
  }

  @override
  String directTimerLabel(String time) {
    return 'Messaggi effimeri: $time';
  }

  @override
  String get directTimerTitle => 'Messaggi effimeri';

  @override
  String get directCloseMenu => 'Chiudi il menu';

  @override
  String get directTimerNote =>
      'I nuovi messaggi spariscono da entrambi i dispositivi, con le loro immagini e i loro file.';

  @override
  String get directEmptyNote =>
      'Ciò che scrivi resta solo sui vostri due computer.';

  @override
  String directPinLineLabel(String line) {
    return '$line: mostralo';
  }

  @override
  String get directShowMessage => 'Mostra il messaggio';

  @override
  String get directPinnedStays => 'Fissato: resta finché non viene tolto';

  @override
  String get directPinned => 'Fissato';

  @override
  String directPinsFull(int max) {
    return 'Si possono fissare al massimo $max messaggi: prima togline uno';
  }

  @override
  String get directDelivered => 'Consegnato';

  @override
  String get directNotDelivered => 'Non consegnato';

  @override
  String get directSending => 'Invio…';

  @override
  String directWaiting(String name) {
    return 'In attesa che $name sia online · verrà inviato quando lo sarete entrambi';
  }

  @override
  String get directAvailableWhenYouOnline => 'disponibile quando torni online';

  @override
  String directAvailableWhenOnline(String name) {
    return 'disponibile quando $name è online';
  }

  @override
  String get directDownloadedByItself => 'scaricata in automatico';

  @override
  String directOpenFile(String name) {
    return 'Apri $name';
  }

  @override
  String directPlayFile(String name) {
    return 'Riproduci $name';
  }

  @override
  String directDownloadSize(String size) {
    return 'Scarica · $size';
  }

  @override
  String directVideoPlaysWhenHere(String name, String length) {
    return '$name · $length · si riproduce appena arriva';
  }

  @override
  String directVideoNotHere(String name, String length, String when) {
    return '$name · $length · $when';
  }

  @override
  String directGettingIt(int percent) {
    return 'Ricezione · $percent%';
  }

  @override
  String directFileNotHere(String size, String when) {
    return '$size · $when';
  }

  @override
  String get directSaveToDownloads => 'Salva in Download';

  @override
  String get directRemovedForSpace => 'Rimosso qui per liberare spazio';

  @override
  String get directAskAgain => 'Richiedi';

  @override
  String get directClosePicture => 'Chiudi l’immagine';

  @override
  String get directCloseVideo => 'Chiudi il video';

  @override
  String get directCannotPlay =>
      'Questo computer non può riprodurre questo video.';

  @override
  String get directPause => 'Pausa';

  @override
  String get directPlay => 'Riproduci';

  @override
  String directRemoveFilesTitle(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Rimuovere i $n file?',
      one: 'Rimuovere il file?',
    );
    return '$_temp0';
  }

  @override
  String get directRemoveFilesText =>
      'Vengono tolti da questo messaggio. I file sul tuo computer restano come sono.';

  @override
  String get directDraftKept =>
      'Ciò che stavi scrivendo resta qui per quando potrai inviarlo.';

  @override
  String directMessagePlaceholder(String name) {
    return 'Scrivi a $name';
  }

  @override
  String get directAddMessageOptional => 'Aggiungi un messaggio (facoltativo)';

  @override
  String get directAutoDownloadNote =>
      'Le immagini fino a 10 MB si scaricano da sole; video e file quando li apri.';

  @override
  String get directVoiceRecord => 'Registra un messaggio vocale';

  @override
  String get directVoiceInCall => 'Non puoi registrare durante una chiamata';

  @override
  String directVoiceWaitingElsewhere(String name) {
    return 'Prima invia o elimina il messaggio vocale nella chat con $name.';
  }

  @override
  String get directVoiceDelete => 'Elimina';

  @override
  String get directVoiceTooShort =>
      'Troppo breve per inviarlo: un messaggio vocale dura almeno mezzo secondo.';

  @override
  String directVoiceLeft(String time) {
    return 'Mancano $time';
  }

  @override
  String directVoiceRecordingFrom(String microphone) {
    return 'Registrazione da $microphone';
  }

  @override
  String get directVoiceRecordingDefault =>
      'Il microfono scelto non è disponibile: registrazione con quello predefinito';

  @override
  String directVoiceKeys(String esc, String enter) {
    return '$esc lo elimina, $enter lo invia';
  }

  @override
  String directVoiceKeySend(String enter) {
    return '$enter lo invia';
  }

  @override
  String directVoicePreview(String length) {
    return 'Messaggio vocale · $length';
  }

  @override
  String get directVoiceMessage => 'Messaggio vocale';

  @override
  String get directVoiceNoMic => 'Fogata non riesce a usare il microfono.';

  @override
  String get directVoiceMicStopped =>
      'Il microfono si è fermato. Puoi comunque inviare ciò che è stato registrato.';

  @override
  String directVoiceNoMicCheck(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Controlla che ce ne sia uno collegato, scelto in Voce e video e consentito in Impostazioni di Sistema › Privacy e sicurezza.',
      'other':
          'Controlla che ce ne sia uno collegato e scelto in Voce e video.',
    });
    return '$_temp0';
  }

  @override
  String get directVoiceSpeed => 'Velocità di riproduzione';

  @override
  String get directVoiceNotPlayed => 'Non ancora ascoltato';

  @override
  String directRemoveFile(String name) {
    return 'Rimuovi $name';
  }

  @override
  String get directAltered =>
      'La cronologia di questa chat su questo computer è cambiata dall’ultimo salvataggio di Fogata. Alcuni messaggi potrebbero mancare.';

  @override
  String directEnded(String name) {
    return 'Tu e $name non siete più amici. Questa chat è di sola lettura.';
  }

  @override
  String get directDeleteChat => 'Elimina chat';

  @override
  String directDeleteTitle(String name) {
    return 'Eliminare la chat con $name?';
  }

  @override
  String directDeleteText(String name) {
    return 'Messaggi, immagini, video e file di questa chat spariscono da questo computer. $name conserva la sua copia.';
  }

  @override
  String get directDelete => 'Elimina';

  @override
  String get directUsageMessages => 'Messaggi';

  @override
  String get directUsagePictures => 'Immagini';

  @override
  String get directUsageVideos => 'Video';

  @override
  String get directUsageFiles => 'File';

  @override
  String get directUsageHeading => 'QUESTA CHAT SU QUESTO DISPOSITIVO';

  @override
  String get directFreeUpSpace => 'Libera spazio…';

  @override
  String get directAddFilesTooltip => 'Aggiungi immagini, video o file';

  @override
  String get directAddFiles => 'Aggiungi file';

  @override
  String get accountMoveLessThanMinuteLeft => 'manca meno di un minuto';

  @override
  String get accountMoveAboutMinuteLeft => 'manca circa un minuto';

  @override
  String accountMoveAboutMinutesLeft(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'mancano circa $n minuti',
      one: 'manca circa un minuto',
    );
    return '$_temp0';
  }

  @override
  String accountMoveNoRoom(String need, String free) {
    return 'Questo computer ha bisogno di $need liberi per l’account, ma ne ha $free.';
  }

  @override
  String accountMoveTooLarge(String size) {
    return 'L’altro computer ha offerto $size, molto più di quanto contenga un account.';
  }

  @override
  String get accountMoveBusy =>
      'Questo computer sta finendo un altro trasferimento: prima completalo o scartalo.';

  @override
  String get accountMoveNothingToFinish =>
      'Questo computer non ha nessun trasferimento da completare da quel computer.';

  @override
  String get accountMoveNetworkStopped => 'La rete ha smesso di funzionare.';

  @override
  String get accountMoveConnectionLost => 'La connessione si è interrotta.';

  @override
  String get accountMovePeerNoRoom =>
      'Il nuovo computer non ha abbastanza spazio libero.';

  @override
  String get accountMovePeerTooLarge =>
      'L’account è troppo grande per essere trasferito.';

  @override
  String get accountMovePeerBusy =>
      'Il nuovo computer sta finendo un altro trasferimento.';

  @override
  String get accountMovePeerCouldNotKeep =>
      'Il nuovo computer non è riuscito a salvare i file.';

  @override
  String get accountMovePeerCancelled =>
      'Il trasferimento è stato annullato sull’altro computer.';

  @override
  String get accountMovePeerCouldNotFinish =>
      'Il nuovo computer non è ancora riuscito a completare il trasferimento.';

  @override
  String get accountMovePeerStopped =>
      'Il trasferimento è stato interrotto sull’altro computer.';

  @override
  String get accountMovePeerNothingToFinish =>
      'Quel computer non ha nessun trasferimento da completare da questo.';

  @override
  String get accountMovePeerNotProven =>
      'Quel computer non ha dimostrato di avere questo trasferimento.';

  @override
  String get accountMoveWrongCode =>
      'L’altro computer non ha accettato il codice, quindi non è stato inviato nulla.';

  @override
  String get accountMoveExpired => 'Il codice è scaduto prima di essere usato.';

  @override
  String get accountMoveTooManyTries =>
      'Sono stati provati diversi codici sbagliati; il codice ha smesso di funzionare.';

  @override
  String get accountMoveNoContact =>
      'I due computer non sono riusciti a raggiungersi.';

  @override
  String get accountMoveUnconfirmed =>
      'La fase finale del trasferimento non è stata confermata.';

  @override
  String get accountMoveCancelled => 'Annullato.';

  @override
  String get accountMoveInterrupted =>
      'Il trasferimento si è interrotto prima della fine; non è cambiato nulla.';

  @override
  String accountMoveInterruptedWhy(String why) {
    return 'Il trasferimento si è interrotto prima della fine; non è cambiato nulla. $why';
  }

  @override
  String get accountMoveDeclined =>
      'L’altro computer ha interrotto il trasferimento.';

  @override
  String accountMoveDeclinedWhy(String why) {
    return 'L’altro computer ha interrotto il trasferimento. $why';
  }

  @override
  String get accountMoveLater =>
      'L’account è arrivato e verrà attivato al prossimo avvio di Fogata su questo computer.';

  @override
  String accountMoveLaterWhy(String why) {
    return 'L’account è arrivato e verrà attivato al prossimo avvio di Fogata su questo computer. $why';
  }

  @override
  String get accountMoveDidNotFinish => 'Non è stato completato.';

  @override
  String get accountMoveAnotherComputer => 'un altro computer';

  @override
  String get accountMoveTheOtherComputer => 'l’altro computer';

  @override
  String get moveTooLateBeingPlaced =>
      'Troppo tardi per fermarlo: l’account è in fase di attivazione.';

  @override
  String get moveTooLateDiscard =>
      'Troppo tardi: l’account è in fase di attivazione.';

  @override
  String get moveTooLateOtherHasIt =>
      'Troppo tardi per fermarlo: ora l’account è sull’altro computer.';

  @override
  String moveFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n amici',
      one: '1 amico',
    );
    return '$_temp0';
  }

  @override
  String moveGroups(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n gruppi',
      one: '1 gruppo',
    );
    return '$_temp0';
  }

  @override
  String moveGroupsWithKeys(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n gruppi, con le loro chiavi',
      one: '1 gruppo, con le sue chiavi',
    );
    return '$_temp0';
  }

  @override
  String moveChats(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n chat',
      one: '1 chat',
    );
    return '$_temp0';
  }

  @override
  String moveInAll(String size) {
    return '$size in tutto';
  }

  @override
  String get moveYourSettings => 'Le tue impostazioni';

  @override
  String get moveNotDevices => 'tranne microfono e videocamera scelti';

  @override
  String get moveFinishingBoth => 'Completamento su entrambi i computer…';

  @override
  String movePercent(int percent) {
    return '$percent%';
  }

  @override
  String movePercentLeft(int percent, String left) {
    return '$percent% · $left';
  }

  @override
  String get moveStepIdentity => 'Identità e profilo';

  @override
  String get moveStepFriends => 'Amici';

  @override
  String get moveStepGroups => 'Gruppi e loro chiavi';

  @override
  String get moveStepChats => 'Chat, immagini e file';

  @override
  String moveBytesOf(String done, String total) {
    return '$done di $total';
  }

  @override
  String get moveStepFinishing => 'Completamento su entrambi i computer';

  @override
  String moveBringingNamed(String name) {
    return 'Trasferimento dell’account di $name';
  }

  @override
  String get moveBringingYours => 'Trasferimento del tuo account';

  @override
  String get moveKeepBothOn =>
      'Tieni accesi entrambi i computer e Fogata aperto.';

  @override
  String get moveKeepOpenNote =>
      'Tieni Fogata aperto su entrambi i computer. È più veloce sulla stessa rete; altrimenti si connettono via internet, direttamente.';

  @override
  String moveFinishBringingNamed(String name) {
    return 'Completa il trasferimento dell’account di $name';
  }

  @override
  String get moveFinishBringingYours =>
      'Completa il trasferimento del tuo account';

  @override
  String get moveEverythingCame =>
      'È arrivato tutto. Digita questo codice sull’altro computer per completare.';

  @override
  String moveEverythingCameFrom(String computer) {
    return 'È arrivato tutto. Digita questo codice su “$computer” per completare.';
  }

  @override
  String get moveBringHereTitle => 'Porta qui il tuo account';

  @override
  String get moveBringHereSub =>
      'Digita questo codice sul computer che ha il tuo account.';

  @override
  String get moveHideCode => 'Nascondi il codice';

  @override
  String get moveShowCode => 'Mostra il codice';

  @override
  String moveExpiresIn(String time) {
    return 'scade tra $time';
  }

  @override
  String moveStepFogataShows(String what) {
    return 'Su quel computer, Fogata mostra $what.';
  }

  @override
  String get moveWaitingToConfirm => 'In attesa di conferma';

  @override
  String get moveStepOpenThere => 'Su quel computer, apri Fogata.';

  @override
  String moveStepTypeUnder(String where) {
    return 'Digita il codice sotto $where.';
  }

  @override
  String get moveFinishTheMove => 'Completa il trasferimento';

  @override
  String moveStepGoTo(String where) {
    return 'Vai in $where.';
  }

  @override
  String get moveSettingsPath => 'Impostazioni → Trasferisci account';

  @override
  String get moveToAnotherComputer => 'Trasferisci su un altro computer';

  @override
  String get moveWaitingForOther => 'In attesa dell’altro computer…';

  @override
  String moveWrongCodesTyped(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Sono stati digitati codici sbagliati $n volte su altri computer.',
      one: 'È stato digitato un codice sbagliato su un altro computer.',
    );
    return '$_temp0';
  }

  @override
  String get moveThrowAwayInstead => 'Oppure scartalo…';

  @override
  String get moveFoundOther => 'Trovato il tuo altro computer';

  @override
  String get moveFoundSameNetwork => 'Stessa rete · cifrato tra i due';

  @override
  String get moveFoundInternet => 'Via internet · cifrato tra i due';

  @override
  String get moveConfirmThere => 'Conferma sull’altro computer per iniziare';

  @override
  String get moveSameAccountNote =>
      'Questo è l’account che è già su questo computer. Portarlo qui non fa perdere nulla.';

  @override
  String get moveTakeYes => 'Portalo qui';

  @override
  String get moveWelcomeBack => 'Eccoti di nuovo';

  @override
  String moveWelcomeBackNamed(String name) {
    return 'Eccoti di nuovo, $name';
  }

  @override
  String get moveDoneSub =>
      'Ora il tuo account è su questo computer. L’altro computer ha smesso di usarlo.';

  @override
  String get moveOpenFogata => 'Apri Fogata';

  @override
  String get moveCodeExpiredLabel => 'scaduto';

  @override
  String get moveCodeStoppedLabel => 'non funziona più';

  @override
  String get moveCodeExpiredNote =>
      'Questo codice è scaduto prima che il tuo altro computer lo usasse. I codici durano 10 minuti.';

  @override
  String get moveCodeStoppedNote =>
      'Sono stati digitati più volte codici sbagliati su altri computer, quindi questo codice ha smesso di funzionare.';

  @override
  String get moveNewCode => 'Nuovo codice';

  @override
  String get moveAlmostThere => 'Ci siamo quasi';

  @override
  String get moveLaterSub =>
      'Il tuo account è arrivato completo, ma non è stato ancora possibile attivarlo. Succederà al prossimo avvio di Fogata su questo computer.';

  @override
  String get moveStartAgain => 'Riavvia Fogata';

  @override
  String get moveUnconfirmedSub =>
      'È arrivato tutto, ma l’altro computer non ha confermato la fine. Mostra un codice e digitalo lì per completare il trasferimento.';

  @override
  String get moveStoppedThereTitle =>
      'Il trasferimento è stato interrotto sull’altro computer';

  @override
  String get moveStoppedThereSub =>
      'Non è cambiato nulla: il tuo account è ancora sull’altro computer e qui non ne è rimasto niente.';

  @override
  String get moveCouldNotStartTitle => 'Impossibile avviare il trasferimento';

  @override
  String get moveSomethingWrongHere =>
      'Qualcosa è andato storto su questo computer.';

  @override
  String get moveHalfwayTitle => 'Il trasferimento si è fermato a metà';

  @override
  String get moveHalfwaySub =>
      'La connessione con il tuo altro computer si è interrotta. Non è cambiato nulla: il tuo account è ancora sull’altro computer e qui non ne è rimasto niente.';

  @override
  String get moveShowCodeToFinish => 'Mostra un codice per completare';

  @override
  String get moveTryNewCode => 'Riprova con un nuovo codice';

  @override
  String moveBackToNamed(String name) {
    return 'Torna a $name';
  }

  @override
  String get moveBackToThisProfile => 'Torna a questo profilo';

  @override
  String get moveNewProfileInstead => 'Crea invece un nuovo profilo';

  @override
  String get moveDiscardTitle => 'Scartare l’account arrivato?';

  @override
  String get moveDiscardText =>
      'Fallo solo se l’altro computer continua a usarlo. Quel computer aspetta di confermare finché chi lo usa non sceglie di continuare a usarlo lì.';

  @override
  String get moveKeepIt => 'Tienilo';

  @override
  String get moveThrowAway => 'Scartalo';

  @override
  String get moveBringFromAnother =>
      'Porta il tuo account da un altro computer';

  @override
  String get moveAlreadyOnFogata => 'Sei già su Fogata?';

  @override
  String get moveCodeTriedBefore =>
      'Quel codice è già stato rifiutato una volta. Ottieni un nuovo codice sul nuovo computer.';

  @override
  String moveMovingTo(String computer) {
    return 'Trasferimento del tuo account su “$computer”';
  }

  @override
  String get moveKeepBothOnOffline =>
      'Tieni accesi entrambi i computer e Fogata aperto. Nel frattempo qui Fogata è offline.';

  @override
  String get moveCodeFieldLabel => 'Il codice mostrato sul nuovo computer';

  @override
  String get moveToAnotherSub =>
      'La tua identità, gli amici, i gruppi, le chat, i file e le impostazioni passano all’altro computer, direttamente da questo e cifrati. Poi questo computer smette di usare il tuo account, così non esiste mai un secondo te.';

  @override
  String moveStepChoose(String choice) {
    return 'Sul nuovo computer, apri Fogata e scegli $choice.';
  }

  @override
  String get moveStepTypeItShows => 'Digita il codice che mostra:';

  @override
  String get moveWaitingForAnswer => 'In attesa della tua risposta…';

  @override
  String get moveLookingForCode =>
      'Ricerca del computer che mostra questo codice…';

  @override
  String get moveOfflineWhileMoving =>
      'Durante il trasferimento Fogata va offline su questo computer.';

  @override
  String get moveWrongCodeNote =>
      'Il nuovo computer non ha accettato quel codice, quindi non è stato inviato nulla. Controllalo e digitalo di nuovo. Se non funziona ancora, ottieni un nuovo codice lì.';

  @override
  String get moveOthersAnswered =>
      'Più computer hanno risposto a questo codice e nessuno lo conosceva, quindi non è stato inviato nulla. Un altro dispositivo potrebbe cercare di impedire il trasferimento. Ottieni un nuovo codice sul nuovo computer e riprova.';

  @override
  String get moveUpdateBoth =>
      'L’altro computer ha una versione più vecchia di Fogata. Aggiorna entrambi i computer alla stessa versione e riprova. Non è stato trasferito nulla.';

  @override
  String get moveCantReachNew => 'Impossibile raggiungere il nuovo computer.';

  @override
  String get moveCantReachNewNote =>
      'Si sono cercati a vicenda per 3 minuti. Mettili sulla stessa rete (lo stesso Wi-Fi o un cavo) e riprova. Il tuo account è ancora qui e Fogata è di nuovo online.';

  @override
  String get moveDeclinedNote =>
      'Il trasferimento è stato interrotto sul nuovo computer. Non è stato trasferito nulla: il tuo account è ancora qui e Fogata è di nuovo online.';

  @override
  String get moveCouldNotStart => 'Impossibile avviare il trasferimento.';

  @override
  String get moveHalfwayNote =>
      'Il trasferimento si è fermato a metà: la connessione con il nuovo computer si è interrotta. Non è cambiato nulla: il tuo account è ancora qui e Fogata è di nuovo online.';

  @override
  String get moveThisProfileWords => 'questo profilo';

  @override
  String get moveBringCardTitle => 'Porta qui un account';

  @override
  String get moveBringCardSub =>
      'Mostra un codice su questo computer e digitalo su quello che ha l’account.';

  @override
  String get moveBringCardButton => 'Porta qui un account…';

  @override
  String moveReplacesHere(String name) {
    return 'Sostituisce $name su questo computer.';
  }

  @override
  String moveReplaceWarning(
    String bold,
    String name,
    int friends,
    int groups,
    int chats,
  ) {
    String _temp0 = intl.Intl.pluralLogic(
      friends,
      locale: localeName,
      other: '$friends amici',
      one: '1 amico',
    );
    String _temp1 = intl.Intl.pluralLogic(
      groups,
      locale: localeName,
      other: '$groups gruppi',
      one: '1 gruppo',
    );
    String _temp2 = intl.Intl.pluralLogic(
      chats,
      locale: localeName,
      other: '$chats chat',
      one: '1 chat',
    );
    return '$bold, il profilo attuale di questo computer, verrà sostituito e qui verranno eliminati $_temp0, $_temp1 e $_temp2. Per tenere $name, prima trasferiscilo su un altro computer.';
  }

  @override
  String moveReplaceAndShow(String name) {
    return 'Sostituisci $name e mostra un codice';
  }

  @override
  String moveConfirmTitle(String computer) {
    return 'Trasferire il tuo account su “$computer”?';
  }

  @override
  String get moveConfirmSameNetwork =>
      'Trovato sulla stessa rete. Tutto viaggia cifrato, direttamente a quel computer.';

  @override
  String get moveConfirmInternet =>
      'Trovato via internet. Tutto viaggia cifrato, direttamente a quel computer.';

  @override
  String moveConfirmReplaces(String name) {
    return 'Sostituisce il profilo $name su quel computer.';
  }

  @override
  String get moveConfirmNote =>
      'Al termine, questo computer smette di usare il tuo account e non potrà usarlo di nuovo. Poi potrai eliminare ciò che resta qui.';

  @override
  String get moveConfirmYes => 'Trasferisci il mio account';

  @override
  String get moveKeepTitle => 'Continuare a usare il tuo account qui?';

  @override
  String get moveKeepText =>
      'Solo se il nuovo computer non ce l’ha: prima apri Fogata lì. Se lo usassero entrambi, i tuoi amici e gruppi ti vedrebbero due volte, e i messaggi arriverebbero solo a uno dei due.';

  @override
  String get moveKeepYes => 'Continua a usarlo qui';

  @override
  String moveWaitingFor(String computer) {
    return 'In attesa della conferma di “$computer”';
  }

  @override
  String get moveWaitingText =>
      'Il tuo account è stato inviato, ma la connessione si è interrotta prima che il nuovo computer dicesse di essere pronto. Nel frattempo questo computer non userà l’account.';

  @override
  String get moveFinishText =>
      'Apri Fogata sul nuovo computer: mostra un codice. Digitalo qui.';

  @override
  String get moveFinish => 'Completa';

  @override
  String get moveFinishWrongCode =>
      'Il nuovo computer non ha accettato quel codice. Controllalo e digitalo di nuovo.';

  @override
  String get moveFinishNoContact =>
      'Impossibile raggiungere il nuovo computer. Mettili entrambi sulla stessa rete e riprova.';

  @override
  String get moveKeepThisOne =>
      'Il nuovo computer non ce l’ha: continua a usare questo…';

  @override
  String moveMovedYours(String computer) {
    return 'Il tuo account è stato trasferito su “$computer”';
  }

  @override
  String moveMovedNamed(String name, String computer) {
    return 'L’account di $name è stato trasferito su “$computer”';
  }

  @override
  String get moveMovedNoteYou =>
      'Questo computer non lo usa più: amici e gruppi ti raggiungono sull’altro.';

  @override
  String moveMovedNoteNamed(String name) {
    return 'Questo computer non lo usa più: amici e gruppi raggiungono $name sull’altro.';
  }

  @override
  String moveMovedTodayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'Oggi all’$time.',
      'other': 'Oggi alle $time.',
    });
    return '$_temp0';
  }

  @override
  String moveMovedOn(String when) {
    return '$when.';
  }

  @override
  String moveMovedWhenAndNote(String when, String note) {
    return '$when $note';
  }

  @override
  String get moveLeftHere => 'Cosa resta qui';

  @override
  String get moveLeftHereText =>
      'Chat, immagini e file di prima del trasferimento, cifrati. L’altro computer ne ha una copia propria.';

  @override
  String get moveDeleteData => 'Elimina i dati di Fogata su questo computer';

  @override
  String get moveAfterDeleting =>
      'Dopo l’eliminazione, Fogata qui riparte da zero.';

  @override
  String get moveCodeNotACode =>
      'Il codice ha dieci lettere e cifre, come XXXXX-XXXXX.';

  @override
  String get moveCodeMistyped =>
      'Quel codice non è corretto: controlla lettere e cifre.';

  @override
  String get welcomeHeadline => 'Parlate come se foste\nnella stessa stanza.';

  @override
  String get welcomeIntro =>
      'Voce, video e chat con amici, famiglia o il tuo team, direttamente dal tuo computer al loro.';

  @override
  String get welcomeNoAccount => 'Senza account';

  @override
  String get welcomeEncrypted => 'Cifrato';

  @override
  String get welcomeEphemeral => 'Stanze effimere';

  @override
  String get welcomeCharacter =>
      'Il personaggio di Fogata, una piccola fiamma con le cuffie che fa il segno della pace';

  @override
  String get welcomeImageUnreadable => 'Impossibile leggere quell’immagine.';

  @override
  String get welcomeKeychainTitle =>
      'I tuoi dati non si possono ancora cifrare';

  @override
  String welcomeKeychainText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' Su Linux il portachiavi è un Secret Service in esecuzione, come KWallet o GNOME Keyring: avviane o sbloccane uno, poi riprova.',
      'other': ' Sblocca o ripara il portachiavi, poi riprova.',
    });
    return 'Fogata cifra la tua identità, gli amici e i messaggi diretti con una chiave che tiene nel portachiavi di sistema, ma il portachiavi non è riuscito a conservarla.$_temp0';
  }

  @override
  String get welcomeKeychainRetry => 'Riprova';

  @override
  String get welcomeKeychainPlain => 'Continua senza cifratura';

  @override
  String get welcomeKeychainPlainNote =>
      'Senza cifratura, chiunque copi i file di questo computer può leggerli. Le impostazioni continueranno a segnalarlo, e Fogata cifrerà tutto la prima volta che si aprirà con un portachiavi.';

  @override
  String get welcomeProfileKeptTitle =>
      'Non è stato possibile leggere il tuo profilo';

  @override
  String welcomeProfileKeptText(String folder) {
    return 'Non è stato possibile leggere il profilo e gli amici creati su questo computer, quindi Fogata si apre come se fosse nuovo. Non è stato eliminato nulla: i file sono conservati in $folder.';
  }

  @override
  String get welcomeProfileKeptConfirm =>
      'Un nuovo profilo inizia senza amici. I file del vecchio profilo restano dove sono, e Fogata non li usa.';

  @override
  String get welcomeProfileKeptMake => 'Crea un nuovo profilo';

  @override
  String get welcomeWriteName => 'Scrivi il tuo nome.';

  @override
  String get welcomeCardTitle => 'Come vuoi che ti vedano i tuoi amici?';

  @override
  String get welcomeCardSub => 'Un nome e un’immagine. Cambiali quando vuoi.';

  @override
  String get welcomeNoServer =>
      'Nulla passa da un server di Fogata: non ce n’è nessuno. La rete pubblica IPFS serve solo come elenco telefonico per trovare i tuoi amici.';

  @override
  String get welcomeYourName => 'Il tuo nome';

  @override
  String welcomePictureLabel(String name) {
    return 'Immagine: $name';
  }

  @override
  String get welcomeUploadPicture => 'Carica un’immagine';

  @override
  String get welcomeNameLabel => 'Nome';

  @override
  String get welcomeCreate => 'Crea';

  @override
  String get welcomeReady => 'Pronto';

  @override
  String get welcomeCreateLabel => 'Crea l’identità';

  @override
  String welcomeKeyLabel(String key) {
    return 'Chiave creata su questo computer: $key';
  }

  @override
  String get welcomeNewKey => 'Crea un’altra chiave';

  @override
  String get stoppedTitle => 'Fogata ha smesso di funzionare.';

  @override
  String get stoppedStartingAgain => 'Riavvio in corso…';

  @override
  String get stoppedRestartToConnect => 'Riavvialo per riconnetterti.';

  @override
  String get stoppedRestartNow => 'Riavvia ora';

  @override
  String get settingsSectionProfile => 'Il mio profilo';

  @override
  String get settingsSectionMove => 'Trasferisci account';

  @override
  String get settingsSectionAppearance => 'Aspetto';

  @override
  String get settingsSectionLanguageTime => 'Lingua e ora';

  @override
  String get settingsSectionVoice => 'Voce e video';

  @override
  String get settingsSectionSounds => 'Suoni';

  @override
  String get settingsSectionBehaviour => 'Comportamento';

  @override
  String get settingsSectionNetwork => 'Rete';

  @override
  String get settingsSectionStorage => 'Spazio e download';

  @override
  String get settingsHeadingUser => 'Utente';

  @override
  String get settingsHeadingApp => 'App';

  @override
  String get settingsTitle => 'Impostazioni';

  @override
  String get settingsAll => 'Tutte le impostazioni';

  @override
  String get settingsCloseTooltip => 'Chiudi le impostazioni';

  @override
  String get settingsUnsavedSaveTitle => 'Salvare le modifiche?';

  @override
  String get settingsUnsavedDiscardTitle => 'Scartare le modifiche?';

  @override
  String get settingsUnsavedSaveText =>
      'Il tuo profilo ha modifiche non ancora salvate.';

  @override
  String get settingsUnsavedDiscardText =>
      'Il tuo profilo ha modifiche che non si possono salvare così come sono.';

  @override
  String get settingsProfileSaved =>
      'Profilo salvato. Amici e gruppi online lo vedono ora, gli altri quando si connettono.';

  @override
  String get settingsBioSaved =>
      'Bio salvata. Gli amici online la vedono ora, gli altri quando si connettono.';

  @override
  String settingsSwitchOn(String what) {
    return '$what: attivo';
  }

  @override
  String settingsSwitchOff(String what) {
    return '$what: disattivato';
  }

  @override
  String get settingsOn => 'Sì';

  @override
  String get settingsOff => 'No';

  @override
  String get settingsIdentity => 'La tua identità';

  @override
  String get settingsIdentityNote =>
      'Il tuo nome e la tua immagine arrivano firmati ai tuoi amici e gruppi, la tua bio ai tuoi amici.';

  @override
  String get settingsPicture => 'Immagine';

  @override
  String get settingsUploadPicture => 'Carica un’immagine o una GIF';

  @override
  String get settingsName => 'Nome';

  @override
  String get settingsBio => 'Bio';

  @override
  String get settingsBioHint => 'Qualche parola su di te';

  @override
  String get settingsNoBio => 'La tua scheda non mostra nessuna bio.';

  @override
  String settingsBioWhere(int lines) {
    return 'I tuoi amici la vedono sulla tua scheda, in massimo $lines righe.';
  }

  @override
  String get settingsBannerTitle => 'Sfondo del pannello';

  @override
  String get settingsBannerNote =>
      'La parte alta della tua scheda, per te e i tuoi amici.';

  @override
  String get settingsBannerAnother => 'Un’altra immagine';

  @override
  String get settingsBannerUpload => 'Usa un’immagine da questo computer';

  @override
  String get settingsFingerprint => 'Impronta di questo computer';

  @override
  String get settingsTheme => 'Tema';

  @override
  String get settingsThemeNote =>
      'I colori della finestra. Sistema segue l’impostazione del computer: Scuro o Chiaro.';

  @override
  String get settingsMicSpeaker => 'Microfono e altoparlante';

  @override
  String get settingsMicSpeakerNote =>
      'Con cosa le chiamate ti sentono e da dove esce l’audio. Si possono cambiare anche durante una chiamata.';

  @override
  String get settingsCamera => 'Videocamera';

  @override
  String get settingsCameraNote =>
      'Cosa mostrano di te le chiamate quando la videocamera è accesa. Provala qui prima di una chiamata.';

  @override
  String get settingsSoundEffects => 'Effetti sonori';

  @override
  String get settingsSoundEffectsNote =>
      'Entrata e uscita da una stanza, persone che vanno e vengono, messaggi in chat, il microfono e la condivisione.';

  @override
  String get settingsSounds => 'Suoni';

  @override
  String settingsChangeFailed(String reason) {
    return 'Fogata non è riuscito a cambiarlo: $reason';
  }

  @override
  String get settingsAutostartApprove =>
      'Consenti Fogata in Impostazioni di Sistema › Generali › Elementi login.';

  @override
  String get settingsAutostartOldMac => 'Richiede macOS 13 o successivo.';

  @override
  String get settingsAutostartUnsupported =>
      'Questa copia di Fogata non può aprirsi da sola.';

  @override
  String get settingsAutostartUnread =>
      'Fogata non è riuscito a leggere questa impostazione dal sistema.';

  @override
  String get settingsAutostart => 'Apri all’avvio';

  @override
  String get settingsAutostartNote =>
      'Apri Fogata all’avvio del computer, dopo l’accesso.';

  @override
  String get settingsKeepRunning => 'Resta in esecuzione';

  @override
  String get settingsKeepRunningNote =>
      'Quando la finestra è chiusa, Fogata resta nell’area di notifica, così gli amici possono ancora chiamarti.';

  @override
  String get settingsNoTray =>
      'Questo desktop non mostra icone nell’area di notifica, quindi chiudere la finestra chiude Fogata.';

  @override
  String get settingsHelping => 'Aiuto ai membri';

  @override
  String get settingsHelpingNote =>
      'Come questo computer aiuta i membri dei tuoi gruppi.';

  @override
  String get settingsHelpRelay => 'Collegare e fare da relay';

  @override
  String get settingsHelpConnect => 'Solo collegare';

  @override
  String get settingsHelpRelayNote =>
      'Quando due membri non riescono a connettersi tra loro, questo computer passa il loro traffico cifrato (senza vederlo), fino a 2 GB per chiamata. Usa il tuo upload.';

  @override
  String get settingsHelpConnectNote =>
      'Questo computer aiuta due membri a trovarsi ma non passa il loro traffico. Se non riescono a connettersi, non si sentiranno.';

  @override
  String get settingsRestartTray =>
      'La modifica ha effetto quando chiudi Fogata dalla sua icona nell’area di notifica e lo riapri.';

  @override
  String get settingsRestart =>
      'La modifica ha effetto quando chiudi Fogata e lo riapri.';

  @override
  String get settingsNetworkCheck => 'Verifica della rete';

  @override
  String get settingsNetworkCheckNote =>
      'Come questo computer vede la rete e cosa significa per raggiungere i tuoi amici. Non esce nulla da qui: guarda solo ciò che la connessione sa già.';

  @override
  String get settingsCheckNetwork => 'Verifica la rete';

  @override
  String get settingsCheckAgain => 'Verifica di nuovo';

  @override
  String get settingsDownloads => 'Download';

  @override
  String get settingsDownloadsNote =>
      'Dove vengono salvati i file scaricati da una stanza.';

  @override
  String settingsVersionData(String version, String path) {
    return 'Fogata $version · Dati in $path';
  }

  @override
  String get settingsNotEncrypted => 'Non cifrato su questo computer';

  @override
  String settingsNotEncryptedText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' Su Linux, il portachiavi è un Secret Service in esecuzione, come KWallet o GNOME Keyring: avviane o installane uno e Fogata cifrerà i suoi dati alla prossima apertura.',
      'other': ' Fogata riproverà alla prossima apertura.',
    });
    return 'La tua identità, gli amici e i messaggi diretti sono salvati su questo computer senza cifratura, perché non è stato trovato nessun portachiavi di sistema per custodire la chiave.$_temp0';
  }

  @override
  String get settingsNotAllEncrypted => 'Non tutto cifrato su questo computer';

  @override
  String get settingsNotAllEncryptedText =>
      'Una parte dei dati su questo computer (la tua identità, gli amici e i messaggi diretti) non è ancora cifrata: Fogata la sta ancora cifrando o non è riuscito a finire, per esempio perché il disco è pieno. Continua ogni volta che Fogata si apre.';

  @override
  String settingsDamagedSecret(String file) {
    return '$file non si apre con la chiave di questo computer: si è danneggiato sul disco. Finché resta lì la cifratura non può terminare, e una copia di prima della cifratura resta non cifrata. Mettilo da parte per terminare: viene conservato così com’è. Se è la tua identità o i tuoi amici, Fogata riprende quella copia e la cifra; il file di un gruppo o di una chat va perso su questo computer: il gruppo dovrà farti entrare di nuovo, e i messaggi precedenti della chat non si vedranno più.';
  }

  @override
  String get settingsSetDamagedAside => 'Mettilo da parte e riavvia';

  @override
  String get storageTitle => 'Spazio';

  @override
  String get storageNote =>
      'I tuoi messaggi diretti esistono solo sui tuoi dispositivi. Le chat delle stanze spariscono quando la stanza si chiude, quindi crescono solo questi.';

  @override
  String get storageDirect => 'Messaggi diretti';

  @override
  String get storageOfNoLimit => 'senza limite';

  @override
  String storageOfLimit(String limit) {
    return 'su $limit';
  }

  @override
  String storageLegendPictures(String size) {
    return 'Immagini $size';
  }

  @override
  String storageLegendVideos(String size) {
    return 'Video $size';
  }

  @override
  String storageLegendFiles(String size) {
    return 'File $size';
  }

  @override
  String storageLegendMessages(String size) {
    return 'Messaggi $size';
  }

  @override
  String get storageRoom => 'Spazio per i messaggi diretti';

  @override
  String get storageNoLimitNote => 'Non viene rimosso nulla in automatico.';

  @override
  String storageOverNote(String limit) {
    return 'Oltre il limite: le immagini, i video e i file più vecchi spariscono, fino a $limit. I messaggi restano sempre.';
  }

  @override
  String get storageLimitNote =>
      'Oltre il limite, spariscono prima le immagini, i video e i file più vecchi. I messaggi restano sempre; al loro posto compare “Rimosso qui per liberare spazio”.';

  @override
  String get storageAutoPictures => 'Scarica le immagini in automatico';

  @override
  String get storageAutoPicturesNote =>
      'Fino a 10 MB ciascuna, appena arrivano.';

  @override
  String get storageAutoFiles => 'Scarica video e file in automatico';

  @override
  String get storageAutoFilesNote =>
      'Se disattivato: arrivano quando li apri, mentre chi li ha inviati è online.';

  @override
  String get storageTimer => 'Messaggi effimeri nelle nuove chat';

  @override
  String get storageTimerNote =>
      'Ogni chat può cambiarlo; entrambe le persone vedono sempre lo stesso timer.';

  @override
  String get storageTimer24h => '24 h';

  @override
  String get storageByChat => 'Per chat';

  @override
  String get storageByChatNote =>
      'Cancellando si rimuovono qui immagini, video e file. I messaggi restano, e l’altra persona conserva la sua copia.';

  @override
  String get storageMessagesOnly => 'Solo messaggi';

  @override
  String get storageClearFiles => 'Cancella i file';

  @override
  String storageClearFilesTooltip(String name) {
    return 'Cancella i file della chat con $name';
  }

  @override
  String get storageCleared => 'Cancellati';

  @override
  String storageConfirmTitle(String limit) {
    return 'Tenere $limit di immagini, video e file?';
  }

  @override
  String storageConfirmText(String limit) {
    return 'I più vecchi spariscono ora, fino a $limit, per ultimi quelli dei messaggi fissati. I messaggi restano sempre, e i tuoi amici conservano le loro copie.';
  }

  @override
  String get storageRemoveOldest => 'Rimuovi i più vecchi';

  @override
  String get themePickerSystem =>
      'Tema di sistema, segue l’impostazione del computer';

  @override
  String themePickerTheme(String name) {
    return 'Tema $name';
  }

  @override
  String get themeDark => 'Scuro';

  @override
  String get themeMidnight => 'Mezzanotte';

  @override
  String get themeDim => 'Tenue';

  @override
  String get themeEmber => 'Brace';

  @override
  String get themeContrast => 'Contrasto';

  @override
  String get themeLight => 'Chiaro';

  @override
  String get themeSystem => 'Sistema';

  @override
  String get themeBannerViolet => 'Viola';

  @override
  String get themeBannerBlue => 'Blu';

  @override
  String get themeBannerGreen => 'Verde';

  @override
  String get themeBannerClay => 'Argilla';

  @override
  String get themeBannerRaspberry => 'Lampone';

  @override
  String get themeBannerMustard => 'Senape';

  @override
  String get themeBannerSlate => 'Ardesia';

  @override
  String get themeBannerNight => 'Notte';

  @override
  String widgetsCopyWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Copia il codice QR',
      'link': 'Copia il link',
      'code': 'Copia il codice',
      'other': 'Copia',
    });
    return '$_temp0';
  }

  @override
  String widgetsShowWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Mostra il codice QR',
      'link': 'Mostra il link',
      'code': 'Mostra il codice',
      'key': 'Mostra la chiave',
      'other': 'Mostra',
    });
    return '$_temp0';
  }

  @override
  String get widgetsHide => 'Nascondi';

  @override
  String get widgetsShowMore => 'Mostra altro';

  @override
  String get widgetsShowLess => 'Mostra meno';

  @override
  String revealFailed(String path) {
    return 'Impossibile aprire la sua cartella. Si trova in $path';
  }

  @override
  String get emojiGroupPeople => 'Persone';

  @override
  String get emojiGroupNature => 'Natura';

  @override
  String get emojiGroupFood => 'Cibo e bevande';

  @override
  String get emojiGroupActivities => 'Attività';

  @override
  String get emojiGroupTravel => 'Viaggi e luoghi';

  @override
  String get emojiGroupObjects => 'Oggetti';

  @override
  String get emojiGroupSymbols => 'Simboli';

  @override
  String get emojiGroupFlags => 'Bandiere';

  @override
  String get emojiPickerLimit => 'Il messaggio ha raggiunto il limite.';

  @override
  String get emojiPickerButton => 'Emoji';

  @override
  String get emojiPickerResults => 'Risultati';

  @override
  String get emojiPickerRecent => 'Usate di recente';

  @override
  String get emojiPickerSearch => 'Cerca emoji';

  @override
  String get emojiPickerSkinTone => 'Tono della pelle';

  @override
  String get emojiPickerNone => 'Nessuna emoji trovata';

  @override
  String get emojiPickerNoFont =>
      'Questo computer non ha un font di emoji a colori, quindi le emoji potrebbero apparire semplici o vuote. Installare Noto Color Emoji risolve il problema.';

  @override
  String get emojiPickerDefaultTone => 'Tono della pelle predefinito';

  @override
  String emojiPickerTone(int n) {
    return 'Tono della pelle $n';
  }

  @override
  String get iconsGroupMountain => 'Montagna';

  @override
  String get iconsGroupStar => 'Stella';

  @override
  String get iconsGroupGameController => 'Controller';

  @override
  String get iconsGroupMusic => 'Musica';

  @override
  String get iconsGroupBook => 'Libro';

  @override
  String get iconsGroupWaves => 'Onde';

  @override
  String get iconsGroupCoffee => 'Caffè';

  @override
  String get iconsGroupHeart => 'Cuore';

  @override
  String get iconsGroupFilm => 'Film';

  @override
  String get newGroupPictureUnreadable => 'Impossibile leggere quell’immagine.';

  @override
  String newGroupMade(String name) {
    return 'Gruppo “$name” creato. I membri lo ricevono quando tornano online.';
  }

  @override
  String get newGroupTitle => 'Nuovo gruppo';

  @override
  String get newGroupSubtitle => 'Il gruppo è permanente. Le sue stanze no.';

  @override
  String get newGroupName => 'Nome del gruppo';

  @override
  String get newGroupNameExample => 'Giro della domenica';

  @override
  String get newGroupIconOrPicture => 'Icona o immagine';

  @override
  String get newGroupChooseFromComputer => 'Scegli dal computer';

  @override
  String get newGroupBannerOptional => 'Banner (facoltativo)';

  @override
  String get newGroupMembers => 'Membri';

  @override
  String newGroupSelected(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n selezionati',
      one: '1 selezionato',
    );
    return '$_temp0';
  }

  @override
  String get newGroupSearchPlaceholder => 'Cerca tra i tuoi amici';

  @override
  String get newGroupSearchLabel => 'Cerca amici';

  @override
  String get newGroupNoFriends =>
      'Ancora nessun amico. Puoi creare il gruppo e aggiungerli dopo, o invitare persone con un link.';

  @override
  String get newGroupKeptOnDevices =>
      'Conservato sul dispositivo di ogni membro.';

  @override
  String get newGroupCreate => 'Crea gruppo';

  @override
  String get groupBannerSettings => 'Impostazioni del gruppo';

  @override
  String get groupBannerPictureUnreadable =>
      'Impossibile leggere quell’immagine.';

  @override
  String get groupBannerChoose => 'Scegli un banner';

  @override
  String get groupBannerAnother => 'Altro banner';

  @override
  String get groupBannerNone => 'Nessun banner';

  @override
  String get groupBannerNote =>
      'Mostrato in cima alla colonna del gruppo, sotto il nome. PNG, JPEG, oppure una GIF o WebP animata fino a 550 KB.';

  @override
  String get homeTitle => 'Home';

  @override
  String get homeSubtitle =>
      'Le stanze si accendono quando qualcuno entra e si spengono quando escono tutti.';

  @override
  String get homeAddFriend => 'Aggiungi un amico';

  @override
  String get homeNewGroup => 'Nuovo gruppo';

  @override
  String get homeLive => 'In diretta';

  @override
  String get homeNoRoom => 'Nessuna stanza';

  @override
  String get homeNoRoomNote => 'Apri una stanza per accenderla';

  @override
  String homeInviteBy(String name, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n membri',
      one: '1 membro',
    );
    return '$name ti ha aggiunto · $_temp0';
  }

  @override
  String get homeInviteNote =>
      'Finché non ti unisci, questo computer non si connette a loro e non invia loro nulla.';

  @override
  String get homeInviteJoin => 'Unisciti';

  @override
  String get homeNoGroups => 'Ancora nessun gruppo';

  @override
  String get homeNoGroupsNote =>
      'Aggiungi i tuoi amici e crea un gruppo con loro. Il gruppo è permanente; le sue stanze no.';

  @override
  String homePersonYou(String name) {
    return '$name (tu)';
  }

  @override
  String homeMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n membri',
      one: '1 membro',
    );
    return '$_temp0';
  }

  @override
  String get homeYouAreIn => 'Sei dentro';

  @override
  String get homeMuteMic => 'Disattiva il microfono';

  @override
  String get homeUnmuteMic => 'Attiva il microfono';

  @override
  String get homeCameraOff => 'Spegni la videocamera';

  @override
  String get homeCameraOn => 'Accendi la videocamera';

  @override
  String get homeMembersTooltip => 'Membri';

  @override
  String get homeReturnToRoom => 'Torna alla stanza';

  @override
  String get homeJoinRoom => 'Entra';

  @override
  String homeMembersOnline(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membri',
      one: '1 membro',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'nessun altro online',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get homeNoRoomsYet => 'Ancora nessuna stanza';

  @override
  String homeLastRoom(String when) {
    return 'Ultima stanza $when';
  }

  @override
  String get homeOpenRoom => 'Apri la stanza';

  @override
  String get membersGroupGone => 'Questo gruppo non c’è più.';

  @override
  String membersSummary(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membri',
      one: '1 membro',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'nessun altro online',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String membersSummaryLastRoom(int members, int online, String when) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membri',
      one: '1 membro',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'nessun altro online',
    );
    return '$_temp0 · $_temp1 · ultima stanza $when';
  }

  @override
  String get membersCopyInviteLink => 'Copia link d’invito';

  @override
  String membersInviteLinkTooltip(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'qualcuno del gruppo',
      'first': 'il primo del gruppo',
      'other': 'uno dei primi $n del gruppo',
    });
    return 'Funziona una volta, per 24 ore, e $_temp0 fa entrare la persona';
  }

  @override
  String membersInviteLinkCopied(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'qualcuno del gruppo',
      'first': 'il primo del gruppo',
      'other': 'uno dei primi $n del gruppo',
    });
    return 'Link d’invito copiato. Funziona una volta, per 24 ore, e $_temp0 fa entrare la persona.';
  }

  @override
  String get membersInviteFriends => 'Invita amici';

  @override
  String get membersBackToRoom => 'Torna alla stanza';

  @override
  String get membersOpenRoom => 'Apri la stanza';

  @override
  String get membersEnterRoom => 'Entra nella stanza';

  @override
  String get membersTitle => 'Membri';

  @override
  String get membersOrderNote => 'In ordine: ognuno agisce su chi sta sotto';

  @override
  String get membersGroupSettings => 'Impostazioni del gruppo';

  @override
  String membersManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Solo i primi $n possono modificare il gruppo.',
      one: 'Solo il primo può modificare il gruppo.',
    );
    return '$_temp0';
  }

  @override
  String get membersLeaveGroup => 'Esci dal gruppo';

  @override
  String membersInviteTitle(String group) {
    return 'Invita amici in “$group”';
  }

  @override
  String get membersInviteNote => 'Ricevono un invito e scelgono se unirsi.';

  @override
  String get membersPresenceYou => 'tu';

  @override
  String membersPresenceOnline(String how) {
    return 'online · $how';
  }

  @override
  String membersPresenceAway(String how) {
    return 'assente · $how';
  }

  @override
  String get membersPresenceOffline => 'offline al momento';

  @override
  String membersYou(String name) {
    return '$name (tu)';
  }

  @override
  String get membersWaitingForKey => 'in attesa della chiave';

  @override
  String membersFingerprint(String fingerprint) {
    return 'Impronta $fingerprint';
  }

  @override
  String membersOptionsFor(String name) {
    return 'Opzioni per $name';
  }

  @override
  String get membersMoveUp => 'Sposta subito sotto di me';

  @override
  String get membersMoveToEnd => 'Sposta in fondo';

  @override
  String get membersTimeOut => 'Sospendi da una stanza…';

  @override
  String get membersRemoveFromGroup => 'Rimuovi dal gruppo';

  @override
  String membersRemoveTitle(String name) {
    return 'Rimuovere $name?';
  }

  @override
  String membersRemoveText(String name) {
    return '$name esce dal gruppo e il gruppo crea una nuova chiave. Ciò che ha già visto, lo conserva.';
  }

  @override
  String get membersFirst => 'Primo posto';

  @override
  String membersTimeOutTitle(String name) {
    return 'Sospendi $name';
  }

  @override
  String membersTimeOutText(String name) {
    return '$name esce dalla stanza e potrà rientrare solo allo scadere del tempo o se qualcuno più in alto glielo consente.';
  }

  @override
  String get membersUntilLetBack => 'Fino a revoca';

  @override
  String membersLeaveTitle(String group) {
    return 'Uscire da “$group”?';
  }

  @override
  String get membersLeaveText =>
      'Smetti di vedere le sue stanze e gli altri smettono di vederti nel gruppo. Il gruppo crea una nuova chiave senza di te. Per tornare, qualcuno del gruppo dovrà farti entrare di nuovo.';

  @override
  String membersLeaveTextFirst(String name) {
    return 'Smetti di vedere le sue stanze e gli altri smettono di vederti nel gruppo. Il gruppo crea una nuova chiave senza di te. Per tornare, qualcuno del gruppo dovrà farti entrare di nuovo. $name passa al primo posto.';
  }

  @override
  String get groupSettingsTabGeneral => 'Generale';

  @override
  String get groupSettingsTabVoiceRooms => 'Stanze vocali';

  @override
  String get groupSettingsTabMembers => 'Membri';

  @override
  String get groupSettingsTabInvite => 'Inviti';

  @override
  String get groupSettingsTabLimits => 'Limiti';

  @override
  String get groupSettingsTabActivity => 'Attività';

  @override
  String get groupSettingsFirstHere => 'Primo posto in questo gruppo';

  @override
  String groupSettingsPlaceHere(String place) {
    return '$place in questo gruppo';
  }

  @override
  String get groupSettingsLeave => 'Esci dal gruppo';

  @override
  String get groupSettingsInviteTitle => 'Invita persone';

  @override
  String get groupSettingsClose => 'Chiudi le impostazioni del gruppo';

  @override
  String get groupSettingsPicture => 'Immagine';

  @override
  String get groupSettingsChooseFromComputer => 'Scegli dal computer';

  @override
  String get groupSettingsUseIcon => 'Usa un’icona';

  @override
  String get groupSettingsPictureNote =>
      'PNG, JPEG o WebP, ritagliata a quadrato e rimpicciolita come le immagini del profilo, oppure una GIF o WebP animata fino a 550 KB.';

  @override
  String get groupSettingsOrIcon => 'Oppure un’icona';

  @override
  String get groupSettingsBanner => 'Banner';

  @override
  String get groupSettingsName => 'Nome';

  @override
  String get groupSettingsNameField => 'Nome del gruppo';

  @override
  String get groupSettingsNameNote =>
      'Tutti nel gruppo vedono il nuovo nome e la nuova immagine.';

  @override
  String get groupSettingsRoomsIntro =>
      'Ogni stanza vocale ha le sue persone, la sua chat, i suoi file e le sue chiamate: chi è in una non si connette con chi è in un’altra. Si sta in una stanza alla volta.';

  @override
  String get groupSettingsRoomName => 'Nome della stanza';

  @override
  String get groupSettingsAccessEveryone => 'Tutti';

  @override
  String get groupSettingsAccessSome => 'Solo alcuni';

  @override
  String groupSettingsWhoCanEnter(String room) {
    return 'Chi può entrare in $room';
  }

  @override
  String get groupSettingsFirstRoomStays => 'La prima stanza resta';

  @override
  String get groupSettingsPeopleInside => 'Ci sono persone dentro';

  @override
  String get groupSettingsDeleteRoom => 'Elimina la stanza';

  @override
  String get groupSettingsAddRoom => 'Aggiungi stanza vocale';

  @override
  String groupSettingsNewRoomName(int n) {
    return 'Stanza $n';
  }

  @override
  String groupSettingsRoomsNote(int rooms, int people) {
    return 'Fino a $rooms stanze, con $people persone ciascuna (Limiti). Una stanza con persone dentro non si può eliminare.';
  }

  @override
  String get groupSettingsKeptOut => 'Esclusi';

  @override
  String get groupSettingsAccessEveryoneLong => 'Tutti nel gruppo';

  @override
  String get groupSettingsAccessChosen => 'Solo i membri che scelgo io';

  @override
  String get groupSettingsAccessNote =>
      'Tu e chi sta sopra di te potete sempre entrare. Chi è escluso vede la stanza con un lucchetto.';

  @override
  String groupSettingsYou(String name) {
    return '$name (tu)';
  }

  @override
  String groupSettingsKeptOutBy(String room, String by) {
    return ' · $room · da $by';
  }

  @override
  String groupSettingsTimeLeft(String time) {
    return 'Ancora $time';
  }

  @override
  String get groupSettingsUntilLetBack => 'Fino a revoca';

  @override
  String get groupSettingsLetBack => 'Fai rientrare';

  @override
  String groupSettingsManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Solo i primi $n possono modificare il gruppo: nome, stanze, inviti e limiti. Tu agisci su chi sta sotto di te.',
      one: 'Solo il primo può modificare il gruppo: nome, stanze, inviti e limiti. Tu agisci su chi sta sotto di te.',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsMembersCount(int n, int max) {
    String _temp0 = intl.Intl.pluralLogic(
      max,
      locale: localeName,
      other: '$max membri',
      one: '1 membro',
    );
    return '$n su $_temp0, in ordine';
  }

  @override
  String get groupSettingsMembersNote =>
      'Ognuno agisce sulle persone sotto di sé: le sposta su fino a subito sotto di sé o giù in fondo, le sospende da una stanza o le rimuove. Chi entra parte dal fondo.';

  @override
  String get groupSettingsWhoManages => 'Chi gestisce questo gruppo';

  @override
  String get groupSettingsManagersEveryone => 'Tutti';

  @override
  String groupSettingsManagersFirstN(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'I primi $n',
      one: 'Il primo',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsManagersMore(int n, int members) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members',
      one: '1',
    );
    return 'Impostato sui primi $n, più di quanti sono ora nel gruppo ($_temp0): per ora significa tutti.';
  }

  @override
  String get groupSettingsManagersEveryoneNote =>
      'Chi gestisce cambia il nome, le immagini, le stanze e i limiti, e fa entrare le persone. Se gestiscono tutti, il gruppo va avanti anche se qualcuno perde il proprio dispositivo.';

  @override
  String groupSettingsManagersFirstNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Solo i primi $n cambiano il nome, le immagini, le stanze e i limiti, e fanno entrare le persone. Se perdono tutti i loro dispositivi, nessuno può più gestire il gruppo: Fogata non ha un server che possa rimediare.',
      one: 'Solo il primo cambia il nome, le immagini, le stanze e i limiti, e fa entrare le persone. Se quel dispositivo va perso, nessuno può più gestire il gruppo: Fogata non ha un server che possa rimediare.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsMoveUp => 'Sposta su';

  @override
  String groupSettingsMoveUpTooltip(String name) {
    return 'Sposta $name subito sotto di te';
  }

  @override
  String get groupSettingsToEnd => 'In fondo';

  @override
  String groupSettingsToEndTooltip(String name) {
    return 'Sposta $name in fondo';
  }

  @override
  String get groupSettingsTimeOut => 'Sospendi da una stanza';

  @override
  String get groupSettingsRemove => 'Rimuovi dal gruppo';

  @override
  String get groupSettingsAskingToJoin => 'Chiedono di unirsi';

  @override
  String get groupSettingsInviteFriend => 'Invita un amico';

  @override
  String get groupSettingsInviteFriendNote =>
      'Ricevono un invito in Fogata e scelgono se unirsi. Qui compaiono solo i tuoi amici.';

  @override
  String get groupSettingsInviteLink => 'Link d’invito';

  @override
  String groupSettingsInviteLinkNote(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'qualcuno del gruppo',
      'first': 'il primo del gruppo',
      'other': 'uno dei primi $n del gruppo',
    });
    return 'Per chi non è ancora tuo amico. Funziona una volta e per 24 ore, e $_temp0 fa entrare la persona: se arriva a qualcun altro, vedi chi chiede di entrare e puoi rifiutare.';
  }

  @override
  String get groupSettingsCopyLink => 'Copia link d’invito';

  @override
  String groupSettingsOpenLinks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n link non ancora usati.',
      one: '1 link non ancora usato.',
      zero: 'Nessun link in attesa di essere usato.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsCancelLinks => 'Annullali';

  @override
  String get groupSettingsNoFriends =>
      'Ancora nessun amico: aggiungili in Amici.';

  @override
  String get groupSettingsAllFriendsIn => 'Tutti i tuoi amici sono nel gruppo.';

  @override
  String get groupSettingsFriendOnline => 'Online';

  @override
  String get groupSettingsFriendOffline => 'Offline al momento';

  @override
  String get groupSettingsInviteButton => 'Invita';

  @override
  String get groupSettingsGroupFull => 'Il gruppo è pieno';

  @override
  String get groupSettingsSayNo => 'Rifiuta';

  @override
  String get groupSettingsLetIn => 'Fai entrare';

  @override
  String get groupSettingsLimitGroup => 'Persone nel gruppo';

  @override
  String groupSettingsLimitGroupNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'le $n persone già presenti',
      one: 'la persona già presente',
    );
    return 'Da 2 a 20. Quando è pieno, inviti e link smettono di funzionare. Non può scendere sotto $_temp0.';
  }

  @override
  String get groupSettingsLimitRoom => 'Persone in ogni stanza vocale';

  @override
  String get groupSettingsLimitRoomNote =>
      'Da 2 a 8. Voce e video viaggiano direttamente tra tutti quelli nella stanza, quindi 8 è il massimo che suona bene; con le videocamere accese, meno persone funzionano meglio. Chi arriva quando è piena vede che la stanza è piena.';

  @override
  String get shellChipCallMany => 'Diretta · cifrata · solo chi è in chiamata';

  @override
  String get shellChipCallTwo => 'Diretta · cifrata · solo voi due';

  @override
  String get shellGroupMembers => 'Membri del gruppo';

  @override
  String get shellMembers => 'Membri';

  @override
  String get shellDirectMessages => 'Messaggi diretti';

  @override
  String get shellPathDirect => 'Diretta · cifrata';

  @override
  String shellPathVia(String name) {
    return 'Tramite $name · cifrata';
  }

  @override
  String get shellPathViaSomeone => 'Tramite un membro · cifrata';

  @override
  String shellCallWith(String names) {
    return 'Chiamata con $names';
  }

  @override
  String get shellRinging => 'Squilla · ';

  @override
  String get shellInCall => 'In chiamata · ';

  @override
  String get shellCallEnded => 'Chiamata terminata';

  @override
  String get shellNewGroup => 'Nuovo gruppo';

  @override
  String get shellSectionCall => 'Chiamata';

  @override
  String shellCallRinging(String name) {
    return '$name · squilla';
  }

  @override
  String get shellSectionLiveNow => 'In diretta ora';

  @override
  String get shellSectionInCall => 'In chiamata';

  @override
  String get shellChat => 'Chat';

  @override
  String get shellFiles => 'File';

  @override
  String get shellSectionGroup => 'Gruppo';

  @override
  String get shellInvite => 'Invita';

  @override
  String get shellSectionVoiceRooms => 'Stanze vocali';

  @override
  String shellVoiceRoomYouAreIn(String room) {
    return '$room · ci sei tu';
  }

  @override
  String get shellNotForYou => 'Non per te ora';

  @override
  String get shellOnlySomeMembers => 'Solo alcuni membri';

  @override
  String shellPersonYou(String name) {
    return '$name (tu)';
  }

  @override
  String get shellLive => 'LIVE';

  @override
  String get shellYourProfile => 'Il tuo profilo';

  @override
  String shellMuteMic(String keys) {
    return 'Disattiva il microfono ($keys)';
  }

  @override
  String shellUnmuteMic(String keys) {
    return 'Attiva il microfono ($keys)';
  }

  @override
  String get shellDeafen => 'Disattiva l’audio';

  @override
  String get shellUndeafen => 'Senti di nuovo gli altri';

  @override
  String get shellStateCalling => 'Squilla…';

  @override
  String get shellStateInCall => 'In chiamata';

  @override
  String get shellStateInRoom => 'Nella stanza';

  @override
  String get shellStateOffline => 'Offline';

  @override
  String get shellStateOnline => 'Online';

  @override
  String get shellStateAway => 'Assente';

  @override
  String get shellShowInFolder => 'Mostra nella cartella';

  @override
  String get shellCopyPath => 'Copia percorso';

  @override
  String shellVolumePercent(int n) {
    return '$n%';
  }

  @override
  String get shellMuteKeys => 'Ctrl+Maiusc+M';

  @override
  String get settingsSectionDonate => 'Dona a Fogata';

  @override
  String get settingsSectionReport => 'Segnala un problema';

  @override
  String get settingsHeadingHelp => 'Aiuto';

  @override
  String get settingsPreRelease => 'Anteprima';

  @override
  String get donateTitle => 'Mantieni Fogata indipendente';

  @override
  String get donateText =>
      'Fogata è gratuito, non ha pubblicità né server che ti osservano. La tua donazione fa sì che resti così.';

  @override
  String get donateButton => 'Dona';

  @override
  String get donateNote =>
      'Apre Ko-fi nel tuo browser. Fogata non vede mai il tuo pagamento.';

  @override
  String get reportWhatTitle => 'Cos’è successo?';

  @override
  String get reportWhatNote =>
      'Cosa stavi facendo, cosa ti aspettavi e cosa hai visto invece.';

  @override
  String get reportIncludeLog =>
      'Includi il registro tecnico degli ultimi 2 giorni';

  @override
  String get reportLogNote =>
      'Versioni, errori e passaggi di connessione. Mai i tuoi messaggi, chiavi, contatti o indirizzi IP.';

  @override
  String get reportSeeSent => 'Vedi cosa viene inviato';

  @override
  String get reportHideSent => 'Nascondi l’elenco';

  @override
  String get reportPreview => 'Leggi tutto il rapporto prima di inviarlo';

  @override
  String get reportHidePreview => 'Nascondi il rapporto';

  @override
  String get reportPreviewNote =>
      'Questo è esattamente ciò che contiene il file, ed è tutto ciò che parte. Nomi, cartelle, nomi dei file, indirizzi e chiavi sono già stati tolti dal registro.';

  @override
  String get reportPreviewWorking => 'Preparazione del rapporto…';

  @override
  String get reportSentWhat => 'Quello che hai scritto sopra.';

  @override
  String get reportSentVersion =>
      'La versione e la build di Fogata, il tuo sistema e la sua versione, e la lingua di Fogata.';

  @override
  String get reportSentLog =>
      'Il registro degli ultimi 2 giorni: quando si è avviato Fogata, i suoi errori e come ha raggiunto gli altri dispositivi. Nomi, messaggi, cartelle, nomi dei file, link, codici, chiavi e indirizzi IP vengono tolti, e ogni dispositivo compare solo come un numero che fuori dal rapporto non significa nulla.';

  @override
  String get reportEmail => 'Invialo via email al team di Fogata';

  @override
  String get reportSave => 'Salva in un file';

  @override
  String reportHowEmail(String address) {
    return 'La tua app di posta si apre con il messaggio pronto per $address, e accanto si apre la cartella con il file del rapporto: allega il file e invia. Nulla esce dal tuo computer finché non lo invii tu.';
  }

  @override
  String get reportReadyTitle => 'La tua email è pronta';

  @override
  String reportReadyText(String file, String address) {
    return 'Allega $file dalla cartella che si è aperta e invialo a $address.';
  }

  @override
  String get reportShowFile => 'Mostra il file';

  @override
  String get reportCopyAddress => 'Copia l’indirizzo';

  @override
  String get reportAddressCopied => 'Indirizzo copiato';

  @override
  String reportSaved(String file) {
    return 'Rapporto salvato: $file';
  }

  @override
  String reportNoMailApp(String address) {
    return 'Non si è aperta nessuna app di posta. Invia il file a $address dalla posta che usi.';
  }

  @override
  String reportFailed(String reason) {
    return 'Impossibile scrivere il rapporto: $reason';
  }

  @override
  String get splashStarting => 'Avvio';

  @override
  String get splashConnecting => 'Connessione';

  @override
  String get splashChecking => 'Ricerca aggiornamenti';

  @override
  String get splashDownloading => 'Download dell’aggiornamento';

  @override
  String get splashInstalling => 'Installazione dell’aggiornamento';

  @override
  String get splashRestarting => 'Riavvio';

  @override
  String splashProgress(String done, String total) {
    return '$done di $total MB';
  }

  @override
  String splashProgressFrom(String done, String total, String name) {
    return '$done di $total MB · da $name';
  }

  @override
  String updateReady(String version) {
    return 'Aggiornamento pronto · $version';
  }

  @override
  String updateRequired(String version) {
    return 'Aggiornamento necessario · $version';
  }

  @override
  String get updateRestart => 'Riavvia';

  @override
  String updateReadyTip(String version) {
    return 'Fogata $version è arrivato. Riavvia per usarlo.';
  }

  @override
  String get updateRequiredTip =>
      'Questa versione non funziona più con quella più recente dei tuoi amici. Riavvia per aggiornare.';

  @override
  String updateFailed(String reason) {
    return 'Impossibile installare l’aggiornamento: $reason';
  }

  @override
  String get errJoinInviteGone =>
      'Quell’invito non funziona più, oppure il gruppo è pieno.';

  @override
  String get errJoinNoKeyYet =>
      'Questo dispositivo non ha ancora la chiave del gruppo. Il nuovo membro entrerà appena l’avrà.';

  @override
  String get errJoinWroteTooMuch =>
      'Questo dispositivo ha fatto modifiche che gli altri membri non hanno ancora ricevuto. Il nuovo membro potrà entrare quando un altro membro sarà online.';

  @override
  String get errJoinAskAgain =>
      'Quella richiesta di unirsi è stata fatta su uno stato precedente del gruppo. La persona deve chiederlo di nuovo.';

  @override
  String get errNoSpaceForFile =>
      'Non c’è abbastanza spazio libero per questo file. Libera spazio e riprova.';

  @override
  String get recoveryTitle => 'Copia di recupero';

  @override
  String get recoveryNote =>
      'Un file che ripristina questo account se il portachiavi o il disco di questo computer vanno persi. Solo la tua chiave di recupero lo apre.';

  @override
  String get recoveryOff => 'Disattivata';

  @override
  String recoveryOn(String id) {
    return 'Attiva · chiave $id';
  }

  @override
  String get recoveryTurnOn => 'Attiva…';

  @override
  String get recoveryChangeKey => 'Cambia chiave…';

  @override
  String get recoveryMakeCopy => 'Fai una copia…';

  @override
  String get recoveryReplacedKept =>
      'Questo computer conserva ancora un account sostituito da un ripristino.';

  @override
  String get recoveryDeleteReplaced => 'Eliminalo…';

  @override
  String get recoveryChangeTitle => 'Creare una nuova chiave di recupero?';

  @override
  String get recoveryChangeText =>
      'Le nuove copie si apriranno solo con la nuova chiave. Quelle fatte prima continuano ad aprirsi solo con la vecchia: conservala finché conservi quelle copie.';

  @override
  String get recoveryChangeConfirm => 'Crea una nuova chiave';

  @override
  String get recoveryKeyTitle => 'La tua chiave di recupero';

  @override
  String get recoveryKeyText =>
      'Scrivila o stampala, e tienila separata dalle tue copie. Viene mostrata solo questa volta.';

  @override
  String recoveryKeyName(String id) {
    return 'Il suo nome breve: $id';
  }

  @override
  String get recoveryKeyWarning =>
      'Fogata non conserva questa chiave da nessuna parte: senza, nessuna copia si apre. Chi ha una copia e questa chiave ha il tuo account.';

  @override
  String get recoveryKeyWritten => 'L’ho scritta';

  @override
  String get recoveryCheckTitle => 'Controlla quello che hai scritto';

  @override
  String get recoveryCheckText =>
      'Digita questi due gruppi della chiave, come li hai annotati.';

  @override
  String recoveryCheckGroup(int n) {
    return 'Gruppo $n';
  }

  @override
  String get recoveryCheckWrong =>
      'Non è questo: ricontrolla quello che hai scritto.';

  @override
  String get recoveryTurnOnButton => 'Attiva le copie';

  @override
  String recoveryTurnedOn(String id) {
    return 'Le copie sono attive, con la chiave $id.';
  }

  @override
  String get recoveryCopyTitle => 'Fai una copia di recupero';

  @override
  String get recoveryCopyText =>
      'Fogata è offline mentre fa la copia, più a lungo con i file delle chat. Tieni il file lontano da questo computer, per esempio su una chiavetta USB.';

  @override
  String get recoveryCopyAttachments => 'Includi i file inviati nelle chat';

  @override
  String recoveryCopyAttachmentsNote(String size) {
    return 'Immagini, video e file: $size in più.';
  }

  @override
  String get recoveryCopySave => 'Scegli dove…';

  @override
  String get recoveryCopyInDataFolder =>
      'Una copia non può stare nella cartella dei dati di Fogata. Scegli un altro posto.';

  @override
  String recoveryCopyMade(String file, String size) {
    return 'Copia salvata: $file ($size).';
  }

  @override
  String recoveryCopyFailed(String why) {
    return 'Non è stato possibile fare la copia. $why';
  }

  @override
  String get recoveryDeleteTitle => 'Eliminare l’account sostituito?';

  @override
  String get recoveryDeleteText =>
      'È stato messo da parte quando una copia è stata ripristinata al suo posto. Una volta eliminato, non si può recuperare.';

  @override
  String get recoveryDelete => 'Elimina';

  @override
  String get recoveryDeleted => 'L’account sostituito è stato eliminato.';

  @override
  String get recoveryFileType => 'Copia di Fogata';

  @override
  String get recoveryLostComputer => 'Hai perso il computer?';

  @override
  String get recoveryRestoreLink => 'Ripristina da una copia';

  @override
  String get recoveryRestoreTitle => 'Ripristina da una copia';

  @override
  String get recoveryRestoreSub =>
      'Scegli la copia di recupero che hai salvato, poi digita la tua chiave di recupero.';

  @override
  String get recoveryChooseCopy => 'Scegli la copia…';

  @override
  String recoveryMadeOn(String date) {
    return 'Una copia fatta il giorno $date.';
  }

  @override
  String get recoveryOtherKey =>
      'È stata fatta per una chiave di recupero diversa da quella di questo computer. Digita la chiave per cui è stata fatta.';

  @override
  String get recoveryKeyField => 'Chiave di recupero';

  @override
  String get recoveryKeyNotAKey =>
      'Una chiave di recupero ha 53 lettere e cifre, in gruppi di quattro.';

  @override
  String get recoveryKeyMistyped =>
      'Una lettera o una cifra è sbagliata: controllale.';

  @override
  String get recoveryProblemNotACopy =>
      'Questa non è una copia di un account Fogata.';

  @override
  String get recoveryProblemNewer =>
      'Questa copia è stata fatta da una versione più recente di Fogata. Aggiorna Fogata per aprirla.';

  @override
  String get recoveryProblemOtherKey =>
      'Questa copia è stata fatta per un’altra chiave di recupero.';

  @override
  String get recoveryProblemDamaged =>
      'Questa copia è danneggiata, o è stata modificata dopo essere stata fatta.';

  @override
  String get recoveryProblemUnproven =>
      'Questa copia non è stata fatta dall’account che contiene, quindi non viene aperta.';

  @override
  String recoveryWhose(String name) {
    return 'L’account di $name';
  }

  @override
  String recoveryHolds(int files, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files file',
      one: '1 file',
    );
    return '$_temp0, $size in tutto';
  }

  @override
  String get recoveryWithAttachments => 'Con i file inviati nelle chat.';

  @override
  String get recoveryWithoutAttachments => 'Senza i file inviati nelle chat.';

  @override
  String get recoveryOnlyIf =>
      'Usa una copia solo se nessun altro computer usa più questo account. Se uno lo usa ancora, trasferisci invece l’account da lì: un account non può stare su due computer.';

  @override
  String get recoveryMoveInstead => 'Trasferiscilo invece';

  @override
  String recoveryHereNamed(String name) {
    return 'Questo computer ha l’account di $name.';
  }

  @override
  String recoveryHereDevice(String fingerprint) {
    return 'Questo computer ha un account senza profilo (dispositivo $fingerprint).';
  }

  @override
  String get recoveryHereUnknown =>
      'Questo computer ha parte di un account che non si apre.';

  @override
  String get recoveryHereReplaced =>
      'Il ripristino lo sostituisce. Viene messo da parte, non eliminato, finché non lo elimini in Impostazioni › Spazio e download.';

  @override
  String get recoveryRestore => 'Ripristina';

  @override
  String get recoveryReplace => 'Sostituisci e ripristina';

  @override
  String recoveryRestoring(String name) {
    return 'Ripristino dell’account di $name…';
  }

  @override
  String recoveryRestored(String name) {
    return 'L’account di $name è stato ripristinato.';
  }

  @override
  String get recoveryRefusedAccountHere =>
      'Ora questo computer ha un account. Scegli di nuovo la copia per vedere cosa sostituisce.';

  @override
  String get recoveryRefusedReplacedKept =>
      'Questo computer conserva ancora un account sostituito da un ripristino precedente. Eliminalo per ripristinare questa copia.';

  @override
  String recoveryRefusedNoRoom(String need, String free) {
    return 'La copia ha bisogno di $need su questo disco, e ne sono liberi $free. Non è cambiato nulla.';
  }

  @override
  String get recoveryRefusedInFolder =>
      'La copia è dentro la cartella dei dati di Fogata. Spostala altrove, poi sceglila di nuovo.';

  @override
  String get recoveryBusy =>
      'Fogata è occupato con una copia di recupero o un trasferimento. Riprova quando ha finito.';

  @override
  String get moveBusyWithCopy =>
      'Fogata è occupato con una copia di recupero. Trasferisci l’account quando ha finito.';

  @override
  String get recoveryDeleteKeptAside => 'Elimina l’account messo da parte…';

  @override
  String get reachOffline =>
      'Da qui Fogata non raggiunge nessuno su internet. Controlla la connessione, oppure un firewall potrebbe bloccare Fogata.';

  @override
  String get reachFirewall =>
      'Il firewall di questo computer non lascia entrare nulla per Fogata. Il comando che lo fa entrare è nelle notifiche.';

  @override
  String get reachUdpBlocked =>
      'Questa rete o questo computer blocca UDP. Le chiamate e le connessioni dirette attraverso i router ne hanno bisogno; finché non è consentito, gli altri ti raggiungono solo tramite un membro di un gruppo o da un indirizzo pubblico.';

  @override
  String get reachSymmetricNat =>
      'Il tuo router assegna una porta pubblica diversa per ogni destinazione (un NAT simmetrico), quindi non si può aprire una via diretta attraverso di esso. Gli amici dietro un altro router ti raggiungono solo tramite un membro di un gruppo che avete in comune.';

  @override
  String get reachNoIpv6 =>
      'Non c’è nemmeno un IPv6 funzionante, che permetterebbe di aggirare il router.';

  @override
  String reachFineFriend(String name) {
    return 'La tua rete sembra andare bene per Fogata, quindi ciò che vi separa è probabilmente dalla parte di $name (offline, o la sua rete).';
  }

  @override
  String get reachFineOthers =>
      'La tua rete sembra andare bene per Fogata, quindi ciò che vi separa è probabilmente dall’altra parte.';

  @override
  String directDropTitle(String name) {
    return 'Rilascia per inviare a $name';
  }

  @override
  String get directDropNote =>
      'Restano in attesa nella casella del messaggio, dove puoi aggiungere un messaggio e controllarli prima che partano.';

  @override
  String directPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n file nascosti o privati sono stati lasciati fuori: file come questi non si aggiungono trascinandoli, perché una pagina web può avviare un trascinamento. Per inviarli, sceglili con il pulsante +.',
      one: 'Un file nascosto o privato è stato lasciato fuori: file come questo non si aggiungono trascinandoli, perché una pagina web può avviare un trascinamento. Per inviarlo, sceglilo con il pulsante +.',
    );
    return '$_temp0';
  }

  @override
  String appMustUpdateToJoin(String name, String group) {
    return '$name deve aggiornare Fogata prima di poter entrare in “$group”. Dopo, entrerà come sempre.';
  }

  @override
  String appUpdateToJoin(String group) {
    return '“$group” richiede una versione più recente di Fogata. Aggiorna Fogata per entrare.';
  }

  @override
  String get errMustUpdateToJoin =>
      'Il tuo amico deve aggiornare Fogata prima di poter entrare in questo gruppo.';

  @override
  String groupSettingsLevelWaiting(String names) {
    return 'Questo gruppo aspetta di attivare una protezione più forte. Si attiva da sola quando tutti hanno aggiornato Fogata. Ancora da aggiornare: $names';
  }

  @override
  String get groupUpdateTitle =>
      'Questo gruppo ha bisogno di un Fogata più recente';

  @override
  String get groupUpdateText =>
      'Qualcuno che lo gestisce ha attivato una protezione più forte, che questa versione di Fogata non ha. Finché non aggiorni, il gruppo resta com’era allora: non vedi cosa è cambiato da allora. Fogata riceve gli aggiornamenti dai tuoi amici e gruppi, e te ne propone uno in alto appena è pronto.';

  @override
  String bellGroupUpdate(String group) {
    return '“$group” ha bisogno di un Fogata più recente';
  }

  @override
  String get bellGroupUpdateText =>
      'Finché non aggiorni, non vedi cosa è cambiato nel gruppo.';

  @override
  String get groupSettingsRaiseAnyway => 'Attivala comunque';

  @override
  String get groupSettingsRaiseAnywayTitle =>
      'Attivare ora la protezione più forte?';

  @override
  String groupSettingsRaiseAnywayText(String names) {
    return '$names dovranno aggiornare Fogata prima di poter partecipare di nuovo a questo gruppo: fino ad allora non vedranno cosa cambia nel gruppo. Il gruppo li ha aspettati una settimana.';
  }

  @override
  String get groupSettingsRaiseAnywayConfirm => 'Attivala';
}
