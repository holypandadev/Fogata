// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for French (`fr`).
class L10nFr extends L10n {
  L10nFr([String locale = 'fr']) : super(locale);

  @override
  String get commonCancel => 'Annuler';

  @override
  String get commonSave => 'Enregistrer';

  @override
  String get commonClose => 'Fermer';

  @override
  String commonNewMessageFrom(String name, String text) {
    return 'Nouveau message de $name : $text';
  }

  @override
  String commonNewMessagesFrom(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n nouveaux messages de $name',
      one: '$n nouveau message de $name',
    );
    return '$_temp0';
  }

  @override
  String commonNewMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n nouveaux messages',
      one: '$n nouveau message',
    );
    return '$_temp0';
  }

  @override
  String get commonAccept => 'Accepter';

  @override
  String get commonDecline => 'Refuser';

  @override
  String get commonSend => 'Envoyer';

  @override
  String get commonTryAgain => 'Réessayer';

  @override
  String get commonCopy => 'Copier';

  @override
  String get commonCopied => 'Copié';

  @override
  String get commonBack => 'Retour';

  @override
  String get commonDiscard => 'Abandonner';

  @override
  String get commonKeepEditing => 'Continuer à modifier';

  @override
  String get commonNotNow => 'Plus tard';

  @override
  String get commonDownload => 'Télécharger';

  @override
  String get commonSettings => 'Paramètres';

  @override
  String get commonHome => 'Accueil';

  @override
  String get commonFriends => 'Amis';

  @override
  String get commonContinue => 'Continuer';

  @override
  String get commonDone => 'Terminé';

  @override
  String get commonRemove => 'Retirer';

  @override
  String get commonYou => 'Toi';

  @override
  String timeYesterdayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'hier à $time',
      'other': 'hier à $time',
    });
    return '$_temp0';
  }

  @override
  String timeDayAt(String date, String time) {
    return '$date, $time';
  }

  @override
  String get timeNow => 'maintenant';

  @override
  String timeMinutes(int n) {
    return '$n min';
  }

  @override
  String timeHours(int n) {
    return '$n h';
  }

  @override
  String timeHoursMinutes(int hours, int minutes) {
    return '$hours h $minutes min';
  }

  @override
  String timeDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n jours',
      one: '$n jour',
    );
    return '$_temp0';
  }

  @override
  String spanMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minutes',
      one: '$n minute',
    );
    return '$_temp0';
  }

  @override
  String spanHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n heures',
      one: '$n heure',
    );
    return '$_temp0';
  }

  @override
  String spanDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n jours',
      one: '$n jour',
    );
    return '$_temp0';
  }

  @override
  String spanWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n semaines',
      one: '$n semaine',
    );
    return '$_temp0';
  }

  @override
  String get agoMoment => 'à l’instant';

  @override
  String agoMinutes(int n) {
    return 'il y a $n min';
  }

  @override
  String agoHours(int n) {
    return 'il y a $n h';
  }

  @override
  String get agoYesterday => 'hier';

  @override
  String agoDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'il y a $n jours',
    );
    return '$_temp0';
  }

  @override
  String agoWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'il y a $n semaines',
      one: 'il y a $n semaine',
    );
    return '$_temp0';
  }

  @override
  String agoMonths(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'il y a $n mois',
    );
    return '$_temp0';
  }

  @override
  String get agoOverYear => 'il y a plus d’un an';

  @override
  String sizeBytes(int n) {
    return '$n o';
  }

  @override
  String sizeKB(String n) {
    return '$n Ko';
  }

  @override
  String sizeMB(String n) {
    return '$n Mo';
  }

  @override
  String sizeGB(String n) {
    return '$n Go';
  }

  @override
  String sizeTB(String n) {
    return '$n To';
  }

  @override
  String listTwo(String first, String second) {
    return '$first et $second';
  }

  @override
  String listMore(String rest, String last) {
    return '$rest et $last';
  }

  @override
  String get listSeparator => ', ';

  @override
  String ordinal(int n) {
    return 'rang $n';
  }

  @override
  String get errLinkExpired => 'Le lien a expiré. Demande-en un nouveau.';

  @override
  String get errGroupInvite => 'C’est une invitation à un groupe.';

  @override
  String get errDeviceNotFound =>
      'L’autre appareil est introuvable. Vérifie que vous êtes tous les deux en ligne.';

  @override
  String get errNotFriendCode =>
      'Ça ne ressemble pas à un lien ou un code Fogata.';

  @override
  String get errAlreadyInGroup => 'Tu es déjà dans ce groupe.';

  @override
  String get errJoinRoomFirst => 'Entre d’abord dans le salon.';

  @override
  String get errNewerVersion =>
      'Ton profil a été enregistré par une version plus récente de Fogata. Mets Fogata à jour pour l’ouvrir.';

  @override
  String get errKeychainUnreachable =>
      'Fogata n’a pas pu obtenir la clé de ton profil depuis le trousseau de cet ordinateur. Déverrouille le trousseau ou autorise Fogata à l’utiliser, puis réessaie.';

  @override
  String get errKeychainNoKey =>
      'Le trousseau de cet ordinateur ne contient plus la clé de ton profil : Fogata ne peut pas l’ouvrir.';

  @override
  String get errKeychainWrongKey =>
      'La clé du trousseau de cet ordinateur n’ouvre pas ton profil.';

  @override
  String get errNoGroupKey =>
      'Ce groupe n’est pas encore prêt sur cet ordinateur. Réessaie dans un instant.';

  @override
  String get errAlreadyRunning =>
      'Fogata est déjà ouvert avec ce profil. Ferme d’abord l’autre.';

  @override
  String get errFileGone => 'Ce fichier n’est plus dans le salon.';

  @override
  String get errNobodyHasFile =>
      'Personne en ligne n’a ce fichier pour le moment.';

  @override
  String get errStillStarting => 'Fogata est encore en train de démarrer.';

  @override
  String get errIdentityExists =>
      'Cet ordinateur a déjà une identité ; sa clé ne peut pas changer.';

  @override
  String get errNameRule => 'Écris un nom de 1 à 40 caractères visibles.';

  @override
  String get errBioRule =>
      'Une bio fait au plus 190 caractères sur 4 lignes, sans caractères cachés.';

  @override
  String get errPictureRule =>
      'L’image doit être un PNG ou un JPEG de 64 Ko au plus.';

  @override
  String get errNotFriendLink => 'Ce n’est pas un lien d’ami Fogata.';

  @override
  String get errLinkDamaged => 'Le lien est incomplet ou abîmé.';

  @override
  String get errLinkNeedsNewer =>
      'Ce lien demande une version plus récente de Fogata.';

  @override
  String get errCodeShape => 'Un code a cette forme : FGT-XXXXX-XXXXX.';

  @override
  String get errCodeWrong => 'Le code est incorrect ou ne fonctionne plus.';

  @override
  String get errNotProven =>
      'L’autre appareil n’a pas pu prouver son identité.';

  @override
  String get errCallGone => 'Cet appel ne sonne plus.';

  @override
  String get errAlreadyInCall => 'Tu es déjà en appel. Raccroche d’abord.';

  @override
  String get errAlreadyFriends => 'Vous êtes déjà amis.';

  @override
  String get errMemberNotConnected =>
      'Ce membre n’est pas connecté pour le moment. Réessaie quand il sera en ligne.';

  @override
  String get errMemberTooOld =>
      'La version de Fogata de ce membre est trop ancienne pour recevoir des demandes d’ami depuis un groupe. Envoie-lui plutôt un lien d’ami.';

  @override
  String get errMemberAskLost =>
      'La demande d’ami n’est pas parvenue à ce membre. Réessaie.';

  @override
  String get errOwnLink =>
      'C’est ton propre lien : envoie-le à l’ami que tu veux ajouter.';

  @override
  String get errLinkUsed =>
      'Ce lien a déjà servi ou a expiré. Demande-en un nouveau.';

  @override
  String get errLinkExpiredUnreached =>
      'Le lien a expiré avant que son appareil ait pu être joint. Demande-en un nouveau.';

  @override
  String get errCodeUnanswered =>
      'Aucun appareil n’a répondu au code. Vérifie-le, et vérifie que l’autre appareil l’affiche encore.';

  @override
  String get errRefused =>
      'L’autre appareil a refusé. Le code a peut-être déjà servi.';

  @override
  String get errNobodyLetIn =>
      'Personne ne t’a fait entrer. Réessaie quand quelqu’un du groupe sera en ligne.';

  @override
  String get errNotBelow =>
      'Tu ne peux faire ça qu’à quelqu’un en dessous de toi dans le groupe.';

  @override
  String get errNotManager =>
      'Seuls ceux qui gèrent le groupe peuvent faire ça.';

  @override
  String get errLastManager =>
      'Tu ne peux pas te retirer toi-même de ceux qui gèrent le groupe.';

  @override
  String get errRoomNotEmpty =>
      'Il y a du monde dans ce salon vocal : supprime-le une fois qu’il sera vide.';

  @override
  String get errTooManyFiles =>
      'Tu partages déjà autant de fichiers qu’un salon en accepte par personne. Arrête d’en partager quelques-uns d’abord.';

  @override
  String get errDiskFull =>
      'Le disque est plein. Libère de l’espace et réessaie.';

  @override
  String get errTooManyChanges =>
      'Cet appareil a fait trop de modifications au groupe qu’aucun autre membre n’a encore reçues. Réessaie après qu’un autre membre du groupe aura fait une modification.';

  @override
  String get errSomething => 'Une erreur s’est produite.';

  @override
  String errRoomFull(int n) {
    return 'Ce salon vocal est plein : il accueille $n personnes au plus.';
  }

  @override
  String get findingOffline =>
      'Aucun pair du réseau public n’a répondu : cet ordinateur ne semble pas accéder à Internet, ou un pare-feu bloque Fogata.';

  @override
  String get findingUdpWorks =>
      'UDP fonctionne. Les appels et le passage à travers les routeurs en ont besoin.';

  @override
  String get findingUdpBlocked =>
      'Ce réseau ou cet ordinateur semble bloquer l’UDP (des pairs qui ont répondu en TCP ne l’ont pas fait en QUIC). Le chat peut encore passer par d’autres membres, mais les connexions directes à travers les routeurs et les appels échoueront le plus souvent. Vérifie le pare-feu ou les règles du réseau.';

  @override
  String get findingCarrierNat =>
      'Cet ordinateur a une adresse de NAT opérateur (100.64.0.0/10) : le fournisseur partage une seule adresse publique entre de nombreux clients.';

  @override
  String get findingPublicIpv4 =>
      'Cet ordinateur a une adresse IPv4 publique : les autres peuvent le joindre directement.';

  @override
  String get findingHardNat =>
      'Derrière un routeur qui donne un port public différent pour chaque destination (un NAT strict).';

  @override
  String get findingEasyNat =>
      'Derrière un routeur qui garde le même port public pour toutes les destinations (un NAT souple) : on peut l’ouvrir des deux côtés à la fois.';

  @override
  String get findingUnknownRouter =>
      'Trop peu de pairs ont vu l’adresse publique de cet ordinateur pour savoir quel type de routeur il y a.';

  @override
  String get findingNoIpv6 =>
      'Pas d’adresse IPv6 publique. Fogata n’en a pas besoin, mais elle facilite les connexions directes.';

  @override
  String get findingIpv6Works =>
      'L’IPv6 public fonctionne : les amis en IPv6 se connectent sans routeur au milieu.';

  @override
  String get findingIpv6Unreached =>
      'Cet ordinateur a une adresse IPv6 publique, mais aucun pair ne l’a encore jointe en IPv6.';

  @override
  String get findingSummaryNoUdp =>
      'En bref : attends-toi à des problèmes de connexion directe tant qu’UDP n’est pas autorisé.';

  @override
  String get findingSummaryPublic =>
      'En bref : tes amis devraient pouvoir joindre cet ordinateur directement.';

  @override
  String get findingSummaryEasyNat =>
      'En bref : les connexions directes devraient fonctionner avec la plupart de tes amis, en ouvrant les deux routeurs à la fois ; avec un ami derrière un NAT strict, il faut un membre du groupe entre vous.';

  @override
  String get findingSummaryHardNatWithIpv6 =>
      'En bref : les amis en IPv6 ou avec une adresse publique se connectent directement ; avec les autres, il faut un membre du groupe entre vous.';

  @override
  String get findingSummaryHardNat =>
      'En bref : seuls les amis avec une adresse publique se connectent directement ; avec les autres, il faut un membre du groupe entre vous.';

  @override
  String get findingSummaryUnknown =>
      'En bref : pas assez d’informations ; vérifie de nouveau dans une minute.';

  @override
  String findingInternet(int n) {
    return 'Internet : cet ordinateur accède au réseau public ($n pairs connus).';
  }

  @override
  String findingDns(String detail) {
    return 'DNS : les paramètres du système n’ont pas pu être lus en entier ($detail)';
  }

  @override
  String findingLanDevices(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n appareils Fogata',
      one: '$n appareil Fogata',
    );
    return '$_temp0 sur ce réseau local : les amis ici se connectent sans passer par Internet.';
  }

  @override
  String findingFirewall(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'dans PowerShell en tant qu’administrateur',
      'macos': 'dans Terminal',
      'other': 'dans un terminal',
    });
    return 'Le pare-feu de cet ordinateur ne laisse rien entrer pour Fogata : un ami au même endroit derrière un autre routeur (deux routeurs à la maison, par exemple) ne peut pas le joindre. Exécute cette commande une fois $_temp0 pour laisser entrer Fogata :';
  }

  @override
  String get settingsLanguage => 'Langue';

  @override
  String get settingsLanguageNote =>
      'La langue des textes de Fogata. Les messages restent tels qu’ils ont été écrits.';

  @override
  String get settingsLanguageChoose => 'Choisir la langue';

  @override
  String get settingsTime => 'Heure';

  @override
  String get settingsTimeNote =>
      'L’affichage des heures dans le chat, les appels et les listes.';

  @override
  String settingsTime24(String example) {
    return '24 heures · $example';
  }

  @override
  String settingsTime12(String example) {
    return '12 heures · $example';
  }

  @override
  String firewallNoticeText(String system, String command) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'dans PowerShell en tant qu’administrateur',
      'macos': 'dans Terminal',
      'other': 'dans un terminal',
    });
    return 'Un ami au même endroit derrière un autre routeur ne peut pas joindre cet ordinateur. Exécute une fois $_temp0 : $command';
  }

  @override
  String firewallCopied(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'dans PowerShell en tant qu’administrateur',
      'macos': 'dans Terminal',
      'other': 'dans un terminal',
    });
    return 'Copié. Colle-la $_temp0 et exécute-la.';
  }

  @override
  String get appRestarted => 'Fogata a cessé de fonctionner et a redémarré.';

  @override
  String appVoiceRoomInGroup(String room, String group) {
    return '$room dans $group';
  }

  @override
  String appBothIn(String room) {
    return 'Vous êtes tous les deux dans $room.';
  }

  @override
  String appHungUp(String name) {
    return '$name a raccroché';
  }

  @override
  String get appYourFriend => 'L’autre personne';

  @override
  String get appCallEndedAlone => 'Personne n’est revenu, l’appel est terminé';

  @override
  String get appAnsweredElsewhere => 'Appel pris sur un autre ordinateur';

  @override
  String appVoiceRoomDeleted(String room) {
    return '« $room » a été supprimé.';
  }

  @override
  String get appSomeone => 'Quelqu’un';

  @override
  String get appAGroup => 'un groupe';

  @override
  String appFriendRequest(String name) {
    return '$name t’a envoyé une demande d’ami.';
  }

  @override
  String appRequestDelivered(String name) {
    return 'Demande remise à $name. Elle attend sa réponse.';
  }

  @override
  String appFriendsNow(String name) {
    return '$name et toi êtes maintenant amis.';
  }

  @override
  String appJoinedGroup(String group) {
    return 'Tu es maintenant dans « $group ».';
  }

  @override
  String appRemovedFromGroup(String group) {
    return 'Tu n’es plus dans « $group ».';
  }

  @override
  String appAddedToGroup(String name, String group) {
    return '$name t’a ajouté à « $group ». Rejoindre ou non : à toi de voir.';
  }

  @override
  String appAsksToJoin(String name, String group) {
    return '$name demande à rejoindre « $group ».';
  }

  @override
  String appJoinWaiting(String group) {
    return 'En attente que quelqu’un de « $group » te fasse entrer.';
  }

  @override
  String appJoinDeclined(String group) {
    return 'Quelqu’un de « $group » a refusé. Cette invitation ne fonctionne plus.';
  }

  @override
  String appDownloadedTo(String path) {
    return 'Téléchargé dans $path';
  }

  @override
  String appDownloadFailed(String why) {
    return 'Téléchargement impossible : $why';
  }

  @override
  String get appOtherSession =>
      'Cette identité est aussi ouverte sur un autre ordinateur. Ferme Fogata là-bas : une identité ne fonctionne que sur un ordinateur à la fois.';

  @override
  String get appFriendsRestored =>
      'Ta liste d’amis était abîmée sur cet ordinateur et a été restaurée à partir de la copie faite avant sa dernière modification. Un ami ajouté ou retiré juste avant devra peut-être être ajouté ou retiré de nouveau.';

  @override
  String appInvitationRanOut(String group) {
    return 'L’invitation à « $group » a expiré avant que quelqu’un du groupe te fasse entrer. Demande-en une nouvelle.';
  }

  @override
  String appTakenOut(String who, String room) {
    return '$who t’a fait sortir de $room.';
  }

  @override
  String appRoomForSome(String room) {
    return '$room est maintenant réservé à certains membres.';
  }

  @override
  String appTakenOutUntilLetBack(String who, String room) {
    return '$who t’a fait sortir de $room jusqu’à ce que quelqu’un au-dessus de toi te laisse revenir.';
  }

  @override
  String appTakenOutFor(String who, String room, String span) {
    return '$who t’a fait sortir de $room pour $span.';
  }

  @override
  String appTakenOutWhile(String who, String room) {
    return '$who t’a fait sortir de $room pour un moment.';
  }

  @override
  String appComeBackIn(String room, String wait) {
    return 'Tu pourras revenir dans $room dans $wait.';
  }

  @override
  String appRoomNotForYou(String room) {
    return '$room ne t’est pas ouvert pour le moment.';
  }

  @override
  String appFoldersNotShared(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'On ne peut pas partager de dossiers, seulement des fichiers : $n dossiers ont été laissés de côté.',
      one: 'On ne peut pas partager de dossiers, seulement des fichiers : le dossier a été laissé de côté.',
      zero:
          'On ne peut pas partager de dossiers, seulement des fichiers : $n dossiers ont été laissés de côté.',
    );
    return '$_temp0';
  }

  @override
  String appSharingOne(String file) {
    return '$file est partagé depuis cet ordinateur. Il ne sera plus disponible quand tu quitteras le salon.';
  }

  @override
  String appSharingMany(int n) {
    return '$n fichiers sont partagés depuis cet ordinateur. Ils ne seront plus disponibles quand tu quitteras le salon.';
  }

  @override
  String appSavedTo(String path) {
    return 'Enregistré dans $path';
  }

  @override
  String appAskingToJoin(String group) {
    return 'Demande envoyée pour rejoindre « $group ». Quelqu’un du groupe te fera entrer dès qu’il la verra.';
  }

  @override
  String get appLookingForDevice =>
      'Recherche de l’autre appareil pour envoyer ta demande…';

  @override
  String callStateNoAnswer(String name) {
    return '$name n’a pas répondu';
  }

  @override
  String callStateDeclined(String name) {
    return '$name ne peut pas parler pour le moment';
  }

  @override
  String callStateBusy(String name) {
    return '$name est dans un autre appel';
  }

  @override
  String callStateOutdated(String name) {
    return '$name a besoin d’une version plus récente de Fogata';
  }

  @override
  String callStateUnreached(String name) {
    return 'Impossible de joindre $name';
  }

  @override
  String get callStateReturn => 'Revenir à l’appel';

  @override
  String get callStateCallBack => 'Rappeler';

  @override
  String get callStateCall => 'Appeler';

  @override
  String noticesDeclined(String name, String group) {
    return '$name a refusé l’invitation à « $group »';
  }

  @override
  String noticesDeclinedToast(String name, String group) {
    return '$name a refusé l’invitation à « $group ».';
  }

  @override
  String noticesMissedInCall(String name) {
    return '$name a appelé pendant que tu étais en appel';
  }

  @override
  String noticesMissedOffline(String name) {
    return '$name a appelé pendant que tu apparaissais hors ligne';
  }

  @override
  String noticesMissedCall(String name) {
    return 'Appel manqué de $name';
  }

  @override
  String get contactsGeneral => 'Général';

  @override
  String get directStateTimerOff => 'Désactivé';

  @override
  String get directStateTimer24h => '24 heures';

  @override
  String directStateTimerDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n jours',
      one: '$n jour',
    );
    return '$_temp0';
  }

  @override
  String directStateDisappearsMinutes(int n) {
    return 'Disparaît dans $n min';
  }

  @override
  String directStateDisappearsHours(int n) {
    return 'Disparaît dans $n h';
  }

  @override
  String directStateDisappearsDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Disparaît dans $n jours',
    );
    return '$_temp0';
  }

  @override
  String get directStateNoLimit => 'Sans limite';

  @override
  String get directStateMessageGone => 'Ce message n’est plus là.';

  @override
  String directStateFoldersNotSent(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'On ne peut pas envoyer de dossiers, seulement des fichiers : $n dossiers ont été laissés de côté.',
      one: 'On ne peut pas envoyer de dossiers, seulement des fichiers : le dossier a été laissé de côté.',
      zero:
          'On ne peut pas envoyer de dossiers, seulement des fichiers : $n dossiers ont été laissés de côté.',
    );
    return '$_temp0';
  }

  @override
  String directStateFileFailed(String why) {
    return 'Impossible d’obtenir le fichier : $why';
  }

  @override
  String movingTooLarge(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Les bannières animées peuvent mesurer jusqu’à $most pixels ; celle-ci mesure $size.',
      'other':
          'Les images animées peuvent mesurer jusqu’à $most pixels ; celle-ci mesure $size.',
    });
    return '$_temp0';
  }

  @override
  String movingTooLong(String what, String most, String count) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Les bannières animées peuvent avoir jusqu’à $most images ; celle-ci en a $count.',
      'other':
          'Les images animées peuvent avoir jusqu’à $most images ; celle-ci en a $count.',
    });
    return '$_temp0';
  }

  @override
  String movingTooHeavy(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Les bannières animées peuvent faire jusqu’à $most ; celle-ci fait $size.',
      'other':
          'Les images animées peuvent faire jusqu’à $most ; celle-ci fait $size.',
    });
    return '$_temp0';
  }

  @override
  String get namesHidden =>
      'Ce nom contient des caractères cachés. Retape-le sans eux.';

  @override
  String get namesBlank =>
      'Ce nom n’a aucun caractère visible. Utilise des lettres, des chiffres ou des emoji.';

  @override
  String get namesDefaultBio => 'Salut ! Je suis sur Fogata.';

  @override
  String get namesBioHidden =>
      'Cette bio contient des caractères cachés. Retape-la sans eux.';

  @override
  String namesBioLines(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Une bio fait $n lignes au plus.',
      one: 'Une bio fait $n ligne au plus.',
    );
    return '$_temp0';
  }

  @override
  String namesBioChars(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Une bio fait $n caractères au plus.',
      one: 'Une bio fait $n caractère au plus.',
    );
    return '$_temp0';
  }

  @override
  String get restartFailed =>
      'Fogata n’a pas pu redémarrer. Quitte-le et rouvre-le toi-même.';

  @override
  String typingOne(String name) {
    return '$name écrit…';
  }

  @override
  String typingTwo(String first, String second) {
    return '$first et $second écrivent…';
  }

  @override
  String get typingSeveral => 'Plusieurs personnes écrivent…';

  @override
  String get outgoingTheRoom => 'le salon';

  @override
  String outgoingDropTitle(String group) {
    return 'Dépose pour partager dans $group';
  }

  @override
  String get outgoingDropNote =>
      'Tu pourras ajouter un message et les vérifier avant l’envoi. Les autres les téléchargent depuis cet ordinateur tant que tu es dans le salon.';

  @override
  String outgoingPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n fichiers cachés ou privés ont été laissés de côté : ces fichiers ne s’ajoutent pas par glisser-déposer, car une page web peut lancer un glissement. Pour les partager, choisis-les avec le trombone.',
      one: 'Un fichier caché ou privé a été laissé de côté : ces fichiers ne s’ajoutent pas par glisser-déposer, car une page web peut lancer un glissement. Pour le partager, choisis-le avec le trombone.',
      zero:
          '$n fichiers cachés ou privés ont été laissés de côté : ces fichiers ne s’ajoutent pas par glisser-déposer, car une page web peut lancer un glissement. Pour les partager, choisis-les avec le trombone.',
    );
    return '$_temp0';
  }

  @override
  String outgoingToShare(int n, String count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$count fichiers à partager · $size',
      one: '$count fichier à partager · $size',
      zero: '$count fichiers à partager · $size',
    );
    return '$_temp0';
  }

  @override
  String get outgoingRemoveAll => 'Tout retirer';

  @override
  String outgoingRemoveFile(String file) {
    return 'Retirer $file';
  }

  @override
  String get windowTrayOpen => 'Ouvrir Fogata';

  @override
  String get windowTrayQuit => 'Quitter Fogata';

  @override
  String get windowMinimize => 'Réduire';

  @override
  String get windowRestore => 'Restaurer';

  @override
  String get windowMaximize => 'Agrandir';

  @override
  String get autostartReason => 'Ouvrir Fogata à l’ouverture de session.';

  @override
  String get devicesMicrophone => 'Micro';

  @override
  String get devicesSpeaker => 'Haut-parleur';

  @override
  String get devicesCamera => 'Caméra';

  @override
  String get devicesChooseMicrophone => 'Choisir le micro';

  @override
  String get devicesChooseSpeaker => 'Choisir le haut-parleur';

  @override
  String get devicesChooseCamera => 'Choisir la caméra';

  @override
  String get devicesNoCamera => 'Aucune caméra n’est branchée.';

  @override
  String get devicesCameraInCall => 'La caméra est allumée dans ton appel.';

  @override
  String devicesCameraFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Impossible d’utiliser la caméra. Autorise-la dans Réglages Système › Confidentialité et sécurité.',
      'other': 'Impossible d’utiliser la caméra.',
    });
    return '$_temp0';
  }

  @override
  String get devicesCameraDenied =>
      'Le système ne laisse pas Fogata utiliser la caméra. Autorise-la dans les paramètres du système, puis réessaie.';

  @override
  String get devicesCameraUnavailable =>
      'Impossible d’accéder à la caméra : ce système n’a pas de portail de caméra.';

  @override
  String get devicesPreviewOnlyYou => 'L’aperçu n’est visible que par toi.';

  @override
  String get devicesStopVideo => 'Arrêter la vidéo';

  @override
  String get devicesTestVideo => 'Tester la vidéo';

  @override
  String get devicesNoiseOn => 'Suppression du bruit activée';

  @override
  String get devicesNoiseOff => 'Suppression du bruit désactivée';

  @override
  String get devicesNoise => 'Suppression du bruit';

  @override
  String get devicesNoiseNote =>
      'Atténue le bruit de fond constant, comme un ventilateur ou un bourdonnement, dans ce que les autres entendent.';

  @override
  String get devicesNoiseShort => 'Atténue un ventilateur ou un bourdonnement.';

  @override
  String get devicesMirror => 'Afficher ma caméra en miroir';

  @override
  String get devicesMirrorNote =>
      'Change seulement la façon dont tu te vois ici. Les autres voient toujours ta caméra telle quelle.';

  @override
  String get devicesMirrorOn => 'Caméra en miroir activée';

  @override
  String get devicesMirrorOff => 'Caméra en miroir désactivée';

  @override
  String get devicesTestMic => 'Tester le micro';

  @override
  String get devicesStopMicTest => 'Arrêter le test';

  @override
  String get devicesMicLevel => 'Niveau du micro';

  @override
  String get devicesHearMyself => 'M’entendre';

  @override
  String get devicesHearMyselfNote =>
      'Diffuse ton micro dans le haut-parleur. Utilise un casque, sinon le micro risque de capter de nouveau le haut-parleur.';

  @override
  String get devicesHearMyselfOn => 'M’entendre activé';

  @override
  String get devicesHearMyselfOff => 'M’entendre désactivé';

  @override
  String get devicesMicTestOnlyYou => 'Le test n’est envoyé à personne.';

  @override
  String get devicesMicInCall => 'Le micro est utilisé par ton appel.';

  @override
  String get devicesNoMic => 'Aucun micro n’est branché.';

  @override
  String get devicesSystemDefault => 'Par défaut du système';

  @override
  String get devicesNotConnected =>
      'Non branché : celui par défaut est utilisé';

  @override
  String get devicesNoCameraShort => 'Aucune caméra n’est branchée';

  @override
  String get devicesMicOptions => 'Options du micro';

  @override
  String get devicesSpeakerOptions => 'Options du haut-parleur';

  @override
  String get devicesCameraOptions => 'Options de la caméra';

  @override
  String get devicesInputDevice => 'Périphérique d’entrée';

  @override
  String get devicesOutputDevice => 'Périphérique de sortie';

  @override
  String get devicesChosenMissing =>
      'L’appareil choisi n’est pas branché : celui par défaut est utilisé.';

  @override
  String get devicesVoiceSettings => 'Paramètres voix et vidéo';

  @override
  String get photoImages => 'Images';

  @override
  String photoTooLarge(int n) {
    return 'Cette image est trop grande : $n mégapixels au plus.';
  }

  @override
  String get photoUnreadable => 'Impossible de lire cette image.';

  @override
  String get callSource => 'Source';

  @override
  String callQuality(String resolution, int fps) {
    return '$resolution · $fps fps';
  }

  @override
  String get callSetupFailed => 'Impossible de lancer l’appel.';

  @override
  String get callNoSoundSystem =>
      'Impossible d’accéder au système audio de cet ordinateur : l’appel ne peut pas commencer.';

  @override
  String callMicGone(String name) {
    return 'Ton micro a été déconnecté : $name est utilisé maintenant.';
  }

  @override
  String get callMicBack => 'Ton micro est de retour.';

  @override
  String callMicSilent(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Impossible d’utiliser le micro. Vérifie qu’il est branché et autorisé dans Réglages Système › Confidentialité et sécurité.',
      'windows': 'Impossible d’utiliser le micro. Vérifie qu’il est branché et autorisé dans Paramètres › Confidentialité et sécurité › Microphone.',
      'other': 'Impossible d’utiliser le micro. Vérifie qu’il est branché.',
    });
    return '$_temp0';
  }

  @override
  String get callNoMicrophone => 'Aucun micro n’est branché.';

  @override
  String callMicFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Impossible d’utiliser le micro. Autorise-le dans Réglages Système › Confidentialité et sécurité.',
      'windows': 'Impossible d’utiliser le micro. Autorise-le dans Paramètres › Confidentialité et sécurité › Microphone.',
      'other': 'Impossible d’utiliser le micro.',
    });
    return '$_temp0';
  }

  @override
  String callScreenFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Impossible de partager l’écran. Autorise-le dans Réglages Système › Confidentialité et sécurité.',
      'other': 'Impossible de partager l’écran.',
    });
    return '$_temp0';
  }

  @override
  String get callShareLost =>
      'Partage arrêté : ce que tu partageais ne peut plus être capturé.';

  @override
  String callKeptOut(String name) {
    return 'Connexion perdue avec $name.';
  }

  @override
  String get qualityGood => 'Bonne';

  @override
  String get qualityOkay => 'Correcte';

  @override
  String get qualityPoor => 'Faible';

  @override
  String get qualityLost => 'Perdue';

  @override
  String qualityDelay(int ms) {
    return '$ms ms';
  }

  @override
  String qualityPercent(String value) {
    return '$value %';
  }

  @override
  String get qualityAnotherMember => 'un autre membre';

  @override
  String qualityLostHint(String name, int seconds) {
    return 'Rien n’est arrivé de $name depuis $seconds s. Fogata continue de le chercher ; la connexion revient d’elle-même dès que son réseau revient.';
  }

  @override
  String qualityBothPoor(String name) {
    return 'Vos deux réseaux sont faibles en ce moment : $name et toi pouvez vous entendre de façon hachée.';
  }

  @override
  String qualityTheirSide(String name) {
    return 'Le problème vient de $name : tout le monde l’entend de façon hachée. Son Wi-Fi ou ses données mobiles sont faibles en ce moment.';
  }

  @override
  String qualityYourSideWith(String name) {
    return 'Le problème vient de toi : $name et les autres peuvent t’entendre de façon hachée. Ton Wi-Fi ou tes données mobiles sont faibles en ce moment.';
  }

  @override
  String qualityTheirsSlow(String name) {
    return 'Le réseau de $name est un peu lent en ce moment : tout le monde peut l’entendre haché de temps en temps.';
  }

  @override
  String qualityAllGoodWith(String name) {
    return '$name t’entend et tu l’entends sans problème.';
  }

  @override
  String qualityMineSlowWith(String name) {
    return 'Ton réseau est un peu lent en ce moment : $name peut t’entendre haché de temps en temps.';
  }

  @override
  String qualityWayThrough(String name, String carrier) {
    return 'Le réseau de $name va bien ; le chemin entre vous deux est plus lent parce qu’il passe par $carrier. Tu peux l’entendre haché de temps en temps, les autres l’entendent bien.';
  }

  @override
  String qualityWaySlowFine(String name) {
    return 'Le réseau de $name va bien ; le chemin entre vous deux est lent en ce moment. Tu peux l’entendre haché de temps en temps, les autres l’entendent bien.';
  }

  @override
  String qualityWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'Le chemin entre toi et $name est un peu lent en ce moment ; ça peut venir de l’un ou l’autre réseau. Vous pouvez vous entendre de façon hachée de temps en temps.',
      'other':
          'Le chemin entre toi et $name est mauvais en ce moment ; ça peut venir de l’un ou l’autre réseau. Vous pouvez vous entendre de façon hachée.',
    });
    return '$_temp0';
  }

  @override
  String qualityWayVia(String level, String name, String carrier) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'Le chemin entre toi et $name est un peu lent en ce moment ; ça peut venir de l’un ou l’autre réseau. Vous pouvez vous entendre de façon hachée de temps en temps. Il passe par $carrier.',
      'other':
          'Le chemin entre toi et $name est mauvais en ce moment ; ça peut venir de l’un ou l’autre réseau. Vous pouvez vous entendre de façon hachée. Il passe par $carrier.',
    });
    return '$_temp0';
  }

  @override
  String get qualityYourSide =>
      'Le problème vient de toi : les autres peuvent t’entendre de façon hachée. Ton Wi-Fi ou tes données mobiles sont faibles en ce moment.';

  @override
  String get qualityMineSlow =>
      'Ton réseau est un peu lent en ce moment : les autres peuvent t’entendre haché de temps en temps.';

  @override
  String qualityLostShort(String name, int seconds) {
    return 'Rien n’est arrivé de $name depuis $seconds s. Fogata continue de le chercher.';
  }

  @override
  String qualityAllWellWith(String name) {
    return 'Tout va bien : $name et toi vous entendez sans problème.';
  }

  @override
  String qualityNoteDropped(String name) {
    return '$name a perdu la connexion et Fogata le cherche de nouveau';
  }

  @override
  String qualityNoteTheirsPoor(String name) {
    return 'le réseau de $name est faible, tout le monde l’entend de façon hachée';
  }

  @override
  String qualityNoteTheirsSlow(String name) {
    return 'le réseau de $name est un peu lent';
  }

  @override
  String qualityNoteThrough(String name, String carrier) {
    return '$name te parvient par $carrier';
  }

  @override
  String qualityNoteWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': 'le chemin vers $name est un peu lent en ce moment',
      'other': 'le chemin vers $name est mauvais en ce moment',
    });
    return '$_temp0';
  }

  @override
  String get qualityNoteSeparator => ' ; ';

  @override
  String get qualityAllWell =>
      'Tout va bien : tu entends tout le monde et tout le monde t’entend.';

  @override
  String qualityAllWellMine(String notes) {
    return 'Tout va bien de ton côté. $notes.';
  }

  @override
  String qualityYourDetails(String level) {
    return 'Ta connexion : $level. Afficher les détails';
  }

  @override
  String qualityTheirDetails(String name, String level) {
    return 'Connexion de $name : $level. Afficher les détails';
  }

  @override
  String get qualityReconnecting => 'Reconnexion…';

  @override
  String get qualityReconnectingLabel => 'Reconnexion';

  @override
  String qualityLevelConnection(String level) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'good': 'Bonne connexion',
      'okay': 'Connexion correcte',
      'weak': 'Connexion faible',
      'other': 'Connexion perdue',
    });
    return '$_temp0';
  }

  @override
  String get qualityPoorConnection => 'Connexion faible';

  @override
  String get qualityMinePoorBanner =>
      'Ton réseau est faible : les autres peuvent t’entendre de façon hachée';

  @override
  String get qualityYourNetwork => 'Ton réseau';

  @override
  String qualityTo(String name) {
    return 'Vers $name';
  }

  @override
  String qualityBetween(String name) {
    return 'Entre toi et $name';
  }

  @override
  String qualityTheirNetwork(String name) {
    return 'Réseau de $name';
  }

  @override
  String get qualityLastHeard => 'Dernier signal';

  @override
  String qualitySecondsAgo(int n) {
    return 'il y a $n s';
  }

  @override
  String get qualityPath => 'Chemin';

  @override
  String get qualityLookingAgain => 'Nouvelle recherche en cours';

  @override
  String get qualityDelayLabel => 'Latence';

  @override
  String get qualityLostPackets => 'Paquets perdus';

  @override
  String get qualityYourConnection => 'Ta connexion';

  @override
  String qualityTheirConnection(String name) {
    return 'Connexion de $name';
  }

  @override
  String roomCountFullLabel(int count, int max) {
    return 'Complet, $count personnes sur $max';
  }

  @override
  String roomCountLabel(int count, int max) {
    return '$count personnes sur $max';
  }

  @override
  String get roomCountFullPrefix => 'Complet · ';

  @override
  String get roomCountFullWord => 'Complet ';

  @override
  String get volumeVoice => 'Volume de la voix';

  @override
  String get volumeOnlyYouHear =>
      'Ces réglages ne changent que ce que tu entends';

  @override
  String get volumeScreenSound => 'Son de l’écran';

  @override
  String volumePercent(int n) {
    return '$n %';
  }

  @override
  String get volumeMuteVoiceForMe => 'Couper sa voix pour moi';

  @override
  String get volumeMuteForMe => 'Rendre muet pour moi';

  @override
  String get volumeMuteScreenForMe => 'Couper le son de l’écran pour moi';

  @override
  String get volumeBackTo100 => 'Revenir à 100 %';

  @override
  String get volumeBelowYou => 'EN DESSOUS DE TOI';

  @override
  String volumeRemoveFrom(String room) {
    return 'Retirer de $room';
  }

  @override
  String volumeRemoveTitle(String name, String room) {
    return 'Retirer $name de $room ?';
  }

  @override
  String volumeRemoveText(String name, String room) {
    return '$name quitte $room maintenant et peut revenir aussitôt. Une exclusion temporaire tient quelqu’un à l’écart pendant un moment.';
  }

  @override
  String volumeTimeOutOf(String room) {
    return 'Exclure temporairement de $room';
  }

  @override
  String volumeTimeOutText(String name, String room) {
    return '$name quitte $room et ne peut pas revenir pendant :';
  }

  @override
  String get volumeUntilLetBack =>
      'Jusqu’à ce que quelqu’un au-dessus de lui le laisse revenir';

  @override
  String get pipYourScreen => 'Ton écran';

  @override
  String pipScreenOf(String name) {
    return 'Écran de $name';
  }

  @override
  String get pipYourCamera => 'Ta caméra';

  @override
  String get pipBackToCall => 'Revenir à l’appel';

  @override
  String get pipBackToRoom => 'Revenir au salon';

  @override
  String get pipStopWatching => 'Arrêter de regarder';

  @override
  String get addToCallTitle => 'Ajouter à l’appel';

  @override
  String addToCallNote(int n) {
    return 'Amis en ligne. Jusqu’à $n personnes par appel.';
  }

  @override
  String get addToCallSearch => 'Rechercher des amis';

  @override
  String get addToCallNoFriends => 'Pas encore d’amis.';

  @override
  String get addToCallNoMatch => 'Aucun ami de ce nom.';

  @override
  String get addToCallInCall => 'Dans l’appel';

  @override
  String get addToCallCalling => 'Appel en cours…';

  @override
  String get addToCallOffline => 'Hors ligne';

  @override
  String get addToCallCall => 'Appeler';

  @override
  String get streamQualityTitle => 'Qualité du partage';

  @override
  String get streamQualityNoteSharing =>
      'Comment ton écran arrive aux autres. Un changement s’applique tout de suite.';

  @override
  String get streamQualityNote =>
      'Comment ton écran arrive aux autres quand tu le partages.';

  @override
  String get streamQualityResolution => 'Résolution';

  @override
  String streamQualitySourceLines(int lines) {
    return 'Source (${lines}p)';
  }

  @override
  String get streamQualitySource => 'Source';

  @override
  String get streamQualityFrameRate => 'Fréquence d’images';

  @override
  String streamQualityFps(int n) {
    return '$n fps';
  }

  @override
  String get streamQualityHigherNote =>
      'Plus c’est élevé, plus ça demande de débit montant et de puissance à ton ordinateur.';

  @override
  String get streamQualitySomeone => 'Quelqu’un';

  @override
  String streamQualityLoweredOne(String name, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Les autres reçoivent ce que tu as choisi.',
      'other': '',
    });
    return 'La connexion de $name ne peut pas suivre pour le moment : l’image qui lui parvient est donc plus petite.$_temp0';
  }

  @override
  String streamQualityLoweredMany(String names, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Les autres reçoivent ce que tu as choisi.',
      'other': '',
    });
    return 'Certaines connexions ne peuvent pas suivre pour le moment : $names reçoivent une image plus petite.$_temp0';
  }

  @override
  String get streamQualityGoingOut => 'ENVOYÉ EN CE MOMENT';

  @override
  String get streamQualityNobody => 'Personne ne regarde encore.';

  @override
  String streamQualityNow(int height, int fps) {
    return '${height}p · $fps fps';
  }

  @override
  String callLayersIsCalling(String name) {
    return '$name t’appelle';
  }

  @override
  String get callLayersGoToCall => 'F6 pour aller à l’appel';

  @override
  String callLayersWith(String names) {
    return 'Avec $names';
  }

  @override
  String get callLayersDirectCall => 'Appel direct · ';

  @override
  String get callLayersAnswerMicOff => 'Répondre avec le micro coupé';

  @override
  String callLayersAnsweringLeaves(String room) {
    return 'Répondre te fait quitter $room';
  }

  @override
  String get callLayersLeaveAndAnswer => 'Quitter et répondre';

  @override
  String callLayersBothIn(String room) {
    return 'Vous êtes tous les deux dans $room.';
  }

  @override
  String get callLayersDismiss => 'Ignorer';

  @override
  String callLayersCallWith(String names) {
    return 'Appel avec $names';
  }

  @override
  String get callLayersRoom => 'Salon';

  @override
  String callLayersBackTo(String title) {
    return 'Revenir à $title';
  }

  @override
  String callLayersRinging(String time) {
    return 'Ça sonne · $time';
  }

  @override
  String callLayersInCall(String time) {
    return 'En appel · $time';
  }

  @override
  String callLayersOnlyYou(String time) {
    return 'Seulement toi · $time';
  }

  @override
  String callLayersInRoom(int n, String time) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n dans le salon',
    );
    return '$_temp0 · $time';
  }

  @override
  String get callLayersMute => 'Couper le micro';

  @override
  String get callLayersUnmute => 'Réactiver le micro';

  @override
  String get callLayersHangUp => 'Raccrocher';

  @override
  String get callLayersLeaveRoom => 'Quitter le salon';

  @override
  String roomDeclineCallTitle(String name) {
    return 'Refuser l’appel de $name ?';
  }

  @override
  String roomDeclineCallText(String caller, String callee) {
    return '$caller t’appelle. Tu ne peux être que dans un appel à la fois : appeler $callee refuse cet appel.';
  }

  @override
  String roomDeclineCallTextSomeone(String caller) {
    return '$caller t’appelle. Tu ne peux être que dans un appel à la fois : appeler quelqu’un d’autre refuse cet appel.';
  }

  @override
  String get roomDeclineAndCall => 'Refuser et appeler';

  @override
  String roomHangUpTitle(String name) {
    return 'Mettre fin à l’appel avec $name ?';
  }

  @override
  String get roomHangUpText =>
      'Tu ne peux être que dans un appel ou un salon à la fois : continuer met fin à l’appel.';

  @override
  String get roomHangUp => 'Raccrocher';

  @override
  String get roomCloseTitle => 'Quitter et fermer le salon ?';

  @override
  String get roomCloseText =>
      'Il ne reste que toi ici. Quand tu pars, le salon se ferme et ce qui s’y est dit disparaît de cet ordinateur ; ce que les autres ont téléchargé reste chez eux.';

  @override
  String get roomCloseChatMessages => 'Messages du chat';

  @override
  String get roomCloseFilesShared => 'Fichiers que tu as partagés';

  @override
  String get roomStay => 'Rester';

  @override
  String get roomLeaveAndClose => 'Quitter et fermer';

  @override
  String get roomLivePrefix => 'En direct · ';

  @override
  String get roomPillConnecting => 'Connexion';

  @override
  String get roomPillOnlyYou => 'Seulement toi';

  @override
  String get roomPillError => 'Erreur';

  @override
  String get roomLastOne => 'Tout le monde est parti sauf toi.';

  @override
  String get roomOnlyYouIn => 'Personne d’autre n’est dans le salon.';

  @override
  String get roomAloneWarning =>
      ' Quand tu pars, il se ferme et tout ce qu’il contient disparaît.';

  @override
  String get roomEarlierPeople => 'Personnes précédentes';

  @override
  String get roomMorePeople => 'Plus de personnes';

  @override
  String roomTileYou(String name) {
    return '$name (toi)';
  }

  @override
  String roomTileNoSound(String name) {
    return '$name · sans son';
  }

  @override
  String roomTileConnecting(String name) {
    return '$name · connexion';
  }

  @override
  String roomTileAway(String name) {
    return '$name · connexion interrompue';
  }

  @override
  String roomTileMicOff(String name) {
    return '$name, micro coupé';
  }

  @override
  String roomTileMutedByYou(String said) {
    return '$said, coupé pour toi';
  }

  @override
  String roomTileVolume(String said, String percent) {
    return '$said, volume $percent';
  }

  @override
  String roomTileLeft(String name) {
    return '$name a quitté · peut revenir';
  }

  @override
  String roomIsSharing(String name) {
    return '$name partage son écran';
  }

  @override
  String get roomNothingSentUntilWatch =>
      'Rien ne t’est envoyé tant que tu ne regardes pas';

  @override
  String get roomWatch => 'Regarder';

  @override
  String get roomLive => 'EN DIRECT';

  @override
  String get roomSharingYourScreen => 'Tu partages ton écran';

  @override
  String get roomShowPreview => 'Afficher l’aperçu';

  @override
  String get roomPreviewMirror =>
      'L’afficher pendant que tu partages Fogata lui-même crée un miroir sans fin';

  @override
  String get roomNobodyWatching => 'Personne ne regarde';

  @override
  String roomPeopleWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n personnes regardent',
      one: '$n personne regarde',
    );
    return '$_temp0';
  }

  @override
  String get roomExitFullScreen => 'Quitter le plein écran';

  @override
  String get roomFullScreen => 'Plein écran';

  @override
  String get roomPlayScreenSound => 'Écouter le son de l’écran';

  @override
  String get roomMuteScreenSound => 'Couper le son de l’écran';

  @override
  String get roomQualityPoor => 'Faible';

  @override
  String get roomQualityOkay => 'Correcte';

  @override
  String get roomQualityGood => 'Bonne';

  @override
  String get roomQualityPoorTitle => 'Connexion faible';

  @override
  String get roomQualityOkayTitle => 'Connexion moyenne';

  @override
  String get roomQualityGoodTitle => 'Bonne connexion';

  @override
  String get roomQualityMeasuring => 'Mesure…';

  @override
  String get roomQualityDelay => 'Latence';

  @override
  String roomQualityMs(String n) {
    return '$n ms';
  }

  @override
  String get roomQualityLost => 'Paquets perdus';

  @override
  String roomQualityPercent(String n) {
    return '$n %';
  }

  @override
  String get roomQualityPicture => 'Image';

  @override
  String get roomQualityPath => 'Chemin';

  @override
  String get roomQualityMayFreeze =>
      'L’image peut se figer. Moins de partages en même temps, ça aide.';

  @override
  String get roomQualityMayShrink =>
      'L’image peut devenir plus petite pour rester fluide.';

  @override
  String roomCallingName(String name) {
    return 'Appel à $name…';
  }

  @override
  String get roomRingsFor30 =>
      'Ça sonne sur son ordinateur pendant 30 secondes au plus.';

  @override
  String get roomReturnToCall => 'Revenir à l’appel';

  @override
  String get roomCallAgain => 'Rappeler';

  @override
  String get roomAloneHint =>
      'Quand quelqu’un du groupe entre, il apparaît ici.';

  @override
  String roomAlreadyLeft(String names) {
    return 'Le salon a déjà vu partir $names.';
  }

  @override
  String get roomGroupFound => 'Groupe trouvé sur le réseau';

  @override
  String get roomLookingForRoute => 'recherche du meilleur chemin…';

  @override
  String get roomConnectingTitle => 'Connexion au salon';

  @override
  String roomSeconds(String n) {
    return '$n s';
  }

  @override
  String roomReachedOf(int reached, int total, String time) {
    return '$reached sur $total · $time';
  }

  @override
  String get roomPathLan => 'réseau local, chiffré';

  @override
  String get roomPathDirect => 'direct, chiffré';

  @override
  String get roomPathThroughMember => 'via un membre, chiffré';

  @override
  String roomPathThrough(String name) {
    return 'via $name, chiffré';
  }

  @override
  String get roomCouldNotConnect => 'Impossible de se connecter au salon';

  @override
  String get roomCouldNotConnectText =>
      'Personne n’a répondu à temps. Ton réseau bloque peut-être les connexions directes entre appareils.';

  @override
  String get roomNoAnswer => 'pas de réponse';

  @override
  String get roomSeeNetworkCheck => 'Voir la vérification du réseau →';

  @override
  String get roomGoesNothing => 'Le salon se ferme pour tout le monde';

  @override
  String roomGoesMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n messages disparaissent',
      one: '$n message disparaît',
    );
    return '$_temp0';
  }

  @override
  String roomGoesFiles(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n fichiers que tu as partagés disparaissent',
      one: '$n fichier que tu as partagé disparaît',
    );
    return '$_temp0';
  }

  @override
  String roomGoesBoth(int messages, int files) {
    String _temp0 = intl.Intl.pluralLogic(
      messages,
      locale: localeName,
      other: '$messages messages',
      one: '$messages message',
    );
    String _temp1 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files fichiers',
      one: '$files fichier',
    );
    return '$_temp0 et $_temp1 disparaissent';
  }

  @override
  String get roomMuteMicrophone => 'Couper le micro';

  @override
  String get roomUnmuteMicrophone => 'Réactiver le micro';

  @override
  String get roomStopSharingScreen => 'Arrêter de partager l’écran';

  @override
  String get roomShareScreen => 'Partager l’écran';

  @override
  String get roomUndeafen => 'Réentendre les autres';

  @override
  String get roomDeafen => 'Ne plus entendre les autres';

  @override
  String get roomCameraOff => 'Éteindre la caméra';

  @override
  String get roomCameraOn => 'Allumer la caméra';

  @override
  String get roomLeave => 'Quitter';

  @override
  String get roomSoundWaitsForMic =>
      'Le son n’est envoyé que quand ton micro est activé';

  @override
  String get roomStopSharingSound => 'Arrêter de partager le son';

  @override
  String get roomShareSoundToo => 'Partager aussi le son';

  @override
  String get roomAddFriendToCall => 'Ajouter un ami à l’appel';

  @override
  String get roomHideChat => 'Masquer le chat';

  @override
  String get roomShowChat => 'Afficher le chat';

  @override
  String get roomTabChat => 'Chat';

  @override
  String get roomTabFiles => 'Fichiers';

  @override
  String roomNoteLeftCall(String name) {
    return '$name a quitté l’appel · ';
  }

  @override
  String roomNoteJoinedCall(String name) {
    return '$name a rejoint l’appel · ';
  }

  @override
  String roomNoteLeftRoom(String name) {
    return '$name a quitté le salon · ';
  }

  @override
  String roomNoteEnteredRoom(String name) {
    return '$name a rejoint le salon · ';
  }

  @override
  String get roomSearchMessages => 'Rechercher des messages';

  @override
  String get roomClearSearch => 'Effacer la recherche';

  @override
  String get roomChatGoneCall => 'Ce chat disparaît à la fin de l’appel';

  @override
  String get roomChatGoneRoom => 'Ce chat disparaît quand le salon se ferme';

  @override
  String roomNoMatches(String text) {
    return 'Aucun message avec « $text »';
  }

  @override
  String roomMatches(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n résultats',
      one: '$n résultat',
    );
    return '$_temp0';
  }

  @override
  String get roomReceivingMessages => 'Réception des messages du salon…';

  @override
  String roomReceivingMessagesFrom(String name) {
    return 'Réception des messages du salon depuis l’ordinateur de $name…';
  }

  @override
  String get roomSomeone => 'Quelqu’un';

  @override
  String get roomShareFiles => 'Partager des fichiers';

  @override
  String get roomAddMoreFiles => 'Ajouter d’autres fichiers';

  @override
  String roomMessageTo(String name) {
    return 'Écrire à $name';
  }

  @override
  String get roomMessageTheRoom => 'Écrire au salon';

  @override
  String get roomMessageTheCall => 'Écrire dans l’appel';

  @override
  String get roomAddMessageOptional => 'Ajouter un message (facultatif)';

  @override
  String get roomMessageLabel => 'Message';

  @override
  String roomEnterToSend(String key) {
    return '$key pour envoyer';
  }

  @override
  String roomEscToRemove(String key) {
    return '$key pour les retirer';
  }

  @override
  String roomOrTwo(String first, String second) {
    return '$first ou $second';
  }

  @override
  String roomOrMore(String rest, String last) {
    return '$rest ou $last';
  }

  @override
  String roomFileNotAvailable(String name) {
    return 'Indisponible · $name a quitté le salon';
  }

  @override
  String get roomSharingFromHere => 'Partagé depuis cet ordinateur';

  @override
  String roomAvailableWhile(String names) {
    return 'Disponible tant que $names reste dans le salon';
  }

  @override
  String roomSizeDownloads(String size, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n téléchargements',
      one: '$n téléchargement',
    );
    return '$size · $_temp0';
  }

  @override
  String roomDownloadFile(String file) {
    return 'Télécharger $file';
  }

  @override
  String roomShowInFolder(String file) {
    return 'Afficher $file dans son dossier';
  }

  @override
  String roomGettingFile(String file) {
    return 'Récupération de $file…';
  }

  @override
  String get roomSaveToDownloads => 'Enregistrer dans Téléchargements';

  @override
  String roomPictureKept(String name) {
    return '$name a quitté le salon · l’image reste ici jusqu’à ton départ';
  }

  @override
  String get roomSaveFile => 'Enregistrer';

  @override
  String get roomClosePicture => 'Fermer l’image';

  @override
  String get roomFilesNote =>
      'Les fichiers ne sont envoyés sur aucun serveur : ils viennent directement de l’ordinateur de celui qui les partage, et seulement tant qu’il reste dans le salon.';

  @override
  String roomFilesAvailable(int n) {
    return 'Disponibles · $n';
  }

  @override
  String roomFilesGone(int n) {
    return 'Plus disponibles · $n';
  }

  @override
  String roomPersonLeft(String name) {
    return '$name a quitté le salon';
  }

  @override
  String get roomPickOrDrop => 'Choisis-les ici ou dépose-les sur le salon';

  @override
  String get roomFileYours => 'À toi';

  @override
  String roomFileFrom(String name) {
    return 'De $name';
  }

  @override
  String roomDownloads(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n téléchargements',
      one: '$n téléchargement',
    );
    return '$_temp0';
  }

  @override
  String get roomStopSharingFile => 'Arrêter';

  @override
  String get roomWhatToShare => 'Que veux-tu partager ?';

  @override
  String roomScreenSource(String name) {
    return 'Écran : $name';
  }

  @override
  String roomPlayVideo(String file) {
    return 'Lire $file';
  }

  @override
  String get roomCloseVideo => 'Fermer la vidéo';

  @override
  String get roomVideoGone => 'Cette vidéo n’est plus dans le salon.';

  @override
  String get roomVideoNotFetched => 'Impossible de récupérer cette vidéo.';

  @override
  String get roomVideoCannotPlay =>
      'Cet ordinateur ne peut pas lire cette vidéo.';

  @override
  String roomGettingVideoPercent(int n) {
    return 'Récupération de la vidéo · $n %';
  }

  @override
  String get roomGettingVideo => 'Récupération de la vidéo…';

  @override
  String get roomPause => 'Pause';

  @override
  String get roomPlay => 'Lecture';

  @override
  String get roomSoundOn => 'Activer le son';

  @override
  String get roomMuteVideo => 'Couper le son';

  @override
  String get roomLeaveFullScreen => 'Quitter le plein écran';

  @override
  String roomCallWaits(String time) {
    return 'Quand il ne reste qu’une personne, l’appel attend $time que les autres reviennent';
  }

  @override
  String roomCallWaitsAlone(String time) {
    return 'Il ne reste que toi. L’appel attend $time que les autres reviennent';
  }

  @override
  String roomOpenFile(String file) {
    return 'Ouvrir $file';
  }

  @override
  String activityCreated(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as créé',
      'other': '$by a créé',
    });
    return '$_temp0 le groupe sous le nom « $name »';
  }

  @override
  String activityRenamed(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as renommé',
      'other': '$by a renommé',
    });
    return '$_temp0 le groupe en « $name »';
  }

  @override
  String activityIcon(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as changé',
      'other': '$by a changé',
    });
    return '$_temp0 l’icône du groupe';
  }

  @override
  String activityPictureRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as retiré',
      'other': '$by a retiré',
    });
    return '$_temp0 l’image du groupe';
  }

  @override
  String activityPicture(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as changé',
      'other': '$by a changé',
    });
    return '$_temp0 l’image du groupe';
  }

  @override
  String activityBannerRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as retiré',
      'other': '$by a retiré',
    });
    return '$_temp0 la bannière du groupe';
  }

  @override
  String activityBanner(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as changé',
      'other': '$by a changé',
    });
    return '$_temp0 la bannière du groupe';
  }

  @override
  String activityAdded(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Tu t’es ajouté',
      'other': 'Tu as ajouté $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by t’a ajouté',
      'other': '$by a ajouté $member',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2';
  }

  @override
  String activityJoined(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as rejoint',
      'other': '$by a rejoint',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'toi',
      'other': '$member',
    });
    return '$_temp0 le groupe avec une invitation créée par $_temp1';
  }

  @override
  String activityLetIn(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Tu es entré',
      'other': 'Tu as fait entrer $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by t’a fait entrer',
      'other': '$by a fait entrer $member',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2';
  }

  @override
  String activityRemoved(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Tu t’es retiré',
      'other': 'Tu as retiré $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by t’a retiré',
      'other': '$by a retiré $member',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2';
  }

  @override
  String activityLeft(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as quitté',
      'other': '$by a quitté',
    });
    return '$_temp0 le groupe';
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
      'yes': 'Tu es monté',
      'other': 'Tu as fait monter $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by t’a fait monter',
      'other': '$by a fait monter $member',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2 au $place';
  }

  @override
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Tu t’es placé',
      'other': 'Tu as placé $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by t’a placé',
      'other': '$by a placé $member',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2 à la fin';
  }

  @override
  String activityManagersAll(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as laissé',
      'other': '$by a laissé',
    });
    return '$_temp0 tout le monde gérer le groupe';
  }

  @override
  String activityManagersFirst(String by, String byMe, int n) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as confié',
      'other': '$by a confié',
    });
    String _temp1 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'aux $n premiers',
      one: 'au premier seulement',
      zero: 'aux $n premiers',
    );
    return '$_temp0 la gestion du groupe $_temp1';
  }

  @override
  String activityLimits(String by, String byMe, int people, int perRoom) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as limité',
      'other': '$by a limité',
    });
    return '$_temp0 le groupe à $people personnes et chaque salon vocal à $perRoom';
  }

  @override
  String activityRoomOpened(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as ouvert',
      'other': '$by a ouvert',
    });
    return '$_temp0 le salon vocal « $room »';
  }

  @override
  String activityRoomRenamed(String by, String byMe, String from, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as renommé',
      'other': '$by a renommé',
    });
    return '$_temp0 le salon vocal « $from » en « $room »';
  }

  @override
  String activityRoomClosed(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as supprimé',
      'other': '$by a supprimé',
    });
    return '$_temp0 le salon vocal « $room »';
  }

  @override
  String activityInvited(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as créé',
      'other': '$by a créé',
    });
    return '$_temp0 un lien d’invitation';
  }

  @override
  String activityRevoked(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as annulé',
      'other': '$by a annulé',
    });
    return '$_temp0 un lien d’invitation';
  }

  @override
  String activityRoomOpenToAll(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as ouvert',
      'other': '$by a ouvert',
    });
    return '$_temp0 « $room » à tout le monde';
  }

  @override
  String activityRoomForAbove(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as réservé',
      'other': '$by a réservé',
    });
    return '$_temp0 « $room » à ceux placés au-dessus';
  }

  @override
  String activityRoomOnly(String by, String byMe, String people, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Tu as réservé',
      'other': '$by a réservé',
    });
    return '$_temp0 « $room » à $people seulement';
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
      'yes': 'Tu t’es exclu',
      'other': 'Tu as exclu $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by t’a exclu',
      'other': '$by a exclu $member',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2 de « $room » jusqu’à nouvel ordre';
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
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Tu t’es exclu',
      'other': 'Tu as exclu $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by t’a exclu',
      'other': '$by a exclu $member',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    String _temp3 = intl.Intl.selectLogic(untilKind, {
      'today': 'jusqu’à $until',
      'todayAtOne': 'jusqu’à $until',
      'yesterday': 'jusqu’à $until',
      'other': 'jusqu’au $until',
    });
    return '$_temp2 de « $room » $_temp3';
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
      'yes': 'Tu es revenu',
      'other': 'Tu as laissé $member revenir',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by t’a laissé revenir',
      'other': '$by a laissé $member revenir',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2 dans « $room »';
  }

  @override
  String get activityDefaultRoom => 'Général';

  @override
  String get activityYouInList => 'toi';

  @override
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'tu avais été retiré',
      'other': '$author avait été retiré',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' par toi',
      'other': ' par $by',
      'none': '',
    });
    return 'Annulé : $_temp0$_temp1 au même moment';
  }

  @override
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'tu avais été placé à la fin',
      'other': '$author avait été placé à la fin',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' par toi',
      'other': ' par $by',
      'none': '',
    });
    return 'Annulé : $_temp0$_temp1 au même moment';
  }

  @override
  String activityUndoneLeft(String author, String authorMe) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'tu avais quitté le groupe',
      'other': '$author avait quitté le groupe',
    });
    return 'Annulé : $_temp0 au même moment';
  }

  @override
  String get activityAboutList =>
      'Tout le monde dans le groupe voit la même liste, tirée du registre du groupe. Les changements d’image et de bannière sont à part : ce sont ceux que cet appareil a vus.';

  @override
  String get activityAboutOrder =>
      'Dans l’ordre propre au groupe ; les heures viennent de l’horloge de chaque auteur.';

  @override
  String get activityNothingYet => 'Rien pour l’instant.';

  @override
  String get activityLoading => 'Chargement…';

  @override
  String get activityShowOlder => 'Afficher les entrées précédentes';

  @override
  String activitySeenHere(String when) {
    return '$when · vu sur cet appareil';
  }

  @override
  String get bellNotifications => 'Notifications';

  @override
  String bellNewNotifications(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n nouvelles notifications',
      one: '$n nouvelle notification',
    );
    return '$_temp0';
  }

  @override
  String get bellClearAll => 'Tout effacer';

  @override
  String get bellNothingNew => 'Rien de nouveau';

  @override
  String get bellFooter =>
      'Les trois dernières restent ici. Les appels manqués et les demandes s’affichent aussi dans Amis.';

  @override
  String bellBothInRoom(String room) {
    return 'Vous êtes tous les deux dans $room.';
  }

  @override
  String get bellDismiss => 'Ignorer';

  @override
  String bellFriendRequest(String name) {
    return 'Demande d’ami de $name';
  }

  @override
  String bellCheckFingerprint(String fingerprint) {
    return 'Vérifie que l’empreinte correspond : $fingerprint';
  }

  @override
  String bellJoinAsk(String name, String group) {
    return '$name demande à rejoindre « $group »';
  }

  @override
  String bellJoinAskNote(String fingerprint) {
    return 'Avec un des liens d’invitation de ton groupe. Empreinte : $fingerprint';
  }

  @override
  String get bellLetIn => 'Faire entrer';

  @override
  String get bellSayNo => 'Refuser';

  @override
  String bellGroupInvite(String name, String group) {
    return '$name t’a ajouté à « $group »';
  }

  @override
  String bellMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n membres',
      one: '$n membre',
    );
    return '$_temp0';
  }

  @override
  String get bellJoin => 'Rejoindre';

  @override
  String bellNewMessages(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n nouveaux messages de $name',
      one: 'Nouveau message de $name',
      zero: '$n nouveaux messages de $name',
    );
    return '$_temp0';
  }

  @override
  String get bellOpen => 'Ouvrir';

  @override
  String get bellMarkRead => 'Marquer comme lu';

  @override
  String get bellFirewallTitle => 'Ton pare-feu bloque certains amis';

  @override
  String get bellCopyCommand => 'Copier la commande';

  @override
  String get bellOtherSessionTitle => 'Ouvert aussi sur un autre ordinateur';

  @override
  String get bellSettingsResetTitle =>
      'Certains paramètres ont été réinitialisés';

  @override
  String get bellSettingsResetText =>
      'Certains de tes paramètres n’ont pas pu être lus et sont revenus à leurs valeurs par défaut : vérifie-les dans Paramètres. L’ancien fichier a été gardé à côté du nouveau, sous le nom app.json.bad.';

  @override
  String get friendCardOnline => 'En ligne';

  @override
  String get friendCardAway => 'Absent';

  @override
  String get friendCardOffline => 'Hors ligne';

  @override
  String get friendCardCallsOnlyOnline =>
      'Les appels ne sonnent que si l’autre personne est en ligne.';

  @override
  String friendCardHangUpFirst(String name) {
    return 'Raccroche d’abord pour appeler $name.';
  }

  @override
  String friendCardBothInRoom(String room) {
    return 'Vous êtes tous les deux dans $room.';
  }

  @override
  String get friendCardMessage => 'Écrire';

  @override
  String friendCardMutualFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n amis en commun',
      one: '$n ami en commun',
    );
    return '$_temp0';
  }

  @override
  String get memberCardSendRequest => 'Envoyer une demande d’ami';

  @override
  String get memberCardRequestSent => 'Demande d’ami envoyée';

  @override
  String get memberCardAccept => 'Accepter la demande d’ami';

  @override
  String memberCardAsksYou(String name) {
    return '$name t’a envoyé une demande d’ami.';
  }

  @override
  String memberCardInGroup(String group) {
    return 'Membre de $group';
  }

  @override
  String memberCardOnlyOnline(String name) {
    return 'Les demandes d’ami ne partent que lorsque $name est en ligne.';
  }

  @override
  String get memberCardShowOwn => 'Afficher ta carte';

  @override
  String get userPanelEditProfile => 'Modifier le profil';

  @override
  String get userPanelStatus => 'Statut';

  @override
  String get userPanelOnline => 'En ligne';

  @override
  String get userPanelAway => 'Absent';

  @override
  String get userPanelOffline => 'Hors ligne';

  @override
  String get userPanelOfflineNote =>
      'Tes amis et les membres de tes groupes te voient hors ligne. Tu peux quand même être dans des salons.';

  @override
  String get userPanelOnlineNote =>
      'Tes amis et les membres de tes groupes te voient en ligne, ou « Absent » après 10 min sans utiliser cet ordinateur.';

  @override
  String get userPanelBio => 'BIO';

  @override
  String get userPanelAppearingOffline => 'Tu apparais hors ligne';

  @override
  String get userPanelCopyLinkLabel => 'Copier un lien d’ami';

  @override
  String userPanelLinkExpiresIn(String time) {
    return 'Expire dans $time';
  }

  @override
  String get userPanelLinkTerms => '1 personne · 10 min';

  @override
  String get userPanelCopyLink => 'Copier le lien d’ami';

  @override
  String get switcherChatEnded => 'Conversation privée · vous n’êtes plus amis';

  @override
  String get switcherChatOnline => 'Conversation privée · en ligne';

  @override
  String get switcherChatAway => 'Conversation privée · absent';

  @override
  String get switcherChat => 'Conversation privée';

  @override
  String switcherFriend(String presence) {
    return 'Ami · $presence';
  }

  @override
  String switcherVoiceRoom(String group) {
    return 'Salon vocal de $group';
  }

  @override
  String switcherVoiceRoomPeople(String group, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Salon vocal de $group · $n personnes',
      one: 'Salon vocal de $group · $n personne',
    );
    return '$_temp0';
  }

  @override
  String switcherGroup(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Groupe · $n membres',
      one: 'Groupe · $n membre',
    );
    return '$_temp0';
  }

  @override
  String get switcherPlaceholder =>
      'Aller à une conversation, un groupe ou un salon vocal';

  @override
  String get switcherGoTo => 'Aller à';

  @override
  String get switcherEmpty =>
      'Pas encore de groupes, d’amis ni de conversations.';

  @override
  String switcherNoMatch(String query) {
    return 'Rien ne correspond à « $query ».';
  }

  @override
  String get switcherHintChoose => 'pour choisir';

  @override
  String get switcherHintGo => 'pour y aller';

  @override
  String get switcherHintClose => 'pour fermer';

  @override
  String switcherTooltip(String shortcut) {
    return 'Aller à une conversation, un groupe ou un salon vocal ($shortcut)';
  }

  @override
  String get friendsIntro =>
      'Chaque amitié n’existe que sur vos deux appareils. Pas d’annuaire, pas de serveur.';

  @override
  String get friendsTabAll => 'Tous';

  @override
  String get friendsTabOnline => 'En ligne';

  @override
  String get friendsTabRequests => 'Demandes';

  @override
  String get friendsSearch => 'Rechercher des amis';

  @override
  String get friendsNone =>
      'Pas encore d’amis. Partage ton code QR, ton lien ou ton code, ou utilise ceux de quelqu’un d’autre.';

  @override
  String get friendsNoneOnline => 'Aucun ami n’est en ligne pour le moment.';

  @override
  String get friendsNoMatch => 'Personne ne correspond à la recherche.';

  @override
  String get friendsInARoom => 'Dans un salon';

  @override
  String friendsInRoomOf(String group) {
    return 'Dans un salon · $group';
  }

  @override
  String get friendsOnline => 'En ligne';

  @override
  String get friendsAway => 'Absent';

  @override
  String get friendsNotSeenYet => 'Pas encore en ligne';

  @override
  String friendsSeen(String when) {
    return 'Dernière connexion $when';
  }

  @override
  String friendsShowCard(String name) {
    return 'Afficher la carte de $name';
  }

  @override
  String friendsMissedCall(String when) {
    return 'Appel manqué · $when';
  }

  @override
  String get friendsMessage => 'Écrire';

  @override
  String friendsOfflineNoCall(String name) {
    return '$name est hors ligne : les appels ne sonnent que si l’autre personne est en ligne';
  }

  @override
  String friendsHangUpFirst(String name) {
    return 'Raccroche d’abord pour appeler $name';
  }

  @override
  String friendsBothInRoom(String room) {
    return 'Vous êtes tous les deux dans $room.';
  }

  @override
  String friendsOptions(String name) {
    return 'Options pour $name';
  }

  @override
  String friendsRemoveTitle(String name) {
    return 'Retirer $name de tes amis ?';
  }

  @override
  String get friendsRemoveText =>
      'Vous disparaissez de vos listes d’amis respectives, sans message. Vous restez ensemble dans les groupes que vous partagez. Pour redevenir amis, il faudra une nouvelle invitation.';

  @override
  String get friendsRemove => 'Retirer';

  @override
  String friendsAddToGroup(String group) {
    return 'Ajouter à « $group »';
  }

  @override
  String get friendsRemoveMenu => 'Retirer des amis…';

  @override
  String get friendsNoRequests => 'Aucune demande en attente.';

  @override
  String friendsRequestIncoming(String when) {
    return 'Demande d’ami reçue $when';
  }

  @override
  String friendsRequestOutgoing(String when) {
    return 'Envoyée $when · en attente de réponse';
  }

  @override
  String get friendsPendingRequest => 'Demande en attente';

  @override
  String friendsCheckFingerprint(String name) {
    return 'Vérifie que l’empreinte correspond à celle que $name voit sur son écran.';
  }

  @override
  String get friendsAddTitle => 'Ajouter un ami';

  @override
  String get friendsAddNote =>
      'Vos appareils échangent leurs clés directement.';

  @override
  String get friendsTabCode => 'Code';

  @override
  String get friendsTabLink => 'Lien';

  @override
  String get friendsTabQr => 'QR';

  @override
  String get friendsMakeInvitation => 'Créer une invitation';

  @override
  String get friendsInvitationExpired => 'Cette invitation a expiré.';

  @override
  String get friendsMakeAnother => 'En créer une autre';

  @override
  String friendsHintQr(String time) {
    return 'Scanne-le depuis Fogata · expire dans $time';
  }

  @override
  String friendsHintLink(String time) {
    return 'Envoie-le à une seule personne · expire dans $time';
  }

  @override
  String friendsHintCode(String time) {
    return 'Lis-le à une seule personne · expire dans $time';
  }

  @override
  String get friendsHiddenCode => 'Masqué au cas où tu partagerais ton écran.';

  @override
  String get friendsHiddenLink =>
      'Masqué au cas où tu partagerais ton écran. Il contient les adresses réseau de cet ordinateur : partage-le seulement en privé.';

  @override
  String get friendsGotLink => 'Tu as un lien d’ami ou de groupe, ou un code ?';

  @override
  String get friendsPastePlaceholder => 'Colle un lien ou un code';

  @override
  String get friendsLinkOrCode => 'Lien ou code';

  @override
  String get friendsAdd => 'Ajouter';

  @override
  String get friendsQrLabel => 'Code QR de ton invitation';

  @override
  String friendsAddedToGroup(String name) {
    return '$name recevra le groupe la prochaine fois que vous serez tous les deux en ligne.';
  }

  @override
  String get directToday => 'Aujourd’hui';

  @override
  String get directYesterday => 'Hier';

  @override
  String directTimerOff(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Tu as désactivé',
      'other': '$name a désactivé',
    });
    return '$_temp0 les messages éphémères';
  }

  @override
  String directTimerFirst(String time) {
    return 'Messages éphémères activés : $time';
  }

  @override
  String directTimerOn(String name, String mine, String time) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Tu as activé',
      'other': '$name a activé',
    });
    return '$_temp0 les messages éphémères : $time';
  }

  @override
  String directPinnedLine(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Tu as épinglé',
      'other': '$name a épinglé',
    });
    return '$_temp0 un message';
  }

  @override
  String get directCallFewSeconds => 'quelques secondes';

  @override
  String directCallMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minutes',
      one: '$n minute',
    );
    return '$_temp0';
  }

  @override
  String directCallHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n heures',
      one: '$n heure',
    );
    return '$_temp0';
  }

  @override
  String directCallHoursMinutes(int hours, int minutes) {
    String _temp0 = intl.Intl.pluralLogic(
      hours,
      locale: localeName,
      other: '$hours heures',
      one: '$hours heure',
    );
    String _temp1 = intl.Intl.pluralLogic(
      minutes,
      locale: localeName,
      other: '$minutes minutes',
      one: '$minutes minute',
    );
    return '$_temp0 $_temp1';
  }

  @override
  String directCallLine(String name, String mine, String length) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Tu as lancé',
      'other': '$name a lancé',
    });
    return '$_temp0 un appel qui a duré $length';
  }

  @override
  String get directBlockedOffline =>
      'Tu apparais hors ligne. Passe en ligne pour écrire.';

  @override
  String directBlockedFriendOffline(String name) {
    return '$name est hors ligne. Tu pourras écrire quand vous serez tous les deux en ligne.';
  }

  @override
  String get directOnline => 'En ligne';

  @override
  String get directAway => 'Absent';

  @override
  String get directOffline => 'Hors ligne';

  @override
  String directOfflineSeen(String when) {
    return 'Hors ligne · dernière connexion $when';
  }

  @override
  String get directHeading => 'MESSAGES PRIVÉS';

  @override
  String directPreviewMine(String text) {
    return 'Toi : $text';
  }

  @override
  String directOpenChat(String name) {
    return 'Ouvrir la conversation avec $name';
  }

  @override
  String directShowCard(String name) {
    return 'Afficher la carte de $name';
  }

  @override
  String get directNoLongerFriend => 'Vous n’êtes plus amis';

  @override
  String directCallOffline(String name) {
    return '$name est hors ligne : les appels ne sonnent que si l’autre personne est en ligne';
  }

  @override
  String directHangUpFirst(String name) {
    return 'Raccroche d’abord pour appeler $name';
  }

  @override
  String directPinnedCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n messages épinglés',
      one: '$n message épinglé',
    );
    return '$_temp0';
  }

  @override
  String get directPinnedTitle => 'Messages épinglés';

  @override
  String directPinnedNote(int max) {
    return 'Épinglés pour vous deux, ils ne disparaissent pas. Jusqu’à $max.';
  }

  @override
  String get directNothingPinned => 'Rien d’épinglé pour l’instant';

  @override
  String get directNothingPinnedNote =>
      'Survole un message et choisis l’épingle pour le garder ici.';

  @override
  String directDayTime(String day, String time) {
    return '$day, $time';
  }

  @override
  String directFilesCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n fichiers',
    );
    return '$_temp0';
  }

  @override
  String directShowMessageFrom(String name) {
    return 'Afficher le message de $name';
  }

  @override
  String get directUnpin => 'Désépingler';

  @override
  String get directPin => 'Épingler';

  @override
  String get directDeleteMessage => 'Supprimer pour moi';

  @override
  String get directDeleteMessageTitle => 'Supprimer ce message ?';

  @override
  String directDeleteMessageNote(String name) {
    return 'Il est supprimé de cet ordinateur, avec ses images et fichiers. $name garde sa copie.';
  }

  @override
  String get directDeleteMessageAction => 'Supprimer';

  @override
  String get directDeleteForBoth => 'Supprimer pour les deux';

  @override
  String directDeleteOwnNote(String name) {
    return 'Supprimer pour les deux le retire aussi de la discussion de $name, dès que son Fogata le reçoit. Supprimer pour moi l’y laisse.';
  }

  @override
  String directTimerLabel(String time) {
    return 'Messages éphémères : $time';
  }

  @override
  String get directTimerTitle => 'Messages éphémères';

  @override
  String get directCloseMenu => 'Fermer le menu';

  @override
  String get directTimerNote =>
      'Les nouveaux messages disparaissent sur les deux appareils, avec leurs images et leurs fichiers.';

  @override
  String get directEmptyNote =>
      'Ce que tu écris n’est gardé que sur vos deux ordinateurs.';

  @override
  String directPinLineLabel(String line) {
    return '$line : l’afficher';
  }

  @override
  String get directShowMessage => 'Afficher le message';

  @override
  String get directPinnedStays =>
      'Épinglé : il reste jusqu’à ce qu’on le désépingle';

  @override
  String get directPinned => 'Épinglé';

  @override
  String directPinsFull(int max) {
    return 'Tu peux épingler $max messages au plus : désépingle-en un d’abord';
  }

  @override
  String get directDelivered => 'Remis';

  @override
  String get directNotDelivered => 'Non remis';

  @override
  String get directSending => 'Envoi…';

  @override
  String directWaiting(String name) {
    return 'En attente que $name soit en ligne · envoyé quand vous le serez tous les deux';
  }

  @override
  String get directAvailableWhenYouOnline =>
      'disponible quand tu seras en ligne';

  @override
  String directAvailableWhenOnline(String name) {
    return 'disponible quand $name sera en ligne';
  }

  @override
  String get directDownloadedByItself => 'téléchargé automatiquement';

  @override
  String directOpenFile(String name) {
    return 'Ouvrir $name';
  }

  @override
  String directPlayFile(String name) {
    return 'Lire $name';
  }

  @override
  String directDownloadSize(String size) {
    return 'Télécharger · $size';
  }

  @override
  String directVideoPlaysWhenHere(String name, String length) {
    return '$name · $length · lecture dès son arrivée';
  }

  @override
  String directVideoNotHere(String name, String length, String when) {
    return '$name · $length · $when';
  }

  @override
  String directGettingIt(int percent) {
    return 'Réception · $percent %';
  }

  @override
  String directFileNotHere(String size, String when) {
    return '$size · $when';
  }

  @override
  String get directSaveToDownloads => 'Enregistrer dans Téléchargements';

  @override
  String get directRemovedForSpace => 'Supprimé ici pour libérer de l’espace';

  @override
  String get directAskAgain => 'Redemander';

  @override
  String get directClosePicture => 'Fermer l’image';

  @override
  String get directCloseVideo => 'Fermer la vidéo';

  @override
  String get directCannotPlay => 'Cet ordinateur ne peut pas lire cette vidéo.';

  @override
  String get directPause => 'Pause';

  @override
  String get directPlay => 'Lecture';

  @override
  String get directSoundOn => 'Activer le son';

  @override
  String get directMute => 'Couper le son';

  @override
  String directRemoveFilesTitle(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Retirer les $n fichiers ?',
      one: 'Retirer le fichier ?',
      zero: 'Retirer les $n fichiers ?',
    );
    return '$_temp0';
  }

  @override
  String get directRemoveFilesText =>
      'Ils sont retirés de ce message. Les fichiers sur ton ordinateur restent tels quels.';

  @override
  String get directDraftKept =>
      'Ce que tu écrivais reste ici pour quand tu pourras l’envoyer.';

  @override
  String directMessagePlaceholder(String name) {
    return 'Écrire à $name';
  }

  @override
  String get directAddMessageOptional => 'Ajouter un message (facultatif)';

  @override
  String get directAutoDownloadNote =>
      'Les images jusqu’à 10 Mo se téléchargent automatiquement ; les vidéos et les fichiers, quand tu les ouvres.';

  @override
  String get directVoiceRecord => 'Enregistrer un message vocal';

  @override
  String get directVoiceInCall => 'Tu ne peux pas enregistrer pendant un appel';

  @override
  String directVoiceWaitingElsewhere(String name) {
    return 'Envoie ou supprime d’abord le message vocal de ta conversation avec $name.';
  }

  @override
  String get directVoiceDelete => 'Supprimer';

  @override
  String get directVoiceTooShort =>
      'Trop court pour être envoyé : un message vocal dure au moins une demi-seconde.';

  @override
  String directVoiceLeft(String time) {
    return 'encore $time';
  }

  @override
  String directVoiceRecordingFrom(String microphone) {
    return 'Enregistrement depuis $microphone';
  }

  @override
  String get directVoiceRecordingDefault =>
      'Le micro choisi n’est pas disponible : enregistrement avec celui par défaut';

  @override
  String directVoiceKeys(String esc, String enter) {
    return '$esc le supprime, $enter l’envoie';
  }

  @override
  String directVoiceKeySend(String enter) {
    return '$enter l’envoie';
  }

  @override
  String directVoicePreview(String length) {
    return 'Message vocal · $length';
  }

  @override
  String get directVoiceMessage => 'Message vocal';

  @override
  String get directVoiceNoMic => 'Fogata ne peut pas utiliser le micro.';

  @override
  String get directVoiceMicStopped =>
      'Le micro s’est arrêté. Tu peux quand même envoyer ce qui a été enregistré.';

  @override
  String directVoiceNoMicCheck(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Vérifie qu’un micro est branché, choisi dans Voix et vidéo et autorisé dans Réglages Système › Confidentialité et sécurité.',
      'other': 'Vérifie qu’un micro est branché et choisi dans Voix et vidéo.',
    });
    return '$_temp0';
  }

  @override
  String get directVoiceSpeed => 'Vitesse de lecture';

  @override
  String get directVoiceNotPlayed => 'Pas encore écouté';

  @override
  String directRemoveFile(String name) {
    return 'Retirer $name';
  }

  @override
  String get directAltered =>
      'L’historique de cette conversation sur cet ordinateur a changé depuis le dernier enregistrement de Fogata. Des messages peuvent manquer.';

  @override
  String directEnded(String name) {
    return '$name et toi n’êtes plus amis. Cette conversation est en lecture seule.';
  }

  @override
  String get directDeleteChat => 'Supprimer la conversation';

  @override
  String directDeleteTitle(String name) {
    return 'Supprimer la conversation avec $name ?';
  }

  @override
  String directDeleteText(String name) {
    return 'Ses messages, images, vidéos et fichiers disparaissent de cet ordinateur. $name garde sa copie.';
  }

  @override
  String get directDelete => 'Supprimer';

  @override
  String get directUsageMessages => 'Messages';

  @override
  String get directUsagePictures => 'Images';

  @override
  String get directUsageVideos => 'Vidéos';

  @override
  String get directUsageFiles => 'Fichiers';

  @override
  String get directUsageHeading => 'CETTE CONVERSATION SUR CET APPAREIL';

  @override
  String get directFreeUpSpace => 'Libérer de l’espace…';

  @override
  String get directAddFilesTooltip =>
      'Ajouter des images, des vidéos ou des fichiers';

  @override
  String get directAddFiles => 'Ajouter des fichiers';

  @override
  String get accountMoveLessThanMinuteLeft => 'moins d’une minute restante';

  @override
  String get accountMoveAboutMinuteLeft => 'environ une minute restante';

  @override
  String accountMoveAboutMinutesLeft(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'environ $n minutes restantes',
      one: 'environ une minute restante',
      zero: 'environ $n minutes restantes',
    );
    return '$_temp0';
  }

  @override
  String accountMoveNoRoom(String need, String free) {
    return 'Cet ordinateur a besoin de $need d’espace libre pour le compte, et n’en a que $free.';
  }

  @override
  String accountMoveTooLarge(String size) {
    return 'L’autre ordinateur a proposé $size, bien plus que ce que contient un compte.';
  }

  @override
  String get accountMoveBusy =>
      'Cet ordinateur termine un autre transfert : termine-le ou abandonne-le d’abord.';

  @override
  String get accountMoveNothingToFinish =>
      'Cet ordinateur n’a aucun transfert à terminer venant de cet ordinateur-là.';

  @override
  String get accountMoveNetworkStopped => 'Le réseau s’est arrêté.';

  @override
  String get accountMoveConnectionLost => 'La connexion a été perdue.';

  @override
  String get accountMovePeerNoRoom =>
      'Le nouvel ordinateur n’a pas assez d’espace libre.';

  @override
  String get accountMovePeerTooLarge =>
      'Le compte est trop gros pour un transfert.';

  @override
  String get accountMovePeerBusy =>
      'Le nouvel ordinateur termine un autre transfert.';

  @override
  String get accountMovePeerCouldNotKeep =>
      'Le nouvel ordinateur n’a pas pu enregistrer les fichiers.';

  @override
  String get accountMovePeerCancelled =>
      'Le transfert a été annulé sur l’autre ordinateur.';

  @override
  String get accountMovePeerCouldNotFinish =>
      'Le nouvel ordinateur n’a pas encore pu terminer le transfert.';

  @override
  String get accountMovePeerStopped =>
      'Le transfert a été arrêté sur l’autre ordinateur.';

  @override
  String get accountMovePeerNothingToFinish =>
      'Cet ordinateur-là n’a aucun transfert à terminer venant de celui-ci.';

  @override
  String get accountMovePeerNotProven =>
      'Cet ordinateur-là n’a pas prouvé qu’il a reçu ce transfert.';

  @override
  String get accountMoveWrongCode =>
      'L’autre ordinateur n’a pas accepté le code : rien n’a été envoyé.';

  @override
  String get accountMoveExpired => 'Le code a expiré avant d’être utilisé.';

  @override
  String get accountMoveTooManyTries =>
      'Plusieurs mauvais codes ont été essayés ; le code ne fonctionne plus.';

  @override
  String get accountMoveNoContact =>
      'Les deux ordinateurs n’ont pas pu se joindre.';

  @override
  String get accountMoveUnconfirmed =>
      'La toute fin du transfert n’a pas été confirmée.';

  @override
  String get accountMoveCancelled => 'Annulé.';

  @override
  String get accountMoveInterrupted =>
      'Le transfert s’est arrêté avant la fin ; rien n’a changé.';

  @override
  String accountMoveInterruptedWhy(String why) {
    return 'Le transfert s’est arrêté avant la fin ; rien n’a changé. $why';
  }

  @override
  String get accountMoveDeclined => 'L’autre ordinateur a arrêté le transfert.';

  @override
  String accountMoveDeclinedWhy(String why) {
    return 'L’autre ordinateur a arrêté le transfert. $why';
  }

  @override
  String get accountMoveLater =>
      'Le compte est arrivé et sera mis en place au prochain démarrage de Fogata ici.';

  @override
  String accountMoveLaterWhy(String why) {
    return 'Le compte est arrivé et sera mis en place au prochain démarrage de Fogata ici. $why';
  }

  @override
  String get accountMoveDidNotFinish => 'Le transfert n’a pas abouti.';

  @override
  String get accountMoveAnotherComputer => 'un autre ordinateur';

  @override
  String get accountMoveTheOtherComputer => 'l’autre ordinateur';

  @override
  String get moveTooLateBeingPlaced =>
      'Trop tard pour arrêter : le compte est en cours de mise en place.';

  @override
  String get moveTooLateDiscard =>
      'Trop tard : le compte est en cours de mise en place.';

  @override
  String get moveTooLateOtherHasIt =>
      'Trop tard pour arrêter : l’autre ordinateur a maintenant le compte.';

  @override
  String moveFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n amis',
      one: '$n ami',
    );
    return '$_temp0';
  }

  @override
  String moveGroups(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n groupes',
      one: '$n groupe',
    );
    return '$_temp0';
  }

  @override
  String moveGroupsWithKeys(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n groupes, avec leurs clés',
      one: '$n groupe, avec ses clés',
    );
    return '$_temp0';
  }

  @override
  String moveChats(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n conversations',
      one: '$n conversation',
    );
    return '$_temp0';
  }

  @override
  String moveInAll(String size) {
    return '$size au total';
  }

  @override
  String get moveYourSettings => 'Tes paramètres';

  @override
  String get moveNotDevices => 'sauf le micro et la caméra choisis';

  @override
  String get moveFinishingBoth => 'Finalisation sur les deux ordinateurs…';

  @override
  String movePercent(int percent) {
    return '$percent %';
  }

  @override
  String movePercentLeft(int percent, String left) {
    return '$percent % · $left';
  }

  @override
  String get moveStepIdentity => 'Identité et profil';

  @override
  String get moveStepFriends => 'Amis';

  @override
  String get moveStepGroups => 'Groupes et leurs clés';

  @override
  String get moveStepChats => 'Conversations, images et fichiers';

  @override
  String moveBytesOf(String done, String total) {
    return '$done sur $total';
  }

  @override
  String get moveStepFinishing => 'Finalisation sur les deux ordinateurs';

  @override
  String moveBringingNamed(String name) {
    return 'Transfert du compte de $name';
  }

  @override
  String get moveBringingYours => 'Transfert de ton compte';

  @override
  String get moveKeepBothOn =>
      'Garde les deux ordinateurs allumés et Fogata ouvert.';

  @override
  String get moveKeepOpenNote =>
      'Garde Fogata ouvert sur les deux ordinateurs. C’est plus rapide sur le même réseau ; sinon ils se connectent directement par Internet.';

  @override
  String moveFinishBringingNamed(String name) {
    return 'Terminer le transfert du compte de $name';
  }

  @override
  String get moveFinishBringingYours => 'Terminer le transfert de ton compte';

  @override
  String get moveEverythingCame =>
      'Tout est arrivé. Tape ce code sur l’autre ordinateur pour terminer.';

  @override
  String moveEverythingCameFrom(String computer) {
    return 'Tout est arrivé. Tape ce code sur « $computer » pour terminer.';
  }

  @override
  String get moveBringHereTitle => 'Transférer ton compte ici';

  @override
  String get moveBringHereSub =>
      'Tape ce code sur l’ordinateur qui a ton compte.';

  @override
  String get moveHideCode => 'Masquer le code';

  @override
  String get moveShowCode => 'Afficher le code';

  @override
  String moveExpiresIn(String time) {
    return 'expire dans $time';
  }

  @override
  String moveStepFogataShows(String what) {
    return 'Sur l’autre ordinateur, Fogata affiche $what.';
  }

  @override
  String get moveWaitingToConfirm => 'En attente de confirmation';

  @override
  String get moveStepOpenThere => 'Sur l’autre ordinateur, ouvre Fogata.';

  @override
  String moveStepTypeUnder(String where) {
    return 'Tape le code sous $where.';
  }

  @override
  String get moveFinishTheMove => 'Terminer le transfert';

  @override
  String moveStepGoTo(String where) {
    return 'Va dans $where.';
  }

  @override
  String get moveSettingsPath => 'Paramètres → Transférer le compte';

  @override
  String get moveToAnotherComputer => 'Transférer vers un autre ordinateur';

  @override
  String get moveWaitingForOther => 'En attente de ton autre ordinateur…';

  @override
  String moveWrongCodesTyped(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'De mauvais codes ont été tapés $n fois sur d’autres ordinateurs.',
      one: 'Un mauvais code a été tapé sur un autre ordinateur.',
      zero: 'De mauvais codes ont été tapés $n fois sur d’autres ordinateurs.',
    );
    return '$_temp0';
  }

  @override
  String get moveThrowAwayInstead => 'Plutôt le jeter…';

  @override
  String get moveFoundOther => 'Ton autre ordinateur a été trouvé';

  @override
  String get moveFoundSameNetwork => 'Même réseau · chiffré entre les deux';

  @override
  String get moveFoundInternet => 'Par Internet · chiffré entre les deux';

  @override
  String get moveConfirmThere =>
      'Confirme sur l’autre ordinateur pour commencer';

  @override
  String get moveSameAccountNote =>
      'C’est le compte qui est déjà sur cet ordinateur. Rien ne se perd en l’amenant ici.';

  @override
  String get moveTakeYes => 'Transférer ici';

  @override
  String get moveWelcomeBack => 'Bon retour';

  @override
  String moveWelcomeBackNamed(String name) {
    return 'Bon retour, $name';
  }

  @override
  String get moveDoneSub =>
      'Ton compte est maintenant sur cet ordinateur. L’autre ordinateur a cessé de l’utiliser.';

  @override
  String get moveOpenFogata => 'Ouvrir Fogata';

  @override
  String get moveCodeExpiredLabel => 'expiré';

  @override
  String get moveCodeStoppedLabel => 'invalidé';

  @override
  String get moveCodeExpiredNote =>
      'Ce code a expiré avant que ton autre ordinateur l’utilise. Les codes durent 10 minutes.';

  @override
  String get moveCodeStoppedNote =>
      'De mauvais codes ont été tapés plusieurs fois sur d’autres ordinateurs : ce code ne fonctionne plus.';

  @override
  String get moveNewCode => 'Nouveau code';

  @override
  String get moveAlmostThere => 'Presque fini';

  @override
  String get moveLaterSub =>
      'Ton compte est arrivé en entier, mais il n’a pas encore pu être mis en place. Il le sera au prochain démarrage de Fogata ici.';

  @override
  String get moveStartAgain => 'Redémarrer Fogata';

  @override
  String get moveUnconfirmedSub =>
      'Tout est arrivé, mais l’autre ordinateur n’a pas confirmé la fin. Affiche un code et tape-le là-bas pour terminer le transfert.';

  @override
  String get moveStoppedThereTitle =>
      'Le transfert a été arrêté sur l’autre ordinateur';

  @override
  String get moveStoppedThereSub =>
      'Rien n’a changé : ton compte est toujours sur l’autre ordinateur, et rien n’en a été gardé ici.';

  @override
  String get moveCouldNotStartTitle => 'Le transfert n’a pas pu commencer';

  @override
  String get moveSomethingWrongHere =>
      'Une erreur s’est produite sur cet ordinateur.';

  @override
  String get moveHalfwayTitle => 'Le transfert s’est arrêté en cours de route';

  @override
  String get moveHalfwaySub =>
      'La connexion avec ton autre ordinateur a été perdue. Rien n’a changé : ton compte est toujours sur l’autre ordinateur, et rien n’en a été gardé ici.';

  @override
  String get moveShowCodeToFinish => 'Afficher un code pour terminer';

  @override
  String get moveTryNewCode => 'Réessayer avec un nouveau code';

  @override
  String moveBackToNamed(String name) {
    return 'Revenir à $name';
  }

  @override
  String get moveBackToThisProfile => 'Revenir à ce profil';

  @override
  String get moveNewProfileInstead => 'Créer plutôt un nouveau profil';

  @override
  String get moveDiscardTitle => 'Jeter le compte reçu ?';

  @override
  String get moveDiscardText =>
      'Fais-le seulement si l’autre ordinateur continue de l’utiliser. Cet ordinateur-là attend de confirmer jusqu’à ce que son utilisateur choisisse de continuer à l’utiliser là-bas.';

  @override
  String get moveKeepIt => 'Le garder';

  @override
  String get moveThrowAway => 'Le jeter';

  @override
  String get moveBringFromAnother =>
      'Transférer ton compte depuis un autre ordinateur';

  @override
  String get moveAlreadyOnFogata => 'Déjà sur Fogata ?';

  @override
  String get moveCodeTriedBefore =>
      'Ce code a déjà échoué une fois. Obtiens un nouveau code sur le nouvel ordinateur.';

  @override
  String moveMovingTo(String computer) {
    return 'Transfert de ton compte vers « $computer »';
  }

  @override
  String get moveKeepBothOnOffline =>
      'Garde les deux ordinateurs allumés et Fogata ouvert. Pendant ce temps, Fogata est hors ligne ici.';

  @override
  String get moveCodeFieldLabel => 'Le code affiché sur le nouvel ordinateur';

  @override
  String get moveToAnotherSub =>
      'Ton identité, tes amis, groupes, conversations, fichiers et paramètres vont sur l’autre ordinateur, directement depuis celui-ci et chiffrés. Ensuite, cet ordinateur cesse d’utiliser ton compte : tu n’existes jamais en double.';

  @override
  String moveStepChoose(String choice) {
    return 'Sur le nouvel ordinateur, ouvre Fogata et choisis $choice.';
  }

  @override
  String get moveStepTypeItShows => 'Tape le code qu’il affiche :';

  @override
  String get moveWaitingForAnswer => 'En attente de ta réponse…';

  @override
  String get moveLookingForCode =>
      'Recherche de l’ordinateur qui affiche ce code…';

  @override
  String get moveOfflineWhileMoving =>
      'Fogata passe hors ligne sur cet ordinateur pendant le transfert.';

  @override
  String get moveWrongCodeNote =>
      'Le nouvel ordinateur n’a pas accepté ce code : rien n’a été envoyé. Vérifie-le et tape-le de nouveau. Si ça échoue encore, obtiens un nouveau code là-bas.';

  @override
  String get moveCantReachNew => 'Impossible de joindre le nouvel ordinateur.';

  @override
  String get moveCantReachNewNote =>
      'Les deux se sont cherchés pendant 3 minutes. Mets-les sur le même réseau (le même Wi-Fi, ou un câble) et réessaie. Ton compte est toujours ici et Fogata est de nouveau en ligne.';

  @override
  String get moveDeclinedNote =>
      'Le transfert a été arrêté sur le nouvel ordinateur. Rien n’a été transféré : ton compte est toujours ici et Fogata est de nouveau en ligne.';

  @override
  String get moveCouldNotStart => 'Le transfert n’a pas pu commencer.';

  @override
  String get moveHalfwayNote =>
      'Le transfert s’est arrêté en cours de route : la connexion avec le nouvel ordinateur a été perdue. Rien n’a changé : ton compte est toujours ici et Fogata est de nouveau en ligne.';

  @override
  String get moveThisProfileWords => 'ce profil';

  @override
  String get moveBringCardTitle => 'Transférer un compte ici';

  @override
  String get moveBringCardSub =>
      'Affiche un code sur cet ordinateur et tape-le sur celui qui a le compte.';

  @override
  String get moveBringCardButton => 'Transférer un compte ici…';

  @override
  String moveReplacesHere(String name) {
    return 'Remplace $name sur cet ordinateur.';
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
      other: '$friends amis',
      one: '$friends ami',
    );
    String _temp1 = intl.Intl.pluralLogic(
      groups,
      locale: localeName,
      other: '$groups groupes',
      one: '$groups groupe',
    );
    String _temp2 = intl.Intl.pluralLogic(
      chats,
      locale: localeName,
      other: '$chats conversations',
      one: '$chats conversation',
    );
    return '$bold, le profil actuel de cet ordinateur, sera remplacé : $_temp0, $_temp1 et $_temp2 seront supprimés ici. Pour garder $name, transfère-le d’abord vers un autre ordinateur.';
  }

  @override
  String moveReplaceAndShow(String name) {
    return 'Remplacer $name et afficher un code';
  }

  @override
  String moveConfirmTitle(String computer) {
    return 'Transférer ton compte vers « $computer » ?';
  }

  @override
  String get moveConfirmSameNetwork =>
      'Trouvé sur le même réseau. Tout part chiffré, directement vers lui.';

  @override
  String get moveConfirmInternet =>
      'Trouvé par Internet. Tout part chiffré, directement vers lui.';

  @override
  String moveConfirmReplaces(String name) {
    return 'Il remplace le profil $name là-bas.';
  }

  @override
  String get moveConfirmNote =>
      'Une fois terminé, cet ordinateur cesse d’utiliser ton compte et ne pourra plus l’utiliser. Tu pourras ensuite supprimer ce qui reste ici.';

  @override
  String get moveConfirmYes => 'Transférer mon compte';

  @override
  String get moveKeepTitle => 'Continuer à utiliser ton compte ici ?';

  @override
  String get moveKeepText =>
      'Seulement si le nouvel ordinateur ne l’a pas : ouvre d’abord Fogata là-bas. Si les deux l’utilisaient, tes amis et tes groupes te verraient en double, et les messages n’arriveraient qu’à l’un des deux.';

  @override
  String get moveKeepYes => 'Continuer ici';

  @override
  String moveWaitingFor(String computer) {
    return 'En attente de la confirmation de « $computer »';
  }

  @override
  String get moveWaitingText =>
      'Ton compte a été envoyé, mais la connexion a été coupée avant que le nouvel ordinateur dise qu’il est prêt. Pendant ce temps, cet ordinateur n’utilise pas le compte.';

  @override
  String get moveFinishText =>
      'Ouvre Fogata sur le nouvel ordinateur : il affiche un code. Tape-le ici.';

  @override
  String get moveFinish => 'Terminer';

  @override
  String get moveFinishWrongCode =>
      'Le nouvel ordinateur n’a pas accepté ce code. Vérifie-le et tape-le de nouveau.';

  @override
  String get moveFinishNoContact =>
      'Impossible de joindre le nouvel ordinateur. Mets les deux sur le même réseau et réessaie.';

  @override
  String get moveKeepThisOne =>
      'Le nouvel ordinateur ne l’a pas : continuer avec celui-ci…';

  @override
  String moveMovedYours(String computer) {
    return 'Ton compte a été transféré vers « $computer »';
  }

  @override
  String moveMovedNamed(String name, String computer) {
    return 'Le compte de $name a été transféré vers « $computer »';
  }

  @override
  String get moveMovedNoteYou =>
      'Cet ordinateur ne l’utilise plus : tes amis et tes groupes te joignent sur l’autre.';

  @override
  String moveMovedNoteNamed(String name) {
    return 'Cet ordinateur ne l’utilise plus : les amis et les groupes joignent $name sur l’autre.';
  }

  @override
  String moveMovedTodayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'Aujourd’hui à $time.',
      'other': 'Aujourd’hui à $time.',
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
  String get moveLeftHere => 'Ce qui reste ici';

  @override
  String get moveLeftHereText =>
      'Les conversations, images et fichiers d’avant le transfert, chiffrés. L’autre ordinateur a sa propre copie.';

  @override
  String get moveDeleteData =>
      'Supprimer les données de Fogata sur cet ordinateur';

  @override
  String get moveAfterDeleting =>
      'Après la suppression, Fogata repart de zéro ici.';

  @override
  String get moveCodeNotACode =>
      'Un code contient dix lettres et chiffres, comme XXXXX-XXXXX.';

  @override
  String get moveCodeMistyped =>
      'Ce code n’est pas correct : vérifie les lettres et les chiffres.';

  @override
  String get welcomeHeadline => 'Comme si on était\ndans la même pièce.';

  @override
  String get welcomeIntro =>
      'Voix, vidéo et chat avec tes amis, ta famille ou ton équipe, directement de ton ordinateur au leur.';

  @override
  String get welcomeNoAccount => 'Sans compte';

  @override
  String get welcomeEncrypted => 'Chiffré';

  @override
  String get welcomeEphemeral => 'Salons éphémères';

  @override
  String get welcomeCharacter =>
      'Le personnage de Fogata, une petite flamme avec un casque, qui fait le signe de la paix';

  @override
  String get welcomeImageUnreadable => 'Impossible de lire cette image.';

  @override
  String get welcomeKeychainTitle =>
      'Tes données ne peuvent pas encore être chiffrées';

  @override
  String welcomeKeychainText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' Sous Linux, le trousseau est un Secret Service lancé, comme KWallet ou GNOME Keyring : lances-en ou déverrouilles-en un, puis réessaie.',
      'other': ' Déverrouille ou répare le trousseau, puis réessaie.',
    });
    return 'Fogata chiffre ton identité, tes amis et tes messages privés avec une clé qu’il garde dans le trousseau du système, et le trousseau ne l’a pas gardée.$_temp0';
  }

  @override
  String get welcomeKeychainRetry => 'Réessayer';

  @override
  String get welcomeKeychainPlain => 'Continuer sans chiffrement';

  @override
  String get welcomeKeychainPlainNote =>
      'Sans chiffrement, quiconque copie les fichiers de cet ordinateur peut les lire. Les paramètres continueront de le signaler, et Fogata chiffrera tout la première fois qu’il s’ouvrira avec un trousseau.';

  @override
  String get welcomeWriteName => 'Écris ton nom.';

  @override
  String get welcomeCardTitle => 'Comment veux-tu que tes proches te voient ?';

  @override
  String get welcomeCardSub => 'Un nom et une image. Change-les quand tu veux.';

  @override
  String get welcomeNoServer =>
      'Rien ne passe par un serveur Fogata : il n’y en a pas. Le réseau public IPFS sert seulement d’annuaire pour trouver tes amis.';

  @override
  String get welcomeYourName => 'Ton nom';

  @override
  String welcomePictureLabel(String name) {
    return 'Image : $name';
  }

  @override
  String get welcomeUploadPicture => 'Choisir une image';

  @override
  String get welcomeNameLabel => 'Nom';

  @override
  String get welcomeCreate => 'Créer';

  @override
  String get welcomeReady => 'Prêt';

  @override
  String get welcomeCreateLabel => 'Créer l’identité';

  @override
  String welcomeKeyLabel(String key) {
    return 'Clé créée sur cet ordinateur : $key';
  }

  @override
  String get welcomeNewKey => 'Créer une autre clé';

  @override
  String get stoppedTitle => 'Fogata a cessé de fonctionner.';

  @override
  String get stoppedStartingAgain => 'Redémarrage…';

  @override
  String get stoppedRestartToConnect => 'Redémarre-le pour te reconnecter.';

  @override
  String get stoppedRestartNow => 'Redémarrer';

  @override
  String get settingsSectionProfile => 'Mon profil';

  @override
  String get settingsSectionMove => 'Transférer le compte';

  @override
  String get settingsSectionAppearance => 'Apparence';

  @override
  String get settingsSectionLanguageTime => 'Langue et heure';

  @override
  String get settingsSectionVoice => 'Voix et vidéo';

  @override
  String get settingsSectionSounds => 'Sons';

  @override
  String get settingsSectionBehaviour => 'Comportement';

  @override
  String get settingsSectionNetwork => 'Réseau';

  @override
  String get settingsSectionStorage => 'Stockage et téléchargements';

  @override
  String get settingsHeadingUser => 'Utilisateur';

  @override
  String get settingsHeadingApp => 'Application';

  @override
  String get settingsTitle => 'Paramètres';

  @override
  String get settingsAll => 'Tous les paramètres';

  @override
  String get settingsCloseTooltip => 'Fermer les paramètres';

  @override
  String get settingsUnsavedSaveTitle => 'Enregistrer tes modifications ?';

  @override
  String get settingsUnsavedDiscardTitle => 'Abandonner tes modifications ?';

  @override
  String get settingsUnsavedSaveText =>
      'Ton profil a des modifications qui ne sont pas encore enregistrées.';

  @override
  String get settingsUnsavedDiscardText =>
      'Ton profil a des modifications qui ne peuvent pas être enregistrées telles quelles.';

  @override
  String get settingsProfileSaved =>
      'Profil enregistré. Les amis et les groupes en ligne le voient maintenant, les autres quand ils se connecteront.';

  @override
  String get settingsBioSaved =>
      'Bio enregistrée. Les amis en ligne la voient maintenant, les autres quand ils se connecteront.';

  @override
  String settingsSwitchOn(String what) {
    return '$what : activé';
  }

  @override
  String settingsSwitchOff(String what) {
    return '$what : désactivé';
  }

  @override
  String get settingsOn => 'Activé';

  @override
  String get settingsOff => 'Désactivé';

  @override
  String get settingsIdentity => 'Ton identité';

  @override
  String get settingsIdentityNote =>
      'Ton nom et ton image arrivent signés à tes amis et à tes groupes, ta bio à tes amis.';

  @override
  String get settingsPicture => 'Image';

  @override
  String get settingsUploadPicture => 'Choisir une image ou un GIF';

  @override
  String get settingsName => 'Nom';

  @override
  String get settingsBio => 'Bio';

  @override
  String get settingsBioHint => 'Quelques mots sur toi';

  @override
  String get settingsNoBio => 'Ta carte n’affiche aucune bio.';

  @override
  String settingsBioWhere(int lines) {
    return 'Tes amis la voient sur ta carte, en $lines lignes au plus.';
  }

  @override
  String get settingsBannerTitle => 'Fond du panneau';

  @override
  String get settingsBannerNote => 'Le haut de ta carte, pour toi et tes amis.';

  @override
  String get settingsBannerAnother => 'Autre image';

  @override
  String get settingsBannerUpload => 'Utiliser une image de cet ordinateur';

  @override
  String get settingsFingerprint => 'Empreinte de cet ordinateur';

  @override
  String get settingsTheme => 'Thème';

  @override
  String get settingsThemeNote =>
      'Les couleurs de la fenêtre. Système suit l’ordinateur entre Sombre et Clair.';

  @override
  String get settingsMicSpeaker => 'Micro et haut-parleur';

  @override
  String get settingsMicSpeakerNote =>
      'Ce qui capte ta voix et diffuse le son pendant les appels. Tu peux aussi les changer en plein appel.';

  @override
  String get settingsCamera => 'Caméra';

  @override
  String get settingsCameraNote =>
      'Ce que les appels montrent de toi quand ta caméra est allumée. Essaie-la ici avant un appel.';

  @override
  String get settingsSoundEffects => 'Effets sonores';

  @override
  String get settingsSoundEffectsNote =>
      'Entrée et sortie d’un salon, arrivées et départs, messages du chat, micro et partage.';

  @override
  String get settingsSounds => 'Sons';

  @override
  String settingsChangeFailed(String reason) {
    return 'Fogata n’a pas pu modifier ce réglage : $reason';
  }

  @override
  String get settingsAutostartApprove =>
      'Autorise Fogata dans Réglages Système › Général › Ouverture.';

  @override
  String get settingsAutostartOldMac =>
      'Il faut macOS 13 ou une version plus récente.';

  @override
  String get settingsAutostartUnsupported =>
      'Cette version de Fogata ne peut pas s’ouvrir toute seule.';

  @override
  String get settingsAutostartUnread =>
      'Fogata n’a pas pu lire ce réglage dans ton système.';

  @override
  String get settingsAutostart => 'Ouvrir au démarrage';

  @override
  String get settingsAutostartNote =>
      'Ouvrir Fogata au démarrage de ton ordinateur, une fois ta session ouverte.';

  @override
  String get settingsKeepRunning => 'Rester actif';

  @override
  String get settingsKeepRunningNote =>
      'Quand la fenêtre est fermée, Fogata reste dans la zone de notification : tes amis peuvent toujours t’appeler.';

  @override
  String get settingsNoTray =>
      'Ce bureau n’affiche pas d’icônes de notification : fermer la fenêtre quitte Fogata.';

  @override
  String get settingsHelping => 'Aider les membres';

  @override
  String get settingsHelpingNote =>
      'Comment cet ordinateur aide les membres de tes groupes.';

  @override
  String get settingsHelpRelay => 'Mise en relation et relais';

  @override
  String get settingsHelpConnect => 'Mise en relation seule';

  @override
  String get settingsHelpRelayNote =>
      'Quand deux membres ne peuvent pas se connecter entre eux, cet ordinateur relaie leur trafic chiffré (sans le voir), jusqu’à 2 Go par appel. Ça utilise ton débit montant.';

  @override
  String get settingsHelpConnectNote =>
      'Cet ordinateur aide deux membres à se trouver, mais ne relaie pas leur trafic. S’ils ne peuvent pas se connecter, ils ne s’entendront pas.';

  @override
  String get settingsRestartTray =>
      'Le changement s’applique quand tu quittes Fogata depuis son icône de notification et que tu le rouvres.';

  @override
  String get settingsRestart =>
      'Le changement s’applique quand tu quittes Fogata et que tu le rouvres.';

  @override
  String get settingsNetworkCheck => 'Vérification du réseau';

  @override
  String get settingsNetworkCheckNote =>
      'Comment cet ordinateur voit le réseau et ce que ça change pour joindre tes amis. Rien ne sort d’ici : la vérification se limite à ce que la connexion sait déjà.';

  @override
  String get settingsCheckNetwork => 'Vérifier le réseau';

  @override
  String get settingsCheckAgain => 'Vérifier de nouveau';

  @override
  String get settingsDownloads => 'Téléchargements';

  @override
  String get settingsDownloadsNote =>
      'Où sont enregistrés les fichiers téléchargés depuis un salon.';

  @override
  String settingsVersionData(String version, String path) {
    return 'Fogata $version · Données dans $path';
  }

  @override
  String get settingsNotEncrypted => 'Non chiffré sur cet ordinateur';

  @override
  String settingsNotEncryptedText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' Sous Linux, le trousseau est un Secret Service lancé, comme KWallet ou GNOME Keyring : lances-en ou installes-en un, et Fogata chiffrera ses données à sa prochaine ouverture.',
      'other': ' Fogata réessaiera à sa prochaine ouverture.',
    });
    return 'Ton identité, tes amis et tes messages privés sont stockés sans chiffrement sur cet ordinateur, car aucun trousseau du système n’a été trouvé pour garder la clé.$_temp0';
  }

  @override
  String get settingsNotAllEncrypted => 'Pas tout chiffré sur cet ordinateur';

  @override
  String get settingsNotAllEncryptedText =>
      'Une partie des données de cet ordinateur (ton identité, tes amis et tes messages privés) n’est pas encore chiffrée : Fogata est encore en train de la chiffrer, ou n’a pas pu finir, par exemple parce que le disque est plein. Il continue à chaque ouverture de Fogata.';

  @override
  String settingsDamagedSecret(String file) {
    return '$file ne s’ouvre pas avec la clé de cet ordinateur : il a été endommagé sur le disque. Tant qu’il est là, le chiffrement ne peut pas se terminer, et une copie d’avant le chiffrement reste non chiffrée. Mets-le de côté pour terminer : il est conservé tel quel. S’il s’agit de ton identité ou de tes amis, Fogata reprend cette copie et la chiffre ; le fichier d’un groupe ou d’une conversation est perdu sur cet ordinateur : le groupe devra te faire entrer de nouveau, et les messages précédents de la conversation ne s’afficheront plus.';
  }

  @override
  String get settingsSetDamagedAside => 'Le mettre de côté et redémarrer';

  @override
  String get storageTitle => 'Stockage';

  @override
  String get storageNote =>
      'Tes messages privés n’existent que sur tes appareils. Les chats des salons disparaissent quand le salon se ferme : seuls les messages privés prennent de la place.';

  @override
  String get storageDirect => 'Messages privés';

  @override
  String get storageOfNoLimit => 'sans limite';

  @override
  String storageOfLimit(String limit) {
    return 'sur $limit';
  }

  @override
  String storageLegendPictures(String size) {
    return 'Images $size';
  }

  @override
  String storageLegendVideos(String size) {
    return 'Vidéos $size';
  }

  @override
  String storageLegendFiles(String size) {
    return 'Fichiers $size';
  }

  @override
  String storageLegendMessages(String size) {
    return 'Messages $size';
  }

  @override
  String get storageRoom => 'Espace pour les messages privés';

  @override
  String get storageNoLimitNote => 'Rien n’est supprimé automatiquement.';

  @override
  String storageOverNote(String limit) {
    return 'Limite dépassée : les images, vidéos et fichiers les plus anciens sont supprimés, jusqu’à revenir à $limit. Les messages restent toujours.';
  }

  @override
  String get storageLimitNote =>
      'Au-delà de la limite, les images, vidéos et fichiers les plus anciens partent en premier. Les messages restent toujours ; à leur place, on lit « Supprimé ici pour libérer de l’espace ».';

  @override
  String get storageAutoPictures => 'Télécharger les images automatiquement';

  @override
  String get storageAutoPicturesNote =>
      'Jusqu’à 10 Mo chacune, dès leur arrivée.';

  @override
  String get storageAutoFiles =>
      'Télécharger les vidéos et fichiers automatiquement';

  @override
  String get storageAutoFilesNote =>
      'Désactivé : ils arrivent quand tu les ouvres, si l’expéditeur est en ligne.';

  @override
  String get storageTimer =>
      'Messages éphémères dans les nouvelles conversations';

  @override
  String get storageTimerNote =>
      'Chaque conversation peut le changer ; les deux personnes voient toujours le même minuteur.';

  @override
  String get storageTimer24h => '24 h';

  @override
  String get storageByChat => 'Par conversation';

  @override
  String get storageByChatNote =>
      'Vider supprime ici les images, vidéos et fichiers. Les messages restent, et l’autre personne garde sa copie.';

  @override
  String get storageMessagesOnly => 'Messages seulement';

  @override
  String get storageClearFiles => 'Vider les fichiers';

  @override
  String storageClearFilesTooltip(String name) {
    return 'Vider les fichiers de la conversation avec $name';
  }

  @override
  String get storageCleared => 'Vidé';

  @override
  String storageConfirmTitle(String limit) {
    return 'Garder $limit d’images, vidéos et fichiers ?';
  }

  @override
  String storageConfirmText(String limit) {
    return 'Les plus anciens sont supprimés maintenant, jusqu’à revenir à $limit, ceux des messages épinglés en dernier. Les messages restent toujours, et tes amis gardent leurs copies.';
  }

  @override
  String get storageRemoveOldest => 'Supprimer les plus anciens';

  @override
  String get themePickerSystem =>
      'Thème système, selon le réglage de l’ordinateur';

  @override
  String themePickerTheme(String name) {
    return 'Thème $name';
  }

  @override
  String get themeDark => 'Sombre';

  @override
  String get themeMidnight => 'Minuit';

  @override
  String get themeDim => 'Tamisé';

  @override
  String get themeEmber => 'Braise';

  @override
  String get themeContrast => 'Contraste';

  @override
  String get themeLight => 'Clair';

  @override
  String get themeSystem => 'Système';

  @override
  String get themeBannerViolet => 'Violet';

  @override
  String get themeBannerBlue => 'Bleu';

  @override
  String get themeBannerGreen => 'Vert';

  @override
  String get themeBannerClay => 'Argile';

  @override
  String get themeBannerRaspberry => 'Framboise';

  @override
  String get themeBannerMustard => 'Moutarde';

  @override
  String get themeBannerSlate => 'Ardoise';

  @override
  String get themeBannerNight => 'Nuit';

  @override
  String widgetsCopyWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Copier le code QR',
      'link': 'Copier le lien',
      'code': 'Copier le code',
      'other': 'Copier',
    });
    return '$_temp0';
  }

  @override
  String widgetsShowWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Afficher le code QR',
      'link': 'Afficher le lien',
      'code': 'Afficher le code',
      'other': 'Afficher',
    });
    return '$_temp0';
  }

  @override
  String get widgetsHide => 'Masquer';

  @override
  String revealFailed(String path) {
    return 'Impossible d’ouvrir son dossier. Il se trouve dans $path';
  }

  @override
  String get emojiGroupPeople => 'Personnes';

  @override
  String get emojiGroupNature => 'Nature';

  @override
  String get emojiGroupFood => 'Nourriture et boissons';

  @override
  String get emojiGroupActivities => 'Activités';

  @override
  String get emojiGroupTravel => 'Voyages et lieux';

  @override
  String get emojiGroupObjects => 'Objets';

  @override
  String get emojiGroupSymbols => 'Symboles';

  @override
  String get emojiGroupFlags => 'Drapeaux';

  @override
  String get emojiPickerLimit => 'Le message a atteint sa limite.';

  @override
  String get emojiPickerButton => 'Emoji';

  @override
  String get emojiPickerResults => 'Résultats';

  @override
  String get emojiPickerRecent => 'Utilisés récemment';

  @override
  String get emojiPickerSearch => 'Rechercher un emoji';

  @override
  String get emojiPickerSkinTone => 'Teint';

  @override
  String get emojiPickerNone => 'Aucun emoji trouvé';

  @override
  String get emojiPickerNoFont =>
      'Cet ordinateur n’a pas de police d’emoji en couleur : les emoji peuvent s’afficher en noir et blanc ou vides. Installer Noto Color Emoji règle le problème.';

  @override
  String get emojiPickerDefaultTone => 'Teint par défaut';

  @override
  String emojiPickerTone(int n) {
    return 'Teint $n';
  }

  @override
  String get iconsGroupMountain => 'Montagne';

  @override
  String get iconsGroupStar => 'Étoile';

  @override
  String get iconsGroupGameController => 'Manette';

  @override
  String get iconsGroupMusic => 'Musique';

  @override
  String get iconsGroupBook => 'Livre';

  @override
  String get iconsGroupWaves => 'Vagues';

  @override
  String get iconsGroupCoffee => 'Café';

  @override
  String get iconsGroupHeart => 'Cœur';

  @override
  String get iconsGroupFilm => 'Film';

  @override
  String get newGroupPictureUnreadable => 'Impossible de lire cette image.';

  @override
  String newGroupMade(String name) {
    return 'Groupe « $name » créé. Ses membres le reçoivent quand ils se connectent.';
  }

  @override
  String get newGroupTitle => 'Nouveau groupe';

  @override
  String get newGroupSubtitle => 'Le groupe est permanent. Ses salons, non.';

  @override
  String get newGroupName => 'Nom du groupe';

  @override
  String get newGroupNameExample => 'Vélo du dimanche';

  @override
  String get newGroupIconOrPicture => 'Icône ou image';

  @override
  String get newGroupChooseFromComputer => 'Choisir sur l’ordinateur';

  @override
  String get newGroupBannerOptional => 'Bannière (facultatif)';

  @override
  String get newGroupMembers => 'Membres';

  @override
  String newGroupSelected(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n sélectionnés',
      one: '$n sélectionné',
    );
    return '$_temp0';
  }

  @override
  String get newGroupSearchPlaceholder => 'Rechercher parmi tes amis';

  @override
  String get newGroupSearchLabel => 'Rechercher des amis';

  @override
  String get newGroupNoFriends =>
      'Pas encore d’amis. Tu peux créer le groupe et les ajouter plus tard, ou inviter des gens avec un lien.';

  @override
  String get newGroupKeptOnDevices => 'Gardé sur l’appareil de chaque membre.';

  @override
  String get newGroupCreate => 'Créer le groupe';

  @override
  String get groupBannerSettings => 'Paramètres du groupe';

  @override
  String get groupBannerPictureUnreadable => 'Impossible de lire cette image.';

  @override
  String get groupBannerChoose => 'Choisir une bannière';

  @override
  String get groupBannerAnother => 'Autre bannière';

  @override
  String get groupBannerNone => 'Sans bannière';

  @override
  String get groupBannerNote =>
      'Affichée en haut de la colonne du groupe, sous son nom. PNG, JPEG, ou un GIF ou WebP animé jusqu’à 550 Ko.';

  @override
  String get homeTitle => 'Accueil';

  @override
  String get homeSubtitle =>
      'Les salons s’allument quand quelqu’un entre et s’éteignent quand tout le monde part.';

  @override
  String get homeAddFriend => 'Ajouter un ami';

  @override
  String get homeNewGroup => 'Nouveau groupe';

  @override
  String get homeLive => 'En direct';

  @override
  String get homeNoRoom => 'Aucun salon';

  @override
  String get homeNoRoomNote => 'Ouvre-en un pour l’allumer';

  @override
  String homeInviteBy(String name, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n membres',
      one: '$n membre',
    );
    return '$name t’a ajouté · $_temp0';
  }

  @override
  String get homeInviteNote =>
      'Tant que tu n’as pas rejoint le groupe, cet ordinateur ne se connecte pas à ses membres et ne leur envoie rien.';

  @override
  String get homeInviteJoin => 'Rejoindre';

  @override
  String get homeNoGroups => 'Pas encore de groupes';

  @override
  String get homeNoGroupsNote =>
      'Ajoute tes amis et crée un groupe avec eux. Le groupe est permanent ; ses salons, non.';

  @override
  String homePersonYou(String name) {
    return '$name (toi)';
  }

  @override
  String homeMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n membres',
      one: '$n membre',
    );
    return '$_temp0';
  }

  @override
  String get homeYouAreIn => 'Tu y es';

  @override
  String get homeMuteMic => 'Couper le micro';

  @override
  String get homeUnmuteMic => 'Réactiver le micro';

  @override
  String get homeCameraOff => 'Éteindre la caméra';

  @override
  String get homeCameraOn => 'Allumer la caméra';

  @override
  String get homeMembersTooltip => 'Membres';

  @override
  String get homeReturnToRoom => 'Revenir au salon';

  @override
  String get homeJoinRoom => 'Rejoindre';

  @override
  String homeMembersOnline(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membres',
      one: '$members membre',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online en ligne',
      zero: 'personne d’autre en ligne',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get homeNoRoomsYet => 'Pas encore de salon';

  @override
  String homeLastRoom(String when) {
    return 'Dernier salon $when';
  }

  @override
  String get homeOpenRoom => 'Ouvrir le salon';

  @override
  String get membersGroupGone => 'Ce groupe n’est plus là.';

  @override
  String membersSummary(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membres',
      one: '$members membre',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online en ligne',
      zero: 'personne d’autre en ligne',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String membersSummaryLastRoom(int members, int online, String when) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membres',
      one: '$members membre',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online en ligne',
      zero: 'personne d’autre en ligne',
    );
    return '$_temp0 · $_temp1 · dernier salon $when';
  }

  @override
  String get membersCopyInviteLink => 'Copier le lien d’invitation';

  @override
  String membersInviteLinkTooltip(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'quelqu’un du groupe',
      'first': 'le premier du groupe',
      'other': 'l’un des $n premiers du groupe',
    });
    return 'Fonctionne une fois, pendant 24 heures, et $_temp0 fait entrer la personne';
  }

  @override
  String membersInviteLinkCopied(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'quelqu’un du groupe',
      'first': 'le premier du groupe',
      'other': 'l’un des $n premiers du groupe',
    });
    return 'Lien d’invitation copié. Il fonctionne une fois, pendant 24 heures, et $_temp0 fait entrer la personne.';
  }

  @override
  String get membersInviteFriends => 'Inviter des amis';

  @override
  String get membersBackToRoom => 'Revenir au salon';

  @override
  String get membersOpenRoom => 'Ouvrir le salon';

  @override
  String get membersEnterRoom => 'Entrer dans le salon';

  @override
  String get membersTitle => 'Membres';

  @override
  String get membersOrderNote =>
      'Dans l’ordre : chacun agit sur ceux en dessous';

  @override
  String get membersGroupSettings => 'Paramètres du groupe';

  @override
  String membersManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Seuls les $n premiers peuvent modifier le groupe.',
      one: 'Seul le premier peut modifier le groupe.',
      zero: 'Seuls les $n premiers peuvent modifier le groupe.',
    );
    return '$_temp0';
  }

  @override
  String get membersLeaveGroup => 'Quitter le groupe';

  @override
  String membersInviteTitle(String group) {
    return 'Inviter des amis dans « $group »';
  }

  @override
  String get membersInviteNote =>
      'Ils reçoivent une invitation et choisissent de rejoindre ou non.';

  @override
  String get membersPresenceYou => 'toi';

  @override
  String membersPresenceOnline(String how) {
    return 'en ligne · $how';
  }

  @override
  String membersPresenceAway(String how) {
    return 'absent · $how';
  }

  @override
  String get membersPresenceOffline => 'pas en ligne';

  @override
  String membersYou(String name) {
    return '$name (toi)';
  }

  @override
  String get membersWaitingForKey => 'en attente de la clé';

  @override
  String membersFingerprint(String fingerprint) {
    return 'Empreinte $fingerprint';
  }

  @override
  String membersOptionsFor(String name) {
    return 'Options pour $name';
  }

  @override
  String get membersMoveUp => 'Placer juste en dessous de moi';

  @override
  String get membersMoveToEnd => 'Placer à la fin';

  @override
  String get membersTimeOut => 'Exclure temporairement d’un salon…';

  @override
  String get membersRemoveFromGroup => 'Retirer du groupe';

  @override
  String membersRemoveTitle(String name) {
    return 'Retirer $name ?';
  }

  @override
  String membersRemoveText(String name) {
    return '$name quitte le groupe et le groupe crée une nouvelle clé. Ce que cette personne a déjà vu, elle le garde.';
  }

  @override
  String get membersFirst => 'Premier rang';

  @override
  String membersTimeOutTitle(String name) {
    return 'Exclure temporairement $name';
  }

  @override
  String membersTimeOutText(String name) {
    return '$name quitte le salon et ne peut pas revenir avant la fin du délai, sauf si quelqu’un au-dessus de lui le laisse revenir.';
  }

  @override
  String get membersUntilLetBack => 'Jusqu’à nouvel ordre';

  @override
  String membersLeaveTitle(String group) {
    return 'Quitter « $group » ?';
  }

  @override
  String get membersLeaveText =>
      'Tu ne vois plus ses salons et les autres ne te voient plus dans le groupe. Le groupe crée une nouvelle clé sans toi. Pour revenir, il faudra que quelqu’un du groupe te fasse entrer de nouveau.';

  @override
  String membersLeaveTextFirst(String name) {
    return 'Tu ne vois plus ses salons et les autres ne te voient plus dans le groupe. Le groupe crée une nouvelle clé sans toi. Pour revenir, il faudra que quelqu’un du groupe te fasse entrer de nouveau. $name passe au premier rang.';
  }

  @override
  String get groupSettingsTabGeneral => 'Général';

  @override
  String get groupSettingsTabVoiceRooms => 'Salons vocaux';

  @override
  String get groupSettingsTabMembers => 'Membres';

  @override
  String get groupSettingsTabInvite => 'Inviter';

  @override
  String get groupSettingsTabLimits => 'Limites';

  @override
  String get groupSettingsTabActivity => 'Activité';

  @override
  String get groupSettingsFirstHere => 'Au premier rang de ce groupe';

  @override
  String groupSettingsPlaceHere(String place) {
    return 'Au $place de ce groupe';
  }

  @override
  String get groupSettingsLeave => 'Quitter le groupe';

  @override
  String get groupSettingsInviteTitle => 'Inviter des personnes';

  @override
  String get groupSettingsClose => 'Fermer les paramètres du groupe';

  @override
  String get groupSettingsPicture => 'Image';

  @override
  String get groupSettingsChooseFromComputer => 'Choisir sur l’ordinateur';

  @override
  String get groupSettingsUseIcon => 'Utiliser une icône';

  @override
  String get groupSettingsPictureNote =>
      'PNG, JPEG ou WebP, recadrée en carré et réduite comme les photos de profil, ou un GIF ou WebP animé jusqu’à 550 Ko.';

  @override
  String get groupSettingsOrIcon => 'Ou une icône';

  @override
  String get groupSettingsBanner => 'Bannière';

  @override
  String get groupSettingsName => 'Nom';

  @override
  String get groupSettingsNameField => 'Nom du groupe';

  @override
  String get groupSettingsNameNote =>
      'Tout le monde dans le groupe voit le nouveau nom et la nouvelle image.';

  @override
  String get groupSettingsRoomsIntro =>
      'Chaque salon vocal a ses propres participants, son chat, ses fichiers et ses appels : ceux d’un salon ne se connectent pas avec ceux d’un autre. On ne peut être que dans un salon à la fois.';

  @override
  String get groupSettingsRoomName => 'Nom du salon';

  @override
  String get groupSettingsAccessEveryone => 'Tout le monde';

  @override
  String get groupSettingsAccessSome => 'Certains';

  @override
  String groupSettingsWhoCanEnter(String room) {
    return 'Qui peut entrer dans $room';
  }

  @override
  String get groupSettingsFirstRoomStays => 'Le premier salon reste';

  @override
  String get groupSettingsPeopleInside => 'Il y a du monde dedans';

  @override
  String get groupSettingsDeleteRoom => 'Supprimer le salon';

  @override
  String get groupSettingsAddRoom => 'Ajouter un salon vocal';

  @override
  String groupSettingsNewRoomName(int n) {
    return 'Salon $n';
  }

  @override
  String groupSettingsRoomsNote(int rooms, int people) {
    return 'Jusqu’à $rooms salons, et $people personnes dans chacun (Limites). Un salon où il y a du monde ne peut pas être supprimé.';
  }

  @override
  String get groupSettingsKeptOut => 'Exclus';

  @override
  String get groupSettingsAccessEveryoneLong => 'Tout le monde dans le groupe';

  @override
  String get groupSettingsAccessChosen =>
      'Seulement les membres que je choisis';

  @override
  String get groupSettingsAccessNote =>
      'Toi et ceux au-dessus de toi pouvez toujours entrer. Les membres non choisis voient le salon avec un cadenas.';

  @override
  String groupSettingsYou(String name) {
    return '$name (toi)';
  }

  @override
  String groupSettingsKeptOutBy(String room, String by) {
    return ' · $room · par $by';
  }

  @override
  String groupSettingsTimeLeft(String time) {
    return 'encore $time';
  }

  @override
  String get groupSettingsUntilLetBack => 'Jusqu’à nouvel ordre';

  @override
  String get groupSettingsLetBack => 'Réintégrer';

  @override
  String groupSettingsManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Seuls les $n premiers peuvent modifier le groupe : son nom, ses salons, ses invitations et ses limites. Tu agis sur ceux en dessous de toi.',
      one: 'Seul le premier peut modifier le groupe : son nom, ses salons, ses invitations et ses limites. Tu agis sur ceux en dessous de toi.',
      zero:
          'Seuls les $n premiers peuvent modifier le groupe : son nom, ses salons, ses invitations et ses limites. Tu agis sur ceux en dessous de toi.',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsMembersCount(int n, int max) {
    String _temp0 = intl.Intl.pluralLogic(
      max,
      locale: localeName,
      other: '$max membres',
      one: '$max membre',
    );
    return '$n sur $_temp0, dans l’ordre';
  }

  @override
  String get groupSettingsMembersNote =>
      'Chacun agit sur les personnes en dessous de lui : il peut les faire monter juste en dessous de lui ou les placer à la fin, les exclure temporairement d’un salon, ou les retirer. Les nouveaux venus commencent à la fin.';

  @override
  String get groupSettingsWhoManages => 'Qui gère ce groupe';

  @override
  String get groupSettingsManagersEveryone => 'Tout le monde';

  @override
  String groupSettingsManagersFirstN(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Les $n premiers',
      one: 'Le premier',
      zero: 'Les $n premiers',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsManagersMore(int n, int members) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membres',
      one: '$members membre',
    );
    return 'Réglé sur les $n premiers, plus que les $_temp0 actuels du groupe : pour l’instant, c’est tout le monde.';
  }

  @override
  String get groupSettingsManagersEveryoneNote =>
      'Ceux qui gèrent le groupe changent le nom, les images, les salons et les limites, et font entrer les gens. Avec tout le monde, le groupe continue même si quelqu’un perd son appareil.';

  @override
  String groupSettingsManagersFirstNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Seuls les $n premiers changent le nom, les images, les salons et les limites, et font entrer les gens. S’ils perdent tous leurs appareils, plus personne ne pourra gérer le groupe : Fogata n’a pas de serveur pour y remédier.',
      one: 'Seul le premier change le nom, les images, les salons et les limites, et fait entrer les gens. Si cet appareil est perdu, plus personne ne pourra gérer le groupe : Fogata n’a pas de serveur pour y remédier.',
      zero:
          'Seuls les $n premiers changent le nom, les images, les salons et les limites, et font entrer les gens. S’ils perdent tous leurs appareils, plus personne ne pourra gérer le groupe : Fogata n’a pas de serveur pour y remédier.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsMoveUp => 'Faire monter';

  @override
  String groupSettingsMoveUpTooltip(String name) {
    return 'Faire monter $name juste en dessous de toi';
  }

  @override
  String get groupSettingsToEnd => 'À la fin';

  @override
  String groupSettingsToEndTooltip(String name) {
    return 'Placer $name à la fin';
  }

  @override
  String get groupSettingsTimeOut => 'Exclure temporairement d’un salon';

  @override
  String get groupSettingsRemove => 'Retirer du groupe';

  @override
  String get groupSettingsAskingToJoin => 'Demandes d’accès';

  @override
  String get groupSettingsInviteFriend => 'Inviter un ami';

  @override
  String get groupSettingsInviteFriendNote =>
      'Ils reçoivent une invitation dans Fogata et choisissent de rejoindre ou non. Seuls tes amis s’affichent ici.';

  @override
  String get groupSettingsInviteLink => 'Lien d’invitation';

  @override
  String groupSettingsInviteLinkNote(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'quelqu’un du groupe',
      'first': 'le premier du groupe',
      'other': 'l’un des $n premiers du groupe',
    });
    return 'Pour quelqu’un qui n’est pas encore ton ami. Il fonctionne une fois et pendant 24 heures, et $_temp0 fait entrer la personne : s’il arrive chez quelqu’un d’autre, tu vois qui demande et tu peux refuser.';
  }

  @override
  String get groupSettingsCopyLink => 'Copier le lien d’invitation';

  @override
  String groupSettingsOpenLinks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n liens pas encore utilisés.',
      one: '1 lien pas encore utilisé.',
      zero: 'Aucun lien en attente d’utilisation.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsCancelLinks => 'Les annuler';

  @override
  String get groupSettingsNoFriends =>
      'Pas encore d’amis : ajoute-les dans Amis.';

  @override
  String get groupSettingsAllFriendsIn => 'Tous tes amis sont dans le groupe.';

  @override
  String get groupSettingsFriendOnline => 'En ligne';

  @override
  String get groupSettingsFriendOffline => 'Pas en ligne';

  @override
  String get groupSettingsInviteButton => 'Inviter';

  @override
  String get groupSettingsGroupFull => 'Le groupe est complet';

  @override
  String get groupSettingsSayNo => 'Refuser';

  @override
  String get groupSettingsLetIn => 'Faire entrer';

  @override
  String get groupSettingsLimitGroup => 'Personnes dans le groupe';

  @override
  String groupSettingsLimitGroupNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n personnes déjà présentes',
      one: '$n personne déjà présente',
    );
    return 'De 2 à 20. Quand il est complet, les invitations et les liens ne fonctionnent plus. La limite ne peut pas être inférieure aux $_temp0.';
  }

  @override
  String get groupSettingsLimitRoom => 'Personnes dans chaque salon vocal';

  @override
  String get groupSettingsLimitRoomNote =>
      'De 2 à 8. La voix et la vidéo passent directement entre tous ceux d’un salon : 8 est le maximum pour un bon son ; avec les caméras allumées, mieux vaut être moins nombreux. Si quelqu’un arrive quand le salon est complet, il voit qu’il est complet.';

  @override
  String get shellChipCallMany =>
      'Direct · chiffré · seulement ceux de l’appel';

  @override
  String get shellChipCallTwo => 'Direct · chiffré · seulement vous deux';

  @override
  String get shellGroupMembers => 'Membres du groupe';

  @override
  String get shellMembers => 'Membres';

  @override
  String get shellDirectMessages => 'Messages privés';

  @override
  String get shellPathDirect => 'Direct · chiffré';

  @override
  String shellPathVia(String name) {
    return 'Via $name · chiffré';
  }

  @override
  String get shellPathViaSomeone => 'Via un membre · chiffré';

  @override
  String shellCallWith(String names) {
    return 'Appel avec $names';
  }

  @override
  String get shellRinging => 'Ça sonne · ';

  @override
  String get shellInCall => 'En appel · ';

  @override
  String get shellCallEnded => 'Appel terminé';

  @override
  String get shellNewGroup => 'Nouveau groupe';

  @override
  String get shellSectionCall => 'Appel';

  @override
  String shellCallRinging(String name) {
    return '$name · ça sonne';
  }

  @override
  String get shellSectionLiveNow => 'En direct';

  @override
  String get shellSectionInCall => 'Dans l’appel';

  @override
  String get shellChat => 'Chat';

  @override
  String get shellFiles => 'Fichiers';

  @override
  String get shellSectionGroup => 'Groupe';

  @override
  String get shellInvite => 'Inviter';

  @override
  String get shellSectionVoiceRooms => 'Salons vocaux';

  @override
  String shellVoiceRoomYouAreIn(String room) {
    return '$room · tu y es';
  }

  @override
  String get shellNotForYou => 'Pas pour toi';

  @override
  String get shellOnlySomeMembers => 'Certains membres seulement';

  @override
  String shellPersonYou(String name) {
    return '$name (toi)';
  }

  @override
  String get shellLive => 'EN DIRECT';

  @override
  String get shellYourProfile => 'Ton profil';

  @override
  String shellMuteMic(String keys) {
    return 'Couper le micro ($keys)';
  }

  @override
  String shellUnmuteMic(String keys) {
    return 'Réactiver le micro ($keys)';
  }

  @override
  String get shellDeafen => 'Ne plus entendre les autres';

  @override
  String get shellUndeafen => 'Réentendre les autres';

  @override
  String get shellStateCalling => 'Appel…';

  @override
  String get shellStateInCall => 'En appel';

  @override
  String get shellStateInRoom => 'Dans le salon';

  @override
  String get shellStateOffline => 'Hors ligne';

  @override
  String get shellStateOnline => 'En ligne';

  @override
  String get shellStateAway => 'Absent';

  @override
  String get shellShowInFolder => 'Afficher dans le dossier';

  @override
  String get shellCopyPath => 'Copier le chemin';

  @override
  String shellVolumePercent(int n) {
    return '$n %';
  }

  @override
  String get shellMuteKeys => 'Ctrl+Maj+M';

  @override
  String get settingsSectionDonate => 'Faire un don à Fogata';

  @override
  String get settingsSectionReport => 'Signaler un problème';

  @override
  String get settingsHeadingHelp => 'Aide';

  @override
  String get settingsPreRelease => 'Préversion';

  @override
  String get donateTitle => 'Garde Fogata indépendant';

  @override
  String get donateText =>
      'Fogata est gratuit, sans publicité et sans serveurs qui te surveillent. Ton don lui permet de le rester.';

  @override
  String get donateButton => 'Faire un don';

  @override
  String get donateNote =>
      'Ouvre Ko-fi dans ton navigateur. Fogata ne voit jamais ton paiement.';

  @override
  String get reportWhatTitle => 'Que s’est-il passé ?';

  @override
  String get reportWhatNote =>
      'Ce que tu faisais, ce que tu attendais et ce que tu as vu à la place.';

  @override
  String get reportIncludeLog =>
      'Inclure le journal technique des 2 derniers jours';

  @override
  String get reportLogNote =>
      'Versions, erreurs et étapes de connexion. Jamais tes messages, clés, contacts ou adresses IP.';

  @override
  String get reportSeeSent => 'Voir ce qui est envoyé';

  @override
  String get reportHideSent => 'Masquer la liste';

  @override
  String get reportPreview => 'Lire tout le rapport avant de l’envoyer';

  @override
  String get reportHidePreview => 'Masquer le rapport';

  @override
  String get reportPreviewNote =>
      'C’est exactement ce que contient le fichier, et tout ce qui part. Les noms, dossiers, noms de fichiers, adresses et clés sont déjà retirés du journal.';

  @override
  String get reportPreviewWorking => 'Préparation du rapport…';

  @override
  String get reportSentWhat => 'Ce que tu as écrit ci-dessus.';

  @override
  String get reportSentVersion =>
      'La version et le build de Fogata, ton système et sa version, et la langue de Fogata.';

  @override
  String get reportSentLog =>
      'Le journal des 2 derniers jours : quand Fogata a démarré, ses erreurs et comment il a joint d’autres appareils. Les noms, messages, dossiers, noms de fichiers, liens, codes, clés et adresses IP en sont retirés, et chaque appareil n’apparaît que sous un numéro qui ne veut rien dire hors du rapport.';

  @override
  String get reportEmail => 'L’envoyer par e-mail à l’équipe Fogata';

  @override
  String get reportSave => 'Enregistrer dans un fichier';

  @override
  String reportHowEmail(String address) {
    return 'Ton app de messagerie s’ouvre avec le message prêt pour $address, et le dossier du fichier de rapport s’ouvre à côté : joins le fichier et envoie. Rien ne quitte ton ordinateur avant que tu le fasses.';
  }

  @override
  String get reportReadyTitle => 'Ton e-mail est prêt';

  @override
  String reportReadyText(String file, String address) {
    return 'Joins $file depuis le dossier qui s’est ouvert et envoie-le à $address.';
  }

  @override
  String get reportShowFile => 'Afficher le fichier';

  @override
  String get reportCopyAddress => 'Copier l’adresse';

  @override
  String get reportAddressCopied => 'Adresse copiée';

  @override
  String reportSaved(String file) {
    return 'Rapport enregistré : $file';
  }

  @override
  String reportNoMailApp(String address) {
    return 'Aucune app de messagerie ne s’est ouverte. Envoie le fichier à $address depuis ta messagerie.';
  }

  @override
  String reportFailed(String reason) {
    return 'Impossible d’écrire le rapport : $reason';
  }

  @override
  String get splashStarting => 'Démarrage';

  @override
  String get splashConnecting => 'Connexion';

  @override
  String get splashChecking => 'Recherche de mises à jour';

  @override
  String get splashDownloading => 'Téléchargement de la mise à jour';

  @override
  String get splashInstalling => 'Installation de la mise à jour';

  @override
  String get splashRestarting => 'Redémarrage';

  @override
  String splashProgress(String done, String total) {
    return '$done sur $total Mo';
  }

  @override
  String splashProgressFrom(String done, String total, String name) {
    return '$done sur $total Mo · de $name';
  }

  @override
  String updateReady(String version) {
    return 'Mise à jour prête · $version';
  }

  @override
  String updateRequired(String version) {
    return 'Mise à jour requise · $version';
  }

  @override
  String get updateRestart => 'Redémarrer';

  @override
  String updateReadyTip(String version) {
    return 'Fogata $version est disponible. Redémarre pour l’utiliser.';
  }

  @override
  String get updateRequiredTip =>
      'Cette version ne fonctionne plus avec la plus récente de tes amis. Redémarre pour mettre à jour.';

  @override
  String updateFailed(String reason) {
    return 'Impossible d’installer la mise à jour : $reason';
  }

  @override
  String get errJoinInviteGone =>
      'Cette invitation ne fonctionne plus, ou le groupe est complet.';

  @override
  String get errJoinNoKeyYet =>
      'Cet appareil n’a pas encore la clé du groupe. Le nouveau membre entrera dès qu’il l’aura.';

  @override
  String get errJoinWroteTooMuch =>
      'Cet appareil a fait des modifications que les autres membres n’ont pas encore reçues. Le nouveau membre pourra entrer quand un autre membre sera en ligne.';

  @override
  String get errJoinAskAgain =>
      'Cette demande pour rejoindre le groupe a été faite sur un état antérieur du groupe. La personne doit redemander.';

  @override
  String get errNoSpaceForFile =>
      'Il n’y a pas assez d’espace libre pour ce fichier. Libère de l’espace et réessaie.';

  @override
  String get recoveryTitle => 'Copie de secours';

  @override
  String get recoveryNote =>
      'Un fichier qui permet de récupérer ce compte si le trousseau ou le disque de cet ordinateur est perdu. Seule ta clé de secours l’ouvre.';

  @override
  String get recoveryOff => 'Désactivée';

  @override
  String recoveryOn(String id) {
    return 'Activée · clé $id';
  }

  @override
  String get recoveryTurnOn => 'Activer…';

  @override
  String get recoveryChangeKey => 'Changer de clé…';

  @override
  String get recoveryMakeCopy => 'Faire une copie…';

  @override
  String get recoveryReplacedKept =>
      'Cet ordinateur garde encore un compte qu’une restauration a remplacé.';

  @override
  String get recoveryDeleteReplaced => 'Le supprimer…';

  @override
  String get recoveryChangeTitle => 'Créer une nouvelle clé de secours ?';

  @override
  String get recoveryChangeText =>
      'Les nouvelles copies ne s’ouvriront qu’avec la nouvelle clé. Celles faites avant ne s’ouvrent toujours qu’avec l’ancienne : garde-la tant que tu gardes ces copies.';

  @override
  String get recoveryChangeConfirm => 'Créer une nouvelle clé';

  @override
  String get recoveryKeyTitle => 'Ta clé de secours';

  @override
  String get recoveryKeyText =>
      'Note-la ou imprime-la, et garde-la à part de tes copies. Elle n’est montrée que cette fois.';

  @override
  String recoveryKeyName(String id) {
    return 'Son nom court : $id';
  }

  @override
  String get recoveryKeyWarning =>
      'Fogata ne garde cette clé nulle part : sans elle, aucune copie ne s’ouvre. Quiconque a une copie et cette clé a ton compte.';

  @override
  String get recoveryKeyWritten => 'Je l’ai notée';

  @override
  String get recoveryCheckTitle => 'Vérifie ce que tu as noté';

  @override
  String get recoveryCheckText =>
      'Tape ces deux groupes de la clé, comme tu les as notés.';

  @override
  String recoveryCheckGroup(int n) {
    return 'Groupe $n';
  }

  @override
  String get recoveryCheckWrong => 'Ce n’est pas ça : relis ce que tu as noté.';

  @override
  String get recoveryTurnOnButton => 'Activer les copies';

  @override
  String recoveryTurnedOn(String id) {
    return 'Les copies sont activées, avec la clé $id.';
  }

  @override
  String get recoveryCopyTitle => 'Faire une copie de secours';

  @override
  String get recoveryCopyText =>
      'Fogata est hors ligne pendant la copie, plus longtemps avec les fichiers des conversations. Garde le fichier loin de cet ordinateur, sur une clé USB par exemple.';

  @override
  String get recoveryCopyAttachments =>
      'Inclure les fichiers envoyés dans les conversations';

  @override
  String recoveryCopyAttachmentsNote(String size) {
    return 'Images, vidéos et fichiers : $size de plus.';
  }

  @override
  String get recoveryCopySave => 'Choisir où…';

  @override
  String get recoveryCopyInDataFolder =>
      'Une copie ne peut pas être gardée dans le dossier de données de Fogata. Choisis un autre endroit.';

  @override
  String recoveryCopyMade(String file, String size) {
    return 'Copie enregistrée : $file ($size).';
  }

  @override
  String recoveryCopyFailed(String why) {
    return 'La copie n’a pas pu être faite. $why';
  }

  @override
  String get recoveryDeleteTitle => 'Supprimer le compte remplacé ?';

  @override
  String get recoveryDeleteText =>
      'Il a été mis de côté quand une copie a été restaurée à sa place. Une fois supprimé, il ne peut plus être récupéré.';

  @override
  String get recoveryDelete => 'Supprimer';

  @override
  String get recoveryDeleted => 'Le compte remplacé a été supprimé.';

  @override
  String get recoveryFileType => 'Copie Fogata';

  @override
  String get recoveryLostComputer => 'Tu as perdu ton ordinateur ?';

  @override
  String get recoveryRestoreLink => 'Restaurer depuis une copie';

  @override
  String get recoveryRestoreTitle => 'Restaurer depuis une copie';

  @override
  String get recoveryRestoreSub =>
      'Choisis la copie de secours que tu as gardée, puis tape ta clé de secours.';

  @override
  String get recoveryChooseCopy => 'Choisir la copie…';

  @override
  String recoveryMadeOn(String date) {
    return 'Une copie faite le $date.';
  }

  @override
  String get recoveryOtherKey =>
      'Elle a été faite pour une autre clé de secours que celle de cet ordinateur. Tape la clé pour laquelle elle a été faite.';

  @override
  String get recoveryKeyField => 'Clé de secours';

  @override
  String get recoveryKeyNotAKey =>
      'Une clé de secours a 53 lettres et chiffres, par groupes de quatre.';

  @override
  String get recoveryKeyMistyped =>
      'Une lettre ou un chiffre est faux : vérifie-les.';

  @override
  String get recoveryProblemNotACopy =>
      'Ce n’est pas une copie d’un compte Fogata.';

  @override
  String get recoveryProblemNewer =>
      'Cette copie a été faite par une version plus récente de Fogata. Mets Fogata à jour pour l’ouvrir.';

  @override
  String get recoveryProblemOtherKey =>
      'Cette copie a été faite pour une autre clé de secours.';

  @override
  String get recoveryProblemDamaged =>
      'Cette copie est abîmée, ou a été modifiée après avoir été faite.';

  @override
  String get recoveryProblemUnproven =>
      'Cette copie n’a pas été faite par le compte qu’elle contient : elle ne sera pas ouverte.';

  @override
  String recoveryWhose(String name) {
    return 'Le compte de $name';
  }

  @override
  String recoveryHolds(int files, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files fichiers',
      one: '$files fichier',
    );
    return '$_temp0, $size en tout';
  }

  @override
  String get recoveryWithAttachments =>
      'Avec les fichiers envoyés dans les conversations.';

  @override
  String get recoveryWithoutAttachments =>
      'Sans les fichiers envoyés dans les conversations.';

  @override
  String get recoveryOnlyIf =>
      'N’utilise une copie que si aucun autre ordinateur n’utilise plus ce compte. Si l’un l’utilise encore, transfère plutôt le compte depuis celui-ci : un compte ne peut pas être sur deux ordinateurs.';

  @override
  String get recoveryMoveInstead => 'Le transférer plutôt';

  @override
  String recoveryHereNamed(String name) {
    return 'Cet ordinateur a le compte de $name.';
  }

  @override
  String recoveryHereDevice(String fingerprint) {
    return 'Cet ordinateur a un compte sans profil (appareil $fingerprint).';
  }

  @override
  String get recoveryHereUnknown =>
      'Cet ordinateur a une partie d’un compte qui ne s’ouvre pas.';

  @override
  String get recoveryHereReplaced =>
      'La restauration le remplace. Il est mis de côté, pas supprimé, jusqu’à ce que tu le supprimes dans Paramètres › Stockage et téléchargements.';

  @override
  String get recoveryRestore => 'Restaurer';

  @override
  String get recoveryReplace => 'Remplacer et restaurer';

  @override
  String recoveryRestoring(String name) {
    return 'Restauration du compte de $name…';
  }

  @override
  String recoveryRestored(String name) {
    return 'Le compte de $name est restauré.';
  }

  @override
  String get recoveryRefusedAccountHere =>
      'Cet ordinateur a maintenant un compte. Choisis de nouveau la copie pour voir ce qu’elle remplace.';

  @override
  String get recoveryRefusedReplacedKept =>
      'Cet ordinateur garde encore un compte qu’une restauration précédente a remplacé. Supprime-le pour restaurer cette copie.';

  @override
  String recoveryRefusedNoRoom(String need, String free) {
    return 'La copie a besoin de $need sur ce disque, et $free sont libres. Rien n’a changé.';
  }

  @override
  String get recoveryRefusedInFolder =>
      'La copie est dans le dossier de données de Fogata. Déplace-la ailleurs, puis choisis-la de nouveau.';

  @override
  String get recoveryBusy =>
      'Fogata est occupé par une copie de secours ou un transfert. Réessaie quand ce sera fini.';

  @override
  String get moveBusyWithCopy =>
      'Fogata est occupé par une copie de secours. Transfère le compte quand ce sera fini.';

  @override
  String get recoveryDeleteKeptAside => 'Supprimer le compte mis de côté…';

  @override
  String get reachOffline =>
      'Fogata ne joint personne sur Internet depuis ici. Vérifie la connexion, ou un pare-feu bloque peut-être Fogata.';

  @override
  String get reachFirewall =>
      'Le pare-feu de cet ordinateur ne laisse rien entrer pour Fogata. La commande qui le laisse entrer est dans les notifications.';

  @override
  String get reachUdpBlocked =>
      'Ce réseau ou cet ordinateur bloque l’UDP. Les appels et les connexions directes à travers les routeurs en ont besoin ; tant qu’il n’est pas autorisé, les autres ne te joignent qu’à travers un membre d’un groupe ou depuis une adresse publique.';

  @override
  String get reachSymmetricNat =>
      'Ton routeur donne un port public différent pour chaque destination (un NAT symétrique) : aucun chemin direct ne peut s’ouvrir à travers lui. Les amis derrière un autre routeur ne te joignent qu’à travers un membre d’un groupe que vous partagez.';

  @override
  String get reachNoIpv6 =>
      'Il n’y a pas non plus d’IPv6 qui fonctionne, qui permettrait de contourner le routeur.';

  @override
  String reachFineFriend(String name) {
    return 'Ton réseau semble convenir à Fogata : ce qui vous sépare est donc probablement du côté de $name (hors ligne, ou son réseau).';
  }

  @override
  String get reachFineOthers =>
      'Ton réseau semble convenir à Fogata : ce qui vous sépare est donc probablement de l’autre côté.';
}
