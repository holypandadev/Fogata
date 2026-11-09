// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for German (`de`).
class L10nDe extends L10n {
  L10nDe([String locale = 'de']) : super(locale);

  @override
  String get commonCancel => 'Abbrechen';

  @override
  String get commonSave => 'Speichern';

  @override
  String get commonClose => 'Schließen';

  @override
  String commonNewMessageFrom(String name, String text) {
    return 'Neue Nachricht von $name: $text';
  }

  @override
  String commonNewMessagesFrom(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n neue Nachrichten von $name',
      one: '1 neue Nachricht von $name',
    );
    return '$_temp0';
  }

  @override
  String commonNewMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n neue Nachrichten',
      one: '1 neue Nachricht',
    );
    return '$_temp0';
  }

  @override
  String get commonAccept => 'Annehmen';

  @override
  String get commonDecline => 'Ablehnen';

  @override
  String get commonSend => 'Senden';

  @override
  String get commonTryAgain => 'Erneut versuchen';

  @override
  String get commonCopy => 'Kopieren';

  @override
  String get commonCopied => 'Kopiert';

  @override
  String get commonBack => 'Zurück';

  @override
  String get commonDiscard => 'Verwerfen';

  @override
  String get commonKeepEditing => 'Weiter bearbeiten';

  @override
  String get commonNotNow => 'Nicht jetzt';

  @override
  String get commonDownload => 'Herunterladen';

  @override
  String get commonSettings => 'Einstellungen';

  @override
  String get commonHome => 'Start';

  @override
  String get commonFriends => 'Freunde';

  @override
  String get commonContinue => 'Weiter';

  @override
  String get commonDone => 'Fertig';

  @override
  String get commonRemove => 'Entfernen';

  @override
  String get commonYou => 'Du';

  @override
  String timeYesterdayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'gestern $time',
      'other': 'gestern $time',
    });
    return '$_temp0';
  }

  @override
  String timeDayAt(String date, String time) {
    return '$date, $time';
  }

  @override
  String get timeNow => 'jetzt';

  @override
  String timeMinutes(int n) {
    return '$n min';
  }

  @override
  String timeHours(int n) {
    return '$n Std.';
  }

  @override
  String timeHoursMinutes(int hours, int minutes) {
    return '$hours Std. $minutes min';
  }

  @override
  String timeDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Tage',
      one: '1 Tag',
    );
    return '$_temp0';
  }

  @override
  String spanMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Minuten',
      one: '1 Minute',
    );
    return '$_temp0';
  }

  @override
  String spanHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Stunden',
      one: '1 Stunde',
    );
    return '$_temp0';
  }

  @override
  String spanDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Tage',
      one: '1 Tag',
    );
    return '$_temp0';
  }

  @override
  String spanWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Wochen',
      one: '1 Woche',
    );
    return '$_temp0';
  }

  @override
  String get agoMoment => 'gerade eben';

  @override
  String agoMinutes(int n) {
    return 'vor $n min';
  }

  @override
  String agoHours(int n) {
    return 'vor $n Std.';
  }

  @override
  String get agoYesterday => 'gestern';

  @override
  String agoDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'vor $n Tagen',
    );
    return '$_temp0';
  }

  @override
  String agoWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'vor $n Wochen',
      one: 'vor 1 Woche',
    );
    return '$_temp0';
  }

  @override
  String agoMonths(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'vor $n Monaten',
      one: 'vor 1 Monat',
    );
    return '$_temp0';
  }

  @override
  String get agoOverYear => 'vor über einem Jahr';

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
    return '$first und $second';
  }

  @override
  String listMore(String rest, String last) {
    return '$rest und $last';
  }

  @override
  String get listSeparator => ', ';

  @override
  String ordinal(int n) {
    return '$n.';
  }

  @override
  String get errLinkExpired => 'Der Link ist abgelaufen. Bitte um einen neuen.';

  @override
  String get errGroupInvite => 'Das ist eine Gruppeneinladung.';

  @override
  String get errDeviceNotFound =>
      'Das andere Gerät wurde nicht gefunden. Prüfe, ob ihr beide online seid.';

  @override
  String get errNotFriendCode =>
      'Das sieht nicht nach einem Fogata-Link oder -Code aus.';

  @override
  String get errAlreadyInGroup => 'Du bist schon in dieser Gruppe.';

  @override
  String get errJoinRoomFirst => 'Betritt zuerst den Raum.';

  @override
  String get errNewerVersion =>
      'Dein Profil wurde von einer neueren Fogata-Version gespeichert. Aktualisiere Fogata, um es zu öffnen.';

  @override
  String get errKeychainUnreachable =>
      'Fogata konnte den Schlüssel zu deinem Profil nicht aus dem Schlüsselbund dieses Computers holen. Entsperre den Schlüsselbund oder erlaube Fogata den Zugriff und versuch es dann erneut.';

  @override
  String get errKeychainNoKey =>
      'Der Schlüsselbund dieses Computers enthält den Schlüssel zu deinem Profil nicht mehr, daher kann Fogata es nicht öffnen.';

  @override
  String get errKeychainWrongKey =>
      'Der Schlüssel im Schlüsselbund dieses Computers öffnet dein Profil nicht.';

  @override
  String get errNoGroupKey =>
      'Diese Gruppe ist auf diesem Computer noch nicht bereit. Versuch es gleich noch einmal.';

  @override
  String get errAlreadyRunning =>
      'Fogata ist mit diesem Profil schon geöffnet. Schließe zuerst das andere Fenster.';

  @override
  String get errFileGone => 'Diese Datei ist nicht mehr im Raum.';

  @override
  String get errNobodyHasFile =>
      'Gerade ist niemand online, der diese Datei hat.';

  @override
  String get errStillStarting => 'Fogata startet noch.';

  @override
  String get errIdentityExists =>
      'Dieser Computer hat schon eine Identität; ihr Schlüssel kann nicht geändert werden.';

  @override
  String get errNameRule =>
      'Gib einen Namen mit 1 bis 40 sichtbaren Zeichen ein.';

  @override
  String get errBioRule =>
      'Eine Bio hat höchstens 190 Zeichen auf 4 Zeilen und keine versteckten Zeichen.';

  @override
  String get errPictureRule =>
      'Das Bild muss ein PNG oder JPEG mit höchstens 64 KB sein.';

  @override
  String get errNotFriendLink => 'Das ist kein Fogata-Freundschaftslink.';

  @override
  String get errLinkDamaged => 'Der Link ist unvollständig oder beschädigt.';

  @override
  String get errLinkNeedsNewer =>
      'Für diesen Link brauchst du eine neuere Fogata-Version.';

  @override
  String get errCodeShape => 'Ein Code sieht so aus: FGT-XXXXX-XXXXX.';

  @override
  String get errCodeWrong =>
      'Der Code ist falsch oder funktioniert nicht mehr.';

  @override
  String get errNotProven =>
      'Das andere Gerät konnte nicht nachweisen, wer es ist.';

  @override
  String get errCallGone => 'Dieser Anruf klingelt nicht mehr.';

  @override
  String get errAlreadyInCall =>
      'Du bist schon in einem Anruf. Leg zuerst auf.';

  @override
  String get errAlreadyFriends => 'Ihr seid schon befreundet.';

  @override
  String get errMemberNotConnected =>
      'Dieses Mitglied ist gerade nicht verbunden. Versuch es noch einmal, wenn es online ist.';

  @override
  String get errMemberTooOld =>
      'Die Fogata-Version dieses Mitglieds ist zu alt für Freundschaftsanfragen aus einer Gruppe. Schick ihm stattdessen einen Freundschaftslink.';

  @override
  String get errMemberAskLost =>
      'Die Freundschaftsanfrage hat dieses Mitglied nicht erreicht. Versuch es noch einmal.';

  @override
  String get errOwnLink =>
      'Das ist dein eigener Link: Schick ihn dem Freund, den du hinzufügen willst.';

  @override
  String get errLinkUsed =>
      'Dieser Link wurde schon benutzt oder ist abgelaufen. Bitte um einen neuen.';

  @override
  String get errLinkExpiredUnreached =>
      'Der Link ist abgelaufen, bevor sein Gerät erreicht wurde. Bitte um einen neuen.';

  @override
  String get errCodeUnanswered =>
      'Kein Gerät hat auf den Code geantwortet. Prüfe ihn und ob das andere Gerät ihn noch anzeigt.';

  @override
  String get errRefused =>
      'Das andere Gerät hat abgelehnt. Vielleicht wurde der Code schon benutzt.';

  @override
  String get errNobodyLetIn =>
      'Niemand hat dich hereingelassen. Versuch es erneut, wenn jemand aus der Gruppe online ist.';

  @override
  String get errNotBelow =>
      'Das kannst du nur bei jemandem tun, der in der Gruppe unter dir steht.';

  @override
  String get errNotManager => 'Das können nur die, die die Gruppe verwalten.';

  @override
  String get errLastManager =>
      'Du kannst dich nicht selbst aus den Verwaltern der Gruppe entfernen.';

  @override
  String get errRoomNotEmpty =>
      'In diesem Sprachraum sind Leute: Entferne ihn erst, wenn er leer ist.';

  @override
  String get errTooManyFiles =>
      'Du teilst schon so viele Dateien, wie ein Raum pro Person erlaubt. Hör zuerst auf, einige zu teilen.';

  @override
  String get errDiskFull =>
      'Der Datenträger ist voll. Gib etwas Speicherplatz frei und versuch es erneut.';

  @override
  String get errTooManyChanges =>
      'Dieses Gerät hat zu viele Änderungen an der Gruppe gemacht, die noch kein anderes Mitglied übernommen hat. Versuch es erneut, nachdem jemand anderes in der Gruppe etwas geändert hat.';

  @override
  String get errSomething => 'Etwas ist schiefgelaufen.';

  @override
  String errRoomFull(int n) {
    return 'Dieser Sprachraum ist voll: Er fasst höchstens $n Personen.';
  }

  @override
  String get findingOffline =>
      'Kein Peer des öffentlichen Netzwerks hat geantwortet: Dieser Computer scheint das Internet nicht zu erreichen, oder eine Firewall blockiert Fogata.';

  @override
  String get findingUdpWorks =>
      'UDP funktioniert. Anrufe und Verbindungen durch Router brauchen es.';

  @override
  String get findingUdpBlocked =>
      'Dieses Netzwerk oder dieser Computer scheint UDP zu blockieren (Peers, die über TCP antworteten, taten es über QUIC nicht). Der Chat kann weiter über andere Mitglieder laufen, aber direkte Verbindungen durch Router und Anrufe scheitern meist. Prüfe die Firewall oder die Netzwerkregeln.';

  @override
  String get findingCarrierNat =>
      'Dieser Computer hat eine Carrier-Grade-NAT-Adresse (100.64.0.0/10): Der Anbieter teilt eine öffentliche Adresse unter vielen Kunden auf.';

  @override
  String get findingPublicIpv4 =>
      'Dieser Computer hat eine öffentliche IPv4-Adresse: Andere können ihn direkt erreichen.';

  @override
  String get findingHardNat =>
      'Hinter einem Router, der jedem Ziel einen anderen öffentlichen Port gibt (ein striktes NAT).';

  @override
  String get findingEasyNat =>
      'Hinter einem Router, der für jedes Ziel denselben öffentlichen Port behält (ein einfaches NAT): Er lässt sich von beiden Seiten gleichzeitig öffnen.';

  @override
  String get findingUnknownRouter =>
      'Zu wenige Peers haben die öffentliche Adresse dieses Computers gesehen, um zu sagen, was für ein Router dahintersteht.';

  @override
  String get findingNoIpv6 =>
      'Keine öffentliche IPv6-Adresse. Fogata braucht sie nicht, aber sie erleichtert direkte Verbindungen.';

  @override
  String get findingIpv6Works =>
      'Öffentliches IPv6 funktioniert: Freunde mit IPv6 verbinden sich ohne Router dazwischen.';

  @override
  String get findingIpv6Unreached =>
      'Dieser Computer hat eine öffentliche IPv6-Adresse, aber noch kein Peer hat ihn über IPv6 erreicht.';

  @override
  String get findingSummaryNoUdp =>
      'Kurz gesagt: Rechne mit Problemen bei direkten Verbindungen, bis UDP erlaubt ist.';

  @override
  String get findingSummaryPublic =>
      'Kurz gesagt: Deine Freunde sollten diesen Computer direkt erreichen können.';

  @override
  String get findingSummaryEasyNat =>
      'Kurz gesagt: Direkte Verbindungen sollten mit den meisten deiner Freunde klappen, indem beide Router gleichzeitig geöffnet werden; bei einem Freund hinter einem strikten NAT braucht es ein Gruppenmitglied dazwischen.';

  @override
  String get findingSummaryHardNatWithIpv6 =>
      'Kurz gesagt: Freunde mit IPv6 oder einer öffentlichen Adresse verbinden sich direkt; bei den anderen braucht es ein Gruppenmitglied dazwischen.';

  @override
  String get findingSummaryHardNat =>
      'Kurz gesagt: Nur Freunde mit einer öffentlichen Adresse verbinden sich direkt; bei den anderen braucht es ein Gruppenmitglied dazwischen.';

  @override
  String get findingSummaryUnknown =>
      'Kurz gesagt: Nicht genug Informationen; prüfe es in einer Minute erneut.';

  @override
  String findingInternet(int n) {
    return 'Internet: Dieser Computer erreicht das öffentliche Netzwerk ($n Peers bekannt).';
  }

  @override
  String findingDns(String detail) {
    return 'DNS: Die Systemeinstellungen konnten nicht vollständig gelesen werden ($detail)';
  }

  @override
  String findingLanDevices(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Fogata-Geräte',
      one: '1 Fogata-Gerät',
    );
    return '$_temp0 in diesem lokalen Netzwerk: Freunde hier verbinden sich ohne Umweg über das Internet.';
  }

  @override
  String findingFirewall(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'in PowerShell als Administrator',
      'macos': 'im Terminal',
      'other': 'in einem Terminal',
    });
    return 'Die Firewall dieses Computers lässt nichts für Fogata herein, daher kann ein Freund am selben Ort hinter einem anderen Router (zum Beispiel zwei Router zu Hause) diesen Computer nicht erreichen. Führe diesen Befehl einmal $_temp0 aus, um Fogata hereinzulassen:';
  }

  @override
  String get settingsLanguage => 'Sprache';

  @override
  String get settingsLanguageNote =>
      'Die Sprache von Fogatas eigenen Texten. Nachrichten bleiben so, wie sie geschrieben wurden.';

  @override
  String get settingsLanguageChoose => 'Sprache auswählen';

  @override
  String get settingsTime => 'Uhrzeit';

  @override
  String get settingsTimeNote =>
      'Wie Uhrzeiten im Chat, in Anrufen und in Listen angezeigt werden.';

  @override
  String settingsTime24(String example) {
    return '24 Stunden · $example';
  }

  @override
  String settingsTime12(String example) {
    return '12 Stunden · $example';
  }

  @override
  String firewallNoticeText(String system, String command) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'in PowerShell als Administrator',
      'macos': 'im Terminal',
      'other': 'in einem Terminal',
    });
    return 'Ein Freund am selben Ort hinter einem anderen Router kann diesen Computer nicht erreichen. Führe einmal $_temp0 diesen Befehl aus: $command';
  }

  @override
  String firewallCopied(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'in PowerShell als Administrator',
      'macos': 'im Terminal',
      'other': 'in einem Terminal',
    });
    return 'Kopiert. Füge ihn $_temp0 ein und führe ihn aus.';
  }

  @override
  String get appRestarted => 'Fogata ist abgestürzt und wurde neu gestartet.';

  @override
  String appVoiceRoomInGroup(String room, String group) {
    return '$room in $group';
  }

  @override
  String appBothIn(String room) {
    return 'Ihr seid beide in $room.';
  }

  @override
  String appHungUp(String name) {
    return '$name hat aufgelegt';
  }

  @override
  String get appYourFriend => 'Dein Freund';

  @override
  String get appCallEndedAlone =>
      'Niemand kam zurück, daher wurde der Anruf beendet';

  @override
  String get appAnsweredElsewhere => 'Auf einem anderen Computer angenommen';

  @override
  String appVoiceRoomDeleted(String room) {
    return '„$room“ wurde gelöscht.';
  }

  @override
  String get appSomeone => 'Jemand';

  @override
  String get appAGroup => 'eine Gruppe';

  @override
  String appFriendRequest(String name) {
    return '$name möchte mit dir befreundet sein.';
  }

  @override
  String appRequestDelivered(String name) {
    return 'Anfrage an $name zugestellt. Die Antwort steht noch aus.';
  }

  @override
  String appFriendsNow(String name) {
    return 'Du und $name seid jetzt befreundet.';
  }

  @override
  String appJoinedGroup(String group) {
    return 'Du bist jetzt in „$group“.';
  }

  @override
  String appRemovedFromGroup(String group) {
    return 'Du bist nicht mehr in „$group“.';
  }

  @override
  String appAddedToGroup(String name, String group) {
    return '$name hat dich zu „$group“ hinzugefügt. Ob du beitrittst, entscheidest du.';
  }

  @override
  String appAsksToJoin(String name, String group) {
    return '$name möchte „$group“ beitreten.';
  }

  @override
  String appJoinWaiting(String group) {
    return 'Warten, bis dich jemand in „$group“ hereinlässt.';
  }

  @override
  String appJoinDeclined(String group) {
    return 'Jemand in „$group“ hat abgelehnt. Diese Einladung funktioniert nicht mehr.';
  }

  @override
  String appDownloadedTo(String path) {
    return 'Heruntergeladen nach $path';
  }

  @override
  String appDownloadFailed(String why) {
    return 'Herunterladen fehlgeschlagen: $why';
  }

  @override
  String get appOtherSession =>
      'Diese Identität ist auch auf einem anderen Computer geöffnet. Schließe Fogata dort: Eine Identität funktioniert immer nur auf einem Computer.';

  @override
  String get appFriendsRestored =>
      'Deine Freundesliste war auf diesem Computer beschädigt und wurde aus der Kopie vor ihrer letzten Änderung wiederhergestellt. Einen kurz davor hinzugefügten oder entfernten Freund musst du eventuell erneut hinzufügen oder entfernen.';

  @override
  String appInvitationRanOut(String group) {
    return 'Die Einladung zu „$group“ ist abgelaufen, bevor dich jemand aus der Gruppe hereingelassen hat. Bitte um eine neue.';
  }

  @override
  String appTakenOut(String who, String room) {
    return '$who hat dich aus $room entfernt.';
  }

  @override
  String appRoomForSome(String room) {
    return '$room ist jetzt nur für bestimmte Mitglieder.';
  }

  @override
  String appTakenOutUntilLetBack(String who, String room) {
    return '$who hat dich aus $room entfernt, bis dich jemand über dir wieder hereinlässt.';
  }

  @override
  String appTakenOutFor(String who, String room, String span) {
    return '$who hat dich für $span aus $room entfernt.';
  }

  @override
  String appTakenOutWhile(String who, String room) {
    return '$who hat dich für eine Weile aus $room entfernt.';
  }

  @override
  String appComeBackIn(String room, String wait) {
    return 'Du kannst in $wait zu $room zurück.';
  }

  @override
  String appRoomNotForYou(String room) {
    return '$room ist gerade nicht für dich.';
  }

  @override
  String appFoldersNotShared(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Ordner können nicht geteilt werden, nur Dateien: $n Ordner wurden weggelassen.',
      one: 'Ordner können nicht geteilt werden, nur Dateien: Der Ordner wurde weggelassen.',
    );
    return '$_temp0';
  }

  @override
  String appSharingOne(String file) {
    return '$file wird von diesem Computer geteilt. Die Datei ist nicht mehr verfügbar, wenn du den Raum verlässt.';
  }

  @override
  String appSharingMany(int n) {
    return '$n Dateien werden von diesem Computer geteilt. Sie sind nicht mehr verfügbar, wenn du den Raum verlässt.';
  }

  @override
  String appSavedTo(String path) {
    return 'Gespeichert unter $path';
  }

  @override
  String appAskingToJoin(String group) {
    return 'Beitritt zu „$group“ angefragt. Jemand aus der Gruppe lässt dich herein, sobald er es sieht.';
  }

  @override
  String get appLookingForDevice =>
      'Das andere Gerät wird gesucht, um deine Anfrage zu senden …';

  @override
  String callStateNoAnswer(String name) {
    return '$name hat nicht abgenommen';
  }

  @override
  String callStateDeclined(String name) {
    return '$name kann gerade nicht sprechen';
  }

  @override
  String callStateBusy(String name) {
    return '$name ist in einem anderen Anruf';
  }

  @override
  String callStateOutdated(String name) {
    return '$name braucht ein neueres Fogata, um beizutreten';
  }

  @override
  String callStateUnreached(String name) {
    return '$name war nicht erreichbar';
  }

  @override
  String get callStateReturn => 'Zurück zum Anruf';

  @override
  String get callStateCallBack => 'Zurückrufen';

  @override
  String get callStateCall => 'Anrufen';

  @override
  String noticesDeclined(String name, String group) {
    return '$name hat die Einladung zu „$group“ abgelehnt';
  }

  @override
  String noticesDeclinedToast(String name, String group) {
    return '$name hat die Einladung zu „$group“ abgelehnt.';
  }

  @override
  String noticesMissedInCall(String name) {
    return '$name hat angerufen, während du in einem Anruf warst';
  }

  @override
  String noticesMissedOffline(String name) {
    return '$name hat angerufen, während du als offline angezeigt wurdest';
  }

  @override
  String noticesMissedCall(String name) {
    return 'Verpasster Anruf von $name';
  }

  @override
  String get contactsGeneral => 'Allgemein';

  @override
  String get directStateTimerOff => 'Aus';

  @override
  String get directStateTimer24h => '24 Stunden';

  @override
  String directStateTimerDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Tage',
      one: '1 Tag',
    );
    return '$_temp0';
  }

  @override
  String directStateDisappearsMinutes(int n) {
    return 'Verschwindet in $n min';
  }

  @override
  String directStateDisappearsHours(int n) {
    return 'Verschwindet in $n Std.';
  }

  @override
  String directStateDisappearsDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Verschwindet in $n Tagen',
    );
    return '$_temp0';
  }

  @override
  String get directStateNoLimit => 'Unbegrenzt';

  @override
  String get directStateMessageGone => 'Diese Nachricht ist nicht mehr da.';

  @override
  String directStateFoldersNotSent(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Ordner können nicht gesendet werden, nur Dateien: $n Ordner wurden weggelassen.',
      one: 'Ordner können nicht gesendet werden, nur Dateien: Der Ordner wurde weggelassen.',
    );
    return '$_temp0';
  }

  @override
  String directStateFileFailed(String why) {
    return 'Datei konnte nicht geladen werden: $why';
  }

  @override
  String movingTooLarge(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Animierte Banner dürfen höchstens $most Pixel groß sein; dieses hat $size.',
      'other':
          'Animierte Bilder dürfen höchstens $most Pixel groß sein; dieses hat $size.',
    });
    return '$_temp0';
  }

  @override
  String movingTooLong(String what, String most, String count) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Animierte Banner dürfen höchstens $most Einzelbilder haben; dieses hat $count.',
      'other':
          'Animierte Bilder dürfen höchstens $most Einzelbilder haben; dieses hat $count.',
    });
    return '$_temp0';
  }

  @override
  String movingTooHeavy(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Animierte Banner dürfen höchstens $most groß sein; dieses hat $size.',
      'other':
          'Animierte Bilder dürfen höchstens $most groß sein; dieses hat $size.',
    });
    return '$_temp0';
  }

  @override
  String get namesHidden =>
      'Dieser Name enthält versteckte Zeichen. Gib ihn ohne diese Zeichen neu ein.';

  @override
  String get namesBlank =>
      'In diesem Namen ist nichts sichtbar. Verwende Buchstaben, Zahlen oder Emoji.';

  @override
  String get namesDefaultBio => 'Hey! Ich bin bei Fogata.';

  @override
  String get namesBioHidden =>
      'Diese Bio enthält versteckte Zeichen. Gib sie ohne diese Zeichen neu ein.';

  @override
  String namesBioLines(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Eine Bio hat höchstens $n Zeilen.',
      one: 'Eine Bio hat höchstens 1 Zeile.',
    );
    return '$_temp0';
  }

  @override
  String namesBioChars(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Eine Bio hat höchstens $n Zeichen.',
      one: 'Eine Bio hat höchstens 1 Zeichen.',
    );
    return '$_temp0';
  }

  @override
  String get restartFailed =>
      'Fogata konnte nicht neu starten. Beende es und öffne es selbst.';

  @override
  String typingOne(String name) {
    return '$name schreibt …';
  }

  @override
  String typingTwo(String first, String second) {
    return '$first und $second schreiben …';
  }

  @override
  String get typingSeveral => 'Mehrere Leute schreiben …';

  @override
  String get outgoingTheRoom => 'diesem Raum';

  @override
  String outgoingDropTitle(String group) {
    return 'Zum Teilen in $group ablegen';
  }

  @override
  String get outgoingDropNote =>
      'Du kannst eine Nachricht hinzufügen und die Dateien vor dem Senden prüfen. Die anderen laden sie von diesem Computer herunter, solange du im Raum bist.';

  @override
  String outgoingPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n versteckte oder private Dateien wurden weggelassen: Solche Dateien werden nicht per Ziehen hinzugefügt, da eine Webseite ein Ziehen starten kann. Wähle sie zum Teilen mit der Büroklammer aus.',
      one: 'Eine versteckte oder private Datei wurde weggelassen: Solche Dateien werden nicht per Ziehen hinzugefügt, da eine Webseite ein Ziehen starten kann. Wähle sie zum Teilen mit der Büroklammer aus.',
    );
    return '$_temp0';
  }

  @override
  String outgoingToShare(int n, String count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$count Dateien zum Teilen · $size',
      one: '$count Datei zum Teilen · $size',
    );
    return '$_temp0';
  }

  @override
  String get outgoingRemoveAll => 'Alle entfernen';

  @override
  String outgoingRemoveFile(String file) {
    return '$file entfernen';
  }

  @override
  String get windowTrayOpen => 'Fogata öffnen';

  @override
  String get windowTrayQuit => 'Fogata beenden';

  @override
  String get windowMinimize => 'Minimieren';

  @override
  String get windowRestore => 'Wiederherstellen';

  @override
  String get windowMaximize => 'Maximieren';

  @override
  String get autostartReason => 'Fogata beim Anmelden öffnen.';

  @override
  String get devicesMicrophone => 'Mikrofon';

  @override
  String get devicesSpeaker => 'Lautsprecher';

  @override
  String get devicesCamera => 'Kamera';

  @override
  String get devicesChooseMicrophone => 'Mikrofon wählen';

  @override
  String get devicesChooseSpeaker => 'Lautsprecher wählen';

  @override
  String get devicesChooseCamera => 'Kamera wählen';

  @override
  String get devicesNoCamera => 'Keine Kamera angeschlossen.';

  @override
  String get devicesCameraInCall => 'Die Kamera ist in deinem Anruf an.';

  @override
  String devicesCameraFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Kamera konnte nicht verwendet werden. Erlaube den Zugriff unter Systemeinstellungen › Datenschutz & Sicherheit.',
      'other': 'Kamera konnte nicht verwendet werden.',
    });
    return '$_temp0';
  }

  @override
  String get devicesCameraDenied =>
      'Das System lässt Fogata die Kamera nicht verwenden. Erlaube es in den Systemeinstellungen und versuche es erneut.';

  @override
  String get devicesCameraUnavailable =>
      'Kein Zugriff auf die Kamera: Dieses System hat kein Kamera-Portal.';

  @override
  String get devicesPreviewOnlyYou => 'Nur du siehst die Vorschau.';

  @override
  String get devicesStopVideo => 'Video stoppen';

  @override
  String get devicesTestVideo => 'Video testen';

  @override
  String get devicesNoiseOn => 'Rauschunterdrückung an';

  @override
  String get devicesNoiseOff => 'Rauschunterdrückung aus';

  @override
  String get devicesNoise => 'Rauschunterdrückung';

  @override
  String get devicesNoiseNote =>
      'Dämpft gleichmäßige Hintergrundgeräusche wie einen Lüfter oder ein Brummen in dem, was die anderen hören.';

  @override
  String get devicesNoiseShort => 'Dämpft Lüfter oder Brummen.';

  @override
  String get devicesMirror => 'Meine Kamera spiegeln';

  @override
  String get devicesMirrorNote =>
      'Ändert nur, wie du dich hier siehst. Die anderen sehen deine Kamera immer so, wie sie ist.';

  @override
  String get devicesMirrorOn => 'Kamera spiegeln an';

  @override
  String get devicesMirrorOff => 'Kamera spiegeln aus';

  @override
  String get devicesTestMic => 'Mikrofon testen';

  @override
  String get devicesStopMicTest => 'Test beenden';

  @override
  String get devicesMicLevel => 'Mikrofonpegel';

  @override
  String get devicesHearMyself => 'Mich selbst hören';

  @override
  String get devicesHearMyselfNote =>
      'Spielt dein Mikrofon über den Lautsprecher ab. Nimm Kopfhörer, sonst kann der Lautsprecher wieder ins Mikrofon gelangen.';

  @override
  String get devicesHearMyselfOn => 'Mich selbst hören an';

  @override
  String get devicesHearMyselfOff => 'Mich selbst hören aus';

  @override
  String get devicesMicTestOnlyYou =>
      'Vom Test wird nichts an jemanden gesendet.';

  @override
  String get devicesMicInCall => 'Das Mikrofon wird in deinem Anruf benutzt.';

  @override
  String get devicesNoMic => 'Kein Mikrofon angeschlossen.';

  @override
  String get devicesSystemDefault => 'Systemstandard';

  @override
  String get devicesNotConnected =>
      'Nicht verbunden: Der Standard wird verwendet';

  @override
  String get devicesNoCameraShort => 'Keine Kamera angeschlossen';

  @override
  String get devicesMicOptions => 'Mikrofonoptionen';

  @override
  String get devicesSpeakerOptions => 'Lautsprecheroptionen';

  @override
  String get devicesCameraOptions => 'Kameraoptionen';

  @override
  String get devicesInputDevice => 'Eingabegerät';

  @override
  String get devicesOutputDevice => 'Ausgabegerät';

  @override
  String get devicesChosenMissing =>
      'Das gewählte Gerät ist nicht verbunden: Der Standard wird verwendet.';

  @override
  String get devicesVoiceSettings => 'Audio- & Videoeinstellungen';

  @override
  String get photoImages => 'Bilder';

  @override
  String photoTooLarge(int n) {
    return 'Dieses Bild ist zu groß: höchstens $n Megapixel.';
  }

  @override
  String get photoUnreadable => 'Dieses Bild konnte nicht gelesen werden.';

  @override
  String get callSource => 'Original';

  @override
  String callQuality(String resolution, int fps) {
    return '$resolution · $fps fps';
  }

  @override
  String get callSetupFailed => 'Der Anruf konnte nicht eingerichtet werden.';

  @override
  String get callNoSoundSystem =>
      'Das Tonsystem dieses Computers ist nicht erreichbar, daher kann der Anruf nicht beginnen.';

  @override
  String callMicGone(String name) {
    return 'Dein Mikrofon wurde getrennt: Jetzt wird $name verwendet.';
  }

  @override
  String get callMicBack => 'Dein Mikrofon ist wieder da.';

  @override
  String callMicSilent(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Mikrofon konnte nicht verwendet werden. Prüfe, ob es angeschlossen und unter Systemeinstellungen › Datenschutz & Sicherheit erlaubt ist.',
      'windows': 'Mikrofon konnte nicht verwendet werden. Prüfe, ob es angeschlossen und unter Einstellungen › Datenschutz und Sicherheit › Mikrofon erlaubt ist.',
      'other': 'Mikrofon konnte nicht verwendet werden. Prüfe, ob es angeschlossen ist.',
    });
    return '$_temp0';
  }

  @override
  String get callNoMicrophone => 'Es ist kein Mikrofon angeschlossen.';

  @override
  String callMicFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Mikrofon konnte nicht verwendet werden. Erlaube den Zugriff unter Systemeinstellungen › Datenschutz & Sicherheit.',
      'windows': 'Mikrofon konnte nicht verwendet werden. Erlaube den Zugriff unter Einstellungen › Datenschutz und Sicherheit › Mikrofon.',
      'other': 'Mikrofon konnte nicht verwendet werden.',
    });
    return '$_temp0';
  }

  @override
  String callScreenFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Bildschirm konnte nicht geteilt werden. Erlaube die Bildschirmaufnahme unter Systemeinstellungen › Datenschutz & Sicherheit.',
      'other': 'Bildschirm konnte nicht geteilt werden.',
    });
    return '$_temp0';
  }

  @override
  String get callShareLost =>
      'Teilen beendet: Was du geteilt hast, kann nicht mehr aufgenommen werden.';

  @override
  String callKeptOut(String name) {
    return 'Die Verbindung zu $name ist abgebrochen.';
  }

  @override
  String get qualityGood => 'Gut';

  @override
  String get qualityOkay => 'Mittel';

  @override
  String get qualityPoor => 'Schlecht';

  @override
  String get qualityLost => 'Getrennt';

  @override
  String qualityDelay(int ms) {
    return '$ms ms';
  }

  @override
  String qualityPercent(String value) {
    return '$value %';
  }

  @override
  String get qualityAnotherMember => 'ein anderes Mitglied';

  @override
  String qualityLostHint(String name, int seconds) {
    return 'Seit $seconds s kommt nichts von $name. Fogata sucht weiter; die Verbindung kommt von selbst zurück, sobald das Netzwerk wieder da ist.';
  }

  @override
  String qualityBothPoor(String name) {
    return 'Beide Netzwerke sind gerade schlecht: Du und $name hört euch vielleicht abgehackt.';
  }

  @override
  String qualityTheirSide(String name) {
    return 'Das Problem liegt bei $name: Die Stimme kommt bei allen abgehackt an. Dort sind WLAN oder mobile Daten gerade schlecht.';
  }

  @override
  String qualityYourSideWith(String name) {
    return 'Das Problem liegt bei dir: $name und die anderen hören dich vielleicht abgehackt. Dein WLAN oder deine mobilen Daten sind gerade schlecht.';
  }

  @override
  String qualityTheirsSlow(String name) {
    return 'Das Netzwerk von $name ist gerade etwas langsam: $name kommt bei allen vielleicht ab und zu abgehackt an.';
  }

  @override
  String qualityAllGoodWith(String name) {
    return 'Du und $name hört euch ohne Probleme.';
  }

  @override
  String qualityMineSlowWith(String name) {
    return 'Dein Netzwerk ist gerade etwas langsam: $name hört dich vielleicht ab und zu abgehackt.';
  }

  @override
  String qualityWayThrough(String name, String carrier) {
    return 'Das Netzwerk von $name selbst ist in Ordnung; der Weg zwischen euch ist langsamer, weil er über $carrier läuft. Bei dir kommt die Stimme vielleicht ab und zu abgehackt an, die anderen hören sie gut.';
  }

  @override
  String qualityWaySlowFine(String name) {
    return 'Das Netzwerk von $name selbst ist in Ordnung; der Weg zwischen euch ist gerade langsam. Bei dir kommt die Stimme vielleicht ab und zu abgehackt an, die anderen hören sie gut.';
  }

  @override
  String qualityWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'Der Weg zwischen dir und $name ist gerade etwas langsam; es kann an beiden Netzwerken liegen. Ihr hört euch vielleicht ab und zu abgehackt.',
      'other':
          'Der Weg zwischen dir und $name ist gerade schlecht; es kann an beiden Netzwerken liegen. Ihr hört euch vielleicht abgehackt.',
    });
    return '$_temp0';
  }

  @override
  String qualityWayVia(String level, String name, String carrier) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'Der Weg zwischen dir und $name ist gerade etwas langsam; es kann an beiden Netzwerken liegen. Ihr hört euch vielleicht ab und zu abgehackt. Er läuft über $carrier.',
      'other':
          'Der Weg zwischen dir und $name ist gerade schlecht; es kann an beiden Netzwerken liegen. Ihr hört euch vielleicht abgehackt. Er läuft über $carrier.',
    });
    return '$_temp0';
  }

  @override
  String get qualityYourSide =>
      'Das Problem liegt bei dir: Die anderen hören dich vielleicht abgehackt. Dein WLAN oder deine mobilen Daten sind gerade schlecht.';

  @override
  String get qualityMineSlow =>
      'Dein Netzwerk ist gerade etwas langsam: Die anderen hören dich vielleicht ab und zu abgehackt.';

  @override
  String qualityLostShort(String name, int seconds) {
    return 'Seit $seconds s kommt nichts von $name. Fogata sucht weiter.';
  }

  @override
  String qualityAllWellWith(String name) {
    return 'Alles in Ordnung: Du und $name hört euch ohne Probleme.';
  }

  @override
  String qualityNoteDropped(String name) {
    return 'die Verbindung zu $name ist abgebrochen und Fogata sucht erneut';
  }

  @override
  String qualityNoteTheirsPoor(String name) {
    return '$name hat ein schlechtes Netzwerk und kommt bei allen abgehackt an';
  }

  @override
  String qualityNoteTheirsSlow(String name) {
    return 'das Netzwerk von $name ist etwas langsam';
  }

  @override
  String qualityNoteThrough(String name, String carrier) {
    return '$name kommt über $carrier zu dir';
  }

  @override
  String qualityNoteWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': 'der Weg zu $name ist gerade etwas langsam',
      'other': 'der Weg zu $name ist gerade schlecht',
    });
    return '$_temp0';
  }

  @override
  String get qualityNoteSeparator => '; ';

  @override
  String get qualityAllWell =>
      'Alles in Ordnung: Du hörst alle und alle hören dich.';

  @override
  String qualityAllWellMine(String notes) {
    return 'Bei dir ist alles in Ordnung. $notes.';
  }

  @override
  String qualityYourDetails(String level) {
    return 'Deine Verbindung: $level. Details anzeigen';
  }

  @override
  String qualityTheirDetails(String name, String level) {
    return 'Verbindung von $name: $level. Details anzeigen';
  }

  @override
  String get qualityReconnecting => 'Verbinde neu …';

  @override
  String get qualityReconnectingLabel => 'Verbinde neu';

  @override
  String qualityLevelConnection(String level) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'good': 'Gute Verbindung',
      'okay': 'Mittlere Verbindung',
      'weak': 'Schlechte Verbindung',
      'other': 'Verbindung verloren',
    });
    return '$_temp0';
  }

  @override
  String get qualityPoorConnection => 'Schlechte Verbindung';

  @override
  String get qualityMinePoorBanner =>
      'Dein Netzwerk ist schlecht: Die anderen hören dich vielleicht abgehackt';

  @override
  String get qualityYourNetwork => 'Dein Netzwerk';

  @override
  String qualityTo(String name) {
    return 'Zu $name';
  }

  @override
  String qualityBetween(String name) {
    return 'Zwischen dir und $name';
  }

  @override
  String qualityTheirNetwork(String name) {
    return 'Netzwerk von $name';
  }

  @override
  String get qualityLastHeard => 'Zuletzt gehört';

  @override
  String qualitySecondsAgo(int n) {
    return 'vor $n s';
  }

  @override
  String get qualityPath => 'Weg';

  @override
  String get qualityLookingAgain => 'Wird erneut gesucht';

  @override
  String get qualityDelayLabel => 'Verzögerung';

  @override
  String get qualityLostPackets => 'Verlorene Pakete';

  @override
  String get qualityYourConnection => 'Deine Verbindung';

  @override
  String qualityTheirConnection(String name) {
    return 'Verbindung von $name';
  }

  @override
  String roomCountFullLabel(int count, int max) {
    return 'Voll, $count von $max Personen';
  }

  @override
  String roomCountLabel(int count, int max) {
    return '$count von $max Personen';
  }

  @override
  String get roomCountFullPrefix => 'Voll · ';

  @override
  String get roomCountFullWord => 'Voll ';

  @override
  String get volumeVoice => 'Stimmlautstärke';

  @override
  String get volumeOnlyYouHear => 'Nur du hörst diese Änderungen';

  @override
  String get volumeScreenSound => 'Bildschirmton';

  @override
  String volumePercent(int n) {
    return '$n %';
  }

  @override
  String get volumeMuteVoiceForMe => 'Stimme für mich stummschalten';

  @override
  String get volumeMuteForMe => 'Für mich stummschalten';

  @override
  String get volumeMuteScreenForMe => 'Bildschirmton für mich stummschalten';

  @override
  String get volumeBackTo100 => 'Zurück auf 100 %';

  @override
  String get volumeBelowYou => 'UNTER DIR';

  @override
  String volumeRemoveFrom(String room) {
    return 'Aus $room entfernen';
  }

  @override
  String volumeRemoveTitle(String name, String room) {
    return '$name aus $room entfernen?';
  }

  @override
  String volumeRemoveText(String name, String room) {
    return '$name verlässt $room jetzt und kann sofort wieder hinein. Aussperren hält jemanden eine Weile draußen.';
  }

  @override
  String volumeTimeOutOf(String room) {
    return 'Aus $room aussperren';
  }

  @override
  String volumeTimeOutText(String name, String room) {
    return '$name verlässt $room und darf so lange nicht zurück:';
  }

  @override
  String get volumeUntilLetBack =>
      'Bis jemand mit höherem Rang die Person wieder hereinlässt';

  @override
  String get pipYourScreen => 'Dein Bildschirm';

  @override
  String pipScreenOf(String name) {
    return 'Bildschirm von $name';
  }

  @override
  String get pipYourCamera => 'Deine Kamera';

  @override
  String get pipBackToCall => 'Zurück zum Anruf';

  @override
  String get pipBackToRoom => 'Zurück zum Raum';

  @override
  String get pipStopWatching => 'Nicht mehr zusehen';

  @override
  String get addToCallTitle => 'Zum Anruf hinzufügen';

  @override
  String addToCallNote(int n) {
    return 'Freunde, die online sind. Bis zu $n Personen pro Anruf.';
  }

  @override
  String get addToCallSearch => 'Freunde suchen';

  @override
  String get addToCallNoFriends => 'Noch keine Freunde.';

  @override
  String get addToCallNoMatch => 'Kein Freund mit diesem Namen.';

  @override
  String get addToCallInCall => 'Im Anruf';

  @override
  String get addToCallCalling => 'Klingelt …';

  @override
  String get addToCallOffline => 'Offline';

  @override
  String get addToCallCall => 'Anrufen';

  @override
  String get streamQualityTitle => 'Streamqualität';

  @override
  String get streamQualityNoteSharing =>
      'Wie dein Bildschirm bei den anderen ankommt. Änderungen gelten sofort.';

  @override
  String get streamQualityNote =>
      'Wie dein Bildschirm bei den anderen ankommt, wenn du ihn teilst.';

  @override
  String get streamQualityResolution => 'Auflösung';

  @override
  String streamQualitySourceLines(int lines) {
    return 'Original (${lines}p)';
  }

  @override
  String get streamQualitySource => 'Original';

  @override
  String get streamQualityFrameRate => 'Bildrate';

  @override
  String streamQualityFps(int n) {
    return '$n fps';
  }

  @override
  String get streamQualityHigherNote =>
      'Mehr Qualität beansprucht deinen Upload und deinen Computer stärker.';

  @override
  String get streamQualitySomeone => 'Jemand';

  @override
  String streamQualityLoweredOne(String name, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Die anderen bekommen, was du gewählt hast.',
      'other': '',
    });
    return 'Die Verbindung von $name schafft gerade nicht mehr, daher kommt dort ein kleineres Bild an.$_temp0';
  }

  @override
  String streamQualityLoweredMany(String names, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Die anderen bekommen, was du gewählt hast.',
      'other': '',
    });
    return 'Einige Verbindungen schaffen gerade nicht mehr: $names bekommen ein kleineres Bild.$_temp0';
  }

  @override
  String get streamQualityGoingOut => 'WIRD GERADE GESENDET';

  @override
  String get streamQualityNobody => 'Noch sieht niemand zu.';

  @override
  String streamQualityNow(int height, int fps) {
    return '${height}p · $fps fps';
  }

  @override
  String callLayersIsCalling(String name) {
    return '$name ruft an';
  }

  @override
  String get callLayersGoToCall => 'F6 springt zum Anruf';

  @override
  String callLayersWith(String names) {
    return 'Mit $names';
  }

  @override
  String get callLayersDirectCall => 'Direktanruf · ';

  @override
  String get callLayersAnswerMicOff => 'Mit ausgeschaltetem Mikrofon annehmen';

  @override
  String callLayersAnsweringLeaves(String room) {
    return 'Beim Annehmen verlässt du $room';
  }

  @override
  String get callLayersLeaveAndAnswer => 'Verlassen und annehmen';

  @override
  String callLayersBothIn(String room) {
    return 'Ihr seid beide in $room.';
  }

  @override
  String get callLayersDismiss => 'Ausblenden';

  @override
  String callLayersCallWith(String names) {
    return 'Anruf mit $names';
  }

  @override
  String get callLayersRoom => 'Raum';

  @override
  String callLayersBackTo(String title) {
    return 'Zurück zu $title';
  }

  @override
  String callLayersRinging(String time) {
    return 'Klingelt · $time';
  }

  @override
  String callLayersInCall(String time) {
    return 'Im Anruf · $time';
  }

  @override
  String callLayersOnlyYou(String time) {
    return 'Nur du · $time';
  }

  @override
  String callLayersInRoom(int n, String time) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n im Raum',
    );
    return '$_temp0 · $time';
  }

  @override
  String get callLayersMute => 'Stummschalten';

  @override
  String get callLayersUnmute => 'Stummschaltung aufheben';

  @override
  String get callLayersHangUp => 'Auflegen';

  @override
  String get callLayersLeaveRoom => 'Raum verlassen';

  @override
  String roomDeclineCallTitle(String name) {
    return 'Anruf von $name ablehnen?';
  }

  @override
  String roomDeclineCallText(String caller, String callee) {
    return '$caller ruft dich an. Du kannst immer nur in einem Anruf sein: Wenn du $callee anrufst, wird dieser Anruf abgelehnt.';
  }

  @override
  String roomDeclineCallTextSomeone(String caller) {
    return '$caller ruft dich an. Du kannst immer nur in einem Anruf sein: Wenn du jemand anderen anrufst, wird dieser Anruf abgelehnt.';
  }

  @override
  String get roomDeclineAndCall => 'Ablehnen und anrufen';

  @override
  String roomHangUpTitle(String name) {
    return 'Anruf mit $name beenden?';
  }

  @override
  String get roomHangUpText =>
      'Du kannst immer nur in einem Anruf oder Raum sein: Wenn du fortfährst, endet der Anruf.';

  @override
  String get roomHangUp => 'Auflegen';

  @override
  String get roomCloseTitle => 'Raum verlassen und schließen?';

  @override
  String get roomCloseText =>
      'Du bist die letzte Person hier. Wenn du gehst, wird der Raum geschlossen und das Gesagte verschwindet von diesem Computer; was andere heruntergeladen haben, bleibt bei ihnen.';

  @override
  String get roomCloseChatMessages => 'Chatnachrichten';

  @override
  String get roomCloseFilesShared => 'Von dir geteilte Dateien';

  @override
  String get roomStay => 'Bleiben';

  @override
  String get roomLeaveAndClose => 'Verlassen und schließen';

  @override
  String get roomLivePrefix => 'Live · ';

  @override
  String get roomPillConnecting => 'Verbinde';

  @override
  String get roomPillOnlyYou => 'Nur du';

  @override
  String get roomPillError => 'Fehler';

  @override
  String get roomLastOne => 'Du bist die letzte Person im Raum.';

  @override
  String get roomOnlyYouIn => 'Nur du bist im Raum.';

  @override
  String get roomAloneWarning =>
      ' Wenn du gehst, schließt er und alles darin ist weg.';

  @override
  String get roomEarlierPeople => 'Vorherige Personen';

  @override
  String get roomMorePeople => 'Weitere Personen';

  @override
  String roomTileYou(String name) {
    return '$name (du)';
  }

  @override
  String roomTileNoSound(String name) {
    return '$name · ohne Ton';
  }

  @override
  String roomTileConnecting(String name) {
    return '$name · verbindet';
  }

  @override
  String roomYourScreenWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Dein Bildschirm · $n sehen zu',
      one: 'Dein Bildschirm · 1 sieht zu',
    );
    return '$_temp0';
  }

  @override
  String roomTileAway(String name) {
    return '$name · keine Verbindung';
  }

  @override
  String roomTileMicOff(String name) {
    return '$name, Mikrofon aus';
  }

  @override
  String roomTileMutedByYou(String said) {
    return '$said, für dich stummgeschaltet';
  }

  @override
  String roomTileVolume(String said, String percent) {
    return '$said, Lautstärke $percent';
  }

  @override
  String roomTileLeft(String name) {
    return '$name ist gegangen · kommt evtl. zurück';
  }

  @override
  String roomIsSharing(String name) {
    return '$name teilt den Bildschirm';
  }

  @override
  String get roomNothingSentUntilWatch =>
      'Bis du zusiehst, wird dir nichts gesendet';

  @override
  String get roomWatch => 'Ansehen';

  @override
  String get roomLive => 'LIVE';

  @override
  String get roomSharingYourScreen => 'Du teilst deinen Bildschirm';

  @override
  String get roomShowPreview => 'Vorschau zeigen';

  @override
  String get roomPreviewMirror =>
      'Zeigst du sie, während du Fogata selbst teilst, entsteht ein endloser Spiegel';

  @override
  String get roomNobodyWatching => 'Niemand sieht zu';

  @override
  String roomPeopleWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Personen sehen zu',
      one: '1 Person sieht zu',
    );
    return '$_temp0';
  }

  @override
  String get roomExitFullScreen => 'Vollbild beenden';

  @override
  String get roomFullScreen => 'Vollbild';

  @override
  String get roomPlayScreenSound => 'Bildschirmton abspielen';

  @override
  String get roomMuteScreenSound => 'Bildschirmton stummschalten';

  @override
  String get roomQualityPoor => 'Schlecht';

  @override
  String get roomQualityOkay => 'Mittel';

  @override
  String get roomQualityGood => 'Gut';

  @override
  String get roomQualityPoorTitle => 'Schlechte Verbindung';

  @override
  String get roomQualityOkayTitle => 'Die Verbindung ist mittelmäßig';

  @override
  String get roomQualityGoodTitle => 'Gute Verbindung';

  @override
  String get roomQualityMeasuring => 'Messung';

  @override
  String get roomQualityDelay => 'Verzögerung';

  @override
  String roomQualityMs(String n) {
    return '$n ms';
  }

  @override
  String get roomQualityLost => 'Verlorene Pakete';

  @override
  String roomQualityPercent(String n) {
    return '$n %';
  }

  @override
  String get roomQualityPicture => 'Bild';

  @override
  String get roomQualityPath => 'Weg';

  @override
  String get roomQualityMayFreeze =>
      'Das Bild kann einfrieren. Es hilft, wenn weniger Leute gleichzeitig teilen.';

  @override
  String get roomQualityMayShrink =>
      'Das Bild wird vielleicht kleiner, damit es flüssig bleibt.';

  @override
  String roomCallingName(String name) {
    return 'Rufe $name an …';
  }

  @override
  String get roomRingsFor30 =>
      'Es klingelt bis zu 30 Sekunden auf dem anderen Computer.';

  @override
  String get roomReturnToCall => 'Zurück zum Anruf';

  @override
  String get roomCallAgain => 'Erneut anrufen';

  @override
  String get roomAloneHint =>
      'Wenn jemand aus der Gruppe hereinkommt, erscheint die Person hier.';

  @override
  String roomAlreadyLeft(String names) {
    return 'Schon gegangen: $names.';
  }

  @override
  String get roomGroupFound => 'Gruppe im Netzwerk gefunden';

  @override
  String get roomLookingForRoute => 'suche den besten Weg …';

  @override
  String get roomConnectingTitle => 'Verbindung zum Raum wird hergestellt';

  @override
  String roomSeconds(String n) {
    return '$n s';
  }

  @override
  String roomReachedOf(int reached, int total, String time) {
    return '$reached von $total · $time';
  }

  @override
  String get roomPathLan => 'lokales Netzwerk, verschlüsselt';

  @override
  String get roomPathDirect => 'direkt, verschlüsselt';

  @override
  String get roomPathThroughMember => 'über ein Mitglied, verschlüsselt';

  @override
  String roomPathThrough(String name) {
    return 'über $name, verschlüsselt';
  }

  @override
  String get roomCouldNotConnect => 'Verbindung zum Raum fehlgeschlagen';

  @override
  String get roomCouldNotConnectText =>
      'Niemand hat rechtzeitig geantwortet. Dein Netzwerk blockiert vielleicht direkte Verbindungen zwischen Geräten.';

  @override
  String get roomNoAnswer => 'keine Antwort';

  @override
  String get roomSeeNetworkCheck => 'Zur Netzwerkprüfung →';

  @override
  String get roomGoesNothing => 'Der Raum schließt für alle';

  @override
  String roomGoesMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Nachrichten gehen verloren',
      one: '1 Nachricht geht verloren',
    );
    return '$_temp0';
  }

  @override
  String roomGoesFiles(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n von dir geteilte Dateien verschwinden',
      one: '1 von dir geteilte Datei verschwindet',
    );
    return '$_temp0';
  }

  @override
  String roomGoesBoth(int messages, int files) {
    String _temp0 = intl.Intl.pluralLogic(
      messages,
      locale: localeName,
      other: '$messages Nachrichten',
      one: '1 Nachricht',
    );
    String _temp1 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files Dateien',
      one: '1 Datei',
    );
    return '$_temp0 und $_temp1 gehen verloren';
  }

  @override
  String get roomMuteMicrophone => 'Mikrofon stummschalten';

  @override
  String get roomUnmuteMicrophone => 'Mikrofon einschalten';

  @override
  String get roomStopSharingScreen => 'Bildschirm nicht mehr teilen';

  @override
  String get roomShareScreen => 'Bildschirm teilen';

  @override
  String get roomUndeafen => 'Die anderen wieder hören';

  @override
  String get roomDeafen => 'Die anderen nicht mehr hören';

  @override
  String get roomCameraOff => 'Kamera ausschalten';

  @override
  String get roomCameraOn => 'Kamera einschalten';

  @override
  String get roomLeave => 'Verlassen';

  @override
  String get roomSoundWaitsForMic =>
      'Der Ton wird nur mit eingeschaltetem Mikrofon gesendet';

  @override
  String get roomStopSharingSound => 'Ton nicht mehr teilen';

  @override
  String get roomShareSoundToo => 'Auch den Ton teilen';

  @override
  String get roomAddFriendToCall => 'Freund zum Anruf hinzufügen';

  @override
  String get roomHideChat => 'Chat ausblenden';

  @override
  String get roomShowChat => 'Chat einblenden';

  @override
  String get roomTabChat => 'Chat';

  @override
  String get roomTabFiles => 'Dateien';

  @override
  String roomNoteLeftCall(String name) {
    return '$name hat den Anruf verlassen · ';
  }

  @override
  String roomNoteJoinedCall(String name) {
    return '$name ist dem Anruf beigetreten · ';
  }

  @override
  String roomNoteLeftRoom(String name) {
    return '$name hat den Raum verlassen · ';
  }

  @override
  String roomNoteEnteredRoom(String name) {
    return '$name hat den Raum betreten · ';
  }

  @override
  String get roomSearchMessages => 'Nachrichten durchsuchen';

  @override
  String get roomClearSearch => 'Suche löschen';

  @override
  String get roomChatGoneCall =>
      'Dieser Chat verschwindet, wenn der Anruf endet';

  @override
  String get roomChatGoneRoom =>
      'Dieser Chat verschwindet, wenn der Raum geschlossen wird';

  @override
  String roomNoMatches(String text) {
    return 'Keine Nachrichten mit „$text“';
  }

  @override
  String roomMatches(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Treffer',
      one: '1 Treffer',
    );
    return '$_temp0';
  }

  @override
  String get roomReceivingMessages => 'Empfange die Nachrichten des Raums …';

  @override
  String roomReceivingMessagesFrom(String name) {
    return 'Empfange die Nachrichten des Raums vom Computer von $name …';
  }

  @override
  String get roomSomeone => 'Jemand';

  @override
  String get roomShareFiles => 'Dateien teilen';

  @override
  String get roomAddMoreFiles => 'Weitere Dateien hinzufügen';

  @override
  String roomMessageTo(String name) {
    return 'Nachricht an $name';
  }

  @override
  String get roomMessageTheRoom => 'Nachricht an den Raum';

  @override
  String get roomMessageTheCall => 'Nachricht an alle im Anruf';

  @override
  String get roomAddMessageOptional => 'Nachricht hinzufügen (optional)';

  @override
  String get roomMessageLabel => 'Nachricht';

  @override
  String roomEnterToSend(String key) {
    return '$key zum Senden';
  }

  @override
  String roomEscToRemove(String key) {
    return '$key zum Entfernen';
  }

  @override
  String roomOrTwo(String first, String second) {
    return '$first oder $second';
  }

  @override
  String roomOrMore(String rest, String last) {
    return '$rest oder $last';
  }

  @override
  String roomFileNotAvailable(String name) {
    return 'Nicht verfügbar · $name ist gegangen';
  }

  @override
  String get roomSharingFromHere => 'Wird von diesem Computer geteilt';

  @override
  String roomAvailableWhile(String names) {
    return 'Verfügbar, solange $names im Raum bleibt';
  }

  @override
  String roomSizeDownloads(String size, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Downloads',
      one: '1 Download',
    );
    return '$size · $_temp0';
  }

  @override
  String roomDownloadFile(String file) {
    return '$file herunterladen';
  }

  @override
  String roomShowInFolder(String file) {
    return '$file im Ordner anzeigen';
  }

  @override
  String roomGettingFile(String file) {
    return 'Lade $file …';
  }

  @override
  String get roomSaveToDownloads => 'In Downloads speichern';

  @override
  String roomPictureKept(String name) {
    return '$name ist gegangen · das Bild bleibt hier, bis du gehst';
  }

  @override
  String get roomSaveFile => 'Speichern';

  @override
  String get roomClosePicture => 'Bild schließen';

  @override
  String get roomFilesNote =>
      'Dateien werden nirgends hochgeladen: Sie kommen direkt vom Computer der Person, die sie teilt, und nur, solange diese im Raum bleibt.';

  @override
  String roomFilesAvailable(int n) {
    return 'Verfügbar · $n';
  }

  @override
  String roomFilesGone(int n) {
    return 'Nicht mehr verfügbar · $n';
  }

  @override
  String roomPersonLeft(String name) {
    return '$name ist gegangen';
  }

  @override
  String get roomPickOrDrop => 'Wähle sie hier aus oder zieh sie in den Raum';

  @override
  String get roomFileYours => 'Von dir';

  @override
  String roomFileFrom(String name) {
    return 'Von $name';
  }

  @override
  String roomDownloads(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Downloads',
      one: '1 Download',
    );
    return '$_temp0';
  }

  @override
  String get roomStopSharingFile => 'Stoppen';

  @override
  String get roomWhatToShare => 'Was möchtest du teilen?';

  @override
  String roomScreenSource(String name) {
    return 'Bildschirm: $name';
  }

  @override
  String roomPlayVideo(String file) {
    return '$file abspielen';
  }

  @override
  String get roomCloseVideo => 'Video schließen';

  @override
  String get roomVideoGone => 'Dieses Video ist nicht mehr im Raum.';

  @override
  String get roomVideoNotFetched => 'Dieses Video konnte nicht geladen werden.';

  @override
  String get roomVideoCannotPlay =>
      'Dieser Computer kann dieses Video nicht abspielen.';

  @override
  String roomGettingVideoPercent(int n) {
    return 'Lade das Video · $n %';
  }

  @override
  String get roomGettingVideo => 'Lade das Video …';

  @override
  String get roomPause => 'Pause';

  @override
  String get roomPlay => 'Abspielen';

  @override
  String get roomSoundOn => 'Ton an';

  @override
  String get roomMuteVideo => 'Stummschalten';

  @override
  String get roomLeaveFullScreen => 'Vollbild beenden';

  @override
  String roomCallWaits(String time) {
    return 'Wenn nur noch eine Person da ist, wartet der Anruf $time auf die anderen';
  }

  @override
  String roomCallWaitsAlone(String time) {
    return 'Nur du bist noch da. Der Anruf wartet $time auf die anderen';
  }

  @override
  String roomOpenFile(String file) {
    return '$file öffnen';
  }

  @override
  String activityCreated(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 die Gruppe als „$name“ gegründet';
  }

  @override
  String activityRenamed(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 die Gruppe in „$name“ umbenannt';
  }

  @override
  String activityIcon(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 das Gruppensymbol geändert';
  }

  @override
  String activityPictureRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 das Gruppenbild entfernt';
  }

  @override
  String activityPicture(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 das Gruppenbild geändert';
  }

  @override
  String activityBannerRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 das Gruppenbanner entfernt';
  }

  @override
  String activityBanner(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 das Gruppenbanner geändert';
  }

  @override
  String activityAdded(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'dich',
      'other': '$member',
    });
    return '$_temp0 $_temp1 hinzugefügt';
  }

  @override
  String activityJoined(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du bist',
      'other': '$by ist',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'dir',
      'other': '$member',
    });
    return '$_temp0 mit einer Einladung von $_temp1 beigetreten';
  }

  @override
  String activityLetIn(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'dich',
      'other': '$member',
    });
    return '$_temp0 $_temp1 hereingelassen';
  }

  @override
  String activityRemoved(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'dich',
      'other': '$member',
    });
    return '$_temp0 $_temp1 entfernt';
  }

  @override
  String activityLeft(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 die Gruppe verlassen';
  }

  @override
  String activityMovedUp(
    String by,
    String byMe,
    String member,
    String memberMe,
    String place,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'dich',
      'other': '$member',
    });
    return '$_temp0 $_temp1 auf den $place Platz hochgestuft';
  }

  @override
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'dich',
      'other': '$member',
    });
    return '$_temp0 $_temp1 ans Ende verschoben';
  }

  @override
  String activityManagersAll(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 allen die Verwaltung der Gruppe erlaubt';
  }

  @override
  String activityManagersFirst(String by, String byMe, int n) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    String _temp1 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'den ersten $n',
      one: 'nur der ersten Person',
    );
    return '$_temp0 die Verwaltung der Gruppe $_temp1 überlassen';
  }

  @override
  String activityLimits(String by, String byMe, int people, int perRoom) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 die Gruppe auf $people Personen und $perRoom pro Sprachraum festgelegt';
  }

  @override
  String activityRoomOpened(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 den Sprachraum „$room“ eröffnet';
  }

  @override
  String activityRoomRenamed(String by, String byMe, String from, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 den Sprachraum „$from“ in „$room“ umbenannt';
  }

  @override
  String activityRoomClosed(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 den Sprachraum „$room“ gelöscht';
  }

  @override
  String activityInvited(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 einen Einladungslink erstellt';
  }

  @override
  String activityRevoked(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 einen Einladungslink widerrufen';
  }

  @override
  String activityRoomOpenToAll(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 „$room“ für alle geöffnet';
  }

  @override
  String activityRoomForAbove(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 „$room“ den Höhergestellten vorbehalten';
  }

  @override
  String activityRoomOnly(String by, String byMe, String people, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    return '$_temp0 „$room“ nur für $people freigegeben';
  }

  @override
  String activityKeptOut(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'dich',
      'other': '$member',
    });
    return '$_temp0 $_temp1 bis auf Weiteres aus „$room“ ausgesperrt';
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
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'dich',
      'other': '$member',
    });
    String _temp2 = intl.Intl.selectLogic(untilKind, {
      'today': 'bis $until',
      'todayAtOne': 'bis $until',
      'yesterday': 'bis $until',
      'other': 'bis zum $until',
    });
    return '$_temp0 $_temp1 $_temp2 aus „$room“ ausgesperrt';
  }

  @override
  String activityLetBack(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Du hast',
      'other': '$by hat',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'dich',
      'other': '$member',
    });
    return '$_temp0 $_temp1 wieder in „$room“ gelassen';
  }

  @override
  String get activityDefaultRoom => 'Allgemein';

  @override
  String get activityYouInList => 'dich';

  @override
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Du warst',
      'other': '$author war',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' von dir',
      'other': ' von $by',
      'none': '',
    });
    return 'Rückgängig gemacht: $_temp0 gleichzeitig$_temp1 entfernt worden';
  }

  @override
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Du warst',
      'other': '$author war',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' von dir',
      'other': ' von $by',
      'none': '',
    });
    return 'Rückgängig gemacht: $_temp0 gleichzeitig$_temp1 ans Ende verschoben worden';
  }

  @override
  String activityUndoneLeft(String author, String authorMe) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Du hattest',
      'other': '$author hatte',
    });
    return 'Rückgängig gemacht: $_temp0 die Gruppe gleichzeitig verlassen';
  }

  @override
  String get activityAboutList =>
      'Alle in der Gruppe sehen dieselbe Liste, die aus den Aufzeichnungen der Gruppe selbst stammt. Änderungen an Bild und Banner werden getrennt geführt: Das sind die, die dieses Gerät gesehen hat.';

  @override
  String get activityAboutOrder =>
      'In der Reihenfolge der Gruppe; die Zeiten stammen von der Uhr der jeweiligen Person.';

  @override
  String get activityNothingYet => 'Noch nichts.';

  @override
  String get activityLoading => 'Wird geladen …';

  @override
  String get activityShowOlder => 'Ältere anzeigen';

  @override
  String activitySeenHere(String when) {
    return '$when · auf diesem Gerät gesehen';
  }

  @override
  String get bellNotifications => 'Benachrichtigungen';

  @override
  String bellNewNotifications(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n neue Benachrichtigungen',
      one: '1 neue Benachrichtigung',
    );
    return '$_temp0';
  }

  @override
  String get bellClearAll => 'Alle löschen';

  @override
  String get bellNothingNew => 'Nichts Neues';

  @override
  String get bellFooter =>
      'Die letzten drei bleiben hier. Verpasste Anrufe und Anfragen erscheinen auch unter „Freunde“.';

  @override
  String bellBothInRoom(String room) {
    return 'Ihr seid beide in $room.';
  }

  @override
  String get bellDismiss => 'Ausblenden';

  @override
  String bellFriendRequest(String name) {
    return '$name möchte mit dir befreundet sein';
  }

  @override
  String bellCheckFingerprint(String fingerprint) {
    return 'Prüfe, ob der Fingerabdruck übereinstimmt: $fingerprint';
  }

  @override
  String bellJoinAsk(String name, String group) {
    return '$name möchte „$group“ beitreten';
  }

  @override
  String bellJoinAskNote(String fingerprint) {
    return 'Mit einem der Einladungslinks deiner Gruppe. Fingerabdruck: $fingerprint';
  }

  @override
  String get bellLetIn => 'Hereinlassen';

  @override
  String get bellSayNo => 'Ablehnen';

  @override
  String bellGroupInvite(String name, String group) {
    return '$name hat dich zu „$group“ hinzugefügt';
  }

  @override
  String bellMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Mitglieder',
      one: '1 Mitglied',
    );
    return '$_temp0';
  }

  @override
  String get bellJoin => 'Beitreten';

  @override
  String bellNewMessages(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n neue Nachrichten von $name',
      one: 'Neue Nachricht von $name',
    );
    return '$_temp0';
  }

  @override
  String get bellOpen => 'Öffnen';

  @override
  String get bellMarkRead => 'Als gelesen markieren';

  @override
  String get bellFirewallTitle => 'Deine Firewall sperrt einige Freunde aus';

  @override
  String get bellCopyCommand => 'Befehl kopieren';

  @override
  String get bellOtherSessionTitle =>
      'Auch auf einem anderen Computer geöffnet';

  @override
  String get bellSettingsResetTitle =>
      'Einige Einstellungen wurden zurückgesetzt';

  @override
  String get bellSettingsResetText =>
      'Einige deiner Einstellungen konnten nicht gelesen werden und stehen wieder auf ihren Standardwerten: Prüfe sie in den Einstellungen. Die alte Datei liegt als app.json.bad neben der neuen.';

  @override
  String get friendCardOnline => 'Online';

  @override
  String get friendCardAway => 'Abwesend';

  @override
  String get friendCardOffline => 'Offline';

  @override
  String get friendCardCallsOnlyOnline =>
      'Anrufe klingeln nur, wenn ein Freund online ist.';

  @override
  String friendCardHangUpFirst(String name) {
    return 'Leg zuerst auf, um $name anzurufen.';
  }

  @override
  String friendCardBothInRoom(String room) {
    return 'Ihr seid beide in $room.';
  }

  @override
  String get friendCardMessage => 'Nachricht';

  @override
  String friendCardMutualFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n gemeinsame Freunde',
      one: '1 gemeinsamer Freund',
    );
    return '$_temp0';
  }

  @override
  String get memberCardSendRequest => 'Freundschaftsanfrage senden';

  @override
  String get memberCardRequestSent => 'Freundschaftsanfrage gesendet';

  @override
  String get memberCardAccept => 'Freundschaftsanfrage annehmen';

  @override
  String memberCardAsksYou(String name) {
    return '$name möchte mit dir befreundet sein.';
  }

  @override
  String memberCardInGroup(String group) {
    return 'Mitglied von $group';
  }

  @override
  String memberCardOnlyOnline(String name) {
    return 'Freundschaftsanfragen gehen nur, solange $name online ist.';
  }

  @override
  String get memberCardShowOwn => 'Deine Karte zeigen';

  @override
  String get userPanelEditProfile => 'Profil bearbeiten';

  @override
  String get userPanelStatus => 'Status';

  @override
  String get userPanelOnline => 'Online';

  @override
  String get userPanelAway => 'Abwesend';

  @override
  String get userPanelOffline => 'Offline';

  @override
  String get userPanelOfflineNote =>
      'Freunde und Mitglieder deiner Gruppen sehen dich als offline. Du kannst trotzdem in Räumen sein.';

  @override
  String get userPanelOnlineNote =>
      'Freunde und Mitglieder deiner Gruppen sehen dich als online – oder als abwesend, wenn du den Computer 10 min nicht benutzt hast.';

  @override
  String get userPanelBio => 'BIO';

  @override
  String get userPanelAppearingOffline => 'Als offline angezeigt';

  @override
  String get userPanelCopyLinkLabel => 'Freundschaftslink kopieren';

  @override
  String userPanelLinkExpiresIn(String time) {
    return 'Läuft in $time ab';
  }

  @override
  String get userPanelLinkTerms => '1 Person · 10 min';

  @override
  String get userPanelCopyLink => 'Freundschaftslink kopieren';

  @override
  String get switcherChatEnded => 'Direktchat · nicht mehr befreundet';

  @override
  String get switcherChatOnline => 'Direktchat · online';

  @override
  String get switcherChatAway => 'Direktchat · abwesend';

  @override
  String get switcherChat => 'Direktchat';

  @override
  String switcherFriend(String presence) {
    return 'Freund · $presence';
  }

  @override
  String switcherVoiceRoom(String group) {
    return 'Sprachraum in $group';
  }

  @override
  String switcherVoiceRoomPeople(String group, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Sprachraum in $group · $n anwesend',
    );
    return '$_temp0';
  }

  @override
  String switcherGroup(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Gruppe · $n Mitglieder',
      one: 'Gruppe · 1 Mitglied',
    );
    return '$_temp0';
  }

  @override
  String get switcherPlaceholder => 'Zu Chat, Gruppe oder Sprachraum wechseln';

  @override
  String get switcherGoTo => 'Wechseln zu';

  @override
  String get switcherEmpty => 'Noch keine Gruppen, Freunde oder Chats.';

  @override
  String switcherNoMatch(String query) {
    return 'Nichts passt zu „$query“.';
  }

  @override
  String get switcherHintChoose => 'zum Auswählen';

  @override
  String get switcherHintGo => 'zum Öffnen';

  @override
  String get switcherHintClose => 'zum Schließen';

  @override
  String switcherTooltip(String shortcut) {
    return 'Zu Chat, Gruppe oder Sprachraum wechseln ($shortcut)';
  }

  @override
  String get friendsIntro =>
      'Jede Freundschaft existiert nur auf euren beiden Geräten. Kein Verzeichnis, kein Server.';

  @override
  String get friendsTabAll => 'Alle';

  @override
  String get friendsTabOnline => 'Online';

  @override
  String get friendsTabRequests => 'Anfragen';

  @override
  String get friendsSearch => 'Freunde suchen';

  @override
  String get friendsNone =>
      'Noch keine Freunde. Teile deinen QR-Code, Link oder Code, oder nutze den einer anderen Person.';

  @override
  String get friendsNoneOnline => 'Gerade ist kein Freund online.';

  @override
  String get friendsNoMatch => 'Niemand passt zur Suche.';

  @override
  String get friendsInARoom => 'In einem Raum';

  @override
  String friendsInRoomOf(String group) {
    return 'Im Raum · $group';
  }

  @override
  String get friendsOnline => 'Online';

  @override
  String get friendsAway => 'Abwesend';

  @override
  String get friendsNotSeenYet => 'Noch nie online gesehen';

  @override
  String friendsSeen(String when) {
    return 'Zuletzt online $when';
  }

  @override
  String friendsShowCard(String name) {
    return 'Karte von $name anzeigen';
  }

  @override
  String friendsMissedCall(String when) {
    return 'Verpasster Anruf · $when';
  }

  @override
  String get friendsMessage => 'Nachricht';

  @override
  String friendsOfflineNoCall(String name) {
    return '$name ist offline: Anrufe klingeln nur, wenn ein Freund online ist';
  }

  @override
  String friendsHangUpFirst(String name) {
    return 'Leg zuerst auf, um $name anzurufen';
  }

  @override
  String friendsBothInRoom(String room) {
    return 'Ihr seid beide in $room.';
  }

  @override
  String friendsOptions(String name) {
    return 'Optionen für $name';
  }

  @override
  String friendsRemoveTitle(String name) {
    return '$name aus deiner Freundesliste entfernen?';
  }

  @override
  String get friendsRemoveText =>
      'Ihr verschwindet gegenseitig aus euren Freundeslisten, ohne Nachricht. In gemeinsamen Gruppen bleibt ihr zusammen. Um wieder Freunde zu werden, braucht es eine neue Einladung.';

  @override
  String get friendsRemove => 'Entfernen';

  @override
  String friendsAddToGroup(String group) {
    return 'Zu „$group“ hinzufügen';
  }

  @override
  String get friendsRemoveMenu => 'Als Freund entfernen …';

  @override
  String get friendsNoRequests => 'Keine offenen Anfragen.';

  @override
  String friendsRequestIncoming(String when) {
    return 'Möchte mit dir befreundet sein · $when';
  }

  @override
  String friendsRequestOutgoing(String when) {
    return 'Gesendet $when · wartet auf Antwort';
  }

  @override
  String get friendsPendingRequest => 'Offene Anfrage';

  @override
  String friendsCheckFingerprint(String name) {
    return 'Prüfe, ob der Fingerabdruck mit dem übereinstimmt, den $name auf dem eigenen Bildschirm sieht.';
  }

  @override
  String get friendsAddTitle => 'Freund hinzufügen';

  @override
  String get friendsAddNote => 'Eure Geräte tauschen Schlüssel direkt aus.';

  @override
  String get friendsTabCode => 'Code';

  @override
  String get friendsTabLink => 'Link';

  @override
  String get friendsTabQr => 'QR';

  @override
  String get friendsMakeInvitation => 'Einladung erstellen';

  @override
  String get friendsInvitationExpired => 'Diese Einladung ist abgelaufen.';

  @override
  String get friendsMakeAnother => 'Neue erstellen';

  @override
  String friendsHintQr(String time) {
    return 'In Fogata scannen · läuft in $time ab';
  }

  @override
  String friendsHintLink(String time) {
    return 'An eine Person senden · läuft in $time ab';
  }

  @override
  String friendsHintCode(String time) {
    return 'Einer Person vorlesen · läuft in $time ab';
  }

  @override
  String get friendsHiddenCode =>
      'Verborgen, falls du deinen Bildschirm teilst.';

  @override
  String get friendsHiddenLink =>
      'Verborgen, falls du deinen Bildschirm teilst. Er enthält die Netzwerkadressen dieses Computers: Teile ihn nur privat.';

  @override
  String get friendsGotLink =>
      'Hast du einen Freundschafts- oder Gruppenlink oder einen Code?';

  @override
  String get friendsPastePlaceholder => 'Link oder Code einfügen';

  @override
  String get friendsLinkOrCode => 'Link oder Code';

  @override
  String get friendsAdd => 'Hinzufügen';

  @override
  String get friendsQrLabel => 'QR-Code deiner Einladung';

  @override
  String friendsAddedToGroup(String name) {
    return '$name bekommt die Gruppe, sobald ihr beide das nächste Mal online seid.';
  }

  @override
  String get directToday => 'Heute';

  @override
  String get directYesterday => 'Gestern';

  @override
  String directTimerOff(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Du hast',
      'other': '$name hat',
    });
    return '$_temp0 verschwindende Nachrichten ausgeschaltet';
  }

  @override
  String directTimerFirst(String time) {
    return 'Verschwindende Nachrichten aktiviert: $time';
  }

  @override
  String directTimerOn(String name, String mine, String time) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Du hast',
      'other': '$name hat',
    });
    return '$_temp0 verschwindende Nachrichten eingeschaltet: $time';
  }

  @override
  String directPinnedLine(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Du hast',
      'other': '$name hat',
    });
    return '$_temp0 eine Nachricht angepinnt';
  }

  @override
  String get directCallFewSeconds => 'ein paar Sekunden';

  @override
  String directCallMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Minuten',
      one: '1 Minute',
    );
    return '$_temp0';
  }

  @override
  String directCallHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Stunden',
      one: '1 Stunde',
    );
    return '$_temp0';
  }

  @override
  String directCallHoursMinutes(int hours, int minutes) {
    String _temp0 = intl.Intl.pluralLogic(
      hours,
      locale: localeName,
      other: '$hours Stunden',
      one: '1 Stunde',
    );
    String _temp1 = intl.Intl.pluralLogic(
      minutes,
      locale: localeName,
      other: '$minutes Minuten',
      one: '1 Minute',
    );
    return '$_temp0 $_temp1';
  }

  @override
  String directCallLine(String name, String mine, String length) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Du hast',
      'other': '$name hat',
    });
    return '$_temp0 einen Anruf gestartet, der $length dauerte';
  }

  @override
  String get directBlockedOffline =>
      'Du wirst als offline angezeigt. Geh online, um zu schreiben.';

  @override
  String directBlockedFriendOffline(String name) {
    return '$name ist offline. Ihr könnt schreiben, wenn ihr beide online seid.';
  }

  @override
  String get directOnline => 'Online';

  @override
  String get directAway => 'Abwesend';

  @override
  String get directOffline => 'Offline';

  @override
  String directOfflineSeen(String when) {
    return 'Offline · zuletzt online $when';
  }

  @override
  String get directHeading => 'DIREKTNACHRICHTEN';

  @override
  String directPreviewMine(String text) {
    return 'Du: $text';
  }

  @override
  String directOpenChat(String name) {
    return 'Chat mit $name öffnen';
  }

  @override
  String directShowCard(String name) {
    return 'Karte von $name anzeigen';
  }

  @override
  String get directNoLongerFriend => 'Nicht mehr befreundet';

  @override
  String directCallOffline(String name) {
    return '$name ist offline: Anrufe klingeln nur, wenn ein Freund online ist';
  }

  @override
  String directHangUpFirst(String name) {
    return 'Leg zuerst auf, um $name anzurufen';
  }

  @override
  String directPinnedCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n angepinnte Nachrichten',
      one: '1 angepinnte Nachricht',
    );
    return '$_temp0';
  }

  @override
  String get directPinnedTitle => 'Angepinnte Nachrichten';

  @override
  String directPinnedNote(int max) {
    return 'Für euch beide angepinnt, und sie verschwinden nicht. Bis zu $max.';
  }

  @override
  String get directNothingPinned => 'Noch nichts angepinnt';

  @override
  String get directNothingPinnedNote =>
      'Fahr mit der Maus über eine Nachricht und wähle die Pinnnadel, um sie hier zu behalten.';

  @override
  String directDayTime(String day, String time) {
    return '$day, $time';
  }

  @override
  String directFilesCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Dateien',
    );
    return '$_temp0';
  }

  @override
  String directShowMessageFrom(String name) {
    return 'Nachricht von $name anzeigen';
  }

  @override
  String get directUnpin => 'Lösen';

  @override
  String get directPin => 'Anpinnen';

  @override
  String get directDeleteMessage => 'Für mich löschen';

  @override
  String get directDeleteMessageTitle => 'Diese Nachricht löschen?';

  @override
  String directDeleteMessageNote(String name) {
    return 'Sie wird mit ihren Bildern und Dateien von diesem Computer gelöscht. $name behält die eigene Kopie.';
  }

  @override
  String get directDeleteMessageAction => 'Löschen';

  @override
  String get directDeleteForBoth => 'Für beide löschen';

  @override
  String directDeleteOwnNote(String name) {
    return 'Für beide löschen entfernt sie auch aus dem Chat von $name, sobald deren Fogata es erhält. Für mich löschen lässt sie dort.';
  }

  @override
  String directTimerLabel(String time) {
    return 'Verschwindende Nachrichten: $time';
  }

  @override
  String get directTimerTitle => 'Verschwindende Nachrichten';

  @override
  String get directCloseMenu => 'Menü schließen';

  @override
  String get directTimerNote =>
      'Neue Nachrichten verschwinden auf beiden Geräten, samt Bildern und Dateien.';

  @override
  String get directEmptyNote =>
      'Was du schreibst, wird nur auf euren beiden Computern gespeichert.';

  @override
  String directPinLineLabel(String line) {
    return '$line: anzeigen';
  }

  @override
  String get directShowMessage => 'Nachricht anzeigen';

  @override
  String get directPinnedStays => 'Angepinnt: bleibt, bis sie gelöst wird';

  @override
  String get directPinned => 'Angepinnt';

  @override
  String directPinsFull(int max) {
    return 'Es können höchstens $max Nachrichten angepinnt werden: Löse zuerst eine';
  }

  @override
  String get directDelivered => 'Zugestellt';

  @override
  String get directNotDelivered => 'Nicht zugestellt';

  @override
  String get directSending => 'Wird gesendet …';

  @override
  String directWaiting(String name) {
    return 'Wartet, bis $name online ist · wird gesendet, wenn ihr beide online seid';
  }

  @override
  String get directAvailableWhenYouOnline => 'verfügbar, wenn du online gehst';

  @override
  String directAvailableWhenOnline(String name) {
    return 'verfügbar, wenn $name online ist';
  }

  @override
  String get directDownloadedByItself => 'automatisch heruntergeladen';

  @override
  String directOpenFile(String name) {
    return '$name öffnen';
  }

  @override
  String directPlayFile(String name) {
    return '$name abspielen';
  }

  @override
  String directDownloadSize(String size) {
    return 'Herunterladen · $size';
  }

  @override
  String directVideoPlaysWhenHere(String name, String length) {
    return '$name · $length · wird abgespielt, sobald es da ist';
  }

  @override
  String directVideoNotHere(String name, String length, String when) {
    return '$name · $length · $when';
  }

  @override
  String directGettingIt(int percent) {
    return 'Wird geladen · $percent %';
  }

  @override
  String directFileNotHere(String size, String when) {
    return '$size · $when';
  }

  @override
  String get directSaveToDownloads => 'In Downloads speichern';

  @override
  String get directRemovedForSpace => 'Hier entfernt, um Speicher freizugeben';

  @override
  String get directAskAgain => 'Erneut anfordern';

  @override
  String get directClosePicture => 'Bild schließen';

  @override
  String get directCloseVideo => 'Video schließen';

  @override
  String get directCannotPlay =>
      'Dieser Computer kann dieses Video nicht abspielen.';

  @override
  String get directPause => 'Pause';

  @override
  String get directPlay => 'Abspielen';

  @override
  String get directSoundOn => 'Ton an';

  @override
  String get directMute => 'Stummschalten';

  @override
  String directRemoveFilesTitle(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Die $n Dateien entfernen?',
      one: 'Datei entfernen?',
    );
    return '$_temp0';
  }

  @override
  String get directRemoveFilesText =>
      'Sie werden aus dieser Nachricht genommen. Die Dateien auf deinem Computer bleiben unverändert.';

  @override
  String get directDraftKept =>
      'Was du geschrieben hast, bleibt hier, bis du es senden kannst.';

  @override
  String directMessagePlaceholder(String name) {
    return 'Nachricht an $name';
  }

  @override
  String get directAddMessageOptional => 'Nachricht hinzufügen (optional)';

  @override
  String get directAutoDownloadNote =>
      'Bilder bis 10 MB werden automatisch heruntergeladen; Videos und Dateien, wenn du sie öffnest.';

  @override
  String get directVoiceRecord => 'Sprachnachricht aufnehmen';

  @override
  String get directVoiceInCall =>
      'Während eines Anrufs kannst du nicht aufnehmen';

  @override
  String directVoiceWaitingElsewhere(String name) {
    return 'Sende oder lösche zuerst die Sprachnachricht in deinem Chat mit $name.';
  }

  @override
  String get directVoiceDelete => 'Löschen';

  @override
  String get directVoiceTooShort =>
      'Zu kurz zum Senden: Eine Sprachnachricht dauert mindestens eine halbe Sekunde.';

  @override
  String directVoiceLeft(String time) {
    return 'noch $time';
  }

  @override
  String directVoiceRecordingFrom(String microphone) {
    return 'Aufnahme über $microphone';
  }

  @override
  String get directVoiceRecordingDefault =>
      'Das gewählte Mikrofon ist nicht verfügbar: Aufnahme mit dem Standardmikrofon';

  @override
  String directVoiceKeys(String esc, String enter) {
    return '$esc löscht sie, $enter sendet sie';
  }

  @override
  String directVoiceKeySend(String enter) {
    return '$enter sendet sie';
  }

  @override
  String directVoicePreview(String length) {
    return 'Sprachnachricht · $length';
  }

  @override
  String get directVoiceMessage => 'Sprachnachricht';

  @override
  String get directVoiceNoMic => 'Fogata kann das Mikrofon nicht verwenden.';

  @override
  String get directVoiceMicStopped =>
      'Das Mikrofon ist ausgefallen. Das Aufgenommene kannst du trotzdem senden.';

  @override
  String directVoiceNoMicCheck(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Prüfe, ob eines angeschlossen, unter Audio & Video ausgewählt und unter Systemeinstellungen › Datenschutz & Sicherheit erlaubt ist.',
      'other': 'Prüfe, ob eines angeschlossen und unter Audio & Video ausgewählt ist.',
    });
    return '$_temp0';
  }

  @override
  String get directVoiceSpeed => 'Wiedergabegeschwindigkeit';

  @override
  String get directVoiceNotPlayed => 'Noch nicht angehört';

  @override
  String directRemoveFile(String name) {
    return '$name entfernen';
  }

  @override
  String get directAltered =>
      'Der Verlauf dieses Chats auf diesem Computer wurde geändert, seit Fogata ihn zuletzt gespeichert hat. Einige Nachrichten fehlen vielleicht.';

  @override
  String directEnded(String name) {
    return 'Du und $name seid nicht mehr befreundet. Dieser Chat kann nur noch gelesen werden.';
  }

  @override
  String get directDeleteChat => 'Chat löschen';

  @override
  String directDeleteTitle(String name) {
    return 'Chat mit $name löschen?';
  }

  @override
  String directDeleteText(String name) {
    return 'Nachrichten, Bilder, Videos und Dateien werden von diesem Computer gelöscht. $name behält die eigene Kopie.';
  }

  @override
  String get directDelete => 'Löschen';

  @override
  String get directUsageMessages => 'Nachrichten';

  @override
  String get directUsagePictures => 'Bilder';

  @override
  String get directUsageVideos => 'Videos';

  @override
  String get directUsageFiles => 'Dateien';

  @override
  String get directUsageHeading => 'DIESER CHAT AUF DIESEM GERÄT';

  @override
  String get directFreeUpSpace => 'Speicher freigeben …';

  @override
  String get directAddFilesTooltip => 'Bilder, Videos oder Dateien hinzufügen';

  @override
  String get directAddFiles => 'Dateien hinzufügen';

  @override
  String get accountMoveLessThanMinuteLeft => 'weniger als eine Minute übrig';

  @override
  String get accountMoveAboutMinuteLeft => 'etwa eine Minute übrig';

  @override
  String accountMoveAboutMinutesLeft(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'etwa $n Minuten übrig',
      one: 'etwa eine Minute übrig',
    );
    return '$_temp0';
  }

  @override
  String accountMoveNoRoom(String need, String free) {
    return 'Dieser Computer braucht $need freien Speicher für das Konto und hat $free.';
  }

  @override
  String accountMoveTooLarge(String size) {
    return 'Der andere Computer hat $size angeboten, weit mehr, als ein Konto umfasst.';
  }

  @override
  String get accountMoveBusy =>
      'Dieser Computer schließt gerade einen anderen Umzug ab: Schließ ihn zuerst ab oder verwirf ihn.';

  @override
  String get accountMoveNothingToFinish =>
      'Dieser Computer hat keinen Umzug vom anderen Computer abzuschließen.';

  @override
  String get accountMoveNetworkStopped => 'Das Netzwerk wurde angehalten.';

  @override
  String get accountMoveConnectionLost => 'Die Verbindung wurde unterbrochen.';

  @override
  String get accountMovePeerNoRoom =>
      'Der neue Computer hat nicht genug freien Speicher.';

  @override
  String get accountMovePeerTooLarge =>
      'Das Konto ist zu groß für einen Umzug.';

  @override
  String get accountMovePeerBusy =>
      'Der neue Computer schließt gerade einen anderen Umzug ab.';

  @override
  String get accountMovePeerCouldNotKeep =>
      'Der neue Computer konnte die Dateien nicht speichern.';

  @override
  String get accountMovePeerCancelled =>
      'Der Umzug wurde auf dem anderen Computer abgebrochen.';

  @override
  String get accountMovePeerCouldNotFinish =>
      'Der neue Computer konnte den Umzug noch nicht abschließen.';

  @override
  String get accountMovePeerStopped =>
      'Der Umzug wurde auf dem anderen Computer gestoppt.';

  @override
  String get accountMovePeerNothingToFinish =>
      'Der andere Computer hat keinen Umzug von hier abzuschließen.';

  @override
  String get accountMovePeerNotProven =>
      'Der andere Computer konnte nicht nachweisen, dass er diesen Umzug hat.';

  @override
  String get accountMoveWrongCode =>
      'Der andere Computer hat den Code nicht akzeptiert, daher wurde nichts gesendet.';

  @override
  String get accountMoveExpired =>
      'Der Code ist abgelaufen, bevor er benutzt wurde.';

  @override
  String get accountMoveTooManyTries =>
      'Es wurden mehrere falsche Codes versucht; der Code funktioniert nicht mehr.';

  @override
  String get accountMoveNoContact =>
      'Die beiden Computer konnten sich nicht erreichen.';

  @override
  String get accountMoveUnconfirmed =>
      'Der letzte Schritt des Umzugs wurde nicht bestätigt.';

  @override
  String get accountMoveCancelled => 'Abgebrochen.';

  @override
  String get accountMoveInterrupted =>
      'Der Umzug wurde vor dem Ende unterbrochen; nichts hat sich geändert.';

  @override
  String accountMoveInterruptedWhy(String why) {
    return 'Der Umzug wurde vor dem Ende unterbrochen; nichts hat sich geändert. $why';
  }

  @override
  String get accountMoveDeclined =>
      'Der andere Computer hat den Umzug gestoppt.';

  @override
  String accountMoveDeclinedWhy(String why) {
    return 'Der andere Computer hat den Umzug gestoppt. $why';
  }

  @override
  String get accountMoveLater =>
      'Das Konto ist angekommen und wird beim nächsten Start von Fogata hier eingerichtet.';

  @override
  String accountMoveLaterWhy(String why) {
    return 'Das Konto ist angekommen und wird beim nächsten Start von Fogata hier eingerichtet. $why';
  }

  @override
  String get accountMoveDidNotFinish => 'Der Umzug wurde nicht abgeschlossen.';

  @override
  String get accountMoveAnotherComputer => 'ein anderer Computer';

  @override
  String get accountMoveTheOtherComputer => 'der andere Computer';

  @override
  String get moveTooLateBeingPlaced =>
      'Zu spät zum Abbrechen: Das Konto wird gerade eingerichtet.';

  @override
  String get moveTooLateDiscard =>
      'Zu spät: Das Konto wird gerade eingerichtet.';

  @override
  String get moveTooLateOtherHasIt =>
      'Zu spät zum Abbrechen: Der andere Computer hat das Konto jetzt.';

  @override
  String moveFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Freunde',
      one: '1 Freund',
    );
    return '$_temp0';
  }

  @override
  String moveGroups(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Gruppen',
      one: '1 Gruppe',
    );
    return '$_temp0';
  }

  @override
  String moveGroupsWithKeys(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Gruppen, mit ihren Schlüsseln',
      one: '1 Gruppe, mit ihren Schlüsseln',
    );
    return '$_temp0';
  }

  @override
  String moveChats(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Chats',
      one: '1 Chat',
    );
    return '$_temp0';
  }

  @override
  String moveInAll(String size) {
    return '$size insgesamt';
  }

  @override
  String get moveYourSettings => 'Deine Einstellungen';

  @override
  String get moveNotDevices => 'ohne Mikrofon- und Kamerawahl';

  @override
  String get moveFinishingBoth => 'Wird auf beiden Computern abgeschlossen …';

  @override
  String movePercent(int percent) {
    return '$percent %';
  }

  @override
  String movePercentLeft(int percent, String left) {
    return '$percent % · $left';
  }

  @override
  String get moveStepIdentity => 'Identität und Profil';

  @override
  String get moveStepFriends => 'Freunde';

  @override
  String get moveStepGroups => 'Gruppen und ihre Schlüssel';

  @override
  String get moveStepChats => 'Chats, Bilder und Dateien';

  @override
  String moveBytesOf(String done, String total) {
    return '$done von $total';
  }

  @override
  String get moveStepFinishing => 'Abschluss auf beiden Computern';

  @override
  String moveBringingNamed(String name) {
    return 'Konto von $name wird übertragen';
  }

  @override
  String get moveBringingYours => 'Dein Konto wird übertragen';

  @override
  String get moveKeepBothOn => 'Lass beide Computer an und Fogata geöffnet.';

  @override
  String get moveKeepOpenNote =>
      'Lass Fogata auf beiden Computern geöffnet. Im selben Netzwerk geht es am schnellsten; sonst verbinden sie sich direkt über das Internet.';

  @override
  String moveFinishBringingNamed(String name) {
    return 'Umzug des Kontos von $name abschließen';
  }

  @override
  String get moveFinishBringingYours => 'Umzug deines Kontos abschließen';

  @override
  String get moveEverythingCame =>
      'Alles ist angekommen. Gib diesen Code auf dem anderen Computer ein, um abzuschließen.';

  @override
  String moveEverythingCameFrom(String computer) {
    return 'Alles ist angekommen. Gib diesen Code auf „$computer“ ein, um abzuschließen.';
  }

  @override
  String get moveBringHereTitle => 'Dein Konto hierher holen';

  @override
  String get moveBringHereSub =>
      'Gib diesen Code auf dem Computer ein, der dein Konto hat.';

  @override
  String get moveHideCode => 'Code verbergen';

  @override
  String get moveShowCode => 'Code anzeigen';

  @override
  String moveExpiresIn(String time) {
    return 'läuft in $time ab';
  }

  @override
  String moveStepFogataShows(String what) {
    return 'Auf dem anderen Computer zeigt Fogata $what.';
  }

  @override
  String get moveWaitingToConfirm => 'Wartet auf Bestätigung';

  @override
  String get moveStepOpenThere => 'Öffne Fogata auf dem anderen Computer.';

  @override
  String moveStepTypeUnder(String where) {
    return 'Gib den Code unter $where ein.';
  }

  @override
  String get moveFinishTheMove => 'Umzug abschließen';

  @override
  String moveStepGoTo(String where) {
    return 'Geh zu $where.';
  }

  @override
  String get moveSettingsPath => 'Einstellungen → Konto umziehen';

  @override
  String get moveToAnotherComputer => 'Auf anderen Computer umziehen';

  @override
  String get moveWaitingForOther => 'Warten auf deinen anderen Computer …';

  @override
  String moveWrongCodesTyped(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Auf anderen Computern wurden $n-mal falsche Codes eingegeben.',
      one: 'Auf einem anderen Computer wurde ein falscher Code eingegeben.',
    );
    return '$_temp0';
  }

  @override
  String get moveThrowAwayInstead => 'Stattdessen verwerfen …';

  @override
  String get moveFoundOther => 'Dein anderer Computer wurde gefunden';

  @override
  String get moveFoundSameNetwork =>
      'Gleiches Netzwerk · zwischen beiden verschlüsselt';

  @override
  String get moveFoundInternet =>
      'Über das Internet · zwischen beiden verschlüsselt';

  @override
  String get moveConfirmThere =>
      'Bestätige auf dem anderen Computer, um zu starten';

  @override
  String get moveSameAccountNote =>
      'Das ist das Konto, das schon auf diesem Computer ist. Beim Herholen geht nichts verloren.';

  @override
  String get moveTakeYes => 'Hierher holen';

  @override
  String get moveWelcomeBack => 'Willkommen zurück';

  @override
  String moveWelcomeBackNamed(String name) {
    return 'Willkommen zurück, $name';
  }

  @override
  String get moveDoneSub =>
      'Dein Konto ist jetzt auf diesem Computer. Der andere Computer verwendet es nicht mehr.';

  @override
  String get moveOpenFogata => 'Fogata öffnen';

  @override
  String get moveCodeExpiredLabel => 'abgelaufen';

  @override
  String get moveCodeStoppedLabel => 'funktioniert nicht mehr';

  @override
  String get moveCodeExpiredNote =>
      'Dieser Code ist abgelaufen, bevor dein anderer Computer ihn benutzt hat. Codes gelten 10 Minuten.';

  @override
  String get moveCodeStoppedNote =>
      'Auf anderen Computern wurden mehrmals falsche Codes eingegeben, daher funktioniert dieser Code nicht mehr.';

  @override
  String get moveNewCode => 'Neuer Code';

  @override
  String get moveAlmostThere => 'Fast geschafft';

  @override
  String get moveLaterSub =>
      'Dein Konto ist vollständig angekommen, konnte aber noch nicht eingerichtet werden. Das passiert beim nächsten Start von Fogata hier.';

  @override
  String get moveStartAgain => 'Fogata neu starten';

  @override
  String get moveUnconfirmedSub =>
      'Alles ist angekommen, aber der andere Computer hat den Abschluss nicht bestätigt. Lass dir einen Code anzeigen und gib ihn dort ein, um den Umzug abzuschließen.';

  @override
  String get moveStoppedThereTitle =>
      'Der Umzug wurde auf dem anderen Computer gestoppt';

  @override
  String get moveStoppedThereSub =>
      'Nichts hat sich geändert: Dein Konto ist noch auf dem anderen Computer, und hier wurde nichts davon behalten.';

  @override
  String get moveCouldNotStartTitle =>
      'Der Umzug konnte nicht gestartet werden';

  @override
  String get moveSomethingWrongHere =>
      'Auf diesem Computer ist etwas schiefgelaufen.';

  @override
  String get moveHalfwayTitle => 'Der Umzug wurde mittendrin unterbrochen';

  @override
  String get moveHalfwaySub =>
      'Die Verbindung zu deinem anderen Computer wurde unterbrochen. Nichts hat sich geändert: Dein Konto ist noch auf dem anderen Computer, und hier wurde nichts davon behalten.';

  @override
  String get moveShowCodeToFinish => 'Code zum Abschließen anzeigen';

  @override
  String get moveTryNewCode => 'Mit neuem Code erneut versuchen';

  @override
  String moveBackToNamed(String name) {
    return 'Zurück zu $name';
  }

  @override
  String get moveBackToThisProfile => 'Zurück zu diesem Profil';

  @override
  String get moveNewProfileInstead => 'Stattdessen neues Profil erstellen';

  @override
  String get moveDiscardTitle => 'Angekommenes Konto verwerfen?';

  @override
  String get moveDiscardText =>
      'Tu das nur, wenn der andere Computer es weiter verwendet. Der andere Computer wartet mit der Bestätigung, bis dort entschieden wird, es weiter zu verwenden.';

  @override
  String get moveKeepIt => 'Behalten';

  @override
  String get moveThrowAway => 'Verwerfen';

  @override
  String get moveBringFromAnother =>
      'Dein Konto von einem anderen Computer holen';

  @override
  String get moveAlreadyOnFogata => 'Schon bei Fogata?';

  @override
  String get moveCodeTriedBefore =>
      'Dieser Code hat schon einmal nicht gepasst. Hol dir auf dem neuen Computer einen neuen Code.';

  @override
  String moveMovingTo(String computer) {
    return 'Dein Konto zieht auf „$computer“ um';
  }

  @override
  String get moveKeepBothOnOffline =>
      'Lass beide Computer an und Fogata geöffnet. Fogata ist hier währenddessen offline.';

  @override
  String get moveCodeFieldLabel =>
      'Der Code, der auf dem neuen Computer angezeigt wird';

  @override
  String get moveToAnotherSub =>
      'Deine Identität, Freunde, Gruppen, Chats, Dateien und Einstellungen gehen verschlüsselt und direkt von hier auf den anderen Computer. Danach verwendet dieser Computer dein Konto nicht mehr, damit es dich nie doppelt gibt.';

  @override
  String moveStepChoose(String choice) {
    return 'Öffne Fogata auf dem neuen Computer und wähle $choice.';
  }

  @override
  String get moveStepTypeItShows => 'Gib den angezeigten Code ein:';

  @override
  String get moveWaitingForAnswer => 'Wartet auf deine Antwort …';

  @override
  String get moveLookingForCode => 'Suche Computer mit diesem Code …';

  @override
  String get moveOfflineWhileMoving =>
      'Fogata geht auf diesem Computer während des Umzugs offline.';

  @override
  String get moveWrongCodeNote =>
      'Der neue Computer hat diesen Code nicht akzeptiert, daher wurde nichts gesendet. Prüfe ihn und gib ihn erneut ein. Wenn es weiter scheitert, hol dir dort einen neuen Code.';

  @override
  String get moveCantReachNew => 'Der neue Computer ist nicht erreichbar.';

  @override
  String get moveCantReachNewNote =>
      'Beide haben sich 3 Minuten lang gesucht. Bring sie ins selbe Netzwerk (dasselbe WLAN oder ein Kabel) und versuch es erneut. Dein Konto ist noch hier und Fogata ist wieder online.';

  @override
  String get moveDeclinedNote =>
      'Der Umzug wurde auf dem neuen Computer gestoppt. Nichts wurde umgezogen: Dein Konto ist noch hier und Fogata ist wieder online.';

  @override
  String get moveCouldNotStart => 'Der Umzug konnte nicht gestartet werden.';

  @override
  String get moveHalfwayNote =>
      'Der Umzug wurde mittendrin unterbrochen: Die Verbindung zum neuen Computer ging verloren. Nichts hat sich geändert: Dein Konto ist noch hier und Fogata ist wieder online.';

  @override
  String get moveThisProfileWords => 'dieses Profil';

  @override
  String get moveBringCardTitle => 'Ein Konto hierher holen';

  @override
  String get moveBringCardSub =>
      'Lass dir auf diesem Computer einen Code anzeigen und gib ihn auf dem Computer ein, der das Konto hat.';

  @override
  String get moveBringCardButton => 'Konto hierher holen …';

  @override
  String moveReplacesHere(String name) {
    return 'Ersetzt $name auf diesem Computer.';
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
      other: '$friends Freunde',
      one: '1 Freund',
    );
    String _temp1 = intl.Intl.pluralLogic(
      groups,
      locale: localeName,
      other: '$groups Gruppen',
      one: '1 Gruppe',
    );
    String _temp2 = intl.Intl.pluralLogic(
      chats,
      locale: localeName,
      other: '$chats Chats',
      one: '1 Chat',
    );
    return '$bold, das aktuelle Profil auf diesem Computer, wird ersetzt; $_temp0, $_temp1 und $_temp2 werden hier gelöscht. Um $name zu behalten, zieh es zuerst auf einen anderen Computer um.';
  }

  @override
  String moveReplaceAndShow(String name) {
    return '$name ersetzen und Code anzeigen';
  }

  @override
  String moveConfirmTitle(String computer) {
    return 'Dein Konto auf „$computer“ umziehen?';
  }

  @override
  String get moveConfirmSameNetwork =>
      'Im selben Netzwerk gefunden. Alles geht verschlüsselt und direkt dorthin.';

  @override
  String get moveConfirmInternet =>
      'Über das Internet gefunden. Alles geht verschlüsselt und direkt dorthin.';

  @override
  String moveConfirmReplaces(String name) {
    return 'Es ersetzt dort das Profil $name.';
  }

  @override
  String get moveConfirmNote =>
      'Danach verwendet dieser Computer dein Konto nicht mehr und kann es auch nicht wieder verwenden. Was hier übrig bleibt, kannst du dann löschen.';

  @override
  String get moveConfirmYes => 'Mein Konto umziehen';

  @override
  String get moveKeepTitle => 'Dein Konto hier weiter verwenden?';

  @override
  String get moveKeepText =>
      'Nur wenn der neue Computer es nicht hat: Öffne dort zuerst Fogata. Wenn beide es verwenden würden, sähen deine Freunde und Gruppen dich doppelt, und Nachrichten kämen nur bei einem an.';

  @override
  String get moveKeepYes => 'Hier weiter verwenden';

  @override
  String moveWaitingFor(String computer) {
    return 'Warten auf Bestätigung von „$computer“';
  }

  @override
  String get moveWaitingText =>
      'Dein Konto wurde gesendet, aber die Verbindung brach ab, bevor der neue Computer gemeldet hat, dass er bereit ist. Dieser Computer verwendet das Konto bis dahin nicht.';

  @override
  String get moveFinishText =>
      'Öffne Fogata auf dem neuen Computer: Es zeigt einen Code an. Gib ihn hier ein.';

  @override
  String get moveFinish => 'Abschließen';

  @override
  String get moveFinishWrongCode =>
      'Der neue Computer hat diesen Code nicht akzeptiert. Prüfe ihn und gib ihn erneut ein.';

  @override
  String get moveFinishNoContact =>
      'Der neue Computer ist nicht erreichbar. Bring beide ins selbe Netzwerk und versuch es erneut.';

  @override
  String get moveKeepThisOne =>
      'Der neue Computer hat es nicht: hier weiter verwenden …';

  @override
  String moveMovedYours(String computer) {
    return 'Dein Konto ist auf „$computer“ umgezogen';
  }

  @override
  String moveMovedNamed(String name, String computer) {
    return 'Das Konto von $name ist auf „$computer“ umgezogen';
  }

  @override
  String get moveMovedNoteYou =>
      'Dieser Computer verwendet es nicht mehr: Freunde und Gruppen erreichen dich auf dem anderen.';

  @override
  String moveMovedNoteNamed(String name) {
    return 'Dieser Computer verwendet es nicht mehr: Freunde und Gruppen erreichen $name auf dem anderen.';
  }

  @override
  String moveMovedTodayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'Heute um $time.',
      'other': 'Heute um $time.',
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
  String get moveLeftHere => 'Was hier übrig ist';

  @override
  String get moveLeftHereText =>
      'Chats, Bilder und Dateien von vor dem Umzug, verschlüsselt. Der andere Computer hat eine eigene Kopie.';

  @override
  String get moveDeleteData => 'Fogata-Daten auf diesem Computer löschen';

  @override
  String get moveAfterDeleting =>
      'Nach dem Löschen startet Fogata hier wie neu.';

  @override
  String get moveCodeNotACode =>
      'Ein Code hat zehn Buchstaben und Ziffern, etwa XXXXX-XXXXX.';

  @override
  String get moveCodeMistyped =>
      'Dieser Code stimmt nicht: Prüfe die Buchstaben und Ziffern.';

  @override
  String get welcomeHeadline => 'Redet, als wärt ihr\nim selben Raum.';

  @override
  String get welcomeIntro =>
      'Sprach-, Video- und Textchat mit Freunden, Familie oder deinem Team, direkt von deinem Computer zu ihrem.';

  @override
  String get welcomeNoAccount => 'Kein Konto';

  @override
  String get welcomeEncrypted => 'Verschlüsselt';

  @override
  String get welcomeEphemeral => 'Flüchtige Räume';

  @override
  String get welcomeCharacter =>
      'Fogatas Figur, eine kleine Flamme mit Kopfhörern, die das Peace-Zeichen macht';

  @override
  String get welcomeImageUnreadable =>
      'Dieses Bild konnte nicht gelesen werden.';

  @override
  String get welcomeKeychainTitle =>
      'Deine Daten können noch nicht verschlüsselt werden';

  @override
  String welcomeKeychainText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' Unter Linux ist der Schlüsselbund ein laufender Secret Service, etwa KWallet oder GNOME Keyring: Starte oder entsperre einen und versuch es dann erneut.',
      'other': ' Entsperre oder repariere den Schlüsselbund und versuch es dann erneut.',
    });
    return 'Fogata verschlüsselt deine Identität, Freunde und Direktnachrichten mit einem Schlüssel im Schlüsselbund des Systems, und der Schlüsselbund hat ihn nicht behalten.$_temp0';
  }

  @override
  String get welcomeKeychainRetry => 'Erneut versuchen';

  @override
  String get welcomeKeychainPlain => 'Unverschlüsselt fortfahren';

  @override
  String get welcomeKeychainPlainNote =>
      'Unverschlüsselt kann sie jeder lesen, der die Dateien dieses Computers kopiert. Die Einstellungen weisen weiter darauf hin, und Fogata verschlüsselt alles, sobald es mit einem Schlüsselbund startet.';

  @override
  String get welcomeWriteName => 'Gib deinen Namen ein.';

  @override
  String get welcomeCardTitle => 'Wie sollen deine Leute dich sehen?';

  @override
  String get welcomeCardSub =>
      'Ein Name und ein Bild. Du kannst sie jederzeit ändern.';

  @override
  String get welcomeNoServer =>
      'Nichts geht an einen Fogata-Server: Es gibt keinen. Das öffentliche IPFS-Netzwerk dient nur als Telefonbuch, um deine Freunde zu finden.';

  @override
  String get welcomeYourName => 'Dein Name';

  @override
  String welcomePictureLabel(String name) {
    return 'Bild: $name';
  }

  @override
  String get welcomeUploadPicture => 'Bild hochladen';

  @override
  String get welcomeNameLabel => 'Name';

  @override
  String get welcomeCreate => 'Erstellen';

  @override
  String get welcomeReady => 'Fertig';

  @override
  String get welcomeCreateLabel => 'Identität erstellen';

  @override
  String welcomeKeyLabel(String key) {
    return 'Auf diesem Computer erstellter Schlüssel: $key';
  }

  @override
  String get welcomeNewKey => 'Neuen Schlüssel erstellen';

  @override
  String get stoppedTitle => 'Fogata funktioniert nicht mehr.';

  @override
  String get stoppedStartingAgain => 'Wird neu gestartet …';

  @override
  String get stoppedRestartToConnect =>
      'Starte es neu, um dich wieder zu verbinden.';

  @override
  String get stoppedRestartNow => 'Jetzt neu starten';

  @override
  String get settingsSectionProfile => 'Mein Profil';

  @override
  String get settingsSectionMove => 'Konto umziehen';

  @override
  String get settingsSectionAppearance => 'Darstellung';

  @override
  String get settingsSectionLanguageTime => 'Sprache & Uhrzeit';

  @override
  String get settingsSectionVoice => 'Audio & Video';

  @override
  String get settingsSectionSounds => 'Töne';

  @override
  String get settingsSectionBehaviour => 'Verhalten';

  @override
  String get settingsSectionNetwork => 'Netzwerk';

  @override
  String get settingsSectionStorage => 'Speicher & Downloads';

  @override
  String get settingsHeadingUser => 'Benutzer';

  @override
  String get settingsHeadingApp => 'App';

  @override
  String get settingsTitle => 'Einstellungen';

  @override
  String get settingsAll => 'Alle Einstellungen';

  @override
  String get settingsCloseTooltip => 'Einstellungen schließen';

  @override
  String get settingsUnsavedSaveTitle => 'Änderungen speichern?';

  @override
  String get settingsUnsavedDiscardTitle => 'Änderungen verwerfen?';

  @override
  String get settingsUnsavedSaveText =>
      'Dein Profil hat Änderungen, die noch nicht gespeichert sind.';

  @override
  String get settingsUnsavedDiscardText =>
      'Dein Profil hat Änderungen, die so nicht gespeichert werden können.';

  @override
  String get settingsProfileSaved =>
      'Profil gespeichert. Freunde und Gruppen, die online sind, sehen es jetzt, die anderen, sobald sie sich verbinden.';

  @override
  String get settingsBioSaved =>
      'Bio gespeichert. Freunde, die online sind, sehen sie jetzt, die anderen, sobald sie sich verbinden.';

  @override
  String settingsSwitchOn(String what) {
    return '$what an';
  }

  @override
  String settingsSwitchOff(String what) {
    return '$what aus';
  }

  @override
  String get settingsOn => 'An';

  @override
  String get settingsOff => 'Aus';

  @override
  String get settingsIdentity => 'Deine Identität';

  @override
  String get settingsIdentityNote =>
      'Dein Name und dein Bild gehen signiert an deine Freunde und Gruppen, deine Bio an deine Freunde.';

  @override
  String get settingsPicture => 'Bild';

  @override
  String get settingsUploadPicture => 'Bild oder GIF hochladen';

  @override
  String get settingsName => 'Name';

  @override
  String get settingsBio => 'Bio';

  @override
  String get settingsBioHint => 'Ein paar Worte über dich';

  @override
  String get settingsNoBio => 'Deine Karte zeigt keine Bio.';

  @override
  String settingsBioWhere(int lines) {
    return 'Deine Freunde sehen sie auf deiner Karte, in bis zu $lines Zeilen.';
  }

  @override
  String get settingsBannerTitle => 'Hintergrund';

  @override
  String get settingsBannerNote =>
      'Der obere Teil deiner Karte, für dich und deine Freunde.';

  @override
  String get settingsBannerAnother => 'Anderes Bild';

  @override
  String get settingsBannerUpload => 'Bild von diesem Computer verwenden';

  @override
  String get settingsFingerprint => 'Fingerabdruck dieses Computers';

  @override
  String get settingsTheme => 'Design';

  @override
  String get settingsThemeNote =>
      'Die Farben des Fensters. System wechselt wie der Computer zwischen Dunkel und Hell.';

  @override
  String get settingsMicSpeaker => 'Mikrofon und Lautsprecher';

  @override
  String get settingsMicSpeakerNote =>
      'Womit Anrufe dich hören und worüber sie abgespielt werden. Beides lässt sich auch mitten im Anruf ändern.';

  @override
  String get settingsCamera => 'Kamera';

  @override
  String get settingsCameraNote =>
      'Was Anrufe von dir zeigen, wenn deine Kamera an ist. Probier es hier vor einem Anruf aus.';

  @override
  String get settingsSoundEffects => 'Soundeffekte';

  @override
  String get settingsSoundEffectsNote =>
      'Raum betreten und verlassen, Leute, die kommen und gehen, Chatnachrichten, das Mikrofon und das Teilen.';

  @override
  String get settingsSounds => 'Töne';

  @override
  String settingsChangeFailed(String reason) {
    return 'Fogata konnte das nicht ändern: $reason';
  }

  @override
  String get settingsAutostartApprove =>
      'Erlaube Fogata unter Systemeinstellungen › Allgemein › Anmeldeobjekte.';

  @override
  String get settingsAutostartOldMac => 'Dafür ist macOS 13 oder neuer nötig.';

  @override
  String get settingsAutostartUnsupported =>
      'Diese Fogata-Installation kann nicht von selbst starten.';

  @override
  String get settingsAutostartUnread =>
      'Fogata konnte diese Einstellung nicht aus deinem System lesen.';

  @override
  String get settingsAutostart => 'Beim Systemstart öffnen';

  @override
  String get settingsAutostartNote =>
      'Fogata beim Start deines Computers öffnen, sobald du dich anmeldest.';

  @override
  String get settingsKeepRunning => 'Weiterlaufen';

  @override
  String get settingsKeepRunningNote =>
      'Wenn das Fenster geschlossen ist, bleibt Fogata im Infobereich, damit Freunde dich weiter anrufen können.';

  @override
  String get settingsNoTray =>
      'Dieser Desktop zeigt keine Symbole im Infobereich, daher wird Fogata beim Schließen des Fensters beendet.';

  @override
  String get settingsHelping => 'Mitgliedern helfen';

  @override
  String get settingsHelpingNote =>
      'Wie dieser Computer den Mitgliedern deiner Gruppen hilft.';

  @override
  String get settingsHelpRelay => 'Verbinden und weiterleiten';

  @override
  String get settingsHelpConnect => 'Nur verbinden';

  @override
  String get settingsHelpRelayNote =>
      'Wenn sich zwei Mitglieder nicht direkt verbinden können, leitet dieser Computer ihren verschlüsselten Datenverkehr weiter (ohne ihn zu sehen), bis zu 2 GB pro Anruf. Das nutzt deinen Upload.';

  @override
  String get settingsHelpConnectNote =>
      'Dieser Computer hilft zwei Mitgliedern, sich zu finden, leitet aber ihren Datenverkehr nicht weiter. Wenn sie sich nicht verbinden können, hören sie sich nicht.';

  @override
  String get settingsRestartTray =>
      'Die Änderung gilt, sobald du Fogata über das Symbol im Infobereich beendest und neu öffnest.';

  @override
  String get settingsRestart =>
      'Die Änderung gilt, sobald du Fogata beendest und neu öffnest.';

  @override
  String get settingsNetworkCheck => 'Netzwerkprüfung';

  @override
  String get settingsNetworkCheckNote =>
      'Wie dieser Computer das Netzwerk sieht und was das für die Erreichbarkeit deiner Freunde bedeutet. Nichts verlässt diesen Computer: Es wird nur ausgewertet, was die Verbindung schon weiß.';

  @override
  String get settingsCheckNetwork => 'Netzwerk prüfen';

  @override
  String get settingsCheckAgain => 'Erneut prüfen';

  @override
  String get settingsDownloads => 'Downloads';

  @override
  String get settingsDownloadsNote =>
      'Wo aus einem Raum heruntergeladene Dateien gespeichert werden.';

  @override
  String settingsVersionData(String version, String path) {
    return 'Fogata $version · Daten in $path';
  }

  @override
  String get settingsNotEncrypted => 'Auf diesem Computer nicht verschlüsselt';

  @override
  String settingsNotEncryptedText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' Unter Linux ist der Schlüsselbund ein laufender Secret Service wie KWallet oder GNOME Keyring: Starte oder installiere einen, dann verschlüsselt Fogata seine Daten beim nächsten Öffnen.',
      'other': ' Fogata versucht es beim nächsten Öffnen erneut.',
    });
    return 'Deine Identität, Freunde und Direktnachrichten sind auf diesem Computer unverschlüsselt gespeichert, weil kein Systemschlüsselbund für den Schlüssel gefunden wurde.$_temp0';
  }

  @override
  String get settingsNotAllEncrypted =>
      'Auf diesem Computer nicht alles verschlüsselt';

  @override
  String get settingsNotAllEncryptedText =>
      'Ein Teil der Daten auf diesem Computer (deine Identität, Freunde und Direktnachrichten) ist noch nicht verschlüsselt: Fogata verschlüsselt ihn noch oder konnte nicht fertig werden, etwa weil der Datenträger voll ist. Es geht bei jedem Öffnen von Fogata weiter.';

  @override
  String settingsDamagedSecret(String file) {
    return '$file lässt sich mit dem Schlüssel dieses Computers nicht öffnen: Die Datei wurde auf dem Datenträger beschädigt. Solange sie da ist, kann die Verschlüsselung nicht abgeschlossen werden, und eine Kopie von vor der Verschlüsselung bleibt unverschlüsselt. Lege sie beiseite, um abzuschließen: Sie bleibt erhalten. Ist es deine Identität oder sind es deine Freunde, holt Fogata die Kopie zurück und verschlüsselt sie; die Datei einer Gruppe oder eines Chats geht auf diesem Computer verloren: Die Gruppe muss dich erneut aufnehmen, und die früheren Nachrichten des Chats werden nicht mehr angezeigt.';
  }

  @override
  String get settingsSetDamagedAside => 'Beiseitelegen und neu starten';

  @override
  String get storageTitle => 'Speicher';

  @override
  String get storageNote =>
      'Deine Direktnachrichten existieren nur auf deinen Geräten. Raumchats verschwinden, wenn der Raum geschlossen wird, daher wachsen nur diese.';

  @override
  String get storageDirect => 'Direktnachrichten';

  @override
  String get storageOfNoLimit => 'ohne Limit';

  @override
  String storageOfLimit(String limit) {
    return 'von $limit';
  }

  @override
  String storageLegendPictures(String size) {
    return 'Bilder $size';
  }

  @override
  String storageLegendVideos(String size) {
    return 'Videos $size';
  }

  @override
  String storageLegendFiles(String size) {
    return 'Dateien $size';
  }

  @override
  String storageLegendMessages(String size) {
    return 'Nachrichten $size';
  }

  @override
  String get storageRoom => 'Platz für Direktnachrichten';

  @override
  String get storageNoLimitNote => 'Nichts wird automatisch entfernt.';

  @override
  String storageOverNote(String limit) {
    return 'Über dem Limit: Die ältesten Bilder, Videos und Dateien werden entfernt, bis auf $limit. Nachrichten bleiben immer.';
  }

  @override
  String get storageLimitNote =>
      'Über dem Limit werden zuerst die ältesten Bilder, Videos und Dateien entfernt. Nachrichten bleiben immer; an ihrer Stelle steht „Hier entfernt, um Speicher freizugeben“.';

  @override
  String get storageAutoPictures => 'Bilder automatisch herunterladen';

  @override
  String get storageAutoPicturesNote =>
      'Bis zu 10 MB pro Bild, sobald sie ankommen.';

  @override
  String get storageAutoFiles => 'Videos und Dateien automatisch herunterladen';

  @override
  String get storageAutoFilesNote =>
      'Aus: Sie kommen, wenn du sie öffnest, solange die sendende Person online ist.';

  @override
  String get storageTimer => 'Verschwindende Nachrichten in neuen Chats';

  @override
  String get storageTimerNote =>
      'In jedem Chat lässt er sich ändern; beide sehen immer denselben Timer.';

  @override
  String get storageTimer24h => '24 Std.';

  @override
  String get storageByChat => 'Nach Chat';

  @override
  String get storageByChatNote =>
      'Entfernen löscht Bilder, Videos und Dateien nur hier. Die Nachrichten bleiben, und die andere Person behält ihre Kopie.';

  @override
  String get storageMessagesOnly => 'Nur Nachrichten';

  @override
  String get storageClearFiles => 'Dateien entfernen';

  @override
  String storageClearFilesTooltip(String name) {
    return 'Dateien des Chats mit $name entfernen';
  }

  @override
  String get storageCleared => 'Entfernt';

  @override
  String storageConfirmTitle(String limit) {
    return '$limit an Bildern, Videos und Dateien behalten?';
  }

  @override
  String storageConfirmText(String limit) {
    return 'Die ältesten werden jetzt entfernt, bis auf $limit, die von angepinnten Nachrichten zuletzt. Nachrichten bleiben immer, und deine Freunde behalten ihre Kopien.';
  }

  @override
  String get storageRemoveOldest => 'Älteste entfernen';

  @override
  String get themePickerSystem => 'Systemdesign, wie am Computer eingestellt';

  @override
  String themePickerTheme(String name) {
    return 'Design $name';
  }

  @override
  String get themeDark => 'Dunkel';

  @override
  String get themeMidnight => 'Mitternacht';

  @override
  String get themeDim => 'Gedämpft';

  @override
  String get themeEmber => 'Glut';

  @override
  String get themeContrast => 'Kontrast';

  @override
  String get themeLight => 'Hell';

  @override
  String get themeSystem => 'System';

  @override
  String get themeBannerViolet => 'Violett';

  @override
  String get themeBannerBlue => 'Blau';

  @override
  String get themeBannerGreen => 'Grün';

  @override
  String get themeBannerClay => 'Terrakotta';

  @override
  String get themeBannerRaspberry => 'Himbeere';

  @override
  String get themeBannerMustard => 'Senf';

  @override
  String get themeBannerSlate => 'Schiefer';

  @override
  String get themeBannerNight => 'Nacht';

  @override
  String widgetsCopyWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'QR-Code kopieren',
      'link': 'Link kopieren',
      'code': 'Code kopieren',
      'other': 'Kopieren',
    });
    return '$_temp0';
  }

  @override
  String widgetsShowWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'QR-Code anzeigen',
      'link': 'Link anzeigen',
      'code': 'Code anzeigen',
      'other': 'Anzeigen',
    });
    return '$_temp0';
  }

  @override
  String get widgetsHide => 'Verbergen';

  @override
  String revealFailed(String path) {
    return 'Der Ordner konnte nicht geöffnet werden. Die Datei liegt unter $path';
  }

  @override
  String get emojiGroupPeople => 'Personen';

  @override
  String get emojiGroupNature => 'Natur';

  @override
  String get emojiGroupFood => 'Essen & Trinken';

  @override
  String get emojiGroupActivities => 'Aktivitäten';

  @override
  String get emojiGroupTravel => 'Reisen & Orte';

  @override
  String get emojiGroupObjects => 'Objekte';

  @override
  String get emojiGroupSymbols => 'Symbole';

  @override
  String get emojiGroupFlags => 'Flaggen';

  @override
  String get emojiPickerLimit =>
      'Die Nachricht hat die maximale Länge erreicht.';

  @override
  String get emojiPickerButton => 'Emoji';

  @override
  String get emojiPickerResults => 'Ergebnisse';

  @override
  String get emojiPickerRecent => 'Zuletzt verwendet';

  @override
  String get emojiPickerSearch => 'Emoji suchen';

  @override
  String get emojiPickerSkinTone => 'Hautfarbe';

  @override
  String get emojiPickerNone => 'Keine Emojis gefunden';

  @override
  String get emojiPickerNoFont =>
      'Dieser Computer hat keine farbige Emoji-Schrift, daher sehen Emoji vielleicht schlicht oder leer aus. Die Installation von Noto Color Emoji behebt das.';

  @override
  String get emojiPickerDefaultTone => 'Standard-Hautfarbe';

  @override
  String emojiPickerTone(int n) {
    return 'Hautfarbe $n';
  }

  @override
  String get iconsGroupMountain => 'Berg';

  @override
  String get iconsGroupStar => 'Stern';

  @override
  String get iconsGroupGameController => 'Gamecontroller';

  @override
  String get iconsGroupMusic => 'Musik';

  @override
  String get iconsGroupBook => 'Buch';

  @override
  String get iconsGroupWaves => 'Wellen';

  @override
  String get iconsGroupCoffee => 'Kaffee';

  @override
  String get iconsGroupHeart => 'Herz';

  @override
  String get iconsGroupFilm => 'Film';

  @override
  String get newGroupPictureUnreadable =>
      'Dieses Bild konnte nicht gelesen werden.';

  @override
  String newGroupMade(String name) {
    return 'Gruppe „$name“ erstellt. Ihre Mitglieder bekommen sie, sobald sie online sind.';
  }

  @override
  String get newGroupTitle => 'Neue Gruppe';

  @override
  String get newGroupSubtitle => 'Die Gruppe bleibt. Ihre Räume nicht.';

  @override
  String get newGroupName => 'Gruppenname';

  @override
  String get newGroupNameExample => 'Sonntagsradeln';

  @override
  String get newGroupIconOrPicture => 'Symbol oder Bild';

  @override
  String get newGroupChooseFromComputer => 'Vom Computer wählen';

  @override
  String get newGroupBannerOptional => 'Banner (optional)';

  @override
  String get newGroupMembers => 'Mitglieder';

  @override
  String newGroupSelected(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n ausgewählt',
      one: '1 ausgewählt',
    );
    return '$_temp0';
  }

  @override
  String get newGroupSearchPlaceholder => 'Deine Freunde durchsuchen';

  @override
  String get newGroupSearchLabel => 'Freunde suchen';

  @override
  String get newGroupNoFriends =>
      'Noch keine Freunde. Du kannst die Gruppe erstellen und sie später hinzufügen oder Leute per Link einladen.';

  @override
  String get newGroupKeptOnDevices =>
      'Auf dem Gerät jedes Mitglieds gespeichert.';

  @override
  String get newGroupCreate => 'Gruppe erstellen';

  @override
  String get groupBannerSettings => 'Gruppeneinstellungen';

  @override
  String get groupBannerPictureUnreadable =>
      'Dieses Bild konnte nicht gelesen werden.';

  @override
  String get groupBannerChoose => 'Banner wählen';

  @override
  String get groupBannerAnother => 'Anderes Banner';

  @override
  String get groupBannerNone => 'Kein Banner';

  @override
  String get groupBannerNote =>
      'Wird oben in der Spalte der Gruppe angezeigt, unter ihrem Namen. PNG, JPEG oder ein animiertes GIF oder WebP bis 550 KB.';

  @override
  String get homeTitle => 'Start';

  @override
  String get homeSubtitle =>
      'Räume leuchten auf, wenn jemand hereinkommt, und erlöschen, wenn alle gehen.';

  @override
  String get homeAddFriend => 'Freund hinzufügen';

  @override
  String get homeNewGroup => 'Neue Gruppe';

  @override
  String get homeLive => 'Live';

  @override
  String get homeNoRoom => 'Kein Raum';

  @override
  String get homeNoRoomNote => 'Öffnen lässt den Raum aufleuchten';

  @override
  String homeInviteBy(String name, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Mitglieder',
      one: '1 Mitglied',
    );
    return '$name hat dich hinzugefügt · $_temp0';
  }

  @override
  String get homeInviteNote =>
      'Bis du beitrittst, verbindet sich dieser Computer weder mit ihnen noch sendet er ihnen etwas.';

  @override
  String get homeInviteJoin => 'Beitreten';

  @override
  String get homeNoGroups => 'Noch keine Gruppen';

  @override
  String get homeNoGroupsNote =>
      'Füge deine Freunde hinzu und erstelle eine Gruppe mit ihnen. Die Gruppe bleibt, ihre Räume nicht.';

  @override
  String homePersonYou(String name) {
    return '$name (du)';
  }

  @override
  String homeMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Mitglieder',
      one: '1 Mitglied',
    );
    return '$_temp0';
  }

  @override
  String get homeYouAreIn => 'Du bist drin';

  @override
  String get homeMuteMic => 'Mikrofon stummschalten';

  @override
  String get homeUnmuteMic => 'Mikrofon einschalten';

  @override
  String get homeCameraOff => 'Kamera ausschalten';

  @override
  String get homeCameraOn => 'Kamera einschalten';

  @override
  String get homeMembersTooltip => 'Mitglieder';

  @override
  String get homeReturnToRoom => 'Zurück zum Raum';

  @override
  String get homeJoinRoom => 'Betreten';

  @override
  String homeMembersOnline(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members Mitglieder',
      one: '1 Mitglied',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'sonst niemand online',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get homeNoRoomsYet => 'Noch keine Räume';

  @override
  String homeLastRoom(String when) {
    return 'Letzter Raum $when';
  }

  @override
  String get homeOpenRoom => 'Raum öffnen';

  @override
  String get membersGroupGone => 'Diese Gruppe ist nicht mehr da.';

  @override
  String membersSummary(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members Mitglieder',
      one: '1 Mitglied',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'sonst niemand online',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String membersSummaryLastRoom(int members, int online, String when) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members Mitglieder',
      one: '1 Mitglied',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'sonst niemand online',
    );
    return '$_temp0 · $_temp1 · letzter Raum $when';
  }

  @override
  String get membersCopyInviteLink => 'Einladungslink kopieren';

  @override
  String membersInviteLinkTooltip(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'jemand aus der Gruppe',
      'first': 'die erste Person der Gruppe',
      'other': 'eine der ersten $n Personen der Gruppe',
    });
    return 'Funktioniert einmal, 24 Stunden lang, und $_temp0 lässt die Person herein';
  }

  @override
  String membersInviteLinkCopied(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'jemand aus der Gruppe',
      'first': 'die erste Person der Gruppe',
      'other': 'eine der ersten $n Personen der Gruppe',
    });
    return 'Einladungslink kopiert. Er funktioniert einmal, 24 Stunden lang, und $_temp0 lässt die Person herein.';
  }

  @override
  String get membersInviteFriends => 'Freunde einladen';

  @override
  String get membersBackToRoom => 'Zurück zum Raum';

  @override
  String get membersOpenRoom => 'Raum öffnen';

  @override
  String get membersEnterRoom => 'Raum betreten';

  @override
  String get membersTitle => 'Mitglieder';

  @override
  String get membersOrderNote =>
      'In Rangfolge: Jede Person verwaltet die unter ihr';

  @override
  String get membersGroupSettings => 'Gruppeneinstellungen';

  @override
  String membersManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Nur die ersten $n können die Gruppe ändern.',
      one: 'Nur die erste Person kann die Gruppe ändern.',
    );
    return '$_temp0';
  }

  @override
  String get membersLeaveGroup => 'Gruppe verlassen';

  @override
  String membersInviteTitle(String group) {
    return 'Freunde zu „$group“ einladen';
  }

  @override
  String get membersInviteNote =>
      'Sie bekommen eine Einladung und entscheiden, ob sie beitreten.';

  @override
  String get membersPresenceYou => 'du';

  @override
  String membersPresenceOnline(String how) {
    return 'online · $how';
  }

  @override
  String membersPresenceAway(String how) {
    return 'abwesend · $how';
  }

  @override
  String get membersPresenceOffline => 'gerade nicht online';

  @override
  String membersYou(String name) {
    return '$name (du)';
  }

  @override
  String get membersWaitingForKey => 'wartet auf den Schlüssel';

  @override
  String membersFingerprint(String fingerprint) {
    return 'Fingerabdruck $fingerprint';
  }

  @override
  String membersOptionsFor(String name) {
    return 'Optionen für $name';
  }

  @override
  String get membersMoveUp => 'Direkt unter mich verschieben';

  @override
  String get membersMoveToEnd => 'Ans Ende verschieben';

  @override
  String get membersTimeOut => 'Aus einem Raum aussperren …';

  @override
  String get membersRemoveFromGroup => 'Aus der Gruppe entfernen';

  @override
  String membersRemoveTitle(String name) {
    return '$name entfernen?';
  }

  @override
  String membersRemoveText(String name) {
    return '$name verlässt die Gruppe und die Gruppe erstellt einen neuen Schlüssel. Bereits Gesehenes bleibt der Person erhalten.';
  }

  @override
  String get membersFirst => 'Erste';

  @override
  String membersTimeOutTitle(String name) {
    return '$name aussperren';
  }

  @override
  String membersTimeOutText(String name) {
    return '$name verlässt den Raum und kann nicht zurückkommen, bis die Zeit um ist oder jemand mit höherem Rang die Person wieder hereinlässt.';
  }

  @override
  String get membersUntilLetBack => 'Bis auf Weiteres';

  @override
  String membersLeaveTitle(String group) {
    return '„$group“ verlassen?';
  }

  @override
  String get membersLeaveText =>
      'Du siehst ihre Räume nicht mehr und die anderen sehen dich nicht mehr in der Gruppe. Die Gruppe erstellt einen neuen Schlüssel ohne dich. Um zurückzukommen, muss dich jemand aus der Gruppe wieder hereinlassen.';

  @override
  String membersLeaveTextFirst(String name) {
    return 'Du siehst ihre Räume nicht mehr und die anderen sehen dich nicht mehr in der Gruppe. Die Gruppe erstellt einen neuen Schlüssel ohne dich. Um zurückzukommen, muss dich jemand aus der Gruppe wieder hereinlassen. $name rückt an die erste Stelle.';
  }

  @override
  String get groupSettingsTabGeneral => 'Allgemein';

  @override
  String get groupSettingsTabVoiceRooms => 'Sprachräume';

  @override
  String get groupSettingsTabMembers => 'Mitglieder';

  @override
  String get groupSettingsTabInvite => 'Einladen';

  @override
  String get groupSettingsTabLimits => 'Limits';

  @override
  String get groupSettingsTabActivity => 'Aktivität';

  @override
  String get groupSettingsFirstHere => 'Erste Person in dieser Gruppe';

  @override
  String groupSettingsPlaceHere(String place) {
    return '$place Platz in dieser Gruppe';
  }

  @override
  String get groupSettingsLeave => 'Gruppe verlassen';

  @override
  String get groupSettingsInviteTitle => 'Leute einladen';

  @override
  String get groupSettingsClose => 'Gruppeneinstellungen schließen';

  @override
  String get groupSettingsPicture => 'Bild';

  @override
  String get groupSettingsChooseFromComputer => 'Vom Computer wählen';

  @override
  String get groupSettingsUseIcon => 'Symbol verwenden';

  @override
  String get groupSettingsPictureNote =>
      'PNG, JPEG oder WebP, quadratisch zugeschnitten und verkleinert wie Profilbilder, oder ein animiertes GIF oder WebP bis 550 KB.';

  @override
  String get groupSettingsOrIcon => 'Oder ein Symbol';

  @override
  String get groupSettingsBanner => 'Banner';

  @override
  String get groupSettingsName => 'Name';

  @override
  String get groupSettingsNameField => 'Gruppenname';

  @override
  String get groupSettingsNameNote =>
      'Alle in der Gruppe sehen den neuen Namen und das neue Bild.';

  @override
  String get groupSettingsRoomsIntro =>
      'Jeder Sprachraum hat eigene Leute, Chat, Dateien und Anrufe: Wer in einem ist, verbindet sich nicht mit denen in einem anderen. Jede Person ist immer nur in einem Raum.';

  @override
  String get groupSettingsRoomName => 'Raumname';

  @override
  String get groupSettingsAccessEveryone => 'Alle';

  @override
  String get groupSettingsAccessSome => 'Nur einige';

  @override
  String groupSettingsWhoCanEnter(String room) {
    return 'Wer $room betreten darf';
  }

  @override
  String get groupSettingsFirstRoomStays => 'Der erste Raum bleibt';

  @override
  String get groupSettingsPeopleInside => 'Es sind Leute drin';

  @override
  String get groupSettingsDeleteRoom => 'Raum löschen';

  @override
  String get groupSettingsAddRoom => 'Sprachraum hinzufügen';

  @override
  String groupSettingsNewRoomName(int n) {
    return 'Raum $n';
  }

  @override
  String groupSettingsRoomsNote(int rooms, int people) {
    return 'Bis zu $rooms Räume mit je $people Personen (Limits). Ein Raum, in dem Leute sind, kann nicht gelöscht werden.';
  }

  @override
  String get groupSettingsKeptOut => 'Ausgesperrt';

  @override
  String get groupSettingsAccessEveryoneLong => 'Alle in der Gruppe';

  @override
  String get groupSettingsAccessChosen => 'Nur die Mitglieder, die ich wähle';

  @override
  String get groupSettingsAccessNote =>
      'Du und alle über dir können immer hinein. Wer ausgeschlossen ist, sieht den Raum mit einem Schloss.';

  @override
  String groupSettingsYou(String name) {
    return '$name (du)';
  }

  @override
  String groupSettingsKeptOutBy(String room, String by) {
    return ' · $room · von $by';
  }

  @override
  String groupSettingsTimeLeft(String time) {
    return 'noch $time';
  }

  @override
  String get groupSettingsUntilLetBack => 'Bis auf Weiteres';

  @override
  String get groupSettingsLetBack => 'Freigeben';

  @override
  String groupSettingsManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Nur die ersten $n können die Gruppe ändern: Name, Räume, Einladungen und Limits. Du verwaltest die Personen unter dir.',
      one: 'Nur die erste Person kann die Gruppe ändern: Name, Räume, Einladungen und Limits. Du verwaltest die Personen unter dir.',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsMembersCount(int n, int max) {
    String _temp0 = intl.Intl.pluralLogic(
      max,
      locale: localeName,
      other: '$max Mitgliedern',
      one: '1 Mitglied',
    );
    return '$n von $_temp0, in Rangfolge';
  }

  @override
  String get groupSettingsMembersNote =>
      'Alle verwalten die Personen unter sich: direkt unter sich hoch- oder ans Ende verschieben, aus einem Raum aussperren oder entfernen. Wer neu hinzukommt, beginnt am Ende.';

  @override
  String get groupSettingsWhoManages => 'Wer diese Gruppe verwaltet';

  @override
  String get groupSettingsManagersEveryone => 'Alle';

  @override
  String groupSettingsManagersFirstN(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Die ersten $n',
      one: 'Die erste Person',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsManagersMore(int n, int members) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members Personen',
      one: '1 Person',
    );
    return 'Auf die ersten $n gesetzt, mehr als die $_temp0 jetzt in der Gruppe: Vorerst sind das alle.';
  }

  @override
  String get groupSettingsManagersEveryoneNote =>
      'Wer verwaltet, ändert Name, Bilder, Räume und Limits und lässt Leute herein. Wenn alle verwalten, besteht die Gruppe weiter, egal wer sein Gerät verliert.';

  @override
  String groupSettingsManagersFirstNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Nur die ersten $n ändern Name, Bilder, Räume und Limits und lassen Leute herein. Verlieren sie alle ihre Geräte, kann niemand mehr die Gruppe verwalten: Fogata hat keinen Server, der das beheben könnte.',
      one: 'Nur die erste Person ändert Name, Bilder, Räume und Limits und lässt Leute herein. Geht dieses Gerät verloren, kann niemand mehr die Gruppe verwalten: Fogata hat keinen Server, der das beheben könnte.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsMoveUp => 'Hochstufen';

  @override
  String groupSettingsMoveUpTooltip(String name) {
    return '$name direkt unter dich verschieben';
  }

  @override
  String get groupSettingsToEnd => 'Ans Ende';

  @override
  String groupSettingsToEndTooltip(String name) {
    return '$name ans Ende verschieben';
  }

  @override
  String get groupSettingsTimeOut => 'Aus einem Raum aussperren';

  @override
  String get groupSettingsRemove => 'Aus der Gruppe entfernen';

  @override
  String get groupSettingsAskingToJoin => 'Beitrittsanfragen';

  @override
  String get groupSettingsInviteFriend => 'Freund einladen';

  @override
  String get groupSettingsInviteFriendNote =>
      'Sie bekommen in Fogata eine Einladung und entscheiden, ob sie beitreten. Hier erscheinen nur deine Freunde.';

  @override
  String get groupSettingsInviteLink => 'Einladungslink';

  @override
  String groupSettingsInviteLinkNote(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'jemand aus der Gruppe',
      'first': 'die erste Person der Gruppe',
      'other': 'eine der ersten $n Personen der Gruppe',
    });
    return 'Für jemanden, der noch nicht mit dir befreundet ist. Der Link funktioniert einmal, 24 Stunden lang, und $_temp0 lässt die Person herein: Gelangt er an jemand anderen, siehst du, wer fragt, und kannst ablehnen.';
  }

  @override
  String get groupSettingsCopyLink => 'Einladungslink kopieren';

  @override
  String groupSettingsOpenLinks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Links noch nicht benutzt.',
      one: '1 Link noch nicht benutzt.',
      zero: 'Keine ungenutzten Links.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsCancelLinks => 'Widerrufen';

  @override
  String get groupSettingsNoFriends =>
      'Noch keine Freunde: Füge sie unter Freunde hinzu.';

  @override
  String get groupSettingsAllFriendsIn =>
      'Alle deine Freunde sind in der Gruppe.';

  @override
  String get groupSettingsFriendOnline => 'Online';

  @override
  String get groupSettingsFriendOffline => 'Gerade nicht online';

  @override
  String get groupSettingsInviteButton => 'Einladen';

  @override
  String get groupSettingsGroupFull => 'Die Gruppe ist voll';

  @override
  String get groupSettingsSayNo => 'Ablehnen';

  @override
  String get groupSettingsLetIn => 'Hereinlassen';

  @override
  String get groupSettingsLimitGroup => 'Personen in der Gruppe';

  @override
  String groupSettingsLimitGroupNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Personen',
      one: '1 Person',
    );
    return 'Von 2 bis 20. Ist die Gruppe voll, funktionieren Einladungen und Links nicht mehr. Der Wert kann nicht unter die $_temp0 sinken, die schon drin sind.';
  }

  @override
  String get groupSettingsLimitRoom => 'Personen pro Sprachraum';

  @override
  String get groupSettingsLimitRoomNote =>
      'Von 2 bis 8. Sprache und Video gehen direkt zwischen allen im Raum hin und her, daher klingt es mit höchstens 8 gut; mit eingeschalteten Kameras sind weniger besser. Wer kommt, wenn der Raum voll ist, sieht, dass er voll ist.';

  @override
  String get shellChipCallMany => 'Direkt · verschlüsselt · nur die im Anruf';

  @override
  String get shellChipCallTwo => 'Direkt · verschlüsselt · nur ihr zwei';

  @override
  String get shellGroupMembers => 'Gruppenmitglieder';

  @override
  String get shellMembers => 'Mitglieder';

  @override
  String get shellDirectMessages => 'Direktnachrichten';

  @override
  String get shellPathDirect => 'Direkt · verschlüsselt';

  @override
  String shellPathVia(String name) {
    return 'Über $name · verschlüsselt';
  }

  @override
  String get shellPathViaSomeone => 'Über ein Mitglied · verschlüsselt';

  @override
  String shellCallWith(String names) {
    return 'Anruf mit $names';
  }

  @override
  String get shellRinging => 'Klingelt · ';

  @override
  String get shellInCall => 'Im Anruf · ';

  @override
  String get shellCallEnded => 'Anruf beendet';

  @override
  String get shellNewGroup => 'Neue Gruppe';

  @override
  String get shellSectionCall => 'Anruf';

  @override
  String shellCallRinging(String name) {
    return '$name · klingelt';
  }

  @override
  String get shellSectionLiveNow => 'Gerade live';

  @override
  String get shellSectionInCall => 'Im Anruf';

  @override
  String get shellChat => 'Chat';

  @override
  String get shellFiles => 'Dateien';

  @override
  String get shellSectionGroup => 'Gruppe';

  @override
  String get shellInvite => 'Einladen';

  @override
  String get shellSectionVoiceRooms => 'Sprachräume';

  @override
  String shellVoiceRoomYouAreIn(String room) {
    return '$room · hier';
  }

  @override
  String get shellNotForYou => 'Gerade gesperrt';

  @override
  String get shellOnlySomeMembers => 'Nur einige Mitglieder';

  @override
  String shellPersonYou(String name) {
    return '$name (du)';
  }

  @override
  String get shellLive => 'LIVE';

  @override
  String get shellYourProfile => 'Dein Profil';

  @override
  String shellMuteMic(String keys) {
    return 'Mikrofon stummschalten ($keys)';
  }

  @override
  String shellUnmuteMic(String keys) {
    return 'Mikrofon einschalten ($keys)';
  }

  @override
  String get shellDeafen => 'Die anderen nicht mehr hören';

  @override
  String get shellUndeafen => 'Die anderen wieder hören';

  @override
  String get shellStateCalling => 'Ruft an …';

  @override
  String get shellStateInCall => 'Im Anruf';

  @override
  String get shellStateInRoom => 'Im Raum';

  @override
  String get shellStateOffline => 'Offline';

  @override
  String get shellStateOnline => 'Online';

  @override
  String get shellStateAway => 'Abwesend';

  @override
  String get shellShowInFolder => 'Im Ordner anzeigen';

  @override
  String get shellCopyPath => 'Pfad kopieren';

  @override
  String shellVolumePercent(int n) {
    return '$n %';
  }

  @override
  String get shellMuteKeys => 'Strg+Umschalt+M';

  @override
  String get settingsSectionDonate => 'An Fogata spenden';

  @override
  String get settingsSectionReport => 'Problem melden';

  @override
  String get settingsHeadingHelp => 'Hilfe';

  @override
  String get settingsPreRelease => 'Vorabversion';

  @override
  String get donateTitle => 'Halte Fogata unabhängig';

  @override
  String get donateText =>
      'Fogata ist kostenlos, hat keine Werbung und keine Server, die dich beobachten. Deine Spende sorgt dafür, dass es so bleibt.';

  @override
  String get donateButton => 'Spenden';

  @override
  String get donateNote =>
      'Öffnet Ko-fi in deinem Browser. Fogata sieht deine Zahlung nie.';

  @override
  String get reportWhatTitle => 'Was ist passiert?';

  @override
  String get reportWhatNote =>
      'Was du gemacht hast, was du erwartet hast und was stattdessen passiert ist.';

  @override
  String get reportIncludeLog =>
      'Das technische Protokoll der letzten 2 Tage mitsenden';

  @override
  String get reportLogNote =>
      'Versionen, Fehler und Verbindungsschritte. Nie deine Nachrichten, Schlüssel, Kontakte oder IP-Adressen.';

  @override
  String get reportSeeSent => 'Ansehen, was gesendet wird';

  @override
  String get reportHideSent => 'Liste ausblenden';

  @override
  String get reportPreview => 'Den ganzen Bericht vor dem Senden lesen';

  @override
  String get reportHidePreview => 'Bericht ausblenden';

  @override
  String get reportPreviewNote =>
      'Genau das steht in der Datei, und nur das wird gesendet. Namen, Ordner, Dateinamen, Adressen und Schlüssel sind schon aus dem Protokoll entfernt.';

  @override
  String get reportPreviewWorking => 'Bericht wird vorbereitet …';

  @override
  String get reportSentWhat => 'Was du oben geschrieben hast.';

  @override
  String get reportSentVersion =>
      'Version und Build von Fogata, dein System und seine Version sowie die Sprache von Fogata.';

  @override
  String get reportSentLog =>
      'Das Protokoll der letzten 2 Tage: wann Fogata gestartet ist, seine Fehler und wie es andere Geräte erreicht hat. Namen, Nachrichten, Ordner, Dateinamen, Links, Codes, Schlüssel und IP-Adressen werden entfernt, und jedes Gerät erscheint nur als Nummer, die außerhalb des Berichts nichts bedeutet.';

  @override
  String get reportEmail => 'Per E-Mail ans Fogata-Team senden';

  @override
  String get reportSave => 'In einer Datei speichern';

  @override
  String reportHowEmail(String address) {
    return 'Deine E-Mail-App öffnet sich mit der fertigen Nachricht an $address, daneben der Ordner mit der Berichtsdatei: Hänge die Datei an und sende sie. Nichts verlässt deinen Computer, bevor du das tust.';
  }

  @override
  String get reportReadyTitle => 'Deine E-Mail ist bereit';

  @override
  String reportReadyText(String file, String address) {
    return 'Hänge $file aus dem geöffneten Ordner an und sende die E-Mail an $address.';
  }

  @override
  String get reportShowFile => 'Datei anzeigen';

  @override
  String get reportCopyAddress => 'Adresse kopieren';

  @override
  String get reportAddressCopied => 'Adresse kopiert';

  @override
  String reportSaved(String file) {
    return 'Bericht gespeichert: $file';
  }

  @override
  String reportNoMailApp(String address) {
    return 'Es hat sich keine E-Mail-App geöffnet. Sende die Datei aus deinem E-Mail-Programm an $address.';
  }

  @override
  String reportFailed(String reason) {
    return 'Der Bericht konnte nicht geschrieben werden: $reason';
  }

  @override
  String get splashStarting => 'Wird gestartet';

  @override
  String get splashConnecting => 'Verbindungsaufbau';

  @override
  String get splashChecking => 'Suche nach Updates';

  @override
  String get splashDownloading => 'Update wird geladen';

  @override
  String get splashInstalling => 'Update wird installiert';

  @override
  String get splashRestarting => 'Neustart';

  @override
  String splashProgress(String done, String total) {
    return '$done von $total MB';
  }

  @override
  String splashProgressFrom(String done, String total, String name) {
    return '$done von $total MB · von $name';
  }

  @override
  String updateReady(String version) {
    return 'Update bereit · $version';
  }

  @override
  String updateRequired(String version) {
    return 'Update nötig · $version';
  }

  @override
  String get updateRestart => 'Neu starten';

  @override
  String updateReadyTip(String version) {
    return 'Fogata $version ist da. Starte neu, um es zu verwenden.';
  }

  @override
  String get updateRequiredTip =>
      'Diese Version funktioniert nicht mehr mit der neueren deiner Freunde. Starte neu, um zu aktualisieren.';

  @override
  String updateFailed(String reason) {
    return 'Das Update konnte nicht installiert werden: $reason';
  }

  @override
  String get errJoinInviteGone =>
      'Diese Einladung funktioniert nicht mehr, oder die Gruppe ist voll.';

  @override
  String get errJoinNoKeyYet =>
      'Dieses Gerät hat den Schlüssel der Gruppe noch nicht. Das neue Mitglied kommt herein, sobald es ihn hat.';

  @override
  String get errJoinWroteTooMuch =>
      'Dieses Gerät hat Änderungen gemacht, die die anderen Mitglieder noch nicht übernommen haben. Das neue Mitglied kann herein, sobald ein anderes Mitglied online ist.';

  @override
  String get errJoinAskAgain =>
      'Diese Beitrittsanfrage bezieht sich auf einen früheren Stand der Gruppe. Die Person muss erneut anfragen.';

  @override
  String get errNoSpaceForFile =>
      'Für diese Datei ist nicht genug Speicherplatz frei. Gib etwas Speicherplatz frei und versuch es erneut.';

  @override
  String get recoveryTitle => 'Wiederherstellungskopie';

  @override
  String get recoveryNote =>
      'Eine Datei, die dieses Konto zurückbringt, wenn der Schlüsselbund oder der Datenträger dieses Computers verloren geht. Nur dein Wiederherstellungsschlüssel öffnet sie.';

  @override
  String get recoveryOff => 'Aus';

  @override
  String recoveryOn(String id) {
    return 'An · Schlüssel $id';
  }

  @override
  String get recoveryTurnOn => 'Einschalten …';

  @override
  String get recoveryChangeKey => 'Schlüssel ändern …';

  @override
  String get recoveryMakeCopy => 'Kopie erstellen …';

  @override
  String get recoveryReplacedKept =>
      'Auf diesem Computer liegt noch ein Konto, das eine Wiederherstellung ersetzt hat.';

  @override
  String get recoveryDeleteReplaced => 'Löschen …';

  @override
  String get recoveryChangeTitle =>
      'Neuen Wiederherstellungsschlüssel erstellen?';

  @override
  String get recoveryChangeText =>
      'Neue Kopien öffnen sich nur mit dem neuen Schlüssel. Frühere Kopien öffnen sich weiterhin nur mit dem alten: Bewahre ihn auf, solange du diese Kopien behältst.';

  @override
  String get recoveryChangeConfirm => 'Neuen Schlüssel erstellen';

  @override
  String get recoveryKeyTitle => 'Dein Wiederherstellungsschlüssel';

  @override
  String get recoveryKeyText =>
      'Schreib ihn auf oder druck ihn aus und bewahre ihn getrennt von deinen Kopien auf. Er wird nur dieses eine Mal gezeigt.';

  @override
  String recoveryKeyName(String id) {
    return 'Sein Kurzname: $id';
  }

  @override
  String get recoveryKeyWarning =>
      'Fogata speichert diesen Schlüssel nirgends: Ohne ihn öffnet sich keine Kopie. Wer eine Kopie und diesen Schlüssel hat, hat dein Konto.';

  @override
  String get recoveryKeyWritten => 'Ich habe ihn notiert';

  @override
  String get recoveryCheckTitle => 'Prüfe, was du notiert hast';

  @override
  String get recoveryCheckText =>
      'Gib diese zwei Gruppen des Schlüssels so ein, wie du sie notiert hast.';

  @override
  String recoveryCheckGroup(int n) {
    return 'Gruppe $n';
  }

  @override
  String get recoveryCheckWrong =>
      'Das stimmt nicht: Sieh noch einmal nach, was du notiert hast.';

  @override
  String get recoveryTurnOnButton => 'Kopien einschalten';

  @override
  String recoveryTurnedOn(String id) {
    return 'Kopien sind an (Schlüssel $id).';
  }

  @override
  String get recoveryCopyTitle => 'Wiederherstellungskopie erstellen';

  @override
  String get recoveryCopyText =>
      'Fogata ist offline, solange die Kopie entsteht, länger mit den Dateien aus Chats. Bewahre die Datei nicht auf diesem Computer auf, sondern zum Beispiel auf einem USB-Stick.';

  @override
  String get recoveryCopyAttachments =>
      'In Chats gesendete Dateien einschließen';

  @override
  String recoveryCopyAttachmentsNote(String size) {
    return 'Bilder, Videos und Dateien: $size mehr.';
  }

  @override
  String get recoveryCopySave => 'Speicherort wählen …';

  @override
  String get recoveryCopyInDataFolder =>
      'Eine Kopie kann nicht im Datenordner von Fogata liegen. Wähle einen anderen Ort.';

  @override
  String recoveryCopyMade(String file, String size) {
    return 'Kopie gespeichert: $file ($size).';
  }

  @override
  String recoveryCopyFailed(String why) {
    return 'Die Kopie konnte nicht erstellt werden. $why';
  }

  @override
  String get recoveryDeleteTitle => 'Ersetztes Konto löschen?';

  @override
  String get recoveryDeleteText =>
      'Es wurde beiseitegelegt, als an seiner Stelle eine Kopie wiederhergestellt wurde. Einmal gelöscht, lässt es sich nicht zurückholen.';

  @override
  String get recoveryDelete => 'Löschen';

  @override
  String get recoveryDeleted => 'Das ersetzte Konto wurde gelöscht.';

  @override
  String get recoveryFileType => 'Fogata-Kopie';

  @override
  String get recoveryLostComputer => 'Computer verloren?';

  @override
  String get recoveryRestoreLink => 'Aus einer Kopie wiederherstellen';

  @override
  String get recoveryRestoreTitle => 'Aus einer Kopie wiederherstellen';

  @override
  String get recoveryRestoreSub =>
      'Wähle die Wiederherstellungskopie, die du gespeichert hast, und gib dann deinen Wiederherstellungsschlüssel ein.';

  @override
  String get recoveryChooseCopy => 'Kopie wählen …';

  @override
  String recoveryMadeOn(String date) {
    return 'Eine Kopie vom $date.';
  }

  @override
  String get recoveryOtherKey =>
      'Sie wurde für einen anderen Wiederherstellungsschlüssel erstellt als den dieses Computers. Gib den Schlüssel ein, für den sie erstellt wurde.';

  @override
  String get recoveryKeyField => 'Wiederherstellungsschlüssel';

  @override
  String get recoveryKeyNotAKey =>
      'Ein Wiederherstellungsschlüssel hat 53 Buchstaben und Ziffern in Vierergruppen.';

  @override
  String get recoveryKeyMistyped =>
      'Ein Buchstabe oder eine Ziffer stimmt nicht: Prüfe sie.';

  @override
  String get recoveryProblemNotACopy =>
      'Das ist keine Kopie eines Fogata-Kontos.';

  @override
  String get recoveryProblemNewer =>
      'Diese Kopie wurde mit einer neueren Version von Fogata erstellt. Aktualisiere Fogata, um sie zu öffnen.';

  @override
  String get recoveryProblemOtherKey =>
      'Diese Kopie wurde für einen anderen Wiederherstellungsschlüssel erstellt.';

  @override
  String get recoveryProblemDamaged =>
      'Diese Kopie ist beschädigt oder wurde nach dem Erstellen verändert.';

  @override
  String get recoveryProblemUnproven =>
      'Diese Kopie wurde nicht von dem Konto erstellt, das sie enthält, darum wird sie nicht geöffnet.';

  @override
  String recoveryWhose(String name) {
    return 'Konto von $name';
  }

  @override
  String recoveryHolds(int files, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files Dateien',
      one: '1 Datei',
    );
    return '$_temp0, insgesamt $size';
  }

  @override
  String get recoveryWithAttachments => 'Mit den in Chats gesendeten Dateien.';

  @override
  String get recoveryWithoutAttachments =>
      'Ohne die in Chats gesendeten Dateien.';

  @override
  String get recoveryOnlyIf =>
      'Nutze eine Kopie nur, wenn kein anderer Computer dieses Konto mehr nutzt. Nutzt es noch einer, zieh das Konto lieber von dort um: Ein Konto kann nicht auf zwei Computern sein.';

  @override
  String get recoveryMoveInstead => 'Lieber umziehen';

  @override
  String recoveryHereNamed(String name) {
    return 'Auf diesem Computer ist das Konto von $name.';
  }

  @override
  String recoveryHereDevice(String fingerprint) {
    return 'Auf diesem Computer ist ein Konto ohne Profil (Gerät $fingerprint).';
  }

  @override
  String get recoveryHereUnknown =>
      'Auf diesem Computer ist ein Teil eines Kontos, der sich nicht öffnen lässt.';

  @override
  String get recoveryHereReplaced =>
      'Die Wiederherstellung ersetzt es. Es wird beiseitegelegt, nicht gelöscht, bis du es unter Einstellungen › Speicher & Downloads löschst.';

  @override
  String get recoveryRestore => 'Wiederherstellen';

  @override
  String get recoveryReplace => 'Ersetzen und wiederherstellen';

  @override
  String recoveryRestoring(String name) {
    return 'Konto von $name wird wiederhergestellt …';
  }

  @override
  String recoveryRestored(String name) {
    return 'Das Konto von $name ist wiederhergestellt.';
  }

  @override
  String get recoveryRefusedAccountHere =>
      'Auf diesem Computer ist jetzt ein Konto. Wähle die Kopie noch einmal, um zu sehen, was sie ersetzt.';

  @override
  String get recoveryRefusedReplacedKept =>
      'Auf diesem Computer liegt noch ein Konto, das eine frühere Wiederherstellung ersetzt hat. Lösche es, um diese Kopie wiederherzustellen.';

  @override
  String recoveryRefusedNoRoom(String need, String free) {
    return 'Die Kopie braucht $need auf diesem Datenträger, und $free sind frei. Nichts wurde geändert.';
  }

  @override
  String get recoveryRefusedInFolder =>
      'Die Kopie liegt im Datenordner von Fogata. Verschiebe sie an einen anderen Ort und wähle sie dann noch einmal.';

  @override
  String get recoveryBusy =>
      'Fogata ist mit einer Wiederherstellungskopie oder einem Umzug beschäftigt. Versuch es danach noch einmal.';

  @override
  String get moveBusyWithCopy =>
      'Fogata ist mit einer Wiederherstellungskopie beschäftigt. Zieh das Konto danach um.';

  @override
  String get recoveryDeleteKeptAside => 'Beiseitegelegtes Konto löschen …';

  @override
  String get reachOffline =>
      'Fogata erreicht von hier aus niemanden im Internet. Prüfe die Verbindung; vielleicht blockiert auch eine Firewall Fogata.';

  @override
  String get reachFirewall =>
      'Die Firewall dieses Computers lässt nichts für Fogata herein. Den Befehl, der Fogata hereinlässt, findest du in den Benachrichtigungen.';

  @override
  String get reachUdpBlocked =>
      'Dieses Netzwerk oder dieser Computer blockiert UDP. Anrufe und direkte Verbindungen durch Router brauchen es; bis es erlaubt ist, erreichen andere dich nur über ein Gruppenmitglied oder von einer öffentlichen Adresse aus.';

  @override
  String get reachSymmetricNat =>
      'Dein Router gibt jedem Ziel einen anderen öffentlichen Port (ein symmetrisches NAT), daher lässt sich durch ihn kein direkter Weg öffnen. Freunde hinter einem anderen Router erreichen dich nur über ein Mitglied einer gemeinsamen Gruppe.';

  @override
  String get reachNoIpv6 =>
      'Es gibt auch kein funktionierendes IPv6, mit dem es am Router vorbeiginge.';

  @override
  String reachFineFriend(String name) {
    return 'Dein Netzwerk scheint für Fogata in Ordnung zu sein, also liegt das, was euch trennt, wahrscheinlich auf der Seite von $name (offline oder ein Netzwerkproblem).';
  }

  @override
  String get reachFineOthers =>
      'Dein Netzwerk scheint für Fogata in Ordnung zu sein, also liegt das, was euch trennt, wahrscheinlich auf der anderen Seite.';
}
