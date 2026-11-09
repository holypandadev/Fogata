// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Spanish Castilian (`es`).
class L10nEs extends L10n {
  L10nEs([String locale = 'es']) : super(locale);

  @override
  String get commonCancel => 'Cancelar';

  @override
  String get commonSave => 'Guardar';

  @override
  String get commonClose => 'Cerrar';

  @override
  String commonNewMessageFrom(String name, String text) {
    return 'Nuevo mensaje de $name: $text';
  }

  @override
  String commonNewMessagesFrom(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n mensajes nuevos de $name',
      one: '1 mensaje nuevo de $name',
    );
    return '$_temp0';
  }

  @override
  String commonNewMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n mensajes nuevos',
      one: '1 mensaje nuevo',
    );
    return '$_temp0';
  }

  @override
  String get commonAccept => 'Aceptar';

  @override
  String get commonDecline => 'Rechazar';

  @override
  String get commonSend => 'Enviar';

  @override
  String get commonTryAgain => 'Reintentar';

  @override
  String get commonCopy => 'Copiar';

  @override
  String get commonCopied => 'Copiado';

  @override
  String get commonBack => 'Atrás';

  @override
  String get commonDiscard => 'Descartar';

  @override
  String get commonKeepEditing => 'Seguir editando';

  @override
  String get commonNotNow => 'Ahora no';

  @override
  String get commonDownload => 'Descargar';

  @override
  String get commonSettings => 'Ajustes';

  @override
  String get commonHome => 'Inicio';

  @override
  String get commonFriends => 'Amigos';

  @override
  String get commonContinue => 'Continuar';

  @override
  String get commonDone => 'Listo';

  @override
  String get commonRemove => 'Quitar';

  @override
  String get commonYou => 'Tú';

  @override
  String timeYesterdayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'ayer $time',
      'other': 'ayer $time',
    });
    return '$_temp0';
  }

  @override
  String timeDayAt(String date, String time) {
    return '$date, $time';
  }

  @override
  String get timeNow => 'ahora';

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
      other: '$n días',
      one: '1 día',
    );
    return '$_temp0';
  }

  @override
  String spanMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minutos',
      one: '1 minuto',
    );
    return '$_temp0';
  }

  @override
  String spanHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n horas',
      one: '1 hora',
    );
    return '$_temp0';
  }

  @override
  String spanDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n días',
      one: '1 día',
    );
    return '$_temp0';
  }

  @override
  String spanWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n semanas',
      one: '1 semana',
    );
    return '$_temp0';
  }

  @override
  String get agoMoment => 'hace un momento';

  @override
  String agoMinutes(int n) {
    return 'hace $n min';
  }

  @override
  String agoHours(int n) {
    return 'hace $n h';
  }

  @override
  String get agoYesterday => 'ayer';

  @override
  String agoDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'hace $n días',
    );
    return '$_temp0';
  }

  @override
  String agoWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'hace $n semanas',
      one: 'hace 1 semana',
    );
    return '$_temp0';
  }

  @override
  String agoMonths(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'hace $n meses',
      one: 'hace 1 mes',
    );
    return '$_temp0';
  }

  @override
  String get agoOverYear => 'hace más de un año';

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
    return '$first y $second';
  }

  @override
  String listMore(String rest, String last) {
    return '$rest y $last';
  }

  @override
  String get listSeparator => ', ';

  @override
  String ordinal(int n) {
    return '$n.º';
  }

  @override
  String get errLinkExpired => 'El enlace ha caducado. Pide uno nuevo.';

  @override
  String get errGroupInvite => 'Eso es una invitación a un grupo.';

  @override
  String get errDeviceNotFound =>
      'No se encontró el otro dispositivo. Comprueba que los dos estén en línea.';

  @override
  String get errNotFriendCode =>
      'Eso no parece un enlace ni un código de Fogata.';

  @override
  String get errAlreadyInGroup => 'Ya estás en ese grupo.';

  @override
  String get errJoinRoomFirst => 'Entra primero en la sala.';

  @override
  String get errNewerVersion =>
      'Tu perfil se guardó con una versión más nueva de Fogata. Actualiza Fogata para abrirlo.';

  @override
  String get errKeychainUnreachable =>
      'Fogata no pudo obtener la clave de tu perfil del llavero de este equipo. Desbloquea el llavero o permite que Fogata lo use, y vuelve a intentarlo.';

  @override
  String get errKeychainNoKey =>
      'El llavero de este equipo ya no guarda la clave de tu perfil, así que Fogata no puede abrirlo.';

  @override
  String get errKeychainWrongKey =>
      'La clave del llavero de este equipo no abre tu perfil.';

  @override
  String get errNoGroupKey =>
      'Este grupo aún no está listo en este equipo. Vuelve a intentarlo en un momento.';

  @override
  String get errAlreadyRunning =>
      'Fogata ya está abierto con este perfil. Cierra el otro primero.';

  @override
  String get errFileGone => 'Ese archivo ya no está en la sala.';

  @override
  String get errNobodyHasFile =>
      'Ahora mismo nadie en línea tiene ese archivo.';

  @override
  String get errStillStarting => 'Fogata aún se está iniciando.';

  @override
  String get errIdentityExists =>
      'Este equipo ya tiene una identidad; su clave no puede cambiar.';

  @override
  String get errNameRule => 'Escribe un nombre de 1 a 40 caracteres visibles.';

  @override
  String get errBioRule =>
      'Una biografía tiene como máximo 190 caracteres en 4 líneas, sin caracteres ocultos.';

  @override
  String get errPictureRule =>
      'La imagen debe ser PNG o JPEG de 64 KB como máximo.';

  @override
  String get errNotFriendLink => 'Eso no es un enlace de amistad de Fogata.';

  @override
  String get errLinkDamaged => 'El enlace está incompleto o dañado.';

  @override
  String get errLinkNeedsNewer =>
      'Ese enlace necesita una versión más nueva de Fogata.';

  @override
  String get errCodeShape => 'Un código tiene esta forma: FGT-XXXXX-XXXXX.';

  @override
  String get errCodeWrong => 'El código es incorrecto o ya no funciona.';

  @override
  String get errNotProven => 'El otro dispositivo no pudo demostrar quién es.';

  @override
  String get errCallGone => 'Esa llamada ya no está sonando.';

  @override
  String get errAlreadyInCall => 'Ya estás en una llamada. Cuelga primero.';

  @override
  String get errAlreadyFriends => 'Ya tienes a esa persona entre tus amigos.';

  @override
  String get errMemberNotConnected =>
      'Ese miembro no está conectado ahora. Vuelve a intentarlo cuando esté en línea.';

  @override
  String get errMemberTooOld =>
      'La copia de Fogata de ese miembro es demasiado antigua para recibir solicitudes de amistad desde un grupo. Envíale un enlace de amistad.';

  @override
  String get errMemberAskLost =>
      'La solicitud de amistad no llegó a ese miembro. Vuelve a intentarlo.';

  @override
  String get errOwnLink =>
      'Ese enlace es tuyo: envíaselo al amigo que quieres añadir.';

  @override
  String get errLinkUsed =>
      'Ese enlace ya se usó o ha caducado. Pide uno nuevo.';

  @override
  String get errLinkExpiredUnreached =>
      'El enlace caducó antes de poder llegar a su dispositivo. Pide uno nuevo.';

  @override
  String get errCodeUnanswered =>
      'Ningún dispositivo respondió al código. Revísalo y comprueba que el otro dispositivo aún lo muestra.';

  @override
  String get errRefused =>
      'El otro dispositivo dijo que no. Puede que el código ya se haya usado.';

  @override
  String get errNobodyLetIn =>
      'Nadie te dejó entrar. Vuelve a intentarlo cuando alguien del grupo esté en línea.';

  @override
  String get errNotBelow =>
      'Solo puedes hacer eso con alguien por debajo de ti en el grupo.';

  @override
  String get errNotManager =>
      'Solo quienes gestionan el grupo pueden hacer eso.';

  @override
  String get errLastManager =>
      'No puedes quitarte de quienes gestionan el grupo.';

  @override
  String get errRoomNotEmpty =>
      'Hay gente en esa sala de voz: elimínala cuando esté vacía.';

  @override
  String get errTooManyFiles =>
      'Ya compartes tantos archivos como una sala admite por persona. Deja de compartir algunos primero.';

  @override
  String get errDiskFull =>
      'El disco está lleno. Libera espacio y vuelve a intentarlo.';

  @override
  String get errTooManyChanges =>
      'Este dispositivo ha hecho demasiados cambios en el grupo que ningún otro miembro ha recibido aún. Vuelve a intentarlo cuando otra persona del grupo haga un cambio.';

  @override
  String get errSomething => 'Algo salió mal.';

  @override
  String errRoomFull(int n) {
    return 'Esa sala de voz está llena: admite $n personas como máximo.';
  }

  @override
  String get findingOffline =>
      'Ningún nodo de la red pública respondió: este equipo no parece llegar a internet, o un firewall bloquea Fogata.';

  @override
  String get findingUdpWorks =>
      'UDP funciona. Lo necesitan las llamadas y las conexiones a través de routers.';

  @override
  String get findingUdpBlocked =>
      'Esta red o este equipo parece bloquear UDP (nodos que respondieron por TCP no lo hicieron por QUIC). El chat puede pasar por otros miembros, pero las conexiones directas a través de routers y las llamadas fallarán casi siempre. Revisa el firewall o las reglas de la red.';

  @override
  String get findingCarrierNat =>
      'Este equipo tiene una dirección de NAT de operador (100.64.0.0/10): el proveedor comparte una dirección pública entre muchos clientes.';

  @override
  String get findingPublicIpv4 =>
      'Este equipo tiene una dirección IPv4 pública: otros pueden llegar a él directamente.';

  @override
  String get findingHardNat =>
      'Detrás de un router que da un puerto público distinto para cada destino (un NAT estricto).';

  @override
  String get findingEasyNat =>
      'Detrás de un router que mantiene el mismo puerto público para todos los destinos (un NAT sencillo): se puede abrir desde los dos lados a la vez.';

  @override
  String get findingUnknownRouter =>
      'Pocos nodos han visto la dirección pública de este equipo como para saber qué tipo de router hay.';

  @override
  String get findingNoIpv6 =>
      'Sin dirección IPv6 pública. Fogata no la necesita, pero facilita las conexiones directas.';

  @override
  String get findingIpv6Works =>
      'La IPv6 pública funciona: los amigos con IPv6 se conectan sin ningún router de por medio.';

  @override
  String get findingIpv6Unreached =>
      'Este equipo tiene una dirección IPv6 pública, pero ningún nodo ha llegado a él por IPv6 todavía.';

  @override
  String get findingSummaryNoUdp =>
      'En resumen: habrá problemas con las conexiones directas hasta que se permita UDP.';

  @override
  String get findingSummaryPublic =>
      'En resumen: tus amigos deberían poder llegar a este equipo directamente.';

  @override
  String get findingSummaryEasyNat =>
      'En resumen: las conexiones directas deberían funcionar con la mayoría de tus amigos, abriendo los dos routers a la vez; con un amigo detrás de un NAT estricto, hace falta un miembro del grupo de por medio.';

  @override
  String get findingSummaryHardNatWithIpv6 =>
      'En resumen: los amigos con IPv6 o una dirección pública se conectan directamente; con los demás, hace falta un miembro del grupo de por medio.';

  @override
  String get findingSummaryHardNat =>
      'En resumen: solo los amigos con una dirección pública se conectan directamente; con los demás, hace falta un miembro del grupo de por medio.';

  @override
  String get findingSummaryUnknown =>
      'En resumen: no hay suficiente información; vuelve a comprobarlo en un minuto.';

  @override
  String findingInternet(int n) {
    return 'Internet: este equipo llega a la red pública ($n nodos conocidos).';
  }

  @override
  String findingDns(String detail) {
    return 'DNS: no se pudo leer toda la configuración del sistema ($detail)';
  }

  @override
  String findingLanDevices(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n dispositivos con Fogata',
      one: '1 dispositivo con Fogata',
    );
    return '$_temp0 en esta red local: los amigos de aquí se conectan sin pasar por internet.';
  }

  @override
  String findingFirewall(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'en PowerShell como administrador',
      'macos': 'en Terminal',
      'other': 'en una terminal',
    });
    return 'El firewall de este equipo no deja entrar nada para Fogata, así que un amigo en el mismo lugar detrás de otro router (dos routers en casa, por ejemplo) no puede llegar a él. Ejecuta este comando una vez $_temp0 para dejar entrar a Fogata:';
  }

  @override
  String get settingsLanguage => 'Idioma';

  @override
  String get settingsLanguageNote =>
      'El idioma de los textos propios de Fogata. Los mensajes se muestran tal como se escribieron.';

  @override
  String get settingsLanguageChoose => 'Elige el idioma';

  @override
  String get settingsTime => 'Hora';

  @override
  String get settingsTimeNote =>
      'Cómo se muestran las horas en el chat, las llamadas y las listas.';

  @override
  String settingsTime24(String example) {
    return '24 horas · $example';
  }

  @override
  String settingsTime12(String example) {
    return '12 horas · $example';
  }

  @override
  String firewallNoticeText(String system, String command) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'en PowerShell como administrador',
      'macos': 'en Terminal',
      'other': 'en una terminal',
    });
    return 'Un amigo en el mismo lugar detrás de otro router no puede llegar a este equipo. Ejecuta una vez $_temp0: $command';
  }

  @override
  String firewallCopied(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'en PowerShell como administrador',
      'macos': 'en Terminal',
      'other': 'en una terminal',
    });
    return 'Copiado. Pégalo $_temp0 y ejecútalo.';
  }

  @override
  String get appRestarted => 'Fogata dejó de funcionar y se volvió a iniciar.';

  @override
  String appVoiceRoomInGroup(String room, String group) {
    return '$room en $group';
  }

  @override
  String appBothIn(String room) {
    return 'Ya estás en $room con esa persona.';
  }

  @override
  String appHungUp(String name) {
    return '$name colgó';
  }

  @override
  String get appYourFriend => 'La otra persona';

  @override
  String get appCallEndedAlone => 'Nadie volvió, así que la llamada terminó';

  @override
  String get appAnsweredElsewhere => 'Se respondió en otro equipo';

  @override
  String appVoiceRoomDeleted(String room) {
    return 'Se eliminó “$room”.';
  }

  @override
  String get appSomeone => 'Alguien';

  @override
  String get appAGroup => 'un grupo';

  @override
  String appFriendRequest(String name) {
    return '$name te envió una solicitud de amistad.';
  }

  @override
  String appRequestDelivered(String name) {
    return 'Solicitud entregada a $name. Espera su respuesta.';
  }

  @override
  String appFriendsNow(String name) {
    return 'Ya tienes a $name entre tus amigos.';
  }

  @override
  String appJoinedGroup(String group) {
    return 'Ya estás en “$group”.';
  }

  @override
  String appRemovedFromGroup(String group) {
    return 'Ya no estás en “$group”.';
  }

  @override
  String appAddedToGroup(String name, String group) {
    return '$name te añadió a “$group”. Unirte o no: tú decides.';
  }

  @override
  String appAsksToJoin(String name, String group) {
    return '$name pide unirse a “$group”.';
  }

  @override
  String appJoinWaiting(String group) {
    return 'Esperando a que alguien de “$group” te deje entrar.';
  }

  @override
  String appJoinDeclined(String group) {
    return 'Alguien de “$group” dijo que no. Esa invitación ya no funciona.';
  }

  @override
  String appDownloadedTo(String path) {
    return 'Descargado en $path';
  }

  @override
  String appDownloadFailed(String why) {
    return 'No se pudo descargar: $why';
  }

  @override
  String get appOtherSession =>
      'Esta identidad también está abierta en otro equipo. Cierra Fogata allí: una identidad funciona en un solo equipo a la vez.';

  @override
  String get appFriendsRestored =>
      'Tu lista de amigos se dañó en este equipo y se restauró desde la copia hecha antes de su último cambio. Puede que tengas que volver a añadir o quitar a un amigo añadido o quitado justo antes.';

  @override
  String appInvitationRanOut(String group) {
    return 'La invitación a “$group” caducó antes de que alguien del grupo te dejara entrar. Pide una nueva.';
  }

  @override
  String appTakenOut(String who, String room) {
    return '$who te sacó de $room.';
  }

  @override
  String appRoomForSome(String room) {
    return '$room ahora es solo para algunos miembros.';
  }

  @override
  String appTakenOutUntilLetBack(String who, String room) {
    return '$who te sacó de $room hasta que alguien por encima de ti te deje volver.';
  }

  @override
  String appTakenOutFor(String who, String room, String span) {
    return '$who te sacó de $room durante $span.';
  }

  @override
  String appTakenOutWhile(String who, String room) {
    return '$who te sacó de $room durante un tiempo.';
  }

  @override
  String appComeBackIn(String room, String wait) {
    return 'Puedes volver a $room dentro de $wait.';
  }

  @override
  String appRoomNotForYou(String room) {
    return '$room no es para ti ahora.';
  }

  @override
  String appFoldersNotShared(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'No se pueden compartir carpetas, solo archivos: se dejaron fuera $n carpetas.',
      one: 'No se pueden compartir carpetas, solo archivos: se dejó fuera la carpeta.',
    );
    return '$_temp0';
  }

  @override
  String appSharingOne(String file) {
    return 'Compartiendo $file desde este equipo. Dejará de estar disponible cuando salgas de la sala.';
  }

  @override
  String appSharingMany(int n) {
    return 'Compartiendo $n archivos desde este equipo. Dejarán de estar disponibles cuando salgas de la sala.';
  }

  @override
  String appSavedTo(String path) {
    return 'Guardado en $path';
  }

  @override
  String appAskingToJoin(String group) {
    return 'Pidiendo unirte a “$group”. Alguien del grupo te dejará entrar cuando lo vea.';
  }

  @override
  String get appLookingForDevice =>
      'Buscando el otro dispositivo para enviar tu solicitud…';

  @override
  String callStateNoAnswer(String name) {
    return '$name no respondió';
  }

  @override
  String callStateDeclined(String name) {
    return '$name no puede hablar ahora';
  }

  @override
  String callStateBusy(String name) {
    return '$name está en otra llamada';
  }

  @override
  String callStateOutdated(String name) {
    return '$name necesita una versión más nueva de Fogata para unirse';
  }

  @override
  String callStateUnreached(String name) {
    return 'No se pudo contactar con $name';
  }

  @override
  String get callStateReturn => 'Volver a la llamada';

  @override
  String get callStateCallBack => 'Devolver llamada';

  @override
  String get callStateCall => 'Llamar';

  @override
  String noticesDeclined(String name, String group) {
    return '$name rechazó la invitación a “$group”';
  }

  @override
  String noticesDeclinedToast(String name, String group) {
    return '$name rechazó la invitación a “$group”.';
  }

  @override
  String noticesMissedInCall(String name) {
    return '$name te llamó mientras estabas en una llamada';
  }

  @override
  String noticesMissedOffline(String name) {
    return '$name te llamó mientras aparecías sin conexión';
  }

  @override
  String noticesMissedCall(String name) {
    return 'Llamada perdida de $name';
  }

  @override
  String get contactsGeneral => 'General';

  @override
  String get directStateTimerOff => 'Desactivado';

  @override
  String get directStateTimer24h => '24 horas';

  @override
  String directStateTimerDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n días',
      one: '1 día',
    );
    return '$_temp0';
  }

  @override
  String directStateDisappearsMinutes(int n) {
    return 'Desaparece en $n min';
  }

  @override
  String directStateDisappearsHours(int n) {
    return 'Desaparece en $n h';
  }

  @override
  String directStateDisappearsDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Desaparece en $n días',
    );
    return '$_temp0';
  }

  @override
  String get directStateNoLimit => 'Sin límite';

  @override
  String get directStateMessageGone => 'Ese mensaje ya no está aquí.';

  @override
  String directStateFoldersNotSent(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'No se pueden enviar carpetas, solo archivos: se dejaron fuera $n carpetas.',
      one: 'No se pueden enviar carpetas, solo archivos: se dejó fuera la carpeta.',
    );
    return '$_temp0';
  }

  @override
  String directStateFileFailed(String why) {
    return 'No se pudo obtener el archivo: $why';
  }

  @override
  String movingTooLarge(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Los banners animados pueden medir hasta $most píxeles; este mide $size.',
      'other':
          'Las imágenes animadas pueden medir hasta $most píxeles; esta mide $size.',
    });
    return '$_temp0';
  }

  @override
  String movingTooLong(String what, String most, String count) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Los banners animados pueden tener hasta $most fotogramas; este tiene $count.',
      'other':
          'Las imágenes animadas pueden tener hasta $most fotogramas; esta tiene $count.',
    });
    return '$_temp0';
  }

  @override
  String movingTooHeavy(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Los banners animados pueden ocupar hasta $most; este ocupa $size.',
      'other':
          'Las imágenes animadas pueden ocupar hasta $most; esta ocupa $size.',
    });
    return '$_temp0';
  }

  @override
  String get namesHidden =>
      'Este nombre tiene caracteres ocultos. Vuelve a escribirlo sin ellos.';

  @override
  String get namesBlank =>
      'Este nombre no tiene nada visible. Usa letras, números o emojis.';

  @override
  String get namesDefaultBio => '¡Hola! Estoy en Fogata.';

  @override
  String get namesBioHidden =>
      'Esta biografía tiene caracteres ocultos. Vuelve a escribirla sin ellos.';

  @override
  String namesBioLines(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Una biografía tiene $n líneas como máximo.',
      one: 'Una biografía tiene 1 línea como máximo.',
    );
    return '$_temp0';
  }

  @override
  String namesBioChars(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Una biografía tiene $n caracteres como máximo.',
      one: 'Una biografía tiene 1 carácter como máximo.',
    );
    return '$_temp0';
  }

  @override
  String get restartFailed =>
      'Fogata no pudo volver a iniciarse. Sal de Fogata y vuelve a abrirlo.';

  @override
  String typingOne(String name) {
    return '$name está escribiendo…';
  }

  @override
  String typingTwo(String first, String second) {
    return '$first y $second están escribiendo…';
  }

  @override
  String get typingSeveral => 'Varias personas están escribiendo…';

  @override
  String get outgoingTheRoom => 'la sala';

  @override
  String outgoingDropTitle(String group) {
    return 'Suelta para compartir en $group';
  }

  @override
  String get outgoingDropNote =>
      'Puedes añadir un mensaje y revisarlos antes de que se envíen. Los demás los descargan desde este equipo mientras estás en la sala.';

  @override
  String outgoingPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Se dejaron fuera $n archivos ocultos o privados: los archivos así no se añaden arrastrando, porque una página web puede iniciar un arrastre. Para compartirlos, elígelos con el clip.',
      one: 'Se dejó fuera un archivo oculto o privado: los archivos así no se añaden arrastrando, porque una página web puede iniciar un arrastre. Para compartirlo, elígelo con el clip.',
    );
    return '$_temp0';
  }

  @override
  String outgoingToShare(int n, String count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$count archivos para compartir · $size',
      one: '$count archivo para compartir · $size',
    );
    return '$_temp0';
  }

  @override
  String get outgoingRemoveAll => 'Quitar todos';

  @override
  String outgoingRemoveFile(String file) {
    return 'Quitar $file';
  }

  @override
  String get windowTrayOpen => 'Abrir Fogata';

  @override
  String get windowTrayQuit => 'Salir de Fogata';

  @override
  String get windowMinimize => 'Minimizar';

  @override
  String get windowRestore => 'Restaurar';

  @override
  String get windowMaximize => 'Maximizar';

  @override
  String get autostartReason => 'Abrir Fogata al iniciar sesión.';

  @override
  String get devicesMicrophone => 'Micrófono';

  @override
  String get devicesSpeaker => 'Altavoz';

  @override
  String get devicesCamera => 'Cámara';

  @override
  String get devicesChooseMicrophone => 'Elegir micrófono';

  @override
  String get devicesChooseSpeaker => 'Elegir altavoz';

  @override
  String get devicesChooseCamera => 'Elegir cámara';

  @override
  String get devicesNoCamera => 'No hay ninguna cámara conectada.';

  @override
  String get devicesCameraInCall => 'La cámara está encendida en tu llamada.';

  @override
  String devicesCameraFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'No se pudo usar la cámara. Permítelo en Ajustes del Sistema › Privacidad y seguridad.',
      'other': 'No se pudo usar la cámara.',
    });
    return '$_temp0';
  }

  @override
  String get devicesCameraDenied =>
      'El sistema no deja que Fogata use la cámara. Permítelo en la configuración del sistema y vuelve a intentarlo.';

  @override
  String get devicesCameraUnavailable =>
      'No se pudo acceder a la cámara: este sistema no tiene portal de cámara.';

  @override
  String get devicesPreviewOnlyYou => 'Solo tú ves la vista previa.';

  @override
  String get devicesStopVideo => 'Detener video';

  @override
  String get devicesTestVideo => 'Probar video';

  @override
  String get devicesNoiseOn => 'Supresión de ruido activada';

  @override
  String get devicesNoiseOff => 'Supresión de ruido desactivada';

  @override
  String get devicesNoise => 'Supresión de ruido';

  @override
  String get devicesNoiseNote =>
      'Reduce el ruido de fondo constante, como un ventilador o un zumbido, en lo que oyen los demás.';

  @override
  String get devicesNoiseShort => 'Reduce un ventilador o zumbido constante.';

  @override
  String get devicesMirror => 'Reflejar mi cámara';

  @override
  String get devicesMirrorNote =>
      'Solo cambia cómo te ves tú aquí. Los demás siempre ven tu cámara tal cual.';

  @override
  String get devicesMirrorOn => 'Reflejar mi cámara activado';

  @override
  String get devicesMirrorOff => 'Reflejar mi cámara desactivado';

  @override
  String get devicesTestMic => 'Probar micrófono';

  @override
  String get devicesStopMicTest => 'Detener prueba';

  @override
  String get devicesMicLevel => 'Nivel del micrófono';

  @override
  String get devicesHearMyself => 'Escucharme';

  @override
  String get devicesHearMyselfNote =>
      'Reproduce tu micrófono por el altavoz. Usa auriculares, o el altavoz puede volver a entrar por el micrófono.';

  @override
  String get devicesHearMyselfOn => 'Escucharme activado';

  @override
  String get devicesHearMyselfOff => 'Escucharme desactivado';

  @override
  String get devicesMicTestOnlyYou => 'Nada de la prueba se envía a nadie.';

  @override
  String get devicesMicInCall => 'El micrófono está en uso en tu llamada.';

  @override
  String get devicesNoMic => 'No hay ningún micrófono conectado.';

  @override
  String get devicesSystemDefault => 'Predeterminado del sistema';

  @override
  String get devicesNotConnected => 'No conectado: se usa el predeterminado';

  @override
  String get devicesNoCameraShort => 'No hay ninguna cámara conectada';

  @override
  String get devicesMicOptions => 'Opciones del micrófono';

  @override
  String get devicesSpeakerOptions => 'Opciones del altavoz';

  @override
  String get devicesCameraOptions => 'Opciones de la cámara';

  @override
  String get devicesInputDevice => 'Dispositivo de entrada';

  @override
  String get devicesOutputDevice => 'Dispositivo de salida';

  @override
  String get devicesChosenMissing =>
      'El elegido no está conectado: se usa el predeterminado.';

  @override
  String get devicesVoiceSettings => 'Ajustes de voz y video';

  @override
  String get photoImages => 'Imágenes';

  @override
  String photoTooLarge(int n) {
    return 'Esa imagen es demasiado grande: como máximo $n megapíxeles.';
  }

  @override
  String get photoUnreadable => 'No se pudo leer esa imagen.';

  @override
  String get callSource => 'Original';

  @override
  String callQuality(String resolution, int fps) {
    return '$resolution · $fps fps';
  }

  @override
  String get callSetupFailed => 'No se pudo preparar la llamada.';

  @override
  String get callNoSoundSystem =>
      'No se pudo acceder al sistema de sonido de este equipo, así que la llamada no puede empezar.';

  @override
  String callMicGone(String name) {
    return 'Se desconectó tu micrófono: ahora se usa $name.';
  }

  @override
  String get callMicBack => 'Tu micrófono volvió.';

  @override
  String callMicSilent(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'No se pudo usar el micrófono. Comprueba que esté conectado y permitido en Ajustes del Sistema › Privacidad y seguridad.',
      'windows': 'No se pudo usar el micrófono. Comprueba que esté conectado y permitido en Configuración › Privacidad y seguridad › Micrófono.',
      'other': 'No se pudo usar el micrófono. Comprueba que esté conectado.',
    });
    return '$_temp0';
  }

  @override
  String get callNoMicrophone => 'No hay ningún micrófono conectado.';

  @override
  String callMicFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'No se pudo usar el micrófono. Permítelo en Ajustes del Sistema › Privacidad y seguridad.',
      'windows': 'No se pudo usar el micrófono. Permítelo en Configuración › Privacidad y seguridad › Micrófono.',
      'other': 'No se pudo usar el micrófono.',
    });
    return '$_temp0';
  }

  @override
  String callScreenFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'No se pudo compartir la pantalla. Permítelo en Ajustes del Sistema › Privacidad y seguridad.',
      'other': 'No se pudo compartir la pantalla.',
    });
    return '$_temp0';
  }

  @override
  String get callShareLost =>
      'Se dejó de compartir: lo que compartías ya no se puede capturar.';

  @override
  String callKeptOut(String name) {
    return 'Se perdió la conexión con $name.';
  }

  @override
  String get qualityGood => 'Buena';

  @override
  String get qualityOkay => 'Regular';

  @override
  String get qualityPoor => 'Mala';

  @override
  String get qualityLost => 'Perdida';

  @override
  String qualityDelay(int ms) {
    return '$ms ms';
  }

  @override
  String qualityPercent(String value) {
    return '$value %';
  }

  @override
  String get qualityAnotherMember => 'otro miembro';

  @override
  String qualityLostHint(String name, int seconds) {
    return 'No llega nada de $name desde hace $seconds s. Fogata sigue intentando reconectar; se reconectará en cuanto vuelva su red.';
  }

  @override
  String qualityBothPoor(String name) {
    return 'Las dos redes van mal ahora mismo: el audio entre $name y tú puede llegar entrecortado.';
  }

  @override
  String qualityTheirSide(String name) {
    return 'El problema está del lado de $name: todos oyen su voz entrecortada. Su Wi-Fi o sus datos móviles van mal ahora mismo.';
  }

  @override
  String qualityYourSideWith(String name) {
    return 'El problema está de tu lado: $name y los demás podrían oír tu voz entrecortada. Tu Wi-Fi o tus datos móviles van mal ahora mismo.';
  }

  @override
  String qualityTheirsSlow(String name) {
    return 'La red de $name va algo lenta ahora mismo: todos podrían oír su voz entrecortada de vez en cuando.';
  }

  @override
  String qualityAllGoodWith(String name) {
    return 'Con $name el audio va bien en los dos sentidos.';
  }

  @override
  String qualityMineSlowWith(String name) {
    return 'Tu red va algo lenta ahora mismo: $name podría oír tu voz entrecortada de vez en cuando.';
  }

  @override
  String qualityWayThrough(String name, String carrier) {
    return 'La red de $name va bien; el camino entre los dos es más lento porque pasa por $carrier. Podrías oír su voz entrecortada de vez en cuando; los demás la oyen bien.';
  }

  @override
  String qualityWaySlowFine(String name) {
    return 'La red de $name va bien; el camino entre los dos va lento ahora mismo. Podrías oír su voz entrecortada de vez en cuando; los demás la oyen bien.';
  }

  @override
  String qualityWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'El camino entre tú y $name va algo lento ahora mismo; puede ser cualquiera de las dos redes. El audio puede llegar entrecortado de vez en cuando.',
      'other':
          'El camino entre tú y $name va mal ahora mismo; puede ser cualquiera de las dos redes. El audio puede llegar entrecortado.',
    });
    return '$_temp0';
  }

  @override
  String qualityWayVia(String level, String name, String carrier) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'El camino entre tú y $name va algo lento ahora mismo; puede ser cualquiera de las dos redes. El audio puede llegar entrecortado de vez en cuando. Pasa por $carrier.',
      'other':
          'El camino entre tú y $name va mal ahora mismo; puede ser cualquiera de las dos redes. El audio puede llegar entrecortado. Pasa por $carrier.',
    });
    return '$_temp0';
  }

  @override
  String get qualityYourSide =>
      'El problema está de tu lado: los demás podrían oír tu voz entrecortada. Tu Wi-Fi o tus datos móviles van mal ahora mismo.';

  @override
  String get qualityMineSlow =>
      'Tu red va algo lenta ahora mismo: los demás podrían oír tu voz entrecortada de vez en cuando.';

  @override
  String qualityLostShort(String name, int seconds) {
    return 'No llega nada de $name desde hace $seconds s. Fogata sigue intentando reconectar.';
  }

  @override
  String qualityAllWellWith(String name) {
    return 'Todo bien: oyes a $name sin problemas y te oye igual de bien.';
  }

  @override
  String qualityNoteDropped(String name) {
    return '$name se desconectó y Fogata está intentando reconectar';
  }

  @override
  String qualityNoteTheirsPoor(String name) {
    return 'la red de $name va mal, así que todos oyen su voz entrecortada';
  }

  @override
  String qualityNoteTheirsSlow(String name) {
    return 'la red de $name va algo lenta';
  }

  @override
  String qualityNoteThrough(String name, String carrier) {
    return '$name te llega a través de $carrier';
  }

  @override
  String qualityNoteWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': 'el camino hasta $name va algo lento ahora mismo',
      'other': 'el camino hasta $name va mal ahora mismo',
    });
    return '$_temp0';
  }

  @override
  String get qualityNoteSeparator => '; ';

  @override
  String get qualityAllWell => 'Todo bien: oyes a todos y todos te oyen.';

  @override
  String qualityAllWellMine(String notes) {
    return 'Todo bien por tu parte. $notes.';
  }

  @override
  String qualityYourDetails(String level) {
    return 'Tu conexión: $level. Mostrar detalles';
  }

  @override
  String qualityTheirDetails(String name, String level) {
    return 'Conexión de $name: $level. Mostrar detalles';
  }

  @override
  String get qualityReconnecting => 'Reconectando…';

  @override
  String get qualityReconnectingLabel => 'Reconectando';

  @override
  String qualityLevelConnection(String level) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'good': 'Buena conexión',
      'okay': 'Conexión regular',
      'weak': 'Mala conexión',
      'other': 'Conexión perdida',
    });
    return '$_temp0';
  }

  @override
  String get qualityPoorConnection => 'Mala conexión';

  @override
  String get qualityMinePoorBanner =>
      'Tu red va mal: los demás podrían oír tu voz entrecortada';

  @override
  String get qualityYourNetwork => 'Tu red';

  @override
  String qualityTo(String name) {
    return 'Con $name';
  }

  @override
  String qualityBetween(String name) {
    return 'Entre tú y $name';
  }

  @override
  String qualityTheirNetwork(String name) {
    return 'Red de $name';
  }

  @override
  String get qualityLastHeard => 'Última señal';

  @override
  String qualitySecondsAgo(int n) {
    return 'hace $n s';
  }

  @override
  String get qualityPath => 'Ruta';

  @override
  String get qualityLookingAgain => 'Buscando de nuevo';

  @override
  String get qualityDelayLabel => 'Latencia';

  @override
  String get qualityLostPackets => 'Paquetes perdidos';

  @override
  String get qualityYourConnection => 'Tu conexión';

  @override
  String qualityTheirConnection(String name) {
    return 'Conexión de $name';
  }

  @override
  String roomCountFullLabel(int count, int max) {
    return 'Llena, $count de $max personas';
  }

  @override
  String roomCountLabel(int count, int max) {
    return '$count de $max personas';
  }

  @override
  String get roomCountFullPrefix => 'Llena · ';

  @override
  String get roomCountFullWord => 'Llena ';

  @override
  String get volumeVoice => 'Volumen de voz';

  @override
  String get volumeOnlyYouHear => 'Solo tú oyes estos cambios';

  @override
  String get volumeScreenSound => 'Sonido de la pantalla';

  @override
  String volumePercent(int n) {
    return '$n %';
  }

  @override
  String get volumeMuteVoiceForMe => 'Silenciar su voz para mí';

  @override
  String get volumeMuteForMe => 'Silenciar para mí';

  @override
  String get volumeMuteScreenForMe =>
      'Silenciar el sonido de la pantalla para mí';

  @override
  String get volumeBackTo100 => 'Volver al 100 %';

  @override
  String get volumeBelowYou => 'POR DEBAJO DE TI';

  @override
  String volumeRemoveFrom(String room) {
    return 'Sacar de $room';
  }

  @override
  String volumeRemoveTitle(String name, String room) {
    return '¿Sacar a $name de $room?';
  }

  @override
  String volumeRemoveText(String name, String room) {
    return '$name sale de $room ahora y puede volver a entrar enseguida. Un tiempo fuera impide volver durante un rato.';
  }

  @override
  String volumeTimeOutOf(String room) {
    return 'Sacar de $room por un tiempo';
  }

  @override
  String volumeTimeOutText(String name, String room) {
    return '$name sale de $room y no puede volver durante:';
  }

  @override
  String get volumeUntilLetBack =>
      'Hasta que alguien por encima le deje volver';

  @override
  String get pipYourScreen => 'Tu pantalla';

  @override
  String pipScreenOf(String name) {
    return 'Pantalla de $name';
  }

  @override
  String get pipYourCamera => 'Tu cámara';

  @override
  String get pipBackToCall => 'Volver a la llamada';

  @override
  String get pipBackToRoom => 'Volver a la sala';

  @override
  String get pipStopWatching => 'Dejar de ver';

  @override
  String get addToCallTitle => 'Añadir a la llamada';

  @override
  String addToCallNote(int n) {
    return 'Amigos en línea. Hasta $n personas en una llamada.';
  }

  @override
  String get addToCallSearch => 'Buscar amigos';

  @override
  String get addToCallNoFriends => 'Aún no tienes amigos.';

  @override
  String get addToCallNoMatch => 'Ningún amigo se llama así.';

  @override
  String get addToCallInCall => 'En la llamada';

  @override
  String get addToCallCalling => 'Llamando…';

  @override
  String get addToCallOffline => 'Sin conexión';

  @override
  String get addToCallCall => 'Llamar';

  @override
  String get streamQualityTitle => 'Calidad de transmisión';

  @override
  String get streamQualityNoteSharing =>
      'Cómo llega tu pantalla a los demás. Los cambios se aplican al momento.';

  @override
  String get streamQualityNote =>
      'Cómo llega tu pantalla a los demás cuando la compartes.';

  @override
  String get streamQualityResolution => 'Resolución';

  @override
  String streamQualitySourceLines(int lines) {
    return 'Original (${lines}p)';
  }

  @override
  String get streamQualitySource => 'Original';

  @override
  String get streamQualityFrameRate => 'Fotogramas por segundo';

  @override
  String streamQualityFps(int n) {
    return '$n fps';
  }

  @override
  String get streamQualityHigherNote =>
      'Más calidad consume más subida y más recursos de tu equipo.';

  @override
  String get streamQualitySomeone => 'Alguien';

  @override
  String streamQualityLoweredOne(String name, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Los demás reciben lo que elegiste.',
      'other': '',
    });
    return 'La conexión de $name no da para más ahora mismo, así que recibe una imagen más pequeña.$_temp0';
  }

  @override
  String streamQualityLoweredMany(String names, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Los demás reciben lo que elegiste.',
      'other': '',
    });
    return 'Algunas conexiones no dan para más ahora mismo: $names reciben una imagen más pequeña.$_temp0';
  }

  @override
  String get streamQualityGoingOut => 'ENVIANDO AHORA';

  @override
  String get streamQualityNobody => 'Nadie la está viendo todavía.';

  @override
  String streamQualityNow(int height, int fps) {
    return '${height}p · $fps fps';
  }

  @override
  String callLayersIsCalling(String name) {
    return '$name te está llamando';
  }

  @override
  String get callLayersGoToCall => 'F6 lleva a la llamada';

  @override
  String callLayersWith(String names) {
    return 'Con $names';
  }

  @override
  String get callLayersDirectCall => 'Llamada directa · ';

  @override
  String get callLayersAnswerMicOff => 'Responder con el micrófono silenciado';

  @override
  String callLayersAnsweringLeaves(String room) {
    return 'Al responder, saldrás de $room';
  }

  @override
  String get callLayersLeaveAndAnswer => 'Salir y responder';

  @override
  String callLayersBothIn(String room) {
    return 'Ya estás en $room con esa persona.';
  }

  @override
  String get callLayersDismiss => 'Descartar';

  @override
  String callLayersCallWith(String names) {
    return 'Llamada con $names';
  }

  @override
  String get callLayersRoom => 'Sala';

  @override
  String callLayersBackTo(String title) {
    return 'Volver a $title';
  }

  @override
  String callLayersRinging(String time) {
    return 'Sonando · $time';
  }

  @override
  String callLayersInCall(String time) {
    return 'En llamada · $time';
  }

  @override
  String callLayersOnlyYou(String time) {
    return 'Solo tú · $time';
  }

  @override
  String callLayersInRoom(int n, String time) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n en la sala',
    );
    return '$_temp0 · $time';
  }

  @override
  String get callLayersMute => 'Silenciar';

  @override
  String get callLayersUnmute => 'Activar micrófono';

  @override
  String get callLayersHangUp => 'Colgar';

  @override
  String get callLayersLeaveRoom => 'Salir de la sala';

  @override
  String roomDeclineCallTitle(String name) {
    return '¿Rechazar la llamada de $name?';
  }

  @override
  String roomDeclineCallText(String caller, String callee) {
    return '$caller te está llamando. Solo puedes estar en una llamada a la vez: llamar a $callee la rechaza.';
  }

  @override
  String roomDeclineCallTextSomeone(String caller) {
    return '$caller te está llamando. Solo puedes estar en una llamada a la vez: llamar a otra persona la rechaza.';
  }

  @override
  String get roomDeclineAndCall => 'Rechazar y llamar';

  @override
  String roomHangUpTitle(String name) {
    return '¿Colgar la llamada con $name?';
  }

  @override
  String get roomHangUpText =>
      'Solo puedes estar en una llamada o sala a la vez: si sigues, la llamada termina.';

  @override
  String get roomHangUp => 'Colgar';

  @override
  String get roomCloseTitle => '¿Salir y cerrar la sala?';

  @override
  String get roomCloseText =>
      'No queda nadie más aquí. Al salir, la sala se cierra y lo que se dijo desaparece de este equipo; lo que otros descargaron se queda en los suyos.';

  @override
  String get roomCloseChatMessages => 'Mensajes del chat';

  @override
  String get roomCloseFilesShared => 'Archivos que compartiste';

  @override
  String get roomStay => 'Quedarme';

  @override
  String get roomLeaveAndClose => 'Salir y cerrar';

  @override
  String get roomLivePrefix => 'En vivo · ';

  @override
  String get roomPillConnecting => 'Conectando';

  @override
  String get roomPillOnlyYou => 'Solo tú';

  @override
  String get roomPillError => 'Error';

  @override
  String get roomLastOne => 'No queda nadie más en la sala.';

  @override
  String get roomOnlyYouIn => 'Solo tú estás en la sala.';

  @override
  String get roomAloneWarning =>
      ' Al salir, se cierra y todo lo que contiene desaparece.';

  @override
  String get roomEarlierPeople => 'Personas anteriores';

  @override
  String get roomMorePeople => 'Más personas';

  @override
  String roomTileYou(String name) {
    return '$name (tú)';
  }

  @override
  String roomTileNoSound(String name) {
    return '$name · sin sonido';
  }

  @override
  String roomTileConnecting(String name) {
    return '$name · conectando';
  }

  @override
  String roomTileAway(String name) {
    return '$name · sin conexión';
  }

  @override
  String roomTileMicOff(String name) {
    return '$name, micrófono silenciado';
  }

  @override
  String roomTileMutedByYou(String said) {
    return '$said, silenciado para ti';
  }

  @override
  String roomTileVolume(String said, String percent) {
    return '$said, volumen $percent';
  }

  @override
  String roomTileLeft(String name) {
    return '$name salió · puede volver';
  }

  @override
  String roomIsSharing(String name) {
    return '$name está compartiendo la pantalla';
  }

  @override
  String get roomNothingSentUntilWatch =>
      'No recibes nada hasta que empieces a ver';

  @override
  String get roomWatch => 'Ver';

  @override
  String get roomLive => 'EN VIVO';

  @override
  String get roomSharingYourScreen => 'Estás compartiendo tu pantalla';

  @override
  String get roomShowPreview => 'Mostrar vista previa';

  @override
  String get roomPreviewMirror =>
      'Mostrarla mientras compartes el propio Fogata crea un espejo sin fin';

  @override
  String get roomNobodyWatching => 'Nadie la está viendo';

  @override
  String roomPeopleWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n personas viendo',
      one: '1 persona viendo',
    );
    return '$_temp0';
  }

  @override
  String get roomExitFullScreen => 'Salir de pantalla completa';

  @override
  String get roomFullScreen => 'Pantalla completa';

  @override
  String get roomPlayScreenSound => 'Reproducir el sonido de la pantalla';

  @override
  String get roomMuteScreenSound => 'Silenciar el sonido de la pantalla';

  @override
  String get roomQualityPoor => 'Mala';

  @override
  String get roomQualityOkay => 'Regular';

  @override
  String get roomQualityGood => 'Buena';

  @override
  String get roomQualityPoorTitle => 'Mala conexión';

  @override
  String get roomQualityOkayTitle => 'La conexión es regular';

  @override
  String get roomQualityGoodTitle => 'Buena conexión';

  @override
  String get roomQualityMeasuring => 'Midiendo';

  @override
  String get roomQualityDelay => 'Latencia';

  @override
  String roomQualityMs(String n) {
    return '$n ms';
  }

  @override
  String get roomQualityLost => 'Paquetes perdidos';

  @override
  String roomQualityPercent(String n) {
    return '$n %';
  }

  @override
  String get roomQualityPicture => 'Imagen';

  @override
  String get roomQualityPath => 'Ruta';

  @override
  String get roomQualityMayFreeze =>
      'La imagen puede congelarse. Ayuda que menos personas compartan a la vez.';

  @override
  String get roomQualityMayShrink =>
      'La imagen puede hacerse más pequeña para seguir fluida.';

  @override
  String roomCallingName(String name) {
    return 'Llamando a $name…';
  }

  @override
  String get roomRingsFor30 => 'Suena en su equipo hasta 30 segundos.';

  @override
  String get roomReturnToCall => 'Volver a la llamada';

  @override
  String get roomCallAgain => 'Volver a llamar';

  @override
  String get roomAloneHint => 'Cuando alguien del grupo entre, aparecerá aquí.';

  @override
  String roomAlreadyLeft(String names) {
    return 'Ya se fueron: $names.';
  }

  @override
  String get roomGroupFound => 'Grupo encontrado en la red';

  @override
  String get roomLookingForRoute => 'buscando la mejor ruta…';

  @override
  String get roomConnectingTitle => 'Conectando a la sala';

  @override
  String roomSeconds(String n) {
    return '$n s';
  }

  @override
  String roomReachedOf(int reached, int total, String time) {
    return '$reached de $total · $time';
  }

  @override
  String get roomPathLan => 'red local, cifrado';

  @override
  String get roomPathDirect => 'directo, cifrado';

  @override
  String get roomPathThroughMember => 'a través de un miembro, cifrado';

  @override
  String roomPathThrough(String name) {
    return 'a través de $name, cifrado';
  }

  @override
  String get roomCouldNotConnect => 'No se pudo conectar a la sala';

  @override
  String get roomCouldNotConnectText =>
      'Nadie respondió a tiempo. Puede que tu red bloquee las conexiones directas entre dispositivos.';

  @override
  String get roomNoAnswer => 'sin respuesta';

  @override
  String get roomSeeNetworkCheck => 'Ver la comprobación de red →';

  @override
  String get roomGoesNothing => 'La sala se cierra para todos';

  @override
  String roomGoesMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Se pierden $n mensajes',
      one: 'Se pierde 1 mensaje',
    );
    return '$_temp0';
  }

  @override
  String roomGoesFiles(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Se van $n archivos que compartiste',
      one: 'Se va 1 archivo que compartiste',
    );
    return '$_temp0';
  }

  @override
  String roomGoesBoth(int messages, int files) {
    String _temp0 = intl.Intl.pluralLogic(
      messages,
      locale: localeName,
      other: '$messages mensajes',
      one: '1 mensaje',
    );
    String _temp1 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files archivos',
      one: '1 archivo',
    );
    return 'Se pierden $_temp0 y $_temp1';
  }

  @override
  String get roomMuteMicrophone => 'Silenciar micrófono';

  @override
  String get roomUnmuteMicrophone => 'Activar micrófono';

  @override
  String get roomStopSharingScreen => 'Dejar de compartir la pantalla';

  @override
  String get roomShareScreen => 'Compartir la pantalla';

  @override
  String get roomUndeafen => 'Volver a oír a los demás';

  @override
  String get roomDeafen => 'Dejar de oír a los demás';

  @override
  String get roomCameraOff => 'Apagar la cámara';

  @override
  String get roomCameraOn => 'Encender la cámara';

  @override
  String get roomLeave => 'Salir';

  @override
  String get roomSoundWaitsForMic =>
      'El sonido se envía con el micrófono activado';

  @override
  String get roomStopSharingSound => 'Dejar de compartir el sonido';

  @override
  String get roomShareSoundToo => 'Compartir también el sonido';

  @override
  String get roomAddFriendToCall => 'Añadir un amigo a la llamada';

  @override
  String get roomHideChat => 'Ocultar el chat';

  @override
  String get roomShowChat => 'Mostrar el chat';

  @override
  String get roomTabChat => 'Chat';

  @override
  String get roomTabFiles => 'Archivos';

  @override
  String roomNoteLeftCall(String name) {
    return '$name salió de la llamada · ';
  }

  @override
  String roomNoteJoinedCall(String name) {
    return '$name se unió a la llamada · ';
  }

  @override
  String roomNoteLeftRoom(String name) {
    return '$name salió de la sala · ';
  }

  @override
  String roomNoteEnteredRoom(String name) {
    return '$name entró en la sala · ';
  }

  @override
  String get roomSearchMessages => 'Buscar mensajes';

  @override
  String get roomClearSearch => 'Borrar la búsqueda';

  @override
  String get roomChatGoneCall =>
      'Este chat desaparece cuando termina la llamada';

  @override
  String get roomChatGoneRoom =>
      'Este chat desaparece cuando se cierra la sala';

  @override
  String roomNoMatches(String text) {
    return 'Ningún mensaje con “$text”';
  }

  @override
  String roomMatches(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n resultados',
      one: '1 resultado',
    );
    return '$_temp0';
  }

  @override
  String get roomReceivingMessages => 'Recibiendo los mensajes de la sala…';

  @override
  String roomReceivingMessagesFrom(String name) {
    return 'Recibiendo los mensajes de la sala desde el equipo de $name…';
  }

  @override
  String get roomSomeone => 'Alguien';

  @override
  String get roomShareFiles => 'Compartir archivos';

  @override
  String get roomAddMoreFiles => 'Añadir más archivos';

  @override
  String roomMessageTo(String name) {
    return 'Mensaje para $name';
  }

  @override
  String get roomMessageTheRoom => 'Mensaje para la sala';

  @override
  String get roomMessageTheCall => 'Mensaje para la llamada';

  @override
  String get roomAddMessageOptional => 'Añade un mensaje (opcional)';

  @override
  String get roomMessageLabel => 'Mensaje';

  @override
  String roomEnterToSend(String key) {
    return '$key para enviar';
  }

  @override
  String roomEscToRemove(String key) {
    return '$key para quitarlos';
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
    return 'No disponible · $name salió';
  }

  @override
  String get roomSharingFromHere => 'Compartiendo desde este equipo';

  @override
  String roomAvailableWhile(String names) {
    return 'Disponible mientras $names siga en la sala';
  }

  @override
  String roomSizeDownloads(String size, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n descargas',
      one: '1 descarga',
    );
    return '$size · $_temp0';
  }

  @override
  String roomDownloadFile(String file) {
    return 'Descargar $file';
  }

  @override
  String roomShowInFolder(String file) {
    return 'Mostrar $file en su carpeta';
  }

  @override
  String roomGettingFile(String file) {
    return 'Obteniendo $file…';
  }

  @override
  String get roomSaveToDownloads => 'Guardar en Descargas';

  @override
  String roomPictureKept(String name) {
    return '$name salió · la imagen se queda aquí hasta que salgas';
  }

  @override
  String get roomSaveFile => 'Guardar';

  @override
  String get roomClosePicture => 'Cerrar la imagen';

  @override
  String get roomFilesNote =>
      'Los archivos no se suben a ningún sitio: van directamente desde el equipo de quien los comparte, y solo mientras esa persona siga en la sala.';

  @override
  String roomFilesAvailable(int n) {
    return 'Disponibles · $n';
  }

  @override
  String roomFilesGone(int n) {
    return 'Ya no disponibles · $n';
  }

  @override
  String roomPersonLeft(String name) {
    return '$name salió';
  }

  @override
  String get roomPickOrDrop => 'Elígelos aquí o suéltalos sobre la sala';

  @override
  String get roomFileYours => 'Tuyo';

  @override
  String roomFileFrom(String name) {
    return 'De $name';
  }

  @override
  String roomDownloads(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n descargas',
      one: '1 descarga',
    );
    return '$_temp0';
  }

  @override
  String get roomStopSharingFile => 'Detener';

  @override
  String get roomWhatToShare => '¿Qué quieres compartir?';

  @override
  String roomScreenSource(String name) {
    return 'Pantalla: $name';
  }

  @override
  String roomPlayVideo(String file) {
    return 'Reproducir $file';
  }

  @override
  String get roomCloseVideo => 'Cerrar el video';

  @override
  String get roomVideoGone => 'Este video ya no está en la sala.';

  @override
  String get roomVideoNotFetched => 'No se pudo obtener este video.';

  @override
  String get roomVideoCannotPlay =>
      'Este equipo no puede reproducir este video.';

  @override
  String roomGettingVideoPercent(int n) {
    return 'Obteniendo el video · $n %';
  }

  @override
  String get roomGettingVideo => 'Obteniendo el video…';

  @override
  String get roomPause => 'Pausar';

  @override
  String get roomPlay => 'Reproducir';

  @override
  String get roomSoundOn => 'Activar sonido';

  @override
  String get roomMuteVideo => 'Silenciar';

  @override
  String get roomLeaveFullScreen => 'Salir de pantalla completa';

  @override
  String roomCallWaits(String time) {
    return 'Cuando solo queda uno, la llamada espera $time a que vuelvan los demás';
  }

  @override
  String roomCallWaitsAlone(String time) {
    return 'Solo quedas tú. La llamada espera $time a que vuelvan los demás';
  }

  @override
  String roomOpenFile(String file) {
    return 'Abrir $file';
  }

  @override
  String activityCreated(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Creaste el grupo como “$name”',
      'other': '$by creó el grupo como “$name”',
    });
    return '$_temp0';
  }

  @override
  String activityRenamed(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Cambiaste el nombre del grupo a “$name”',
      'other': '$by cambió el nombre del grupo a “$name”',
    });
    return '$_temp0';
  }

  @override
  String activityIcon(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Cambiaste el icono del grupo',
      'other': '$by cambió el icono del grupo',
    });
    return '$_temp0';
  }

  @override
  String activityPictureRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Quitaste la imagen del grupo',
      'other': '$by quitó la imagen del grupo',
    });
    return '$_temp0';
  }

  @override
  String activityPicture(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Cambiaste la imagen del grupo',
      'other': '$by cambió la imagen del grupo',
    });
    return '$_temp0';
  }

  @override
  String activityBannerRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Quitaste el banner del grupo',
      'other': '$by quitó el banner del grupo',
    });
    return '$_temp0';
  }

  @override
  String activityBanner(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Cambiaste el banner del grupo',
      'other': '$by cambió el banner del grupo',
    });
    return '$_temp0';
  }

  @override
  String activityAdded(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Te añadiste',
      'other': 'Añadiste a $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by te añadió',
      'other': '$by añadió a $member',
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
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Te uniste con una invitación tuya',
      'other': 'Te uniste con una invitación de $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by se unió con una invitación tuya',
      'other': '$by se unió con una invitación de $member',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2';
  }

  @override
  String activityLetIn(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Te dejaste entrar',
      'other': 'Dejaste entrar a $member',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by te dejó entrar',
      'other': '$by dejó entrar a $member',
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
      'yes': 'Te sacaste del grupo',
      'other': 'Sacaste a $member del grupo',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by te sacó del grupo',
      'other': '$by sacó a $member del grupo',
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
      'yes': 'Saliste del grupo',
      'other': '$by salió del grupo',
    });
    return '$_temp0';
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
      'yes': 'Te subiste al $place puesto',
      'other': 'Subiste a $member al $place puesto',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by te subió al $place puesto',
      'other': '$by subió a $member al $place puesto',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2';
  }

  @override
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Te moviste al final',
      'other': 'Moviste a $member al final',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by te movió al final',
      'other': '$by movió a $member al final',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2';
  }

  @override
  String activityManagersAll(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Dejaste que todos gestionaran el grupo',
      'other': '$by dejó que todos gestionaran el grupo',
    });
    return '$_temp0';
  }

  @override
  String activityManagersFirst(String by, String byMe, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'a las $n primeras personas',
      one: 'solo a la primera persona',
    );
    String _temp1 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'a las $n primeras personas',
      one: 'solo a la primera persona',
    );
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': 'Dejaste la gestión del grupo $_temp0',
      'other': '$by dejó la gestión del grupo $_temp1',
    });
    return '$_temp2';
  }

  @override
  String activityLimits(String by, String byMe, int people, int perRoom) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes':
          'Pusiste el límite del grupo en $people personas y $perRoom por sala de voz',
      'other':
          '$by puso el límite del grupo en $people personas y $perRoom por sala de voz',
    });
    return '$_temp0';
  }

  @override
  String activityRoomOpened(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Abriste la sala de voz “$room”',
      'other': '$by abrió la sala de voz “$room”',
    });
    return '$_temp0';
  }

  @override
  String activityRoomRenamed(String by, String byMe, String from, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Cambiaste el nombre de la sala de voz “$from” a “$room”',
      'other': '$by cambió el nombre de la sala de voz “$from” a “$room”',
    });
    return '$_temp0';
  }

  @override
  String activityRoomClosed(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Eliminaste la sala de voz “$room”',
      'other': '$by eliminó la sala de voz “$room”',
    });
    return '$_temp0';
  }

  @override
  String activityInvited(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Creaste un enlace de invitación',
      'other': '$by creó un enlace de invitación',
    });
    return '$_temp0';
  }

  @override
  String activityRevoked(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Anulaste un enlace de invitación',
      'other': '$by anuló un enlace de invitación',
    });
    return '$_temp0';
  }

  @override
  String activityRoomOpenToAll(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Abriste “$room” a todos',
      'other': '$by abrió “$room” a todos',
    });
    return '$_temp0';
  }

  @override
  String activityRoomForAbove(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Reservaste “$room” para quienes están por encima',
      'other': '$by reservó “$room” para quienes están por encima',
    });
    return '$_temp0';
  }

  @override
  String activityRoomOnly(String by, String byMe, String people, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Limitaste la entrada a “$room” a: $people',
      'other': '$by limitó la entrada a “$room” a: $people',
    });
    return '$_temp0';
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
      'yes': 'Te dejaste fuera de “$room” hasta que alguien te deje volver',
      'other':
          'Dejaste a $member fuera de “$room” hasta que alguien le deje volver',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by te dejó fuera de “$room” hasta que alguien te deje volver',
      'other':
          '$by dejó a $member fuera de “$room” hasta que alguien le deje volver',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2';
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
      'today': 'hasta las $until',
      'todayAtOne': 'hasta la $until',
      'yesterday': 'hasta $until',
      'other': 'hasta el $until',
    });
    String _temp1 = intl.Intl.selectLogic(untilKind, {
      'today': 'hasta las $until',
      'todayAtOne': 'hasta la $until',
      'yesterday': 'hasta $until',
      'other': 'hasta el $until',
    });
    String _temp2 = intl.Intl.selectLogic(memberMe, {
      'yes': 'Te sacaste de “$room” $_temp0',
      'other': 'Sacaste a $member de “$room” $_temp1',
    });
    String _temp3 = intl.Intl.selectLogic(untilKind, {
      'today': 'hasta las $until',
      'todayAtOne': 'hasta la $until',
      'yesterday': 'hasta $until',
      'other': 'hasta el $until',
    });
    String _temp4 = intl.Intl.selectLogic(untilKind, {
      'today': 'hasta las $until',
      'todayAtOne': 'hasta la $until',
      'yesterday': 'hasta $until',
      'other': 'hasta el $until',
    });
    String _temp5 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by te sacó de “$room” $_temp3',
      'other': '$by sacó a $member de “$room” $_temp4',
    });
    String _temp6 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp2',
      'other': '$_temp5',
    });
    return '$_temp6';
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
      'yes': 'Te dejaste volver a “$room”',
      'other': 'Dejaste volver a $member a “$room”',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '$by te dejó volver a “$room”',
      'other': '$by dejó volver a $member a “$room”',
    });
    String _temp2 = intl.Intl.selectLogic(byMe, {
      'yes': '$_temp0',
      'other': '$_temp1',
    });
    return '$_temp2';
  }

  @override
  String get activityDefaultRoom => 'General';

  @override
  String get activityYouInList => 'tú';

  @override
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Deshecho: al mismo tiempo te sacaste del grupo',
      'other': 'Deshecho: al mismo tiempo sacaste del grupo a $author',
    });
    String _temp1 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Deshecho: al mismo tiempo $by te sacó del grupo',
      'other': 'Deshecho: al mismo tiempo $by sacó del grupo a $author',
    });
    String _temp2 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Deshecho: al mismo tiempo te sacaron del grupo',
      'other': 'Deshecho: al mismo tiempo sacaron del grupo a $author',
    });
    String _temp3 = intl.Intl.selectLogic(byWho, {
      'me': '$_temp0',
      'other': '$_temp1',
      'none': '$_temp2',
    });
    return '$_temp3';
  }

  @override
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Deshecho: al mismo tiempo te moviste al final',
      'other': 'Deshecho: al mismo tiempo moviste a $author al final',
    });
    String _temp1 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Deshecho: al mismo tiempo $by te movió al final',
      'other': 'Deshecho: al mismo tiempo $by movió a $author al final',
    });
    String _temp2 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Deshecho: al mismo tiempo te movieron al final',
      'other': 'Deshecho: al mismo tiempo movieron a $author al final',
    });
    String _temp3 = intl.Intl.selectLogic(byWho, {
      'me': '$_temp0',
      'other': '$_temp1',
      'none': '$_temp2',
    });
    return '$_temp3';
  }

  @override
  String activityUndoneLeft(String author, String authorMe) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'Deshecho: saliste del grupo al mismo tiempo',
      'other': 'Deshecho: $author salió del grupo al mismo tiempo',
    });
    return '$_temp0';
  }

  @override
  String get activityAboutList =>
      'Todos en el grupo ven la misma lista, sacada del propio registro del grupo. Los cambios de imagen y banner se guardan aparte: son los que vio este dispositivo.';

  @override
  String get activityAboutOrder =>
      'En el orden propio del grupo; las horas son las del reloj de cada autor.';

  @override
  String get activityNothingYet => 'Nada todavía.';

  @override
  String get activityLoading => 'Cargando…';

  @override
  String get activityShowOlder => 'Mostrar anteriores';

  @override
  String activitySeenHere(String when) {
    return '$when · visto en este dispositivo';
  }

  @override
  String get bellNotifications => 'Notificaciones';

  @override
  String bellNewNotifications(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n notificaciones nuevas',
      one: '1 notificación nueva',
    );
    return '$_temp0';
  }

  @override
  String get bellClearAll => 'Borrar todo';

  @override
  String get bellNothingNew => 'Nada nuevo';

  @override
  String get bellFooter =>
      'Las tres últimas se quedan aquí. Las llamadas perdidas y las solicitudes también aparecen en Amigos.';

  @override
  String bellBothInRoom(String room) {
    return 'Ya estás en $room con esa persona.';
  }

  @override
  String get bellDismiss => 'Descartar';

  @override
  String bellFriendRequest(String name) {
    return '$name te envió una solicitud de amistad';
  }

  @override
  String bellCheckFingerprint(String fingerprint) {
    return 'Comprueba que la huella coincida: $fingerprint';
  }

  @override
  String bellJoinAsk(String name, String group) {
    return '$name pide unirse a “$group”';
  }

  @override
  String bellJoinAskNote(String fingerprint) {
    return 'Con uno de los enlaces de invitación de tu grupo. Huella: $fingerprint';
  }

  @override
  String get bellLetIn => 'Dejar entrar';

  @override
  String get bellSayNo => 'Rechazar';

  @override
  String bellGroupInvite(String name, String group) {
    return '$name te añadió a “$group”';
  }

  @override
  String bellMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n miembros',
      one: '1 miembro',
    );
    return '$_temp0';
  }

  @override
  String get bellJoin => 'Unirme';

  @override
  String bellNewMessages(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n mensajes nuevos de $name',
      one: 'Nuevo mensaje de $name',
    );
    return '$_temp0';
  }

  @override
  String get bellOpen => 'Abrir';

  @override
  String get bellMarkRead => 'Marcar como leído';

  @override
  String get bellFirewallTitle => 'Tu firewall deja fuera a algunos amigos';

  @override
  String get bellCopyCommand => 'Copiar comando';

  @override
  String get bellOtherSessionTitle => 'Abierto también en otro equipo';

  @override
  String get bellSettingsResetTitle => 'Se restablecieron algunos ajustes';

  @override
  String get bellSettingsResetText =>
      'Algunos de tus ajustes no se pudieron leer y han vuelto a sus valores predeterminados: revísalos en Ajustes. El archivo anterior se guardó junto al nuevo, como app.json.bad.';

  @override
  String get friendCardOnline => 'En línea';

  @override
  String get friendCardAway => 'Ausente';

  @override
  String get friendCardOffline => 'Sin conexión';

  @override
  String get friendCardCallsOnlyOnline =>
      'Las llamadas solo suenan cuando la otra persona está en línea.';

  @override
  String friendCardHangUpFirst(String name) {
    return 'Cuelga primero para llamar a $name.';
  }

  @override
  String friendCardBothInRoom(String room) {
    return 'Ya estás en $room con esa persona.';
  }

  @override
  String get friendCardMessage => 'Mensaje';

  @override
  String friendCardMutualFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n amigos en común',
      one: '1 amigo en común',
    );
    return '$_temp0';
  }

  @override
  String get memberCardSendRequest => 'Enviar solicitud de amistad';

  @override
  String get memberCardRequestSent => 'Solicitud de amistad enviada';

  @override
  String get memberCardAccept => 'Aceptar solicitud de amistad';

  @override
  String memberCardAsksYou(String name) {
    return '$name te envió una solicitud de amistad.';
  }

  @override
  String memberCardInGroup(String group) {
    return 'Miembro de $group';
  }

  @override
  String memberCardOnlyOnline(String name) {
    return 'Las solicitudes de amistad solo llegan mientras $name está en línea.';
  }

  @override
  String get memberCardShowOwn => 'Mostrar tu tarjeta';

  @override
  String get userPanelEditProfile => 'Editar perfil';

  @override
  String get userPanelStatus => 'Estado';

  @override
  String get userPanelOnline => 'En línea';

  @override
  String get userPanelAway => 'Ausente';

  @override
  String get userPanelOffline => 'Sin conexión';

  @override
  String get userPanelOfflineNote =>
      'Tus amigos y los miembros de tus grupos te ven sin conexión. Puedes seguir entrando en salas.';

  @override
  String get userPanelOnlineNote =>
      'Tus amigos y los miembros de tus grupos te ven en línea, o ausente tras 10 min sin usar este equipo.';

  @override
  String get userPanelBio => 'BIOGRAFÍA';

  @override
  String get userPanelAppearingOffline => 'Apareces sin conexión';

  @override
  String get userPanelCopyLinkLabel => 'Copiar un enlace de amistad';

  @override
  String userPanelLinkExpiresIn(String time) {
    return 'Caduca en $time';
  }

  @override
  String get userPanelLinkTerms => '1 persona · 10 min';

  @override
  String get userPanelCopyLink => 'Copiar enlace de amistad';

  @override
  String get switcherChatEnded => 'Chat directo · ya no está entre tus amigos';

  @override
  String get switcherChatOnline => 'Chat directo · en línea';

  @override
  String get switcherChatAway => 'Chat directo · ausente';

  @override
  String get switcherChat => 'Chat directo';

  @override
  String switcherFriend(String presence) {
    return 'Amigo · $presence';
  }

  @override
  String switcherVoiceRoom(String group) {
    return 'Sala de voz en $group';
  }

  @override
  String switcherVoiceRoomPeople(String group, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Sala de voz en $group · $n personas',
      one: 'Sala de voz en $group · $n persona',
    );
    return '$_temp0';
  }

  @override
  String switcherGroup(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Grupo · $n miembros',
      one: 'Grupo · 1 miembro',
    );
    return '$_temp0';
  }

  @override
  String get switcherPlaceholder => 'Ir a un chat, grupo o sala de voz';

  @override
  String get switcherGoTo => 'Ir a';

  @override
  String get switcherEmpty => 'Aún no hay grupos, amigos ni chats.';

  @override
  String switcherNoMatch(String query) {
    return 'Nada coincide con “$query”.';
  }

  @override
  String get switcherHintChoose => 'para elegir';

  @override
  String get switcherHintGo => 'para ir';

  @override
  String get switcherHintClose => 'para cerrar';

  @override
  String switcherTooltip(String shortcut) {
    return 'Ir a un chat, grupo o sala de voz ($shortcut)';
  }

  @override
  String get friendsIntro =>
      'Cada amistad vive solo en los dos dispositivos. Sin directorio, sin servidor.';

  @override
  String get friendsTabAll => 'Todos';

  @override
  String get friendsTabOnline => 'En línea';

  @override
  String get friendsTabRequests => 'Solicitudes';

  @override
  String get friendsSearch => 'Buscar amigos';

  @override
  String get friendsNone =>
      'Aún no tienes amigos. Comparte tu QR, enlace o código, o usa el de otra persona.';

  @override
  String get friendsNoneOnline => 'Ningún amigo está en línea ahora mismo.';

  @override
  String get friendsNoMatch => 'Nadie coincide con la búsqueda.';

  @override
  String get friendsInARoom => 'En una sala';

  @override
  String friendsInRoomOf(String group) {
    return 'En sala · $group';
  }

  @override
  String get friendsOnline => 'En línea';

  @override
  String get friendsAway => 'Ausente';

  @override
  String get friendsNotSeenYet => 'Aún no se ha conectado';

  @override
  String friendsSeen(String when) {
    return 'Última vez $when';
  }

  @override
  String friendsShowCard(String name) {
    return 'Mostrar la tarjeta de $name';
  }

  @override
  String friendsMissedCall(String when) {
    return 'Llamada perdida · $when';
  }

  @override
  String get friendsMessage => 'Mensaje';

  @override
  String friendsOfflineNoCall(String name) {
    return '$name no está en línea: las llamadas solo suenan cuando la otra persona está en línea';
  }

  @override
  String friendsHangUpFirst(String name) {
    return 'Cuelga primero para llamar a $name';
  }

  @override
  String friendsBothInRoom(String room) {
    return 'Ya estás en $room con esa persona.';
  }

  @override
  String friendsOptions(String name) {
    return 'Opciones de $name';
  }

  @override
  String friendsRemoveTitle(String name) {
    return '¿Quitar a $name de tus amigos?';
  }

  @override
  String get friendsRemoveText =>
      'Cada uno desaparece de la lista de amigos del otro, sin aviso. Los grupos en común no cambian. Para volver a ser amigos hace falta otra invitación.';

  @override
  String get friendsRemove => 'Quitar';

  @override
  String friendsAddToGroup(String group) {
    return 'Añadir a “$group”';
  }

  @override
  String get friendsRemoveMenu => 'Quitar de amigos…';

  @override
  String get friendsNoRequests => 'No hay solicitudes pendientes.';

  @override
  String friendsRequestIncoming(String when) {
    return 'Solicitud de amistad · $when';
  }

  @override
  String friendsRequestOutgoing(String when) {
    return 'Enviada $when · esperando respuesta';
  }

  @override
  String get friendsPendingRequest => 'Solicitud pendiente';

  @override
  String friendsCheckFingerprint(String name) {
    return 'Comprueba que la huella coincida con la que $name ve en su pantalla.';
  }

  @override
  String get friendsAddTitle => 'Añadir un amigo';

  @override
  String get friendsAddNote =>
      'Tus dispositivos intercambian claves directamente.';

  @override
  String get friendsTabCode => 'Código';

  @override
  String get friendsTabLink => 'Enlace';

  @override
  String get friendsTabQr => 'QR';

  @override
  String get friendsMakeInvitation => 'Crear una invitación';

  @override
  String get friendsInvitationExpired => 'Esta invitación ha caducado.';

  @override
  String get friendsMakeAnother => 'Crear otra';

  @override
  String friendsHintQr(String time) {
    return 'Escanéalo desde Fogata · caduca en $time';
  }

  @override
  String friendsHintLink(String time) {
    return 'Envíalo a una sola persona · caduca en $time';
  }

  @override
  String friendsHintCode(String time) {
    return 'Díselo en voz alta a una sola persona · caduca en $time';
  }

  @override
  String get friendsHiddenCode => 'Oculto por si compartes la pantalla.';

  @override
  String get friendsHiddenLink =>
      'Oculto por si compartes la pantalla. Lleva las direcciones de red de este equipo: compártelo solo en privado.';

  @override
  String get friendsGotLink =>
      '¿Tienes un enlace de amistad o de grupo, o un código?';

  @override
  String get friendsPastePlaceholder => 'Pega un enlace o código';

  @override
  String get friendsLinkOrCode => 'Enlace o código';

  @override
  String get friendsAdd => 'Añadir';

  @override
  String get friendsQrLabel => 'Código QR de tu invitación';

  @override
  String friendsAddedToGroup(String name) {
    return '$name recibirá el grupo la próxima vez que los dos estén en línea.';
  }

  @override
  String get directToday => 'Hoy';

  @override
  String get directYesterday => 'Ayer';

  @override
  String directTimerOff(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Desactivaste los mensajes temporales',
      'other': '$name desactivó los mensajes temporales',
    });
    return '$_temp0';
  }

  @override
  String directTimerFirst(String time) {
    return 'Mensajes temporales activados: $time';
  }

  @override
  String directTimerOn(String name, String mine, String time) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Activaste los mensajes temporales: $time',
      'other': '$name activó los mensajes temporales: $time',
    });
    return '$_temp0';
  }

  @override
  String directPinnedLine(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Fijaste un mensaje',
      'other': '$name fijó un mensaje',
    });
    return '$_temp0';
  }

  @override
  String get directCallFewSeconds => 'unos segundos';

  @override
  String directCallMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minutos',
      one: '1 minuto',
    );
    return '$_temp0';
  }

  @override
  String directCallHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n horas',
      one: '1 hora',
    );
    return '$_temp0';
  }

  @override
  String directCallHoursMinutes(int hours, int minutes) {
    String _temp0 = intl.Intl.pluralLogic(
      hours,
      locale: localeName,
      other: '$hours horas',
      one: '1 hora',
    );
    String _temp1 = intl.Intl.pluralLogic(
      minutes,
      locale: localeName,
      other: '$minutes minutos',
      one: '1 minuto',
    );
    return '$_temp0 $_temp1';
  }

  @override
  String directCallLine(String name, String mine, String length) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Iniciaste una llamada que duró $length',
      'other': '$name inició una llamada que duró $length',
    });
    return '$_temp0';
  }

  @override
  String get directBlockedOffline =>
      'Apareces sin conexión. Conéctate para escribir.';

  @override
  String directBlockedFriendOffline(String name) {
    return '$name no está en línea. Podrás escribir cuando los dos estén en línea.';
  }

  @override
  String get directOnline => 'En línea';

  @override
  String get directAway => 'Ausente';

  @override
  String get directOffline => 'Sin conexión';

  @override
  String directOfflineSeen(String when) {
    return 'Desconectado · última vez $when';
  }

  @override
  String get directHeading => 'MENSAJES DIRECTOS';

  @override
  String directPreviewMine(String text) {
    return 'Tú: $text';
  }

  @override
  String directOpenChat(String name) {
    return 'Abrir el chat con $name';
  }

  @override
  String directShowCard(String name) {
    return 'Mostrar la tarjeta de $name';
  }

  @override
  String get directNoLongerFriend => 'Ya no está entre tus amigos';

  @override
  String directCallOffline(String name) {
    return '$name no está en línea: las llamadas solo suenan cuando la otra persona está en línea';
  }

  @override
  String directHangUpFirst(String name) {
    return 'Cuelga primero para llamar a $name';
  }

  @override
  String directPinnedCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n mensajes fijados',
      one: '1 mensaje fijado',
    );
    return '$_temp0';
  }

  @override
  String get directPinnedTitle => 'Mensajes fijados';

  @override
  String directPinnedNote(int max) {
    return 'Fijados para los dos, y no desaparecen. Hasta $max.';
  }

  @override
  String get directNothingPinned => 'Aún no hay nada fijado';

  @override
  String get directNothingPinnedNote =>
      'Pasa el cursor sobre un mensaje y haz clic en Fijar para guardarlo aquí.';

  @override
  String directDayTime(String day, String time) {
    return '$day, $time';
  }

  @override
  String directFilesCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n archivos',
    );
    return '$_temp0';
  }

  @override
  String directShowMessageFrom(String name) {
    return 'Mostrar el mensaje de $name';
  }

  @override
  String get directUnpin => 'Desfijar';

  @override
  String get directPin => 'Fijar';

  @override
  String get directDeleteMessage => 'Borrar para mí';

  @override
  String get directDeleteMessageTitle => '¿Borrar este mensaje?';

  @override
  String directDeleteMessageNote(String name) {
    return 'Se borra de este equipo, con sus imágenes y archivos. $name conserva su copia.';
  }

  @override
  String get directDeleteMessageAction => 'Borrar';

  @override
  String get directDeleteForBoth => 'Borrar para los dos';

  @override
  String directDeleteOwnNote(String name) {
    return 'Borrar para los dos también lo quita del chat de $name en cuanto su Fogata lo reciba. Borrar para mí lo deja allí.';
  }

  @override
  String directTimerLabel(String time) {
    return 'Mensajes temporales: $time';
  }

  @override
  String get directTimerTitle => 'Mensajes temporales';

  @override
  String get directCloseMenu => 'Cerrar el menú';

  @override
  String get directTimerNote =>
      'Los mensajes nuevos desaparecen en los dos dispositivos, con sus imágenes y archivos.';

  @override
  String get directEmptyNote =>
      'Lo que escribes se guarda solo en tu equipo y en el suyo.';

  @override
  String directPinLineLabel(String line) {
    return '$line: mostrarlo';
  }

  @override
  String get directShowMessage => 'Mostrar el mensaje';

  @override
  String get directPinnedStays => 'Fijado: se queda hasta que lo desfijen';

  @override
  String get directPinned => 'Fijado';

  @override
  String directPinsFull(int max) {
    return 'Se pueden fijar hasta $max mensajes: desfija uno primero';
  }

  @override
  String get directDelivered => 'Entregado';

  @override
  String get directNotDelivered => 'No entregado';

  @override
  String get directSending => 'Enviando…';

  @override
  String directWaiting(String name) {
    return 'Esperando a que $name se conecte · se enviará cuando los dos estén en línea';
  }

  @override
  String get directAvailableWhenYouOnline => 'disponible cuando te conectes';

  @override
  String directAvailableWhenOnline(String name) {
    return 'disponible cuando $name esté en línea';
  }

  @override
  String get directDownloadedByItself => 'descargada automáticamente';

  @override
  String directOpenFile(String name) {
    return 'Abrir $name';
  }

  @override
  String directPlayFile(String name) {
    return 'Reproducir $name';
  }

  @override
  String directDownloadSize(String size) {
    return 'Descargar · $size';
  }

  @override
  String directVideoPlaysWhenHere(String name, String length) {
    return '$name · $length · se reproduce cuando llegue';
  }

  @override
  String directVideoNotHere(String name, String length, String when) {
    return '$name · $length · $when';
  }

  @override
  String directGettingIt(int percent) {
    return 'Recibiendo · $percent %';
  }

  @override
  String directFileNotHere(String size, String when) {
    return '$size · $when';
  }

  @override
  String get directSaveToDownloads => 'Guardar en Descargas';

  @override
  String get directRemovedForSpace => 'Eliminado aquí para liberar espacio';

  @override
  String get directAskAgain => 'Pedir de nuevo';

  @override
  String get directClosePicture => 'Cerrar la imagen';

  @override
  String get directCloseVideo => 'Cerrar el video';

  @override
  String get directCannotPlay => 'Este equipo no puede reproducir este video.';

  @override
  String get directPause => 'Pausar';

  @override
  String get directPlay => 'Reproducir';

  @override
  String get directSoundOn => 'Activar sonido';

  @override
  String get directMute => 'Silenciar';

  @override
  String directRemoveFilesTitle(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '¿Quitar los $n archivos?',
      one: '¿Quitar el archivo?',
    );
    return '$_temp0';
  }

  @override
  String get directRemoveFilesText =>
      'Se quitan de este mensaje. Los archivos de tu equipo se quedan como están.';

  @override
  String get directDraftKept =>
      'Lo que estabas escribiendo se queda aquí para cuando puedas enviarlo.';

  @override
  String directMessagePlaceholder(String name) {
    return 'Mensaje para $name';
  }

  @override
  String get directAddMessageOptional => 'Añade un mensaje (opcional)';

  @override
  String get directAutoDownloadNote =>
      'Las imágenes de hasta 10 MB se descargan solas; los videos y archivos, cuando los abres.';

  @override
  String get directVoiceRecord => 'Grabar un mensaje de voz';

  @override
  String get directVoiceInCall => 'No puedes grabar durante una llamada';

  @override
  String directVoiceWaitingElsewhere(String name) {
    return 'Primero envía o elimina el mensaje de voz de tu chat con $name.';
  }

  @override
  String get directVoiceDelete => 'Eliminar';

  @override
  String get directVoiceTooShort =>
      'Demasiado corto para enviarlo: un mensaje de voz dura al menos medio segundo.';

  @override
  String directVoiceLeft(String time) {
    return 'Quedan $time';
  }

  @override
  String directVoiceRecordingFrom(String microphone) {
    return 'Grabando desde $microphone';
  }

  @override
  String get directVoiceRecordingDefault =>
      'El micrófono elegido no está disponible: grabando con el predeterminado';

  @override
  String directVoiceKeys(String esc, String enter) {
    return '$esc lo elimina, $enter lo envía';
  }

  @override
  String directVoiceKeySend(String enter) {
    return '$enter lo envía';
  }

  @override
  String directVoicePreview(String length) {
    return 'Mensaje de voz · $length';
  }

  @override
  String get directVoiceMessage => 'Mensaje de voz';

  @override
  String get directVoiceNoMic => 'Fogata no puede usar el micrófono.';

  @override
  String get directVoiceMicStopped =>
      'El micrófono se detuvo. Aún puedes enviar lo grabado.';

  @override
  String directVoiceNoMicCheck(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Comprueba que haya uno conectado, elegido en Voz y video y permitido en Ajustes del Sistema › Privacidad y seguridad.',
      'other': 'Comprueba que haya uno conectado y elegido en Voz y video.',
    });
    return '$_temp0';
  }

  @override
  String get directVoiceSpeed => 'Velocidad de reproducción';

  @override
  String get directVoiceNotPlayed => 'Sin escuchar';

  @override
  String directRemoveFile(String name) {
    return 'Quitar $name';
  }

  @override
  String get directAltered =>
      'El historial de este chat en este equipo cambió desde la última vez que Fogata lo guardó. Puede que falten mensajes.';

  @override
  String directEnded(String name) {
    return '$name ya no está entre tus amigos. Este chat es de solo lectura.';
  }

  @override
  String get directDeleteChat => 'Eliminar chat';

  @override
  String directDeleteTitle(String name) {
    return '¿Eliminar el chat con $name?';
  }

  @override
  String directDeleteText(String name) {
    return 'Sus mensajes, imágenes, videos y archivos desaparecen de este equipo. $name conserva su copia.';
  }

  @override
  String get directDelete => 'Eliminar';

  @override
  String get directUsageMessages => 'Mensajes';

  @override
  String get directUsagePictures => 'Imágenes';

  @override
  String get directUsageVideos => 'Videos';

  @override
  String get directUsageFiles => 'Archivos';

  @override
  String get directUsageHeading => 'ESTE CHAT EN ESTE DISPOSITIVO';

  @override
  String get directFreeUpSpace => 'Liberar espacio…';

  @override
  String get directAddFilesTooltip => 'Añadir imágenes, videos o archivos';

  @override
  String get directAddFiles => 'Añadir archivos';

  @override
  String get accountMoveLessThanMinuteLeft => 'queda menos de un minuto';

  @override
  String get accountMoveAboutMinuteLeft => 'queda cerca de un minuto';

  @override
  String accountMoveAboutMinutesLeft(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'quedan unos $n minutos',
      one: 'queda cerca de un minuto',
    );
    return '$_temp0';
  }

  @override
  String accountMoveNoRoom(String need, String free) {
    return 'Este equipo necesita $need libres para la cuenta, y tiene $free.';
  }

  @override
  String accountMoveTooLarge(String size) {
    return 'El otro equipo ofreció $size, mucho más de lo que ocupa una cuenta.';
  }

  @override
  String get accountMoveBusy =>
      'Este equipo está terminando otro traslado: termínalo o descártalo primero.';

  @override
  String get accountMoveNothingToFinish =>
      'Este equipo no tiene ningún traslado pendiente desde ese equipo.';

  @override
  String get accountMoveNetworkStopped => 'La red se detuvo.';

  @override
  String get accountMoveConnectionLost => 'Se perdió la conexión.';

  @override
  String get accountMovePeerNoRoom =>
      'El equipo nuevo no tiene suficiente espacio libre.';

  @override
  String get accountMovePeerTooLarge =>
      'La cuenta es más grande de lo que admite un traslado.';

  @override
  String get accountMovePeerBusy =>
      'El equipo nuevo está terminando otro traslado.';

  @override
  String get accountMovePeerCouldNotKeep =>
      'El equipo nuevo no pudo guardar los archivos.';

  @override
  String get accountMovePeerCancelled =>
      'El traslado se canceló en el otro equipo.';

  @override
  String get accountMovePeerCouldNotFinish =>
      'El equipo nuevo aún no pudo terminar el traslado.';

  @override
  String get accountMovePeerStopped =>
      'El traslado se detuvo en el otro equipo.';

  @override
  String get accountMovePeerNothingToFinish =>
      'Ese equipo no tiene ningún traslado pendiente desde este.';

  @override
  String get accountMovePeerNotProven =>
      'Ese equipo no demostró que tenga este traslado.';

  @override
  String get accountMoveWrongCode =>
      'El otro equipo no aceptó el código, así que no se envió nada.';

  @override
  String get accountMoveExpired => 'El código caducó antes de usarse.';

  @override
  String get accountMoveTooManyTries =>
      'Se probaron varios códigos incorrectos; el código dejó de funcionar.';

  @override
  String get accountMoveNoContact =>
      'Los dos equipos no pudieron conectarse entre sí.';

  @override
  String get accountMoveUnconfirmed => 'No se confirmó el final del traslado.';

  @override
  String get accountMoveCancelled => 'Cancelado.';

  @override
  String get accountMoveInterrupted =>
      'El traslado se detuvo antes de terminar; no cambió nada.';

  @override
  String accountMoveInterruptedWhy(String why) {
    return 'El traslado se detuvo antes de terminar; no cambió nada. $why';
  }

  @override
  String get accountMoveDeclined => 'El otro equipo detuvo el traslado.';

  @override
  String accountMoveDeclinedWhy(String why) {
    return 'El otro equipo detuvo el traslado. $why';
  }

  @override
  String get accountMoveLater =>
      'La cuenta llegó y se instalará la próxima vez que Fogata se inicie aquí.';

  @override
  String accountMoveLaterWhy(String why) {
    return 'La cuenta llegó y se instalará la próxima vez que Fogata se inicie aquí. $why';
  }

  @override
  String get accountMoveDidNotFinish => 'No terminó.';

  @override
  String get accountMoveAnotherComputer => 'otro equipo';

  @override
  String get accountMoveTheOtherComputer => 'el otro equipo';

  @override
  String get moveTooLateBeingPlaced =>
      'Demasiado tarde para detenerlo: la cuenta se está instalando.';

  @override
  String get moveTooLateDiscard =>
      'Demasiado tarde: la cuenta se está instalando.';

  @override
  String get moveTooLateOtherHasIt =>
      'Demasiado tarde para detenerlo: el otro equipo ya tiene la cuenta.';

  @override
  String moveFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n amigos',
      one: '1 amigo',
    );
    return '$_temp0';
  }

  @override
  String moveGroups(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n grupos',
      one: '1 grupo',
    );
    return '$_temp0';
  }

  @override
  String moveGroupsWithKeys(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n grupos, con sus claves',
      one: '1 grupo, con sus claves',
    );
    return '$_temp0';
  }

  @override
  String moveChats(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n chats',
      one: '1 chat',
    );
    return '$_temp0';
  }

  @override
  String moveInAll(String size) {
    return '$size en total';
  }

  @override
  String get moveYourSettings => 'Tus ajustes';

  @override
  String get moveNotDevices => 'sin el micrófono ni la cámara elegidos';

  @override
  String get moveFinishingBoth => 'Terminando en los dos equipos…';

  @override
  String movePercent(int percent) {
    return '$percent %';
  }

  @override
  String movePercentLeft(int percent, String left) {
    return '$percent % · $left';
  }

  @override
  String get moveStepIdentity => 'Identidad y perfil';

  @override
  String get moveStepFriends => 'Amigos';

  @override
  String get moveStepGroups => 'Grupos y sus claves';

  @override
  String get moveStepChats => 'Chats, imágenes y archivos';

  @override
  String moveBytesOf(String done, String total) {
    return '$done de $total';
  }

  @override
  String get moveStepFinishing => 'Terminando en los dos equipos';

  @override
  String moveBringingNamed(String name) {
    return 'Trayendo la cuenta de $name';
  }

  @override
  String get moveBringingYours => 'Trayendo tu cuenta';

  @override
  String get moveKeepBothOn =>
      'Mantén los dos equipos encendidos y Fogata abierto.';

  @override
  String get moveKeepOpenNote =>
      'Mantén Fogata abierto en los dos equipos. Es más rápido en la misma red; si no, se conectan por internet, directamente.';

  @override
  String moveFinishBringingNamed(String name) {
    return 'Termina de traer la cuenta de $name';
  }

  @override
  String get moveFinishBringingYours => 'Termina de traer tu cuenta';

  @override
  String get moveEverythingCame =>
      'Ya llegó todo. Escribe este código en el otro equipo para terminar.';

  @override
  String moveEverythingCameFrom(String computer) {
    return 'Ya llegó todo. Escribe este código en “$computer” para terminar.';
  }

  @override
  String get moveBringHereTitle => 'Trae tu cuenta aquí';

  @override
  String get moveBringHereSub =>
      'Escribe este código en el equipo que tiene tu cuenta.';

  @override
  String get moveHideCode => 'Ocultar el código';

  @override
  String get moveShowCode => 'Mostrar el código';

  @override
  String moveExpiresIn(String time) {
    return 'caduca en $time';
  }

  @override
  String moveStepFogataShows(String what) {
    return 'En ese equipo, Fogata muestra $what.';
  }

  @override
  String get moveWaitingToConfirm => 'Esperando confirmación';

  @override
  String get moveStepOpenThere => 'En ese equipo, abre Fogata.';

  @override
  String moveStepTypeUnder(String where) {
    return 'Escribe el código en $where.';
  }

  @override
  String get moveFinishTheMove => 'Terminar el traslado';

  @override
  String moveStepGoTo(String where) {
    return 'Ve a $where.';
  }

  @override
  String get moveSettingsPath => 'Ajustes → Mover cuenta';

  @override
  String get moveToAnotherComputer => 'Mover a otro equipo';

  @override
  String get moveWaitingForOther => 'Esperando a tu otro equipo…';

  @override
  String moveWrongCodesTyped(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Se escribieron códigos incorrectos $n veces en otros equipos.',
      one: 'Se escribió un código incorrecto en otro equipo.',
    );
    return '$_temp0';
  }

  @override
  String get moveThrowAwayInstead => 'Descartarla en su lugar…';

  @override
  String get moveFoundOther => 'Se encontró tu otro equipo';

  @override
  String get moveFoundSameNetwork => 'Misma red · cifrado entre los dos';

  @override
  String get moveFoundInternet => 'Por internet · cifrado entre los dos';

  @override
  String get moveConfirmThere => 'Confirma en el otro equipo para empezar';

  @override
  String get moveSameAccountNote =>
      'Esta es la cuenta que ya está en este equipo. No se pierde nada al traerla aquí.';

  @override
  String get moveTakeYes => 'Traerla aquí';

  @override
  String get moveWelcomeBack => 'Hola de nuevo';

  @override
  String moveWelcomeBackNamed(String name) {
    return 'Hola de nuevo, $name';
  }

  @override
  String get moveDoneSub =>
      'Tu cuenta ya está en este equipo. El otro equipo dejó de usarla.';

  @override
  String get moveOpenFogata => 'Abrir Fogata';

  @override
  String get moveCodeExpiredLabel => 'caducado';

  @override
  String get moveCodeStoppedLabel => 'dejó de funcionar';

  @override
  String get moveCodeExpiredNote =>
      'Este código caducó antes de que tu otro equipo lo usara. Los códigos duran 10 minutos.';

  @override
  String get moveCodeStoppedNote =>
      'Se escribieron códigos incorrectos varias veces en otros equipos, así que este código dejó de funcionar.';

  @override
  String get moveNewCode => 'Nuevo código';

  @override
  String get moveAlmostThere => 'Casi listo';

  @override
  String get moveLaterSub =>
      'Tu cuenta llegó completa, pero aún no se pudo instalar. Se instalará la próxima vez que Fogata se inicie aquí.';

  @override
  String get moveStartAgain => 'Reiniciar Fogata';

  @override
  String get moveUnconfirmedSub =>
      'Llegó todo, pero el otro equipo no confirmó el final. Muestra un código y escríbelo allí para terminar el traslado.';

  @override
  String get moveStoppedThereTitle => 'El traslado se detuvo en el otro equipo';

  @override
  String get moveStoppedThereSub =>
      'No cambió nada: tu cuenta sigue en el otro equipo y aquí no se guardó nada de ella.';

  @override
  String get moveCouldNotStartTitle => 'No se pudo iniciar el traslado';

  @override
  String get moveSomethingWrongHere => 'Algo salió mal en este equipo.';

  @override
  String get moveHalfwayTitle => 'El traslado se detuvo a medias';

  @override
  String get moveHalfwaySub =>
      'Se perdió la conexión con tu otro equipo. No cambió nada: tu cuenta sigue en el otro equipo y aquí no se guardó nada de ella.';

  @override
  String get moveShowCodeToFinish => 'Mostrar un código para terminar';

  @override
  String get moveTryNewCode => 'Reintentar con un código nuevo';

  @override
  String moveBackToNamed(String name) {
    return 'Volver a $name';
  }

  @override
  String get moveBackToThisProfile => 'Volver a este perfil';

  @override
  String get moveNewProfileInstead => 'Crear un perfil nuevo en su lugar';

  @override
  String get moveDiscardTitle => '¿Descartar la cuenta que llegó?';

  @override
  String get moveDiscardText =>
      'Hazlo solo si el otro equipo sigue usándola. Ese equipo espera para confirmar hasta que su usuario decida seguir usándola allí.';

  @override
  String get moveKeepIt => 'Conservarla';

  @override
  String get moveThrowAway => 'Descartarla';

  @override
  String get moveBringFromAnother => 'Trae tu cuenta desde otro equipo';

  @override
  String get moveAlreadyOnFogata => '¿Ya usas Fogata?';

  @override
  String get moveCodeTriedBefore =>
      'Ese código ya falló una vez. Consigue un código nuevo en el equipo nuevo.';

  @override
  String moveMovingTo(String computer) {
    return 'Moviendo tu cuenta a “$computer”';
  }

  @override
  String get moveKeepBothOnOffline =>
      'Mantén los dos equipos encendidos y Fogata abierto. Mientras tanto, Fogata está desconectado aquí.';

  @override
  String get moveCodeFieldLabel => 'El código que muestra el equipo nuevo';

  @override
  String get moveToAnotherSub =>
      'Tu identidad, amigos, grupos, chats, archivos y ajustes pasan al otro equipo, directamente desde este y cifrados. Después, este equipo deja de usar tu cuenta, así que nunca hay un segundo tú.';

  @override
  String moveStepChoose(String choice) {
    return 'En el equipo nuevo, abre Fogata y elige $choice.';
  }

  @override
  String get moveStepTypeItShows => 'Escribe el código que muestra:';

  @override
  String get moveWaitingForAnswer => 'Esperando tu respuesta…';

  @override
  String get moveLookingForCode =>
      'Buscando el equipo que muestra este código…';

  @override
  String get moveOfflineWhileMoving =>
      'Fogata se desconecta en este equipo mientras se mueve la cuenta.';

  @override
  String get moveWrongCodeNote =>
      'El equipo nuevo no aceptó ese código, así que no se envió nada. Revísalo y vuelve a escribirlo. Si sigue fallando, consigue un código nuevo allí.';

  @override
  String get moveCantReachNew => 'No se puede conectar con el equipo nuevo.';

  @override
  String get moveCantReachNewNote =>
      'Los dos se buscaron durante 3 minutos. Ponlos en la misma red (la misma Wi-Fi o un cable) y vuelve a intentarlo. Tu cuenta sigue aquí y Fogata vuelve a estar en línea.';

  @override
  String get moveDeclinedNote =>
      'El traslado se detuvo en el equipo nuevo. No se movió nada: tu cuenta sigue aquí y Fogata vuelve a estar en línea.';

  @override
  String get moveCouldNotStart => 'No se pudo iniciar el traslado.';

  @override
  String get moveHalfwayNote =>
      'El traslado se detuvo a medias: se perdió la conexión con el equipo nuevo. No cambió nada: tu cuenta sigue aquí y Fogata vuelve a estar en línea.';

  @override
  String get moveThisProfileWords => 'este perfil';

  @override
  String get moveBringCardTitle => 'Traer una cuenta aquí';

  @override
  String get moveBringCardSub =>
      'Muestra un código en este equipo y escríbelo en el que tiene la cuenta.';

  @override
  String get moveBringCardButton => 'Traer una cuenta aquí…';

  @override
  String moveReplacesHere(String name) {
    return 'Reemplaza a $name en este equipo.';
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
      other: '$friends amigos',
      one: '1 amigo',
    );
    String _temp1 = intl.Intl.pluralLogic(
      groups,
      locale: localeName,
      other: '$groups grupos',
      one: '1 grupo',
    );
    String _temp2 = intl.Intl.pluralLogic(
      chats,
      locale: localeName,
      other: '$chats chats',
      one: '1 chat',
    );
    return '$bold, el perfil que hay ahora en este equipo, será reemplazado, y sus $_temp0, $_temp1 y $_temp2 se eliminarán aquí. Para conservar $name, muévelo antes a otro equipo.';
  }

  @override
  String moveReplaceAndShow(String name) {
    return 'Reemplazar $name y mostrar un código';
  }

  @override
  String moveConfirmTitle(String computer) {
    return '¿Mover tu cuenta a “$computer”?';
  }

  @override
  String get moveConfirmSameNetwork =>
      'Encontrado en la misma red. Todo va cifrado, directamente a él.';

  @override
  String get moveConfirmInternet =>
      'Encontrado por internet. Todo va cifrado, directamente a él.';

  @override
  String moveConfirmReplaces(String name) {
    return 'Reemplaza allí el perfil $name.';
  }

  @override
  String get moveConfirmNote =>
      'Cuando termine, este equipo dejará de usar tu cuenta y no podrá volver a usarla. Después puedes eliminar lo que quede aquí.';

  @override
  String get moveConfirmYes => 'Mover mi cuenta';

  @override
  String get moveKeepTitle => '¿Seguir usando tu cuenta aquí?';

  @override
  String get moveKeepText =>
      'Solo si el equipo nuevo no la tiene: abre Fogata allí primero. Si los dos la usaran, tus amigos y grupos verían dos cuentas tuyas, y los mensajes solo llegarían a una.';

  @override
  String get moveKeepYes => 'Seguir usándola aquí';

  @override
  String moveWaitingFor(String computer) {
    return 'Esperando a que “$computer” confirme';
  }

  @override
  String get moveWaitingText =>
      'Tu cuenta se envió, pero la conexión se cortó antes de que el equipo nuevo dijera que está listo. Mientras tanto, este equipo no usará la cuenta.';

  @override
  String get moveFinishText =>
      'Abre Fogata en el equipo nuevo: muestra un código. Escríbelo aquí.';

  @override
  String get moveFinish => 'Terminar';

  @override
  String get moveFinishWrongCode =>
      'El equipo nuevo no aceptó ese código. Revísalo y vuelve a escribirlo.';

  @override
  String get moveFinishNoContact =>
      'No se puede conectar con el equipo nuevo. Pon los dos en la misma red y vuelve a intentarlo.';

  @override
  String get moveKeepThisOne =>
      'El equipo nuevo no la tiene: seguir usándola aquí…';

  @override
  String moveMovedYours(String computer) {
    return 'Tu cuenta se movió a “$computer”';
  }

  @override
  String moveMovedNamed(String name, String computer) {
    return 'La cuenta de $name se movió a “$computer”';
  }

  @override
  String get moveMovedNoteYou =>
      'Este equipo ya no la usa: tus amigos y grupos te encuentran en el otro.';

  @override
  String moveMovedNoteNamed(String name) {
    return 'Este equipo ya no la usa: los amigos y grupos encuentran a $name en el otro.';
  }

  @override
  String moveMovedTodayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'Hoy a la $time.',
      'other': 'Hoy a las $time.',
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
  String get moveLeftHere => 'Lo que queda aquí';

  @override
  String get moveLeftHereText =>
      'Chats, imágenes y archivos de antes del traslado, cifrados. El otro equipo tiene su propia copia.';

  @override
  String get moveDeleteData => 'Eliminar los datos de Fogata en este equipo';

  @override
  String get moveAfterDeleting =>
      'Después de eliminarlos, Fogata empieza aquí como nuevo.';

  @override
  String get moveCodeNotACode =>
      'Un código tiene diez letras y números, como XXXXX-XXXXX.';

  @override
  String get moveCodeMistyped =>
      'Ese código no es válido: revisa las letras y los números.';

  @override
  String get welcomeHeadline => 'Habla como si estuvieras\nen la misma sala.';

  @override
  String get welcomeIntro =>
      'Voz, video y chat con amigos, familia o compañeros, directamente de tu equipo al suyo.';

  @override
  String get welcomeNoAccount => 'Sin cuenta';

  @override
  String get welcomeEncrypted => 'Cifrado';

  @override
  String get welcomeEphemeral => 'Salas efímeras';

  @override
  String get welcomeCharacter =>
      'El personaje de Fogata, una pequeña llama con auriculares, haciendo el signo de la paz';

  @override
  String get welcomeImageUnreadable => 'No se pudo leer esa imagen.';

  @override
  String get welcomeKeychainTitle => 'Tus datos aún no se pueden cifrar';

  @override
  String welcomeKeychainText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' En Linux, el llavero es un Secret Service en ejecución, como KWallet o GNOME Keyring: inicia o desbloquea uno y vuelve a intentarlo.',
      'other': ' Desbloquea o repara el llavero y vuelve a intentarlo.',
    });
    return 'Fogata cifra tu identidad, tus amigos y tus mensajes directos con una clave que guarda en el llavero del sistema, y el llavero no la guardó.$_temp0';
  }

  @override
  String get welcomeKeychainRetry => 'Reintentar';

  @override
  String get welcomeKeychainPlain => 'Seguir sin cifrar';

  @override
  String get welcomeKeychainPlainNote =>
      'Sin cifrar, cualquiera que copie los archivos de este equipo puede leerlos. Ajustes seguirá avisándolo, y Fogata lo cifrará todo la primera vez que se abra con un llavero.';

  @override
  String get welcomeWriteName => 'Escribe tu nombre.';

  @override
  String get welcomeCardTitle => '¿Cómo quieres que te vea tu gente?';

  @override
  String get welcomeCardSub =>
      'Un nombre y una imagen. Cámbialos cuando quieras.';

  @override
  String get welcomeNoServer =>
      'Nada va a un servidor de Fogata: no hay ninguno. La red pública IPFS solo se usa como guía telefónica para encontrar a tus amigos.';

  @override
  String get welcomeYourName => 'Tu nombre';

  @override
  String welcomePictureLabel(String name) {
    return 'Imagen: $name';
  }

  @override
  String get welcomeUploadPicture => 'Subir una imagen';

  @override
  String get welcomeNameLabel => 'Nombre';

  @override
  String get welcomeCreate => 'Crear';

  @override
  String get welcomeReady => 'Listo';

  @override
  String get welcomeCreateLabel => 'Crear identidad';

  @override
  String welcomeKeyLabel(String key) {
    return 'Clave creada en este equipo: $key';
  }

  @override
  String get welcomeNewKey => 'Crear otra clave';

  @override
  String get stoppedTitle => 'Fogata dejó de funcionar.';

  @override
  String get stoppedStartingAgain => 'Volviendo a iniciarlo…';

  @override
  String get stoppedRestartToConnect => 'Reinícialo para volver a conectarte.';

  @override
  String get stoppedRestartNow => 'Reiniciar ahora';

  @override
  String get settingsSectionProfile => 'Mi perfil';

  @override
  String get settingsSectionMove => 'Mover cuenta';

  @override
  String get settingsSectionAppearance => 'Apariencia';

  @override
  String get settingsSectionLanguageTime => 'Idioma y hora';

  @override
  String get settingsSectionVoice => 'Voz y video';

  @override
  String get settingsSectionSounds => 'Sonidos';

  @override
  String get settingsSectionBehaviour => 'Comportamiento';

  @override
  String get settingsSectionNetwork => 'Red';

  @override
  String get settingsSectionStorage => 'Almacenamiento y descargas';

  @override
  String get settingsHeadingUser => 'Usuario';

  @override
  String get settingsHeadingApp => 'Aplicación';

  @override
  String get settingsTitle => 'Ajustes';

  @override
  String get settingsAll => 'Todos los ajustes';

  @override
  String get settingsCloseTooltip => 'Cerrar ajustes';

  @override
  String get settingsUnsavedSaveTitle => '¿Guardar los cambios?';

  @override
  String get settingsUnsavedDiscardTitle => '¿Descartar los cambios?';

  @override
  String get settingsUnsavedSaveText =>
      'Tu perfil tiene cambios que aún no se han guardado.';

  @override
  String get settingsUnsavedDiscardText =>
      'Tu perfil tiene cambios que no se pueden guardar tal como están.';

  @override
  String get settingsProfileSaved =>
      'Perfil guardado. Los amigos y grupos en línea ya lo ven; el resto, cuando se conecten.';

  @override
  String get settingsBioSaved =>
      'Biografía guardada. Los amigos en línea ya la ven; el resto, cuando se conecten.';

  @override
  String settingsSwitchOn(String what) {
    return '$what: activado';
  }

  @override
  String settingsSwitchOff(String what) {
    return '$what: desactivado';
  }

  @override
  String get settingsOn => 'Sí';

  @override
  String get settingsOff => 'No';

  @override
  String get settingsIdentity => 'Tu identidad';

  @override
  String get settingsIdentityNote =>
      'Tu nombre y tu imagen viajan firmados a tus amigos y grupos; tu biografía, a tus amigos.';

  @override
  String get settingsPicture => 'Imagen';

  @override
  String get settingsUploadPicture => 'Subir una imagen o un GIF';

  @override
  String get settingsName => 'Nombre';

  @override
  String get settingsBio => 'Biografía';

  @override
  String get settingsBioHint => 'Unas palabras sobre ti';

  @override
  String get settingsNoBio => 'Tu tarjeta no muestra biografía.';

  @override
  String settingsBioWhere(int lines) {
    return 'Tus amigos la ven en tu tarjeta, en hasta $lines líneas.';
  }

  @override
  String get settingsBannerTitle => 'Fondo del panel';

  @override
  String get settingsBannerNote =>
      'La parte superior de tu tarjeta, para ti y tus amigos.';

  @override
  String get settingsBannerAnother => 'Otra imagen';

  @override
  String get settingsBannerUpload => 'Usar una imagen de este equipo';

  @override
  String get settingsFingerprint => 'Huella de este equipo';

  @override
  String get settingsTheme => 'Tema';

  @override
  String get settingsThemeNote =>
      'Los colores de la ventana. Sistema cambia entre Oscuro y Claro según el equipo.';

  @override
  String get settingsMicSpeaker => 'Micrófono y altavoz';

  @override
  String get settingsMicSpeakerNote =>
      'Con qué te oyen en las llamadas y por dónde suenan. También se pueden cambiar en mitad de una llamada.';

  @override
  String get settingsCamera => 'Cámara';

  @override
  String get settingsCameraNote =>
      'Lo que las llamadas muestran de ti cuando tu cámara está encendida. Pruébala aquí antes de una llamada.';

  @override
  String get settingsSoundEffects => 'Efectos de sonido';

  @override
  String get settingsSoundEffectsNote =>
      'Entrar y salir de una sala, gente que llega y se va, mensajes del chat, el micrófono y compartir.';

  @override
  String get settingsSounds => 'Sonidos';

  @override
  String settingsChangeFailed(String reason) {
    return 'Fogata no pudo cambiar eso: $reason';
  }

  @override
  String get settingsAutostartApprove =>
      'Permite Fogata en Ajustes del Sistema › General › Ítems de inicio.';

  @override
  String get settingsAutostartOldMac => 'Esto necesita macOS 13 o posterior.';

  @override
  String get settingsAutostartUnsupported =>
      'Esta copia de Fogata no puede abrirse sola.';

  @override
  String get settingsAutostartUnread =>
      'Fogata no pudo leer este ajuste de tu sistema.';

  @override
  String get settingsAutostart => 'Abrir al iniciar';

  @override
  String get settingsAutostartNote =>
      'Fogata se abre al encender tu equipo, cuando inicias sesión.';

  @override
  String get settingsKeepRunning => 'Seguir en ejecución';

  @override
  String get settingsKeepRunningNote =>
      'Al cerrar la ventana, Fogata se queda en la bandeja del sistema, así tus amigos pueden seguir llamándote.';

  @override
  String get settingsNoTray =>
      'Este escritorio no muestra iconos en la bandeja, así que cerrar la ventana cierra Fogata.';

  @override
  String get settingsHelping => 'Ayudar a los miembros';

  @override
  String get settingsHelpingNote =>
      'Cómo ayuda este equipo a los miembros de tus grupos.';

  @override
  String get settingsHelpRelay => 'Conectar y retransmitir';

  @override
  String get settingsHelpConnect => 'Solo conectar';

  @override
  String get settingsHelpRelayNote =>
      'Cuando dos miembros no pueden conectarse entre sí, este equipo pasa su tráfico cifrado (sin verlo), hasta 2 GB por llamada. Usa tu velocidad de subida.';

  @override
  String get settingsHelpConnectNote =>
      'Este equipo ayuda a dos miembros a encontrarse, pero no pasa su tráfico. Si no pueden conectarse, no se oirán.';

  @override
  String get settingsRestartTray =>
      'El cambio se aplicará cuando salgas de Fogata desde su icono de la bandeja y lo vuelvas a abrir.';

  @override
  String get settingsRestart =>
      'El cambio se aplicará cuando salgas de Fogata y lo vuelvas a abrir.';

  @override
  String get settingsNetworkCheck => 'Comprobación de red';

  @override
  String get settingsNetworkCheckNote =>
      'Cómo ve la red este equipo y qué significa para llegar a tus amigos. No sale nada de aquí: solo mira lo que la conexión ya sabe.';

  @override
  String get settingsCheckNetwork => 'Comprobar la red';

  @override
  String get settingsCheckAgain => 'Volver a comprobar';

  @override
  String get settingsDownloads => 'Descargas';

  @override
  String get settingsDownloadsNote =>
      'Dónde se guardan los archivos descargados de una sala.';

  @override
  String settingsVersionData(String version, String path) {
    return 'Fogata $version · Datos en $path';
  }

  @override
  String get settingsNotEncrypted => 'Sin cifrar en este equipo';

  @override
  String settingsNotEncryptedText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' En Linux, el llavero es un Secret Service en ejecución, como KWallet o GNOME Keyring: inicia o instala uno y Fogata cifrará sus datos la próxima vez que se abra.',
      'other': ' Fogata lo volverá a intentar la próxima vez que se abra.',
    });
    return 'Tu identidad, tus amigos y tus mensajes directos se guardan sin cifrar en este equipo, porque no se encontró ningún llavero del sistema para guardar la clave.$_temp0';
  }

  @override
  String get settingsNotAllEncrypted => 'No todo está cifrado en este equipo';

  @override
  String get settingsNotAllEncryptedText =>
      'Parte de los datos de este equipo (tu identidad, tus amigos y tus mensajes directos) aún no está cifrada: Fogata la sigue cifrando o no pudo terminar, como cuando el disco está lleno. Continúa cada vez que Fogata se abre.';

  @override
  String settingsDamagedSecret(String file) {
    return '$file no se abre con la clave de este equipo: se dañó en el disco. Mientras siga ahí no se puede terminar de cifrar, y una copia de antes de cifrar sigue sin cifrar. Apártalo para terminar: se guarda tal cual. Si es tu identidad o tus amigos, Fogata recupera esa copia y la cifra; el archivo de un grupo o de un chat se pierde en este equipo: el grupo tendrá que dejarte entrar de nuevo, y los mensajes anteriores del chat dejarán de verse.';
  }

  @override
  String get settingsSetDamagedAside => 'Apartarlo y reiniciar';

  @override
  String get storageTitle => 'Almacenamiento';

  @override
  String get storageNote =>
      'Tus mensajes directos viven solo en tus dispositivos. Los chats de las salas desaparecen al cerrarse la sala, así que solo crecen estos.';

  @override
  String get storageDirect => 'Mensajes directos';

  @override
  String get storageOfNoLimit => 'sin límite';

  @override
  String storageOfLimit(String limit) {
    return 'de $limit';
  }

  @override
  String storageLegendPictures(String size) {
    return 'Imágenes $size';
  }

  @override
  String storageLegendVideos(String size) {
    return 'Videos $size';
  }

  @override
  String storageLegendFiles(String size) {
    return 'Archivos $size';
  }

  @override
  String storageLegendMessages(String size) {
    return 'Mensajes $size';
  }

  @override
  String get storageRoom => 'Espacio para mensajes directos';

  @override
  String get storageNoLimitNote => 'No se elimina nada automáticamente.';

  @override
  String storageOverNote(String limit) {
    return 'Por encima del límite: se eliminan las imágenes, videos y archivos más antiguos hasta bajar a $limit. Los mensajes siempre se quedan.';
  }

  @override
  String get storageLimitNote =>
      'Al pasar el límite, se eliminan primero las imágenes, videos y archivos más antiguos. Los mensajes siempre se quedan; donde estaba el archivo se lee “Eliminado aquí para liberar espacio”.';

  @override
  String get storageAutoPictures => 'Descargar imágenes automáticamente';

  @override
  String get storageAutoPicturesNote => 'Hasta 10 MB cada una, según llegan.';

  @override
  String get storageAutoFiles => 'Descargar videos y archivos automáticamente';

  @override
  String get storageAutoFilesNote =>
      'Desactivado: llegan cuando los abres, mientras quien los envió esté en línea.';

  @override
  String get storageTimer => 'Mensajes temporales en chats nuevos';

  @override
  String get storageTimerNote =>
      'Cada chat puede cambiarlo; las dos personas siempre ven el mismo temporizador.';

  @override
  String get storageTimer24h => '24 h';

  @override
  String get storageByChat => 'Por chat';

  @override
  String get storageByChatNote =>
      'Vaciar elimina aquí las imágenes, videos y archivos. Los mensajes se quedan, y la otra persona conserva su copia.';

  @override
  String get storageMessagesOnly => 'Solo mensajes';

  @override
  String get storageClearFiles => 'Vaciar archivos';

  @override
  String storageClearFilesTooltip(String name) {
    return 'Vaciar los archivos del chat con $name';
  }

  @override
  String get storageCleared => 'Vaciado';

  @override
  String storageConfirmTitle(String limit) {
    return '¿Conservar $limit de imágenes, videos y archivos?';
  }

  @override
  String storageConfirmText(String limit) {
    return 'Los más antiguos se eliminan ahora hasta bajar a $limit; los de mensajes fijados, al final. Los mensajes siempre se quedan, y tus amigos conservan sus copias.';
  }

  @override
  String get storageRemoveOldest => 'Eliminar los más antiguos';

  @override
  String get themePickerSystem =>
      'Tema del sistema, según la configuración del equipo';

  @override
  String themePickerTheme(String name) {
    return 'Tema $name';
  }

  @override
  String get themeDark => 'Oscuro';

  @override
  String get themeMidnight => 'Medianoche';

  @override
  String get themeDim => 'Tenue';

  @override
  String get themeEmber => 'Brasa';

  @override
  String get themeContrast => 'Contraste';

  @override
  String get themeLight => 'Claro';

  @override
  String get themeSystem => 'Sistema';

  @override
  String get themeBannerViolet => 'Violeta';

  @override
  String get themeBannerBlue => 'Azul';

  @override
  String get themeBannerGreen => 'Verde';

  @override
  String get themeBannerClay => 'Arcilla';

  @override
  String get themeBannerRaspberry => 'Frambuesa';

  @override
  String get themeBannerMustard => 'Mostaza';

  @override
  String get themeBannerSlate => 'Pizarra';

  @override
  String get themeBannerNight => 'Noche';

  @override
  String widgetsCopyWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Copiar código QR',
      'link': 'Copiar enlace',
      'code': 'Copiar código',
      'other': 'Copiar',
    });
    return '$_temp0';
  }

  @override
  String widgetsShowWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Mostrar código QR',
      'link': 'Mostrar enlace',
      'code': 'Mostrar código',
      'other': 'Mostrar',
    });
    return '$_temp0';
  }

  @override
  String get widgetsHide => 'Ocultar';

  @override
  String revealFailed(String path) {
    return 'No se pudo abrir su carpeta. Está en $path';
  }

  @override
  String get emojiGroupPeople => 'Personas';

  @override
  String get emojiGroupNature => 'Naturaleza';

  @override
  String get emojiGroupFood => 'Comida y bebida';

  @override
  String get emojiGroupActivities => 'Actividades';

  @override
  String get emojiGroupTravel => 'Viajes y lugares';

  @override
  String get emojiGroupObjects => 'Objetos';

  @override
  String get emojiGroupSymbols => 'Símbolos';

  @override
  String get emojiGroupFlags => 'Banderas';

  @override
  String get emojiPickerLimit => 'El mensaje ya alcanzó el límite.';

  @override
  String get emojiPickerButton => 'Emoji';

  @override
  String get emojiPickerResults => 'Resultados';

  @override
  String get emojiPickerRecent => 'Usados recientemente';

  @override
  String get emojiPickerSearch => 'Buscar emoji (en inglés)';

  @override
  String get emojiPickerSkinTone => 'Tono de piel';

  @override
  String get emojiPickerNone => 'No se encontró ningún emoji';

  @override
  String get emojiPickerNoFont =>
      'Este equipo no tiene una fuente de emojis en color, así que los emojis pueden verse simples o vacíos. Instalar Noto Color Emoji lo soluciona.';

  @override
  String get emojiPickerDefaultTone => 'Tono de piel predeterminado';

  @override
  String emojiPickerTone(int n) {
    return 'Tono de piel $n';
  }

  @override
  String get iconsGroupMountain => 'Montaña';

  @override
  String get iconsGroupStar => 'Estrella';

  @override
  String get iconsGroupGameController => 'Mando de juego';

  @override
  String get iconsGroupMusic => 'Música';

  @override
  String get iconsGroupBook => 'Libro';

  @override
  String get iconsGroupWaves => 'Olas';

  @override
  String get iconsGroupCoffee => 'Café';

  @override
  String get iconsGroupHeart => 'Corazón';

  @override
  String get iconsGroupFilm => 'Película';

  @override
  String get newGroupPictureUnreadable => 'No se pudo leer esa imagen.';

  @override
  String newGroupMade(String name) {
    return 'Grupo “$name” creado. Sus miembros lo recibirán cuando se conecten.';
  }

  @override
  String get newGroupTitle => 'Nuevo grupo';

  @override
  String get newGroupSubtitle => 'El grupo es permanente. Sus salas, no.';

  @override
  String get newGroupName => 'Nombre del grupo';

  @override
  String get newGroupNameExample => 'Ruta del domingo';

  @override
  String get newGroupIconOrPicture => 'Icono o imagen';

  @override
  String get newGroupChooseFromComputer => 'Elegir del equipo';

  @override
  String get newGroupBannerOptional => 'Banner (opcional)';

  @override
  String get newGroupMembers => 'Miembros';

  @override
  String newGroupSelected(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n seleccionados',
      one: '1 seleccionado',
    );
    return '$_temp0';
  }

  @override
  String get newGroupSearchPlaceholder => 'Busca entre tus amigos';

  @override
  String get newGroupSearchLabel => 'Buscar amigos';

  @override
  String get newGroupNoFriends =>
      'Aún no tienes amigos. Puedes crear el grupo y añadirlos después, o invitar a gente con un enlace.';

  @override
  String get newGroupKeptOnDevices =>
      'Se guarda en el dispositivo de cada miembro.';

  @override
  String get newGroupCreate => 'Crear grupo';

  @override
  String get groupBannerSettings => 'Ajustes del grupo';

  @override
  String get groupBannerPictureUnreadable => 'No se pudo leer esa imagen.';

  @override
  String get groupBannerChoose => 'Elegir un banner';

  @override
  String get groupBannerAnother => 'Otro banner';

  @override
  String get groupBannerNone => 'Sin banner';

  @override
  String get groupBannerNote =>
      'Se muestra arriba en la columna del grupo, bajo su nombre. PNG, JPEG, o un GIF o WebP animado de hasta 550 KB.';

  @override
  String get homeTitle => 'Inicio';

  @override
  String get homeSubtitle =>
      'Las salas se encienden cuando alguien entra y se apagan cuando todos salen.';

  @override
  String get homeAddFriend => 'Añadir un amigo';

  @override
  String get homeNewGroup => 'Nuevo grupo';

  @override
  String get homeLive => 'En vivo';

  @override
  String get homeNoRoom => 'Sin sala';

  @override
  String get homeNoRoomNote => 'Abre la sala de un grupo para encenderla';

  @override
  String homeInviteBy(String name, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n miembros',
      one: '1 miembro',
    );
    return '$name te añadió · $_temp0';
  }

  @override
  String get homeInviteNote =>
      'Hasta que te unas, este equipo no se conecta con ellos ni les envía nada.';

  @override
  String get homeInviteJoin => 'Unirme';

  @override
  String get homeNoGroups => 'Aún no hay grupos';

  @override
  String get homeNoGroupsNote =>
      'Añade a tus amigos y crea un grupo con ellos. El grupo es permanente; sus salas, no.';

  @override
  String homePersonYou(String name) {
    return '$name (tú)';
  }

  @override
  String homeMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n miembros',
      one: '1 miembro',
    );
    return '$_temp0';
  }

  @override
  String get homeYouAreIn => 'Estás dentro';

  @override
  String get homeMuteMic => 'Silenciar micrófono';

  @override
  String get homeUnmuteMic => 'Activar micrófono';

  @override
  String get homeCameraOff => 'Apagar la cámara';

  @override
  String get homeCameraOn => 'Encender la cámara';

  @override
  String get homeMembersTooltip => 'Miembros';

  @override
  String get homeReturnToRoom => 'Volver a la sala';

  @override
  String get homeJoinRoom => 'Entrar';

  @override
  String homeMembersOnline(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members miembros',
      one: '1 miembro',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online en línea',
      zero: 'nadie más en línea',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get homeNoRoomsYet => 'Aún no hay salas';

  @override
  String homeLastRoom(String when) {
    return 'Última sala $when';
  }

  @override
  String get homeOpenRoom => 'Abrir sala';

  @override
  String get membersGroupGone => 'Este grupo ya no está aquí.';

  @override
  String membersSummary(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members miembros',
      one: '1 miembro',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online en línea',
      zero: 'nadie más en línea',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String membersSummaryLastRoom(int members, int online, String when) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members miembros',
      one: '1 miembro',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online en línea',
      zero: 'nadie más en línea',
    );
    return '$_temp0 · $_temp1 · última sala $when';
  }

  @override
  String get membersCopyInviteLink => 'Copiar enlace de invitación';

  @override
  String membersInviteLinkTooltip(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'alguien del grupo',
      'first': 'el primero del grupo',
      'other': 'uno de los $n primeros del grupo',
    });
    return 'Funciona una vez, durante 24 horas, y $_temp0 le deja entrar';
  }

  @override
  String membersInviteLinkCopied(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'alguien del grupo',
      'first': 'el primero del grupo',
      'other': 'uno de los $n primeros del grupo',
    });
    return 'Enlace de invitación copiado. Funciona una vez, durante 24 horas, y $_temp0 le deja entrar.';
  }

  @override
  String get membersInviteFriends => 'Invitar amigos';

  @override
  String get membersBackToRoom => 'Volver a la sala';

  @override
  String get membersOpenRoom => 'Abrir sala';

  @override
  String get membersEnterRoom => 'Entrar en la sala';

  @override
  String get membersTitle => 'Miembros';

  @override
  String get membersOrderNote => 'En orden: cada uno gestiona a los de abajo';

  @override
  String get membersGroupSettings => 'Ajustes del grupo';

  @override
  String membersManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Solo los $n primeros pueden cambiar el grupo.',
      one: 'Solo el primero puede cambiar el grupo.',
    );
    return '$_temp0';
  }

  @override
  String get membersLeaveGroup => 'Salir del grupo';

  @override
  String membersInviteTitle(String group) {
    return 'Invitar amigos a “$group”';
  }

  @override
  String get membersInviteNote => 'Reciben una invitación y eligen si unirse.';

  @override
  String get membersPresenceYou => 'tú';

  @override
  String membersPresenceOnline(String how) {
    return 'en línea · $how';
  }

  @override
  String membersPresenceAway(String how) {
    return 'ausente · $how';
  }

  @override
  String get membersPresenceOffline => 'sin conexión ahora';

  @override
  String membersYou(String name) {
    return '$name (tú)';
  }

  @override
  String get membersWaitingForKey => 'esperando la clave';

  @override
  String membersFingerprint(String fingerprint) {
    return 'Huella $fingerprint';
  }

  @override
  String membersOptionsFor(String name) {
    return 'Opciones de $name';
  }

  @override
  String get membersMoveUp => 'Subir justo debajo de mí';

  @override
  String get membersMoveToEnd => 'Mover al final';

  @override
  String get membersTimeOut => 'Sacar de una sala por un tiempo…';

  @override
  String get membersRemoveFromGroup => 'Sacar del grupo';

  @override
  String membersRemoveTitle(String name) {
    return '¿Sacar a $name?';
  }

  @override
  String membersRemoveText(String name) {
    return '$name sale del grupo y el grupo crea una clave nueva. Conserva lo que ya vio.';
  }

  @override
  String get membersFirst => 'Primer puesto';

  @override
  String membersTimeOutTitle(String name) {
    return 'Sacar a $name por un tiempo';
  }

  @override
  String membersTimeOutText(String name) {
    return '$name sale de la sala y no puede volver hasta que pase el tiempo o alguien por encima le deje volver.';
  }

  @override
  String get membersUntilLetBack => 'Hasta nuevo permiso';

  @override
  String membersLeaveTitle(String group) {
    return '¿Salir de “$group”?';
  }

  @override
  String get membersLeaveText =>
      'Dejas de ver sus salas y los demás dejan de verte en el grupo. El grupo crea una clave nueva sin ti. Para volver, alguien del grupo tiene que dejarte entrar de nuevo.';

  @override
  String membersLeaveTextFirst(String name) {
    return 'Dejas de ver sus salas y los demás dejan de verte en el grupo. El grupo crea una clave nueva sin ti. Para volver, alguien del grupo tiene que dejarte entrar de nuevo. $name pasa al primer puesto.';
  }

  @override
  String get groupSettingsTabGeneral => 'General';

  @override
  String get groupSettingsTabVoiceRooms => 'Salas de voz';

  @override
  String get groupSettingsTabMembers => 'Miembros';

  @override
  String get groupSettingsTabInvite => 'Invitar';

  @override
  String get groupSettingsTabLimits => 'Límites';

  @override
  String get groupSettingsTabActivity => 'Actividad';

  @override
  String get groupSettingsFirstHere => 'Primer puesto en este grupo';

  @override
  String groupSettingsPlaceHere(String place) {
    return '$place en este grupo';
  }

  @override
  String get groupSettingsLeave => 'Salir del grupo';

  @override
  String get groupSettingsInviteTitle => 'Invitar a gente';

  @override
  String get groupSettingsClose => 'Cerrar los ajustes del grupo';

  @override
  String get groupSettingsPicture => 'Imagen';

  @override
  String get groupSettingsChooseFromComputer => 'Elegir del equipo';

  @override
  String get groupSettingsUseIcon => 'Usar un icono';

  @override
  String get groupSettingsPictureNote =>
      'PNG, JPEG o WebP, que se recorta en cuadrado y se reduce como las fotos de perfil, o un GIF o WebP animado de hasta 550 KB.';

  @override
  String get groupSettingsOrIcon => 'O un icono';

  @override
  String get groupSettingsBanner => 'Banner';

  @override
  String get groupSettingsName => 'Nombre';

  @override
  String get groupSettingsNameField => 'Nombre del grupo';

  @override
  String get groupSettingsNameNote =>
      'Todos en el grupo ven el nuevo nombre y la nueva imagen.';

  @override
  String get groupSettingsRoomsIntro =>
      'Cada sala de voz tiene su propia gente, chat, archivos y llamadas: quienes están en una no se conectan con los de otra. Cada persona está en una sola sala a la vez.';

  @override
  String get groupSettingsRoomName => 'Nombre de la sala';

  @override
  String get groupSettingsAccessEveryone => 'Todos';

  @override
  String get groupSettingsAccessSome => 'Solo algunos';

  @override
  String groupSettingsWhoCanEnter(String room) {
    return 'Quién puede entrar en $room';
  }

  @override
  String get groupSettingsFirstRoomStays => 'La primera sala se queda';

  @override
  String get groupSettingsPeopleInside => 'Hay gente dentro';

  @override
  String get groupSettingsDeleteRoom => 'Eliminar sala';

  @override
  String get groupSettingsAddRoom => 'Añadir sala de voz';

  @override
  String groupSettingsNewRoomName(int n) {
    return 'Sala $n';
  }

  @override
  String groupSettingsRoomsNote(int rooms, int people) {
    return 'Hasta $rooms salas, y $people personas en cada una (Límites). Una sala con gente dentro no se puede eliminar.';
  }

  @override
  String get groupSettingsKeptOut => 'Sin acceso';

  @override
  String get groupSettingsAccessEveryoneLong => 'Todos en el grupo';

  @override
  String get groupSettingsAccessChosen => 'Solo los miembros que yo elija';

  @override
  String get groupSettingsAccessNote =>
      'Siempre puedes entrar, igual que quienes están por encima de ti. Quien quede fuera ve la sala con un candado.';

  @override
  String groupSettingsYou(String name) {
    return '$name (tú)';
  }

  @override
  String groupSettingsKeptOutBy(String room, String by) {
    return ' · $room · por $by';
  }

  @override
  String groupSettingsTimeLeft(String time) {
    return 'Quedan: $time';
  }

  @override
  String get groupSettingsUntilLetBack => 'Hasta nuevo permiso';

  @override
  String get groupSettingsLetBack => 'Dejar volver';

  @override
  String groupSettingsManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Solo los $n primeros pueden cambiar el grupo: su nombre, salas, invitaciones y límites. Tú gestionas a los que están por debajo de ti.',
      one: 'Solo el primero puede cambiar el grupo: su nombre, salas, invitaciones y límites. Tú gestionas a los que están por debajo de ti.',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsMembersCount(int n, int max) {
    String _temp0 = intl.Intl.pluralLogic(
      max,
      locale: localeName,
      other: '$max miembros',
      one: '1 miembro',
    );
    return '$n de $_temp0, en orden';
  }

  @override
  String get groupSettingsMembersNote =>
      'Cada uno gestiona a quienes están por debajo: los sube justo debajo de sí o los baja al final, los saca de una sala por un tiempo o los saca del grupo. Quien entra empieza al final.';

  @override
  String get groupSettingsWhoManages => 'Quién gestiona este grupo';

  @override
  String get groupSettingsManagersEveryone => 'Todos';

  @override
  String groupSettingsManagersFirstN(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Los $n primeros',
      one: 'El primero',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsManagersMore(int n, int members) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: 'los $members miembros',
      one: 'el único miembro',
    );
    return 'Fijado en los $n primeros, más que $_temp0 que hay ahora en el grupo: por ahora son todos.';
  }

  @override
  String get groupSettingsManagersEveryoneNote =>
      'Quienes gestionan cambian el nombre, las imágenes, las salas y los límites, y dejan entrar a la gente. Con todos, el grupo sigue adelante aunque alguien pierda su dispositivo.';

  @override
  String groupSettingsManagersFirstNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Solo los $n primeros cambian el nombre, las imágenes, las salas y los límites, y dejan entrar a la gente. Si todos pierden sus dispositivos, nadie podrá gestionar el grupo nunca más: Fogata no tiene ningún servidor que pueda arreglarlo.',
      one: 'Solo el primero cambia el nombre, las imágenes, las salas y los límites, y deja entrar a la gente. Si se pierde ese dispositivo, nadie podrá gestionar el grupo nunca más: Fogata no tiene ningún servidor que pueda arreglarlo.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsMoveUp => 'Subir';

  @override
  String groupSettingsMoveUpTooltip(String name) {
    return 'Subir a $name justo debajo de ti';
  }

  @override
  String get groupSettingsToEnd => 'Al final';

  @override
  String groupSettingsToEndTooltip(String name) {
    return 'Mover a $name al final';
  }

  @override
  String get groupSettingsTimeOut => 'Sacar de una sala por un tiempo';

  @override
  String get groupSettingsRemove => 'Sacar del grupo';

  @override
  String get groupSettingsAskingToJoin => 'Piden unirse';

  @override
  String get groupSettingsInviteFriend => 'Invitar a un amigo';

  @override
  String get groupSettingsInviteFriendNote =>
      'Reciben una invitación en Fogata y eligen si unirse. Aquí solo aparecen tus amigos.';

  @override
  String get groupSettingsInviteLink => 'Enlace de invitación';

  @override
  String groupSettingsInviteLinkNote(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'alguien del grupo',
      'first': 'el primero del grupo',
      'other': 'uno de los $n primeros del grupo',
    });
    return 'Para alguien que aún no es tu amigo. Funciona una vez y durante 24 horas, y $_temp0 le deja entrar: si llega a otra persona, verás quién lo pide y podrás rechazarlo.';
  }

  @override
  String get groupSettingsCopyLink => 'Copiar enlace de invitación';

  @override
  String groupSettingsOpenLinks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n enlaces sin usar todavía.',
      one: '1 enlace sin usar todavía.',
      zero: 'No hay enlaces pendientes de usar.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsCancelLinks => 'Anularlos';

  @override
  String get groupSettingsNoFriends =>
      'Aún no tienes amigos: añádelos en Amigos.';

  @override
  String get groupSettingsAllFriendsIn => 'Todos tus amigos están en el grupo.';

  @override
  String get groupSettingsFriendOnline => 'En línea';

  @override
  String get groupSettingsFriendOffline => 'Sin conexión ahora';

  @override
  String get groupSettingsInviteButton => 'Invitar';

  @override
  String get groupSettingsGroupFull => 'El grupo está lleno';

  @override
  String get groupSettingsSayNo => 'Rechazar';

  @override
  String get groupSettingsLetIn => 'Dejar entrar';

  @override
  String get groupSettingsLimitGroup => 'Personas en el grupo';

  @override
  String groupSettingsLimitGroupNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'las $n personas',
      one: 'la única persona',
    );
    return 'De 2 a 20. Cuando está lleno, las invitaciones y los enlaces dejan de funcionar. No puede bajar de $_temp0 que ya hay.';
  }

  @override
  String get groupSettingsLimitRoom => 'Personas en cada sala de voz';

  @override
  String get groupSettingsLimitRoomNote =>
      'De 2 a 8. La voz y el video van directamente entre todos los de una sala, así que 8 es el máximo que suena bien; con las cámaras encendidas, funciona mejor con menos. Quien llegue cuando esté llena verá que la sala está llena.';

  @override
  String get shellChipCallMany =>
      'Directa · cifrada · solo quienes están en la llamada';

  @override
  String get shellChipCallTwo => 'Directa · cifrada · solo entre los dos';

  @override
  String get shellGroupMembers => 'Miembros del grupo';

  @override
  String get shellMembers => 'Miembros';

  @override
  String get shellDirectMessages => 'Mensajes directos';

  @override
  String get shellPathDirect => 'Directo · cifrado';

  @override
  String shellPathVia(String name) {
    return 'A través de $name · cifrado';
  }

  @override
  String get shellPathViaSomeone => 'A través de un miembro · cifrado';

  @override
  String shellCallWith(String names) {
    return 'Llamada con $names';
  }

  @override
  String get shellRinging => 'Sonando · ';

  @override
  String get shellInCall => 'En llamada · ';

  @override
  String get shellCallEnded => 'Llamada terminada';

  @override
  String get shellNewGroup => 'Nuevo grupo';

  @override
  String get shellSectionCall => 'Llamada';

  @override
  String shellCallRinging(String name) {
    return '$name · sonando';
  }

  @override
  String get shellSectionLiveNow => 'En vivo ahora';

  @override
  String get shellSectionInCall => 'En la llamada';

  @override
  String get shellChat => 'Chat';

  @override
  String get shellFiles => 'Archivos';

  @override
  String get shellSectionGroup => 'Grupo';

  @override
  String get shellInvite => 'Invitar';

  @override
  String get shellSectionVoiceRooms => 'Salas de voz';

  @override
  String shellVoiceRoomYouAreIn(String room) {
    return '$room · estás aquí';
  }

  @override
  String get shellNotForYou => 'No es para ti';

  @override
  String get shellOnlySomeMembers => 'Solo algunos miembros';

  @override
  String shellPersonYou(String name) {
    return '$name (tú)';
  }

  @override
  String get shellLive => 'EN VIVO';

  @override
  String get shellYourProfile => 'Tu perfil';

  @override
  String shellMuteMic(String keys) {
    return 'Silenciar micrófono ($keys)';
  }

  @override
  String shellUnmuteMic(String keys) {
    return 'Activar micrófono ($keys)';
  }

  @override
  String get shellDeafen => 'Dejar de oír a los demás';

  @override
  String get shellUndeafen => 'Volver a oír a los demás';

  @override
  String get shellStateCalling => 'Llamando…';

  @override
  String get shellStateInCall => 'En llamada';

  @override
  String get shellStateInRoom => 'En la sala';

  @override
  String get shellStateOffline => 'Sin conexión';

  @override
  String get shellStateOnline => 'En línea';

  @override
  String get shellStateAway => 'Ausente';

  @override
  String get shellShowInFolder => 'Mostrar en carpeta';

  @override
  String get shellCopyPath => 'Copiar ruta';

  @override
  String shellVolumePercent(int n) {
    return '$n %';
  }

  @override
  String get shellMuteKeys => 'Ctrl+Mayús+M';

  @override
  String get settingsSectionDonate => 'Donar a Fogata';

  @override
  String get settingsSectionReport => 'Informar de un problema';

  @override
  String get settingsHeadingHelp => 'Ayuda';

  @override
  String get settingsPreRelease => 'Pre-release';

  @override
  String get donateTitle => 'Mantén Fogata independiente';

  @override
  String get donateText =>
      'Fogata es gratis, no tiene anuncios ni servidores que te vigilen. Tu donación hace que siga así.';

  @override
  String get donateButton => 'Donar';

  @override
  String get donateNote =>
      'Abre Ko-fi en tu navegador. Fogata nunca ve tu pago.';

  @override
  String get reportWhatTitle => '¿Qué pasó?';

  @override
  String get reportWhatNote =>
      'Qué estabas haciendo, qué esperabas y qué viste en su lugar.';

  @override
  String get reportIncludeLog =>
      'Incluir el registro técnico de los últimos 2 días';

  @override
  String get reportLogNote =>
      'Versiones, errores y pasos de conexión. Nunca tus mensajes, claves, contactos ni direcciones IP.';

  @override
  String get reportSeeSent => 'Ver qué se envía';

  @override
  String get reportHideSent => 'Ocultar la lista';

  @override
  String get reportPreview => 'Leer el informe completo antes de enviarlo';

  @override
  String get reportHidePreview => 'Ocultar el informe';

  @override
  String get reportPreviewNote =>
      'Esto es exactamente lo que lleva el archivo, y es todo lo que se envía. Los nombres, carpetas, nombres de archivo, direcciones y claves ya se han quitado del registro.';

  @override
  String get reportPreviewWorking => 'Preparando el informe…';

  @override
  String get reportSentWhat => 'Lo que escribiste arriba.';

  @override
  String get reportSentVersion =>
      'La versión y la compilación de Fogata, tu sistema y su versión, y el idioma en que está Fogata.';

  @override
  String get reportSentLog =>
      'El registro de los últimos 2 días: cuándo se inició Fogata, sus errores y cómo llegó a otros dispositivos. Se quitan los nombres, mensajes, carpetas, nombres de archivo, enlaces, códigos, claves y direcciones IP, y cada dispositivo aparece solo como un número que no significa nada fuera del informe.';

  @override
  String get reportEmail => 'Enviarlo por correo a Fogata';

  @override
  String get reportSave => 'Guardar en un archivo';

  @override
  String reportHowEmail(String address) {
    return 'Tu aplicación de correo se abre con el mensaje listo para $address, y la carpeta con el archivo del informe se abre al lado: adjunta el archivo y envíalo. Nada sale de tu equipo hasta que lo hagas.';
  }

  @override
  String get reportReadyTitle => 'Tu correo está listo';

  @override
  String reportReadyText(String file, String address) {
    return 'Adjunta $file desde la carpeta que se abrió y envíalo a $address.';
  }

  @override
  String get reportShowFile => 'Mostrar el archivo';

  @override
  String get reportCopyAddress => 'Copiar la dirección';

  @override
  String get reportAddressCopied => 'Dirección copiada';

  @override
  String reportSaved(String file) {
    return 'Informe guardado: $file';
  }

  @override
  String reportNoMailApp(String address) {
    return 'No se abrió ninguna aplicación de correo. Envía el archivo a $address desde el correo que uses.';
  }

  @override
  String reportFailed(String reason) {
    return 'No se pudo escribir el informe: $reason';
  }

  @override
  String get splashStarting => 'Iniciando';

  @override
  String get splashConnecting => 'Conectando';

  @override
  String get splashChecking => 'Buscando actualizaciones';

  @override
  String get splashDownloading => 'Descargando actualización';

  @override
  String get splashInstalling => 'Instalando actualización';

  @override
  String get splashRestarting => 'Reiniciando';

  @override
  String splashProgress(String done, String total) {
    return '$done de $total MB';
  }

  @override
  String splashProgressFrom(String done, String total, String name) {
    return '$done de $total MB · de $name';
  }

  @override
  String updateReady(String version) {
    return 'Actualización lista · $version';
  }

  @override
  String updateRequired(String version) {
    return 'Actualización necesaria · $version';
  }

  @override
  String get updateRestart => 'Reiniciar';

  @override
  String updateReadyTip(String version) {
    return 'Fogata $version ya está aquí. Reinicia para usarlo.';
  }

  @override
  String get updateRequiredTip =>
      'Esta versión ya no funciona con la más nueva de tus amigos. Reinicia para actualizar.';

  @override
  String updateFailed(String reason) {
    return 'No se pudo instalar la actualización: $reason';
  }

  @override
  String get errJoinInviteGone =>
      'Esa invitación ya no funciona o el grupo está lleno.';

  @override
  String get errJoinNoKeyYet =>
      'Este dispositivo aún no tiene la clave del grupo. El nuevo miembro entrará en cuanto la tenga.';

  @override
  String get errJoinWroteTooMuch =>
      'Este dispositivo ha hecho cambios que los demás miembros aún no han recibido. Podrás dejar entrar al nuevo miembro cuando otro miembro esté en línea.';

  @override
  String get errJoinAskAgain =>
      'Esa solicitud para unirse se hizo sobre un estado anterior del grupo. La persona tiene que volver a pedirlo.';

  @override
  String get errNoSpaceForFile =>
      'No hay suficiente espacio libre para este archivo. Libera espacio y vuelve a intentarlo.';

  @override
  String get recoveryTitle => 'Copia de recuperación';

  @override
  String get recoveryNote =>
      'Un archivo que recupera esta cuenta si se pierden el llavero o el disco de este equipo. Solo tu clave de recuperación lo abre.';

  @override
  String get recoveryOff => 'Desactivada';

  @override
  String recoveryOn(String id) {
    return 'Activada · clave $id';
  }

  @override
  String get recoveryTurnOn => 'Activar…';

  @override
  String get recoveryChangeKey => 'Cambiar la clave…';

  @override
  String get recoveryMakeCopy => 'Hacer una copia…';

  @override
  String get recoveryReplacedKept =>
      'Este equipo aún guarda una cuenta que una recuperación reemplazó.';

  @override
  String get recoveryDeleteReplaced => 'Eliminarla…';

  @override
  String get recoveryChangeTitle => '¿Crear una clave de recuperación nueva?';

  @override
  String get recoveryChangeText =>
      'Las copias nuevas se abrirán solo con la clave nueva. Las que hiciste antes siguen abriéndose solo con la antigua: guárdala mientras guardes esas copias.';

  @override
  String get recoveryChangeConfirm => 'Crear una clave nueva';

  @override
  String get recoveryKeyTitle => 'Tu clave de recuperación';

  @override
  String get recoveryKeyText =>
      'Apúntala o imprímela, y guárdala aparte de tus copias. Solo se muestra esta vez.';

  @override
  String recoveryKeyName(String id) {
    return 'Su nombre corto: $id';
  }

  @override
  String get recoveryKeyWarning =>
      'Fogata no guarda esta clave en ningún sitio: sin ella, ninguna copia se abre. Quien tenga una copia y esta clave tiene tu cuenta.';

  @override
  String get recoveryKeyWritten => 'Ya la apunté';

  @override
  String get recoveryCheckTitle => 'Comprueba lo que apuntaste';

  @override
  String get recoveryCheckText =>
      'Escribe estos dos grupos de la clave, tal como los apuntaste.';

  @override
  String recoveryCheckGroup(int n) {
    return 'Grupo $n';
  }

  @override
  String get recoveryCheckWrong =>
      'No es ese: vuelve a mirar lo que apuntaste.';

  @override
  String get recoveryTurnOnButton => 'Activar las copias';

  @override
  String recoveryTurnedOn(String id) {
    return 'Las copias están activadas, con la clave $id.';
  }

  @override
  String get recoveryCopyTitle => 'Hacer una copia de recuperación';

  @override
  String get recoveryCopyText =>
      'Fogata se desconecta mientras hace la copia, más tiempo si incluye los archivos de los chats. Guarda el archivo lejos de este equipo, por ejemplo en una memoria USB.';

  @override
  String get recoveryCopyAttachments =>
      'Incluir los archivos enviados en los chats';

  @override
  String recoveryCopyAttachmentsNote(String size) {
    return 'Imágenes, videos y archivos: $size más.';
  }

  @override
  String get recoveryCopySave => 'Elegir dónde…';

  @override
  String get recoveryCopyInDataFolder =>
      'Una copia no se puede guardar en la carpeta de datos de Fogata. Elige otro sitio.';

  @override
  String recoveryCopyMade(String file, String size) {
    return 'Copia guardada: $file ($size).';
  }

  @override
  String recoveryCopyFailed(String why) {
    return 'No se pudo hacer la copia. $why';
  }

  @override
  String get recoveryDeleteTitle => '¿Eliminar la cuenta reemplazada?';

  @override
  String get recoveryDeleteText =>
      'Se apartó al recuperar una copia en su lugar. Una vez eliminada, no se puede recuperar.';

  @override
  String get recoveryDelete => 'Eliminar';

  @override
  String get recoveryDeleted => 'Se eliminó la cuenta reemplazada.';

  @override
  String get recoveryFileType => 'Copia de Fogata';

  @override
  String get recoveryLostComputer => '¿Perdiste tu equipo?';

  @override
  String get recoveryRestoreLink => 'Recuperar desde una copia';

  @override
  String get recoveryRestoreTitle => 'Recuperar desde una copia';

  @override
  String get recoveryRestoreSub =>
      'Elige la copia de recuperación que guardaste y luego escribe tu clave de recuperación.';

  @override
  String get recoveryChooseCopy => 'Elegir la copia…';

  @override
  String recoveryMadeOn(String date) {
    return 'Una copia hecha el $date.';
  }

  @override
  String get recoveryOtherKey =>
      'Se hizo para otra clave de recuperación, no la de este equipo. Escribe la clave para la que se hizo.';

  @override
  String get recoveryKeyField => 'Clave de recuperación';

  @override
  String get recoveryKeyNotAKey =>
      'Una clave de recuperación tiene 53 letras y números, en grupos de cuatro.';

  @override
  String get recoveryKeyMistyped =>
      'Una letra o un número no es correcto: revísalos.';

  @override
  String get recoveryProblemNotACopy =>
      'Esto no es una copia de una cuenta de Fogata.';

  @override
  String get recoveryProblemNewer =>
      'Esta copia se hizo con una versión más nueva de Fogata. Actualiza Fogata para abrirla.';

  @override
  String get recoveryProblemOtherKey =>
      'Esta copia se hizo para otra clave de recuperación.';

  @override
  String get recoveryProblemDamaged =>
      'Esta copia está dañada, o se cambió después de hacerse.';

  @override
  String get recoveryProblemUnproven =>
      'Esta copia no la hizo la cuenta que contiene, así que no se abre.';

  @override
  String recoveryWhose(String name) {
    return 'La cuenta de $name';
  }

  @override
  String recoveryHolds(int files, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files archivos',
      one: '1 archivo',
    );
    return '$_temp0, $size en total';
  }

  @override
  String get recoveryWithAttachments =>
      'Con los archivos enviados en los chats.';

  @override
  String get recoveryWithoutAttachments =>
      'Sin los archivos enviados en los chats.';

  @override
  String get recoveryOnlyIf =>
      'Usa una copia solo si ningún otro equipo usa ya esta cuenta. Si alguno la usa todavía, mueve la cuenta desde él: una cuenta no puede estar en dos equipos.';

  @override
  String get recoveryMoveInstead => 'Mejor moverla';

  @override
  String recoveryHereNamed(String name) {
    return 'Este equipo tiene la cuenta de $name.';
  }

  @override
  String recoveryHereDevice(String fingerprint) {
    return 'Este equipo tiene una cuenta sin perfil (dispositivo $fingerprint).';
  }

  @override
  String get recoveryHereUnknown =>
      'Este equipo tiene parte de una cuenta que no se abre.';

  @override
  String get recoveryHereReplaced =>
      'Recuperar la copia la reemplaza. Se aparta, no se elimina, hasta que la elimines en Ajustes › Almacenamiento y descargas.';

  @override
  String get recoveryRestore => 'Recuperar';

  @override
  String get recoveryReplace => 'Reemplazar y recuperar';

  @override
  String recoveryRestoring(String name) {
    return 'Recuperando la cuenta de $name…';
  }

  @override
  String recoveryRestored(String name) {
    return 'Se recuperó la cuenta de $name.';
  }

  @override
  String get recoveryRefusedAccountHere =>
      'Ahora este equipo tiene una cuenta. Vuelve a elegir la copia para ver qué reemplaza.';

  @override
  String get recoveryRefusedReplacedKept =>
      'Este equipo aún guarda una cuenta que una recuperación anterior reemplazó. Elimínala para recuperar esta copia.';

  @override
  String recoveryRefusedNoRoom(String need, String free) {
    return 'La copia necesita $need en este disco y hay $free libres. No cambió nada.';
  }

  @override
  String get recoveryRefusedInFolder =>
      'La copia está dentro de la carpeta de datos de Fogata. Muévela a otro sitio y vuelve a elegirla.';

  @override
  String get recoveryBusy =>
      'Fogata está ocupado con una copia de recuperación o un traslado. Vuelve a intentarlo cuando termine.';

  @override
  String get moveBusyWithCopy =>
      'Fogata está ocupado con una copia de recuperación. Mueve la cuenta cuando termine.';

  @override
  String get recoveryDeleteKeptAside => 'Eliminar la cuenta apartada…';

  @override
  String get reachOffline =>
      'Fogata no llega a nadie en internet desde aquí. Revisa la conexión, o puede que un firewall esté bloqueando a Fogata.';

  @override
  String get reachFirewall =>
      'El firewall de este equipo no deja entrar nada para Fogata. El comando que lo deja entrar está en las notificaciones.';

  @override
  String get reachUdpBlocked =>
      'Esta red o este equipo bloquea UDP. Las llamadas y las conexiones directas a través de routers lo necesitan; hasta que se permita, los demás solo te alcanzan a través de un miembro de un grupo o desde una dirección pública.';

  @override
  String get reachSymmetricNat =>
      'Tu router da un puerto público distinto para cada destino (un NAT simétrico), así que no se puede abrir un camino directo a través de él. Los amigos detrás de otro router solo te alcanzan a través de un miembro de un grupo en común.';

  @override
  String get reachNoIpv6 =>
      'Tampoco hay IPv6 que funcione, que permitiría sortear el router.';

  @override
  String reachFineFriend(String name) {
    return 'Tu red parece estar bien para Fogata, así que el problema probablemente está del lado de $name (sin conexión, o su red).';
  }

  @override
  String get reachFineOthers =>
      'Tu red parece estar bien para Fogata, así que el problema probablemente está del otro lado.';
}
