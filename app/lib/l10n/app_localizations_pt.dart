// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Portuguese (`pt`).
class L10nPt extends L10n {
  L10nPt([String locale = 'pt']) : super(locale);

  @override
  String get commonCancel => 'Cancelar';

  @override
  String get commonSave => 'Salvar';

  @override
  String get commonClose => 'Fechar';

  @override
  String commonNewMessageFrom(String name, String text) {
    return 'Nova mensagem de $name: $text';
  }

  @override
  String commonNewMessagesFrom(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n novas mensagens de $name',
      one: '$n nova mensagem de $name',
    );
    return '$_temp0';
  }

  @override
  String commonNewMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n novas mensagens',
      one: '$n nova mensagem',
    );
    return '$_temp0';
  }

  @override
  String get commonAccept => 'Aceitar';

  @override
  String get commonDecline => 'Recusar';

  @override
  String get commonSend => 'Enviar';

  @override
  String get commonTryAgain => 'Tentar de novo';

  @override
  String get commonCopy => 'Copiar';

  @override
  String get commonCopied => 'Copiado';

  @override
  String get commonBack => 'Voltar';

  @override
  String get commonDiscard => 'Descartar';

  @override
  String get commonKeepEditing => 'Continuar editando';

  @override
  String get commonNotNow => 'Agora não';

  @override
  String get commonDownload => 'Baixar';

  @override
  String get commonSettings => 'Configurações';

  @override
  String get commonHome => 'Início';

  @override
  String get commonFriends => 'Amigos';

  @override
  String get commonContinue => 'Continuar';

  @override
  String get commonDone => 'Concluir';

  @override
  String get commonRemove => 'Remover';

  @override
  String get commonYou => 'Você';

  @override
  String timeYesterdayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'ontem à $time',
      'other': 'ontem às $time',
    });
    return '$_temp0';
  }

  @override
  String timeDayAt(String date, String time) {
    return '$date, $time';
  }

  @override
  String get timeNow => 'agora';

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
      other: '$n dias',
      one: '1 dia',
      zero: '$n dias',
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
      zero: '$n minutos',
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
      zero: '$n horas',
    );
    return '$_temp0';
  }

  @override
  String spanDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n dias',
      one: '1 dia',
      zero: '$n dias',
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
      zero: '$n semanas',
    );
    return '$_temp0';
  }

  @override
  String get agoMoment => 'há pouco';

  @override
  String agoMinutes(int n) {
    return 'há $n min';
  }

  @override
  String agoHours(int n) {
    return 'há $n h';
  }

  @override
  String get agoYesterday => 'ontem';

  @override
  String agoDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'há $n dias',
    );
    return '$_temp0';
  }

  @override
  String agoWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'há $n semanas',
      one: 'há 1 semana',
      zero: 'há $n semanas',
    );
    return '$_temp0';
  }

  @override
  String agoMonths(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'há $n meses',
      one: 'há 1 mês',
      zero: 'há $n meses',
    );
    return '$_temp0';
  }

  @override
  String get agoOverYear => 'há mais de um ano';

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
  String get errLinkExpired => 'O link expirou. Peça um novo.';

  @override
  String get errGroupInvite => 'Esse é um convite para um grupo.';

  @override
  String get errDeviceNotFound =>
      'O outro dispositivo não foi encontrado. Confira se vocês dois estão online.';

  @override
  String get errNotFriendCode => 'Isso não parece um link ou código do Fogata.';

  @override
  String get errAlreadyInGroup => 'Você já está nesse grupo.';

  @override
  String get errJoinRoomFirst => 'Entre na sala primeiro.';

  @override
  String get errNewerVersion =>
      'Seu perfil foi salvo por uma versão mais nova do Fogata. Atualize o Fogata para abri-lo.';

  @override
  String get errKeychainUnreachable =>
      'O Fogata não conseguiu obter a chave do seu perfil no chaveiro deste computador. Desbloqueie o chaveiro ou permita que o Fogata o use e tente de novo.';

  @override
  String get errKeychainNoKey =>
      'O chaveiro deste computador não tem mais a chave do seu perfil, então o Fogata não consegue abri-lo.';

  @override
  String get errKeychainWrongKey =>
      'A chave no chaveiro deste computador não abre o seu perfil.';

  @override
  String get errNoGroupKey =>
      'Este grupo ainda não está pronto neste computador. Tente de novo daqui a pouco.';

  @override
  String get errAlreadyRunning =>
      'O Fogata já está aberto com este perfil. Feche o outro primeiro.';

  @override
  String get errFileGone => 'Esse arquivo não está mais na sala.';

  @override
  String get errNobodyHasFile => 'Ninguém online tem esse arquivo agora.';

  @override
  String get errStillStarting => 'O Fogata ainda está iniciando.';

  @override
  String get errIdentityExists =>
      'Este computador já tem uma identidade; a chave dela não pode mudar.';

  @override
  String get errNameRule => 'Escreva um nome de 1 a 40 caracteres visíveis.';

  @override
  String get errBioRule =>
      'Uma bio tem no máximo 190 caracteres em 4 linhas, sem caracteres ocultos.';

  @override
  String get errPictureRule => 'A imagem precisa ser PNG ou JPEG de até 64 KB.';

  @override
  String get errNotFriendLink => 'Isso não é um link de amizade do Fogata.';

  @override
  String get errLinkDamaged => 'O link está incompleto ou danificado.';

  @override
  String get errLinkNeedsNewer =>
      'Esse link precisa de uma versão mais nova do Fogata.';

  @override
  String get errCodeShape => 'Um código tem o formato FGT-XXXXX-XXXXX.';

  @override
  String get errCodeWrong => 'O código está errado ou não funciona mais.';

  @override
  String get errNotProven => 'O outro dispositivo não conseguiu provar quem é.';

  @override
  String get errCallGone => 'Essa chamada não está mais tocando.';

  @override
  String get errAlreadyInCall =>
      'Você já está em uma chamada. Desligue primeiro.';

  @override
  String get errAlreadyFriends => 'Vocês já são amigos.';

  @override
  String get errMemberNotConnected =>
      'Esse membro não está conectado agora. Tente de novo quando ele estiver online.';

  @override
  String get errMemberTooOld =>
      'A versão do Fogata desse membro é antiga demais para receber pedidos de amizade de um grupo. Envie um link de amizade em vez disso.';

  @override
  String get errMemberAskLost =>
      'O pedido de amizade não chegou a esse membro. Tente de novo.';

  @override
  String get errOwnLink =>
      'Esse link é o seu: envie para o amigo que você quer adicionar.';

  @override
  String get errLinkUsed => 'Esse link já foi usado ou expirou. Peça um novo.';

  @override
  String get errLinkExpiredUnreached =>
      'O link expirou antes de alcançar o dispositivo de quem o criou. Peça um novo.';

  @override
  String get errCodeUnanswered =>
      'Nenhum dispositivo respondeu ao código. Confira o código e se o outro dispositivo ainda o mostra.';

  @override
  String get errRefused =>
      'O outro dispositivo recusou. Talvez o código já tenha sido usado.';

  @override
  String get errNobodyLetIn =>
      'Ninguém deixou você entrar. Tente de novo quando alguém do grupo estiver online.';

  @override
  String get errNotBelow =>
      'Você só pode fazer isso com alguém abaixo de você no grupo.';

  @override
  String get errNotManager => 'Só quem administra o grupo pode fazer isso.';

  @override
  String get errLastManager =>
      'Você não pode se remover da administração do grupo.';

  @override
  String get errRoomNotEmpty =>
      'Há pessoas nessa sala de voz: exclua-a quando ela estiver vazia.';

  @override
  String get errTooManyFiles =>
      'Você já compartilha o máximo de arquivos que uma sala aceita por pessoa. Pare de compartilhar alguns primeiro.';

  @override
  String get errDiskFull =>
      'O disco está cheio. Libere espaço e tente de novo.';

  @override
  String get errTooManyChanges =>
      'Este dispositivo fez mudanças demais no grupo que nenhum outro membro recebeu ainda. Tente de novo depois que outra pessoa do grupo fizer uma mudança.';

  @override
  String get errSomething => 'Algo deu errado.';

  @override
  String errRoomFull(int n) {
    return 'Essa sala de voz está cheia: ela aceita no máximo $n pessoas.';
  }

  @override
  String get findingOffline =>
      'Nenhum peer da rede pública respondeu: este computador parece não acessar a internet, ou um firewall bloqueia o Fogata.';

  @override
  String get findingUdpWorks =>
      'O UDP funciona. As chamadas e a passagem por roteadores precisam dele.';

  @override
  String get findingUdpBlocked =>
      'Esta rede ou este computador parece bloquear o UDP (peers que responderam por TCP não responderam por QUIC). O chat ainda pode passar por outros membros, mas as conexões diretas através de roteadores e as chamadas vão falhar na maioria das vezes. Confira o firewall ou as regras da rede.';

  @override
  String get findingCarrierNat =>
      'Este computador tem um endereço de NAT de operadora (100.64.0.0/10): o provedor divide um endereço público entre muitos clientes.';

  @override
  String get findingPublicIpv4 =>
      'Este computador tem um endereço IPv4 público: outros podem alcançá-lo diretamente.';

  @override
  String get findingHardNat =>
      'Atrás de um roteador que dá uma porta pública diferente para cada destino (um NAT difícil).';

  @override
  String get findingEasyNat =>
      'Atrás de um roteador que mantém a mesma porta pública para todos os destinos (um NAT fácil): dá para abri-lo dos dois lados ao mesmo tempo.';

  @override
  String get findingUnknownRouter =>
      'Poucos peers viram o endereço público deste computador para identificar o tipo de roteador.';

  @override
  String get findingNoIpv6 =>
      'Sem endereço IPv6 público. O Fogata não precisa dele, mas ele facilita as conexões diretas.';

  @override
  String get findingIpv6Works =>
      'O IPv6 público funciona: amigos com IPv6 se conectam sem nenhum roteador no caminho.';

  @override
  String get findingIpv6Unreached =>
      'Este computador tem um endereço IPv6 público, mas nenhum peer o alcançou por IPv6 ainda.';

  @override
  String get findingSummaryNoUdp =>
      'Em resumo: espere problemas com conexões diretas até o UDP ser liberado.';

  @override
  String get findingSummaryPublic =>
      'Em resumo: seus amigos devem conseguir alcançar este computador diretamente.';

  @override
  String get findingSummaryEasyNat =>
      'Em resumo: as conexões diretas devem funcionar com a maioria dos seus amigos, abrindo os dois roteadores ao mesmo tempo; com um amigo atrás de um NAT difícil, é preciso um membro do grupo no meio.';

  @override
  String get findingSummaryHardNatWithIpv6 =>
      'Em resumo: amigos com IPv6 ou com endereço público se conectam diretamente; com os outros, é preciso um membro do grupo no meio.';

  @override
  String get findingSummaryHardNat =>
      'Em resumo: só amigos com endereço público se conectam diretamente; com os outros, é preciso um membro do grupo no meio.';

  @override
  String get findingSummaryUnknown =>
      'Em resumo: não há informação suficiente; verifique de novo daqui a um minuto.';

  @override
  String findingInternet(int n) {
    return 'Internet: este computador acessa a rede pública ($n peers conhecidos).';
  }

  @override
  String findingDns(String detail) {
    return 'DNS: não foi possível ler as configurações do sistema por completo ($detail)';
  }

  @override
  String findingLanDevices(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n dispositivos Fogata',
      one: '1 dispositivo Fogata',
      zero: '$n dispositivos Fogata',
    );
    return '$_temp0 nesta rede local: os amigos daqui se conectam sem passar pela internet.';
  }

  @override
  String findingFirewall(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'no PowerShell como administrador',
      'macos': 'no Terminal',
      'other': 'em um terminal',
    });
    return 'O firewall deste computador não deixa nada entrar para o Fogata, então um amigo no mesmo lugar atrás de outro roteador (dois roteadores em casa, por exemplo) não consegue alcançá-lo. Execute este comando uma vez $_temp0 para liberar o Fogata:';
  }

  @override
  String get settingsLanguage => 'Idioma';

  @override
  String get settingsLanguageNote =>
      'O idioma dos textos do próprio Fogata. As mensagens ficam como foram escritas.';

  @override
  String get settingsLanguageChoose => 'Escolher o idioma';

  @override
  String get settingsTime => 'Hora';

  @override
  String get settingsTimeNote =>
      'Como os horários aparecem no chat, nas chamadas e nas listas.';

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
      'windows': 'no PowerShell como administrador',
      'macos': 'no Terminal',
      'other': 'em um terminal',
    });
    return 'Um amigo no mesmo lugar atrás de outro roteador não consegue alcançar este computador. Execute uma vez $_temp0: $command';
  }

  @override
  String firewallCopied(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'no PowerShell como administrador',
      'macos': 'no Terminal',
      'other': 'em um terminal',
    });
    return 'Copiado. Cole $_temp0 e execute.';
  }

  @override
  String get appRestarted => 'O Fogata parou de funcionar e foi reiniciado.';

  @override
  String appVoiceRoomInGroup(String room, String group) {
    return '$room em $group';
  }

  @override
  String appBothIn(String room) {
    return 'Vocês dois estão em $room.';
  }

  @override
  String appHungUp(String name) {
    return '$name desligou';
  }

  @override
  String get appYourFriend => 'Seu amigo';

  @override
  String get appCallEndedAlone => 'Ninguém voltou, então a chamada terminou';

  @override
  String get appAnsweredElsewhere => 'Atendida em outro computador';

  @override
  String appVoiceRoomDeleted(String room) {
    return '“$room” foi excluída.';
  }

  @override
  String get appSomeone => 'Alguém';

  @override
  String get appAGroup => 'um grupo';

  @override
  String appFriendRequest(String name) {
    return '$name quer ser seu amigo.';
  }

  @override
  String appRequestDelivered(String name) {
    return 'Pedido entregue para $name. Agora é só esperar a resposta.';
  }

  @override
  String appFriendsNow(String name) {
    return 'Você e $name agora são amigos.';
  }

  @override
  String appJoinedGroup(String group) {
    return 'Agora você está em “$group”.';
  }

  @override
  String appRemovedFromGroup(String group) {
    return 'Você não está mais em “$group”.';
  }

  @override
  String appAddedToGroup(String name, String group) {
    return '$name adicionou você a “$group”. Entrar ou não: você decide.';
  }

  @override
  String appAsksToJoin(String name, String group) {
    return '$name pede para entrar em “$group”.';
  }

  @override
  String appJoinWaiting(String group) {
    return 'Esperando alguém de “$group” deixar você entrar.';
  }

  @override
  String appJoinDeclined(String group) {
    return 'Alguém de “$group” recusou. Esse convite não funciona mais.';
  }

  @override
  String appDownloadedTo(String path) {
    return 'Baixado em $path';
  }

  @override
  String appDownloadFailed(String why) {
    return 'Não foi possível baixar: $why';
  }

  @override
  String get appOtherSession =>
      'Esta identidade também está aberta em outro computador. Feche o Fogata lá: uma identidade funciona em um computador por vez.';

  @override
  String get appFriendsRestored =>
      'Sua lista de amigos estava danificada neste computador e foi restaurada da cópia feita antes da última mudança. Um amigo adicionado ou removido logo antes talvez precise ser adicionado ou removido de novo.';

  @override
  String appInvitationRanOut(String group) {
    return 'O convite para “$group” expirou antes de alguém do grupo deixar você entrar. Peça um novo.';
  }

  @override
  String appTakenOut(String who, String room) {
    return '$who tirou você de $room.';
  }

  @override
  String appRoomForSome(String room) {
    return '$room agora é só para alguns membros.';
  }

  @override
  String appTakenOutUntilLetBack(String who, String room) {
    return '$who tirou você de $room até alguém acima de você deixar você voltar.';
  }

  @override
  String appTakenOutFor(String who, String room, String span) {
    return '$who tirou você de $room por $span.';
  }

  @override
  String appTakenOutWhile(String who, String room) {
    return '$who tirou você de $room por um tempo.';
  }

  @override
  String appComeBackIn(String room, String wait) {
    return 'Você pode voltar para $room em $wait.';
  }

  @override
  String appRoomNotForYou(String room) {
    return '$room não está aberta para você agora.';
  }

  @override
  String appFoldersNotShared(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Não dá para compartilhar pastas, só arquivos: $n pastas ficaram de fora.',
      one: 'Não dá para compartilhar pastas, só arquivos: a pasta ficou de fora.',
      zero:
          'Não dá para compartilhar pastas, só arquivos: $n pastas ficaram de fora.',
    );
    return '$_temp0';
  }

  @override
  String appSharingOne(String file) {
    return 'Compartilhando $file deste computador. Ele deixa de ficar disponível quando você sai da sala.';
  }

  @override
  String appSharingMany(int n) {
    return 'Compartilhando $n arquivos deste computador. Eles deixam de ficar disponíveis quando você sai da sala.';
  }

  @override
  String appSavedTo(String path) {
    return 'Salvo em $path';
  }

  @override
  String appAskingToJoin(String group) {
    return 'Pedindo para entrar em “$group”. Alguém do grupo deixa você entrar quando vir o pedido.';
  }

  @override
  String get appLookingForDevice =>
      'Procurando o outro dispositivo para enviar seu pedido…';

  @override
  String callStateNoAnswer(String name) {
    return '$name não atendeu';
  }

  @override
  String callStateDeclined(String name) {
    return '$name não pode falar agora';
  }

  @override
  String callStateBusy(String name) {
    return '$name está em outra chamada';
  }

  @override
  String callStateOutdated(String name) {
    return '$name precisa de um Fogata mais novo para entrar';
  }

  @override
  String callStateUnreached(String name) {
    return 'Não foi possível alcançar $name';
  }

  @override
  String get callStateReturn => 'Voltar à chamada';

  @override
  String get callStateCallBack => 'Ligar de volta';

  @override
  String get callStateCall => 'Ligar';

  @override
  String noticesDeclined(String name, String group) {
    return '$name recusou o convite para “$group”';
  }

  @override
  String noticesDeclinedToast(String name, String group) {
    return '$name recusou o convite para “$group”.';
  }

  @override
  String noticesMissedInCall(String name) {
    return '$name ligou enquanto você estava em uma chamada';
  }

  @override
  String noticesMissedOffline(String name) {
    return '$name ligou enquanto você aparecia offline';
  }

  @override
  String noticesMissedCall(String name) {
    return 'Chamada perdida de $name';
  }

  @override
  String get contactsGeneral => 'Geral';

  @override
  String get directStateTimerOff => 'Desativado';

  @override
  String get directStateTimer24h => '24 horas';

  @override
  String directStateTimerDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n dias',
      one: '1 dia',
      zero: '$n dias',
    );
    return '$_temp0';
  }

  @override
  String directStateDisappearsMinutes(int n) {
    return 'Some em $n min';
  }

  @override
  String directStateDisappearsHours(int n) {
    return 'Some em $n h';
  }

  @override
  String directStateDisappearsDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Some em $n dias',
    );
    return '$_temp0';
  }

  @override
  String get directStateNoLimit => 'Sem limite';

  @override
  String get directStateMessageGone => 'Essa mensagem não está mais aqui.';

  @override
  String directStateFoldersNotSent(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Não dá para enviar pastas, só arquivos: $n pastas ficaram de fora.',
      one: 'Não dá para enviar pastas, só arquivos: a pasta ficou de fora.',
      zero:
          'Não dá para enviar pastas, só arquivos: $n pastas ficaram de fora.',
    );
    return '$_temp0';
  }

  @override
  String directStateFileFailed(String why) {
    return 'Não foi possível obter o arquivo: $why';
  }

  @override
  String movingTooLarge(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners': 'Banners animados podem ter até $most pixels; este tem $size.',
      'other': 'Imagens animadas podem ter até $most pixels; esta tem $size.',
    });
    return '$_temp0';
  }

  @override
  String movingTooLong(String what, String most, String count) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Banners animados podem ter até $most quadros; este tem $count.',
      'other': 'Imagens animadas podem ter até $most quadros; esta tem $count.',
    });
    return '$_temp0';
  }

  @override
  String movingTooHeavy(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners': 'Banners animados podem ter até $most; este tem $size.',
      'other': 'Imagens animadas podem ter até $most; esta tem $size.',
    });
    return '$_temp0';
  }

  @override
  String get namesHidden =>
      'Este nome tem caracteres ocultos. Digite de novo sem eles.';

  @override
  String get namesBlank =>
      'Este nome fica invisível. Use letras, números ou emojis.';

  @override
  String get namesDefaultBio => 'Oi! Estou no Fogata.';

  @override
  String get namesBioHidden =>
      'Esta bio tem caracteres ocultos. Digite de novo sem eles.';

  @override
  String namesBioLines(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Uma bio tem no máximo $n linhas.',
      one: 'Uma bio tem no máximo 1 linha.',
      zero: 'Uma bio tem no máximo $n linhas.',
    );
    return '$_temp0';
  }

  @override
  String namesBioChars(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Uma bio tem no máximo $n caracteres.',
      one: 'Uma bio tem no máximo 1 caractere.',
      zero: 'Uma bio tem no máximo $n caracteres.',
    );
    return '$_temp0';
  }

  @override
  String get restartFailed =>
      'O Fogata não conseguiu reiniciar. Feche o app e abra de novo.';

  @override
  String typingOne(String name) {
    return '$name está digitando…';
  }

  @override
  String typingTwo(String first, String second) {
    return '$first e $second estão digitando…';
  }

  @override
  String get typingSeveral => 'Várias pessoas estão digitando…';

  @override
  String get outgoingTheRoom => 'a sala';

  @override
  String outgoingDropTitle(String group) {
    return 'Solte para compartilhar em $group';
  }

  @override
  String get outgoingDropNote =>
      'Você pode adicionar uma mensagem e conferir os arquivos antes de enviá-los. Os outros baixam os arquivos deste computador enquanto você estiver na sala.';

  @override
  String outgoingPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n arquivos ocultos ou privados ficaram de fora: arquivos assim não são adicionados arrastando, porque uma página da web pode iniciar um arraste. Para compartilhá-los, escolha-os com o clipe.',
      one: 'Um arquivo oculto ou privado ficou de fora: arquivos assim não são adicionados arrastando, porque uma página da web pode iniciar um arraste. Para compartilhá-lo, escolha-o com o clipe.',
      zero:
          '$n arquivos ocultos ou privados ficaram de fora: arquivos assim não são adicionados arrastando, porque uma página da web pode iniciar um arraste. Para compartilhá-los, escolha-os com o clipe.',
    );
    return '$_temp0';
  }

  @override
  String outgoingToShare(int n, String count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$count arquivos para compartilhar · $size',
      one: '$count arquivo para compartilhar · $size',
      zero: '$count arquivos para compartilhar · $size',
    );
    return '$_temp0';
  }

  @override
  String get outgoingRemoveAll => 'Remover todos';

  @override
  String outgoingRemoveFile(String file) {
    return 'Remover $file';
  }

  @override
  String get windowTrayOpen => 'Abrir o Fogata';

  @override
  String get windowTrayQuit => 'Sair do Fogata';

  @override
  String get windowMinimize => 'Minimizar';

  @override
  String get windowRestore => 'Restaurar';

  @override
  String get windowMaximize => 'Maximizar';

  @override
  String get autostartReason => 'Abrir o Fogata quando você fizer login.';

  @override
  String get devicesMicrophone => 'Microfone';

  @override
  String get devicesSpeaker => 'Alto-falante';

  @override
  String get devicesCamera => 'Câmera';

  @override
  String get devicesChooseMicrophone => 'Escolher microfone';

  @override
  String get devicesChooseSpeaker => 'Escolher alto-falante';

  @override
  String get devicesChooseCamera => 'Escolher câmera';

  @override
  String get devicesNoCamera => 'Nenhuma câmera está conectada.';

  @override
  String get devicesCameraInCall => 'A câmera está ligada na sua chamada.';

  @override
  String devicesCameraFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Não foi possível usar a câmera. Permita em Ajustes do Sistema › Privacidade e Segurança.',
      'other': 'Não foi possível usar a câmera.',
    });
    return '$_temp0';
  }

  @override
  String get devicesCameraDenied =>
      'O sistema não deixa o Fogata usar a câmera. Permita nas configurações do sistema e tente de novo.';

  @override
  String get devicesCameraUnavailable =>
      'Não foi possível acessar a câmera: este sistema não tem portal de câmera.';

  @override
  String get devicesPreviewOnlyYou => 'Só você vê a prévia.';

  @override
  String get devicesStopVideo => 'Parar vídeo';

  @override
  String get devicesTestVideo => 'Testar vídeo';

  @override
  String get devicesNoiseOn => 'Supressão de ruído ativada';

  @override
  String get devicesNoiseOff => 'Supressão de ruído desativada';

  @override
  String get devicesNoise => 'Supressão de ruído';

  @override
  String get devicesNoiseNote =>
      'Reduz ruídos de fundo constantes, como um ventilador ou um zumbido, no que os outros ouvem.';

  @override
  String get devicesNoiseShort => 'Reduz ventilador ou zumbido constante.';

  @override
  String get devicesMirror => 'Espelhar minha câmera';

  @override
  String get devicesMirrorNote =>
      'Muda só como você se vê aqui. Os outros sempre veem sua câmera como ela é.';

  @override
  String get devicesMirrorOn => 'Espelhar minha câmera ativado';

  @override
  String get devicesMirrorOff => 'Espelhar minha câmera desativado';

  @override
  String get devicesTestMic => 'Testar microfone';

  @override
  String get devicesStopMicTest => 'Parar teste';

  @override
  String get devicesMicLevel => 'Nível do microfone';

  @override
  String get devicesHearMyself => 'Me ouvir';

  @override
  String get devicesHearMyselfNote =>
      'Toca seu microfone pelo alto-falante. Use fones de ouvido, senão o som do alto-falante pode voltar para o microfone.';

  @override
  String get devicesHearMyselfOn => 'Me ouvir ativado';

  @override
  String get devicesHearMyselfOff => 'Me ouvir desativado';

  @override
  String get devicesMicTestOnlyYou => 'Nada do teste é enviado a ninguém.';

  @override
  String get devicesMicInCall => 'O microfone está em uso na sua chamada.';

  @override
  String get devicesNoMic => 'Nenhum microfone está conectado.';

  @override
  String get devicesSystemDefault => 'Padrão do sistema';

  @override
  String get devicesNotConnected => 'Desconectado: usando o padrão';

  @override
  String get devicesNoCameraShort => 'Nenhuma câmera conectada';

  @override
  String get devicesMicOptions => 'Opções do microfone';

  @override
  String get devicesSpeakerOptions => 'Opções do alto-falante';

  @override
  String get devicesCameraOptions => 'Opções da câmera';

  @override
  String get devicesInputDevice => 'Dispositivo de entrada';

  @override
  String get devicesOutputDevice => 'Dispositivo de saída';

  @override
  String get devicesChosenMissing =>
      'O dispositivo escolhido não está conectado: usando o padrão.';

  @override
  String get devicesVoiceSettings => 'Configurações de voz e vídeo';

  @override
  String get photoImages => 'Imagens';

  @override
  String photoTooLarge(int n) {
    return 'Essa imagem é grande demais: no máximo $n megapixels.';
  }

  @override
  String get photoUnreadable => 'Não foi possível ler essa imagem.';

  @override
  String get callSource => 'Original';

  @override
  String callQuality(String resolution, int fps) {
    return '$resolution · $fps fps';
  }

  @override
  String get callSetupFailed => 'Não foi possível preparar a chamada.';

  @override
  String get callNoSoundSystem =>
      'Não foi possível acessar o sistema de som deste computador, então a chamada não pode começar.';

  @override
  String callMicGone(String name) {
    return 'Seu microfone foi desconectado: usando $name agora.';
  }

  @override
  String get callMicBack => 'Seu microfone voltou.';

  @override
  String callMicSilent(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Não foi possível usar o microfone. Verifique se ele está conectado e permitido em Ajustes do Sistema › Privacidade e Segurança.',
      'windows': 'Não foi possível usar o microfone. Verifique se ele está conectado e permitido em Configurações › Privacidade e segurança › Microfone.',
      'other':
          'Não foi possível usar o microfone. Verifique se ele está conectado.',
    });
    return '$_temp0';
  }

  @override
  String get callNoMicrophone => 'Nenhum microfone está conectado.';

  @override
  String callMicFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Não foi possível usar o microfone. Permita em Ajustes do Sistema › Privacidade e Segurança.',
      'windows': 'Não foi possível usar o microfone. Permita em Configurações › Privacidade e segurança › Microfone.',
      'other': 'Não foi possível usar o microfone.',
    });
    return '$_temp0';
  }

  @override
  String callScreenFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Não foi possível compartilhar a tela. Permita em Ajustes do Sistema › Privacidade e Segurança.',
      'other': 'Não foi possível compartilhar a tela.',
    });
    return '$_temp0';
  }

  @override
  String get callShareLost =>
      'O compartilhamento parou: o que você compartilhava não pode mais ser capturado.';

  @override
  String callKeptOut(String name) {
    return 'A conexão com $name foi perdida.';
  }

  @override
  String get qualityGood => 'Boa';

  @override
  String get qualityOkay => 'Razoável';

  @override
  String get qualityPoor => 'Ruim';

  @override
  String get qualityLost => 'Perdida';

  @override
  String qualityDelay(int ms) {
    return '$ms ms';
  }

  @override
  String qualityPercent(String value) {
    return '$value %';
  }

  @override
  String get qualityAnotherMember => 'outro membro';

  @override
  String qualityLostHint(String name, int seconds) {
    return 'Nada chega de $name há $seconds s. O Fogata continua procurando; a pessoa volta sozinha assim que a rede dela voltar.';
  }

  @override
  String qualityBothPoor(String name) {
    return 'As duas redes estão ruins agora: você e $name podem se ouvir picotados.';
  }

  @override
  String qualityTheirSide(String name) {
    return 'O problema está do lado de $name: todo mundo ouve a voz picotada. O Wi-Fi ou os dados móveis dessa pessoa estão ruins agora.';
  }

  @override
  String qualityYourSideWith(String name) {
    return 'O problema está do seu lado: $name e os outros podem ouvir sua voz picotada. Seu Wi-Fi ou seus dados móveis estão ruins agora.';
  }

  @override
  String qualityTheirsSlow(String name) {
    return 'A rede de $name está um pouco lenta agora: todo mundo pode ouvir a voz picotada de vez em quando.';
  }

  @override
  String qualityAllGoodWith(String name) {
    return 'Você e $name se ouvem sem problemas.';
  }

  @override
  String qualityMineSlowWith(String name) {
    return 'Sua rede está um pouco lenta agora: $name pode ouvir sua voz picotada de vez em quando.';
  }

  @override
  String qualityWayThrough(String name, String carrier) {
    return 'A rede de $name está bem; o caminho entre vocês dois está mais lento porque passa por $carrier. A voz pode chegar picotada para você de vez em quando; os outros ouvem bem.';
  }

  @override
  String qualityWaySlowFine(String name) {
    return 'A rede de $name está bem; o caminho entre vocês dois está lento agora. A voz pode chegar picotada para você de vez em quando; os outros ouvem bem.';
  }

  @override
  String qualityWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'O caminho entre você e $name está um pouco lento agora; pode ser qualquer uma das redes. Vocês podem se ouvir picotados de vez em quando.',
      'other':
          'O caminho entre você e $name está ruim agora; pode ser qualquer uma das redes. Vocês podem se ouvir picotados.',
    });
    return '$_temp0';
  }

  @override
  String qualityWayVia(String level, String name, String carrier) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'O caminho entre você e $name está um pouco lento agora; pode ser qualquer uma das redes. Vocês podem se ouvir picotados de vez em quando. Ele passa por $carrier.',
      'other':
          'O caminho entre você e $name está ruim agora; pode ser qualquer uma das redes. Vocês podem se ouvir picotados. Ele passa por $carrier.',
    });
    return '$_temp0';
  }

  @override
  String get qualityYourSide =>
      'O problema está do seu lado: os outros podem ouvir sua voz picotada. Seu Wi-Fi ou seus dados móveis estão ruins agora.';

  @override
  String get qualityMineSlow =>
      'Sua rede está um pouco lenta agora: os outros podem ouvir sua voz picotada de vez em quando.';

  @override
  String qualityLostShort(String name, int seconds) {
    return 'Nada chega de $name há $seconds s. O Fogata continua procurando.';
  }

  @override
  String qualityAllWellWith(String name) {
    return 'Tudo certo: você e $name se ouvem sem problemas.';
  }

  @override
  String qualityNoteDropped(String name) {
    return '$name caiu e o Fogata está procurando de novo';
  }

  @override
  String qualityNoteTheirsPoor(String name) {
    return 'a rede de $name está ruim, então todo mundo ouve a voz picotada';
  }

  @override
  String qualityNoteTheirsSlow(String name) {
    return 'a rede de $name está um pouco lenta';
  }

  @override
  String qualityNoteThrough(String name, String carrier) {
    return '$name chega até você por $carrier';
  }

  @override
  String qualityNoteWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': 'o caminho até $name está um pouco lento agora',
      'other': 'o caminho até $name está ruim agora',
    });
    return '$_temp0';
  }

  @override
  String get qualityNoteSeparator => '; ';

  @override
  String get qualityAllWell =>
      'Tudo certo: você ouve todo mundo e todo mundo ouve você.';

  @override
  String qualityAllWellMine(String notes) {
    return 'Tudo certo do seu lado. $notes.';
  }

  @override
  String qualityYourDetails(String level) {
    return 'Sua conexão: $level. Mostrar detalhes';
  }

  @override
  String qualityTheirDetails(String name, String level) {
    return 'Conexão de $name: $level. Mostrar detalhes';
  }

  @override
  String get qualityReconnecting => 'Reconectando…';

  @override
  String get qualityReconnectingLabel => 'Reconectando';

  @override
  String qualityLevelConnection(String level) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'good': 'Conexão boa',
      'okay': 'Conexão razoável',
      'weak': 'Conexão ruim',
      'other': 'Conexão perdida',
    });
    return '$_temp0';
  }

  @override
  String get qualityPoorConnection => 'Conexão ruim';

  @override
  String get qualityMinePoorBanner =>
      'Sua rede está ruim: os outros podem ouvir sua voz picotada';

  @override
  String get qualityYourNetwork => 'Sua rede';

  @override
  String qualityTo(String name) {
    return 'Até $name';
  }

  @override
  String qualityBetween(String name) {
    return 'Entre você e $name';
  }

  @override
  String qualityTheirNetwork(String name) {
    return 'Rede de $name';
  }

  @override
  String get qualityLastHeard => 'Último sinal';

  @override
  String qualitySecondsAgo(int n) {
    return 'há $n s';
  }

  @override
  String get qualityPath => 'Caminho';

  @override
  String get qualityLookingAgain => 'Procurando de novo';

  @override
  String get qualityDelayLabel => 'Latência';

  @override
  String get qualityLostPackets => 'Pacotes perdidos';

  @override
  String get qualityYourConnection => 'Sua conexão';

  @override
  String qualityTheirConnection(String name) {
    return 'Conexão de $name';
  }

  @override
  String roomCountFullLabel(int count, int max) {
    return 'Cheia, $count de $max pessoas';
  }

  @override
  String roomCountLabel(int count, int max) {
    return '$count de $max pessoas';
  }

  @override
  String get roomCountFullPrefix => 'Cheia · ';

  @override
  String get roomCountFullWord => 'Cheia ';

  @override
  String get volumeVoice => 'Volume da voz';

  @override
  String get volumeOnlyYouHear => 'Só você ouve essas mudanças';

  @override
  String get volumeScreenSound => 'Som da tela';

  @override
  String volumePercent(int n) {
    return '$n%';
  }

  @override
  String get volumeMuteVoiceForMe => 'Silenciar voz para mim';

  @override
  String get volumeMuteForMe => 'Silenciar para mim';

  @override
  String get volumeMuteScreenForMe => 'Silenciar som da tela para mim';

  @override
  String get volumeBackTo100 => 'Voltar para 100%';

  @override
  String get volumeBelowYou => 'ABAIXO DE VOCÊ';

  @override
  String volumeRemoveFrom(String room) {
    return 'Tirar de $room';
  }

  @override
  String volumeRemoveTitle(String name, String room) {
    return 'Tirar $name de $room?';
  }

  @override
  String volumeRemoveText(String name, String room) {
    return '$name sai de $room agora e pode voltar na hora. Uma suspensão mantém a pessoa fora por um tempo.';
  }

  @override
  String volumeTimeOutOf(String room) {
    return 'Suspender de $room';
  }

  @override
  String volumeTimeOutText(String name, String room) {
    return '$name sai de $room e não pode voltar por:';
  }

  @override
  String get volumeUntilLetBack => 'Até alguém acima permitir a volta';

  @override
  String get pipYourScreen => 'Sua tela';

  @override
  String pipScreenOf(String name) {
    return 'Tela de $name';
  }

  @override
  String get pipYourCamera => 'Sua câmera';

  @override
  String get pipBackToCall => 'Voltar para a chamada';

  @override
  String get pipBackToRoom => 'Voltar para a sala';

  @override
  String get pipStopWatching => 'Parar de assistir';

  @override
  String get addToCallTitle => 'Adicionar à chamada';

  @override
  String addToCallNote(int n) {
    return 'Amigos que estão online. Até $n pessoas em uma chamada.';
  }

  @override
  String get addToCallSearch => 'Buscar amigos';

  @override
  String get addToCallNoFriends => 'Nenhum amigo ainda.';

  @override
  String get addToCallNoMatch => 'Nenhum amigo com esse nome.';

  @override
  String get addToCallInCall => 'Na chamada';

  @override
  String get addToCallCalling => 'Ligando…';

  @override
  String get addToCallOffline => 'Offline';

  @override
  String get addToCallCall => 'Ligar';

  @override
  String get streamQualityTitle => 'Qualidade da transmissão';

  @override
  String get streamQualityNoteSharing =>
      'Como sua tela chega aos outros. Uma mudança vale na hora.';

  @override
  String get streamQualityNote =>
      'Como sua tela chega aos outros quando você a compartilha.';

  @override
  String get streamQualityResolution => 'Resolução';

  @override
  String streamQualitySourceLines(int lines) {
    return 'Original (${lines}p)';
  }

  @override
  String get streamQualitySource => 'Original';

  @override
  String get streamQualityFrameRate => 'Taxa de quadros';

  @override
  String streamQualityFps(int n) {
    return '$n fps';
  }

  @override
  String get streamQualityHigherNote =>
      'Quanto mais alta, mais usa do seu upload e do seu computador.';

  @override
  String get streamQualitySomeone => 'Alguém';

  @override
  String streamQualityLoweredOne(String name, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Os outros recebem o que você escolheu.',
      'other': '',
    });
    return 'A conexão de $name não aguenta mais agora, então essa pessoa recebe uma imagem menor.$_temp0';
  }

  @override
  String streamQualityLoweredMany(String names, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Os outros recebem o que você escolheu.',
      'other': '',
    });
    return 'Algumas conexões não aguentam mais agora: $names recebem uma imagem menor.$_temp0';
  }

  @override
  String get streamQualityGoingOut => 'ENVIANDO AGORA';

  @override
  String get streamQualityNobody => 'Ninguém está assistindo ainda.';

  @override
  String streamQualityNow(int height, int fps) {
    return '${height}p · $fps fps';
  }

  @override
  String callLayersIsCalling(String name) {
    return '$name está ligando';
  }

  @override
  String get callLayersGoToCall => 'F6 vai para a chamada';

  @override
  String callLayersWith(String names) {
    return 'Com $names';
  }

  @override
  String get callLayersDirectCall => 'Chamada direta · ';

  @override
  String get callLayersAnswerMicOff => 'Atender com o microfone desligado';

  @override
  String callLayersAnsweringLeaves(String room) {
    return 'Atender faz você sair de $room';
  }

  @override
  String get callLayersLeaveAndAnswer => 'Sair e atender';

  @override
  String callLayersBothIn(String room) {
    return 'Vocês dois estão em $room.';
  }

  @override
  String get callLayersDismiss => 'Dispensar';

  @override
  String callLayersCallWith(String names) {
    return 'Chamada com $names';
  }

  @override
  String get callLayersRoom => 'Sala';

  @override
  String callLayersBackTo(String title) {
    return 'Voltar para $title';
  }

  @override
  String callLayersRinging(String time) {
    return 'Tocando · $time';
  }

  @override
  String callLayersInCall(String time) {
    return 'Em chamada · $time';
  }

  @override
  String callLayersOnlyYou(String time) {
    return 'Só você · $time';
  }

  @override
  String callLayersInRoom(int n, String time) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n na sala',
    );
    return '$_temp0 · $time';
  }

  @override
  String get callLayersMute => 'Silenciar microfone';

  @override
  String get callLayersUnmute => 'Ativar microfone';

  @override
  String get callLayersHangUp => 'Desligar';

  @override
  String get callLayersLeaveRoom => 'Sair da sala';

  @override
  String roomDeclineCallTitle(String name) {
    return 'Recusar a chamada de $name?';
  }

  @override
  String roomDeclineCallText(String caller, String callee) {
    return '$caller está ligando para você. Você só pode estar em uma chamada por vez: ligar para $callee recusa essa chamada.';
  }

  @override
  String roomDeclineCallTextSomeone(String caller) {
    return '$caller está ligando para você. Você só pode estar em uma chamada por vez: ligar para outra pessoa recusa essa chamada.';
  }

  @override
  String get roomDeclineAndCall => 'Recusar e ligar';

  @override
  String roomHangUpTitle(String name) {
    return 'Desligar a chamada com $name?';
  }

  @override
  String get roomHangUpText =>
      'Você só pode estar em uma chamada ou sala por vez: continuar encerra a chamada.';

  @override
  String get roomHangUp => 'Desligar';

  @override
  String get roomCloseTitle => 'Sair e fechar a sala?';

  @override
  String get roomCloseText =>
      'Só resta você aqui. Quando você sair, a sala fecha e o que foi dito some deste computador; o que os outros baixaram fica com eles.';

  @override
  String get roomCloseChatMessages => 'Mensagens do chat';

  @override
  String get roomCloseFilesShared => 'Arquivos que você compartilhou';

  @override
  String get roomStay => 'Ficar';

  @override
  String get roomLeaveAndClose => 'Sair e fechar';

  @override
  String get roomLivePrefix => 'Ao vivo · ';

  @override
  String get roomPillConnecting => 'Conectando';

  @override
  String get roomPillOnlyYou => 'Só você';

  @override
  String get roomPillError => 'Erro';

  @override
  String get roomLastOne => 'Só resta você na sala.';

  @override
  String get roomOnlyYouIn => 'Só você está na sala.';

  @override
  String get roomAloneWarning =>
      ' Quando você sair, ela fecha e tudo o que há nela some.';

  @override
  String get roomEarlierPeople => 'Pessoas anteriores';

  @override
  String get roomMorePeople => 'Mais pessoas';

  @override
  String roomTileYou(String name) {
    return '$name (você)';
  }

  @override
  String roomTileNoSound(String name) {
    return '$name · sem som';
  }

  @override
  String roomTileConnecting(String name) {
    return '$name · conectando';
  }

  @override
  String roomTileAway(String name) {
    return '$name · sem conexão';
  }

  @override
  String roomTileMicOff(String name) {
    return '$name, microfone desligado';
  }

  @override
  String roomTileMutedByYou(String said) {
    return '$said, silenciado para você';
  }

  @override
  String roomTileVolume(String said, String percent) {
    return '$said, volume $percent';
  }

  @override
  String roomTileLeft(String name) {
    return '$name saiu · pode voltar';
  }

  @override
  String roomIsSharing(String name) {
    return '$name está compartilhando a tela';
  }

  @override
  String get roomNothingSentUntilWatch =>
      'Nada é enviado para você até você assistir';

  @override
  String get roomWatch => 'Assistir';

  @override
  String get roomLive => 'AO VIVO';

  @override
  String get roomSharingYourScreen => 'Você está compartilhando sua tela';

  @override
  String get roomShowPreview => 'Mostrar prévia';

  @override
  String get roomPreviewMirror =>
      'Mostrá-la enquanto você compartilha o próprio Fogata cria um espelho sem fim';

  @override
  String get roomNobodyWatching => 'Ninguém assistindo';

  @override
  String roomPeopleWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n pessoas assistindo',
      one: '1 pessoa assistindo',
      zero: '$n pessoas assistindo',
    );
    return '$_temp0';
  }

  @override
  String get roomExitFullScreen => 'Sair da tela cheia';

  @override
  String get roomFullScreen => 'Tela cheia';

  @override
  String get roomPlayScreenSound => 'Tocar o som da tela';

  @override
  String get roomMuteScreenSound => 'Silenciar o som da tela';

  @override
  String get roomQualityPoor => 'Ruim';

  @override
  String get roomQualityOkay => 'Razoável';

  @override
  String get roomQualityGood => 'Boa';

  @override
  String get roomQualityPoorTitle => 'Conexão ruim';

  @override
  String get roomQualityOkayTitle => 'A conexão está mais ou menos';

  @override
  String get roomQualityGoodTitle => 'Conexão boa';

  @override
  String get roomQualityMeasuring => 'Medindo';

  @override
  String get roomQualityDelay => 'Latência';

  @override
  String roomQualityMs(String n) {
    return '$n ms';
  }

  @override
  String get roomQualityLost => 'Pacotes perdidos';

  @override
  String roomQualityPercent(String n) {
    return '$n %';
  }

  @override
  String get roomQualityPicture => 'Imagem';

  @override
  String get roomQualityPath => 'Caminho';

  @override
  String get roomQualityMayFreeze =>
      'A imagem pode travar. Menos pessoas compartilhando ao mesmo tempo ajuda.';

  @override
  String get roomQualityMayShrink =>
      'A imagem pode diminuir para continuar fluindo.';

  @override
  String roomCallingName(String name) {
    return 'Ligando para $name…';
  }

  @override
  String get roomRingsFor30 =>
      'Toca no computador da pessoa por até 30 segundos.';

  @override
  String get roomReturnToCall => 'Voltar à chamada';

  @override
  String get roomCallAgain => 'Ligar de novo';

  @override
  String get roomAloneHint =>
      'Quando alguém do grupo entrar, vai aparecer aqui.';

  @override
  String roomAlreadyLeft(String names) {
    return 'Já saíram: $names.';
  }

  @override
  String get roomGroupFound => 'Grupo encontrado na rede';

  @override
  String get roomLookingForRoute => 'procurando o melhor caminho…';

  @override
  String get roomConnectingTitle => 'Conectando à sala';

  @override
  String roomSeconds(String n) {
    return '${n}s';
  }

  @override
  String roomReachedOf(int reached, int total, String time) {
    return '$reached de $total · $time';
  }

  @override
  String get roomPathLan => 'rede local, com criptografia';

  @override
  String get roomPathDirect => 'direto, com criptografia';

  @override
  String get roomPathThroughMember => 'por um membro, com criptografia';

  @override
  String roomPathThrough(String name) {
    return 'por $name, com criptografia';
  }

  @override
  String get roomCouldNotConnect => 'Não foi possível conectar à sala';

  @override
  String get roomCouldNotConnectText =>
      'Ninguém respondeu a tempo. Sua rede pode estar bloqueando conexões diretas entre dispositivos.';

  @override
  String get roomNoAnswer => 'sem resposta';

  @override
  String get roomSeeNetworkCheck => 'Ver a verificação de rede →';

  @override
  String get roomGoesNothing => 'A sala fecha para todos';

  @override
  String roomGoesMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n mensagens somem',
      one: '1 mensagem some',
      zero: '$n mensagens somem',
    );
    return '$_temp0';
  }

  @override
  String roomGoesFiles(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n arquivos que você compartilhou vão embora',
      one: '1 arquivo que você compartilhou vai embora',
      zero: '$n arquivos que você compartilhou vão embora',
    );
    return '$_temp0';
  }

  @override
  String roomGoesBoth(int messages, int files) {
    String _temp0 = intl.Intl.pluralLogic(
      messages,
      locale: localeName,
      other: '$messages mensagens',
      one: '1 mensagem',
      zero: '$messages mensagens',
    );
    String _temp1 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files arquivos',
      one: '1 arquivo',
      zero: '$files arquivos',
    );
    return '$_temp0 e $_temp1 somem';
  }

  @override
  String get roomMuteMicrophone => 'Silenciar microfone';

  @override
  String get roomUnmuteMicrophone => 'Ativar microfone';

  @override
  String get roomStopSharingScreen => 'Parar de compartilhar a tela';

  @override
  String get roomShareScreen => 'Compartilhar a tela';

  @override
  String get roomUndeafen => 'Ouvir os outros de novo';

  @override
  String get roomDeafen => 'Parar de ouvir os outros';

  @override
  String get roomCameraOff => 'Desligar a câmera';

  @override
  String get roomCameraOn => 'Ligar a câmera';

  @override
  String get roomLeave => 'Sair';

  @override
  String get roomSoundWaitsForMic =>
      'O som só é enviado com o microfone ligado';

  @override
  String get roomStopSharingSound => 'Parar de compartilhar o som';

  @override
  String get roomShareSoundToo => 'Compartilhar o som também';

  @override
  String get roomAddFriendToCall => 'Adicionar um amigo à chamada';

  @override
  String get roomHideChat => 'Esconder o chat';

  @override
  String get roomShowChat => 'Mostrar o chat';

  @override
  String get roomTabChat => 'Chat';

  @override
  String get roomTabFiles => 'Arquivos';

  @override
  String roomNoteLeftCall(String name) {
    return '$name saiu da chamada · ';
  }

  @override
  String roomNoteJoinedCall(String name) {
    return '$name entrou na chamada · ';
  }

  @override
  String roomNoteLeftRoom(String name) {
    return '$name saiu da sala · ';
  }

  @override
  String roomNoteEnteredRoom(String name) {
    return '$name entrou na sala · ';
  }

  @override
  String get roomSearchMessages => 'Buscar mensagens';

  @override
  String get roomClearSearch => 'Limpar a busca';

  @override
  String get roomChatGoneCall => 'Este chat some quando a chamada terminar';

  @override
  String get roomChatGoneRoom => 'Este chat some quando a sala fechar';

  @override
  String roomNoMatches(String text) {
    return 'Nenhuma mensagem com “$text”';
  }

  @override
  String roomMatches(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n resultados',
      one: '1 resultado',
      zero: '$n resultados',
    );
    return '$_temp0';
  }

  @override
  String get roomReceivingMessages => 'Recebendo as mensagens da sala…';

  @override
  String roomReceivingMessagesFrom(String name) {
    return 'Recebendo as mensagens da sala do computador de $name…';
  }

  @override
  String get roomSomeone => 'Alguém';

  @override
  String get roomShareFiles => 'Compartilhar arquivos';

  @override
  String get roomAddMoreFiles => 'Adicionar mais arquivos';

  @override
  String roomMessageTo(String name) {
    return 'Mensagem para $name';
  }

  @override
  String get roomMessageTheRoom => 'Mensagem para a sala';

  @override
  String get roomMessageTheCall => 'Mensagem para a chamada';

  @override
  String get roomAddMessageOptional => 'Adicionar uma mensagem (opcional)';

  @override
  String get roomMessageLabel => 'Mensagem';

  @override
  String roomEnterToSend(String key) {
    return '$key para enviar';
  }

  @override
  String roomEscToRemove(String key) {
    return '$key para removê-los';
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
    return 'Indisponível · $name saiu';
  }

  @override
  String get roomSharingFromHere => 'Compartilhando deste computador';

  @override
  String roomAvailableWhile(String names) {
    return 'Disponível enquanto $names estiver na sala';
  }

  @override
  String roomSizeDownloads(String size, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n downloads',
      one: '1 download',
      zero: '$n downloads',
    );
    return '$size · $_temp0';
  }

  @override
  String roomDownloadFile(String file) {
    return 'Baixar $file';
  }

  @override
  String roomShowInFolder(String file) {
    return 'Mostrar $file na pasta';
  }

  @override
  String roomGettingFile(String file) {
    return 'Obtendo $file…';
  }

  @override
  String get roomSaveToDownloads => 'Salvar em Downloads';

  @override
  String roomPictureKept(String name) {
    return '$name saiu · a imagem fica aqui até você sair';
  }

  @override
  String get roomSaveFile => 'Salvar';

  @override
  String get roomClosePicture => 'Fechar a imagem';

  @override
  String get roomFilesNote =>
      'Os arquivos não são enviados para nenhum servidor: vão direto do computador de quem compartilha, e só enquanto essa pessoa estiver na sala.';

  @override
  String roomFilesAvailable(int n) {
    return 'Disponíveis · $n';
  }

  @override
  String roomFilesGone(int n) {
    return 'Indisponíveis · $n';
  }

  @override
  String roomPersonLeft(String name) {
    return '$name saiu';
  }

  @override
  String get roomPickOrDrop => 'Escolha aqui ou solte na sala';

  @override
  String get roomFileYours => 'Seu';

  @override
  String roomFileFrom(String name) {
    return 'De $name';
  }

  @override
  String roomDownloads(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n downloads',
      one: '1 download',
      zero: '$n downloads',
    );
    return '$_temp0';
  }

  @override
  String get roomStopSharingFile => 'Parar';

  @override
  String get roomWhatToShare => 'O que você quer compartilhar?';

  @override
  String roomScreenSource(String name) {
    return 'Tela: $name';
  }

  @override
  String roomPlayVideo(String file) {
    return 'Reproduzir $file';
  }

  @override
  String get roomCloseVideo => 'Fechar o vídeo';

  @override
  String get roomVideoGone => 'Este vídeo não está mais na sala.';

  @override
  String get roomVideoNotFetched => 'Não foi possível obter este vídeo.';

  @override
  String get roomVideoCannotPlay =>
      'Este computador não consegue reproduzir este vídeo.';

  @override
  String roomGettingVideoPercent(int n) {
    return 'Obtendo o vídeo · $n%';
  }

  @override
  String get roomGettingVideo => 'Obtendo o vídeo…';

  @override
  String get roomPause => 'Pausar';

  @override
  String get roomPlay => 'Reproduzir';

  @override
  String get roomSoundOn => 'Ativar som';

  @override
  String get roomMuteVideo => 'Silenciar';

  @override
  String get roomLeaveFullScreen => 'Sair da tela cheia';

  @override
  String roomCallWaits(String time) {
    return 'Quando só resta uma pessoa, a chamada espera $time até os outros voltarem';
  }

  @override
  String roomCallWaitsAlone(String time) {
    return 'Só resta você. A chamada espera $time até os outros voltarem';
  }

  @override
  String roomOpenFile(String file) {
    return 'Abrir $file';
  }

  @override
  String activityCreated(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 criou o grupo como “$name”';
  }

  @override
  String activityRenamed(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 renomeou o grupo para “$name”';
  }

  @override
  String activityIcon(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 mudou o ícone do grupo';
  }

  @override
  String activityPictureRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 removeu a imagem do grupo';
  }

  @override
  String activityPicture(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 mudou a imagem do grupo';
  }

  @override
  String activityBannerRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 removeu o banner do grupo';
  }

  @override
  String activityBanner(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 mudou o banner do grupo';
  }

  @override
  String activityAdded(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'você',
      'other': '$member',
    });
    return '$_temp0 adicionou $_temp1';
  }

  @override
  String activityJoined(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'você',
      'other': '$member',
    });
    return '$_temp0 entrou com um convite de $_temp1';
  }

  @override
  String activityLetIn(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'você',
      'other': '$member',
    });
    return '$_temp0 deixou $_temp1 entrar';
  }

  @override
  String activityRemoved(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'você',
      'other': '$member',
    });
    return '$_temp0 removeu $_temp1';
  }

  @override
  String activityLeft(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 saiu do grupo';
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
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'você',
      'other': '$member',
    });
    return '$_temp0 subiu $_temp1 para o $place lugar';
  }

  @override
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'você',
      'other': '$member',
    });
    return '$_temp0 moveu $_temp1 para o fim';
  }

  @override
  String activityManagersAll(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 deixou todos administrarem o grupo';
  }

  @override
  String activityManagersFirst(String by, String byMe, int n) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'para os $n primeiros',
      one: 'só para o primeiro',
      zero: 'para os $n primeiros',
    );
    return '$_temp0 deixou a administração do grupo $_temp1';
  }

  @override
  String activityLimits(String by, String byMe, int people, int perRoom) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 definiu o limite do grupo em $people pessoas e $perRoom por sala de voz';
  }

  @override
  String activityRoomOpened(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 abriu a sala de voz “$room”';
  }

  @override
  String activityRoomRenamed(String by, String byMe, String from, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 renomeou a sala de voz “$from” para “$room”';
  }

  @override
  String activityRoomClosed(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 excluiu a sala de voz “$room”';
  }

  @override
  String activityInvited(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 criou um link de convite';
  }

  @override
  String activityRevoked(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 cancelou um link de convite';
  }

  @override
  String activityRoomOpenToAll(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 abriu “$room” para todos';
  }

  @override
  String activityRoomForAbove(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 reservou “$room” para quem está acima';
  }

  @override
  String activityRoomOnly(String by, String byMe, String people, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Você',
      'other': '$by',
    });
    return '$_temp0 deixou só $people entrar em “$room”';
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
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'você',
      'other': '$member',
    });
    return '$_temp0 manteve $_temp1 fora de “$room” até nova liberação';
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
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'você',
      'other': '$member',
    });
    String _temp2 = intl.Intl.selectLogic(untilKind, {
      'today': 'até as $until',
      'todayAtOne': 'até a $until',
      'yesterday': 'até $until',
      'other': 'até o dia $until',
    });
    return '$_temp0 suspendeu $_temp1 de “$room” $_temp2';
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
      'yes': 'Você',
      'other': '$by',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'você',
      'other': '$member',
    });
    return '$_temp0 deixou $_temp1 voltar para “$room”';
  }

  @override
  String get activityDefaultRoom => 'Geral';

  @override
  String get activityYouInList => 'você';

  @override
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'você',
      'other': '$author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' por você',
      'other': ' por $by',
      'none': '',
    });
    return 'Desfeito: $_temp0 tinha sido removido$_temp1 ao mesmo tempo';
  }

  @override
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'você',
      'other': '$author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' por você',
      'other': ' por $by',
      'none': '',
    });
    return 'Desfeito: $_temp0 tinha sido movido para o fim$_temp1 ao mesmo tempo';
  }

  @override
  String activityUndoneLeft(String author, String authorMe) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'você',
      'other': '$author',
    });
    return 'Desfeito: $_temp0 tinha saído ao mesmo tempo';
  }

  @override
  String get activityAboutList =>
      'Todos no grupo veem a mesma lista, tirada do registro do próprio grupo. As mudanças de imagem e de banner ficam à parte: são as que este dispositivo viu.';

  @override
  String get activityAboutOrder =>
      'Na ordem do próprio grupo; os horários são do relógio de cada autor.';

  @override
  String get activityNothingYet => 'Nada ainda.';

  @override
  String get activityLoading => 'Carregando…';

  @override
  String get activityShowOlder => 'Mostrar mais antigos';

  @override
  String activitySeenHere(String when) {
    return '$when · visto neste dispositivo';
  }

  @override
  String get bellNotifications => 'Notificações';

  @override
  String bellNewNotifications(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n notificações novas',
      one: '1 notificação nova',
      zero: '$n notificações novas',
    );
    return '$_temp0';
  }

  @override
  String get bellClearAll => 'Limpar tudo';

  @override
  String get bellNothingNew => 'Nada de novo';

  @override
  String get bellFooter =>
      'As três últimas ficam aqui. Chamadas perdidas e pedidos também aparecem em Amigos.';

  @override
  String bellBothInRoom(String room) {
    return 'Vocês dois estão em $room.';
  }

  @override
  String get bellDismiss => 'Dispensar';

  @override
  String bellFriendRequest(String name) {
    return '$name quer ser seu amigo';
  }

  @override
  String bellCheckFingerprint(String fingerprint) {
    return 'Confira se a impressão digital bate: $fingerprint';
  }

  @override
  String bellJoinAsk(String name, String group) {
    return '$name pede para entrar em “$group”';
  }

  @override
  String bellJoinAskNote(String fingerprint) {
    return 'Com um dos links de convite do seu grupo. Impressão digital: $fingerprint';
  }

  @override
  String get bellLetIn => 'Deixar entrar';

  @override
  String get bellSayNo => 'Recusar';

  @override
  String bellGroupInvite(String name, String group) {
    return '$name adicionou você a “$group”';
  }

  @override
  String bellMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n membros',
      one: '1 membro',
      zero: '$n membros',
    );
    return '$_temp0';
  }

  @override
  String get bellJoin => 'Entrar';

  @override
  String bellNewMessages(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n mensagens novas de $name',
      one: 'Nova mensagem de $name',
      zero: '$n mensagens novas de $name',
    );
    return '$_temp0';
  }

  @override
  String get bellOpen => 'Abrir';

  @override
  String get bellMarkRead => 'Marcar como lidas';

  @override
  String get bellFirewallTitle => 'Seu firewall deixa alguns amigos de fora';

  @override
  String get bellCopyCommand => 'Copiar comando';

  @override
  String get bellOtherSessionTitle => 'Aberto também em outro computador';

  @override
  String get bellSettingsResetTitle =>
      'Algumas configurações foram redefinidas';

  @override
  String get bellSettingsResetText =>
      'Algumas das suas configurações não puderam ser lidas e voltaram ao padrão: confira-as em Configurações. O arquivo antigo foi mantido ao lado do novo, como app.json.bad.';

  @override
  String get friendCardOnline => 'Online';

  @override
  String get friendCardAway => 'Ausente';

  @override
  String get friendCardOffline => 'Offline';

  @override
  String get friendCardCallsOnlyOnline =>
      'As chamadas só tocam enquanto o amigo está online.';

  @override
  String friendCardHangUpFirst(String name) {
    return 'Desligue primeiro para ligar para $name.';
  }

  @override
  String friendCardBothInRoom(String room) {
    return 'Vocês dois estão em $room.';
  }

  @override
  String get friendCardMessage => 'Mensagem';

  @override
  String friendCardMutualFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n amigos em comum',
      one: '1 amigo em comum',
      zero: '$n amigos em comum',
    );
    return '$_temp0';
  }

  @override
  String get memberCardSendRequest => 'Enviar pedido de amizade';

  @override
  String get memberCardRequestSent => 'Pedido de amizade enviado';

  @override
  String get memberCardAccept => 'Aceitar pedido de amizade';

  @override
  String memberCardAsksYou(String name) {
    return '$name quer ser seu amigo.';
  }

  @override
  String memberCardInGroup(String group) {
    return 'Membro de $group';
  }

  @override
  String memberCardOnlyOnline(String name) {
    return 'Pedidos de amizade só chegam enquanto $name está online.';
  }

  @override
  String get memberCardShowOwn => 'Mostrar seu cartão';

  @override
  String get userPanelEditProfile => 'Editar perfil';

  @override
  String get userPanelStatus => 'Status';

  @override
  String get userPanelOnline => 'Online';

  @override
  String get userPanelAway => 'Ausente';

  @override
  String get userPanelOffline => 'Offline';

  @override
  String get userPanelOfflineNote =>
      'Seus amigos e os membros dos seus grupos veem você como offline. Você ainda pode estar em salas.';

  @override
  String get userPanelOnlineNote =>
      'Seus amigos e os membros dos seus grupos veem você online, ou ausente após 10 min sem usar este computador.';

  @override
  String get userPanelBio => 'BIO';

  @override
  String get userPanelAppearingOffline => 'Aparecendo offline';

  @override
  String get userPanelCopyLinkLabel => 'Copiar um link de amizade';

  @override
  String userPanelLinkExpiresIn(String time) {
    return 'Expira em $time';
  }

  @override
  String get userPanelLinkTerms => '1 pessoa · 10 min';

  @override
  String get userPanelCopyLink => 'Copiar link de amizade';

  @override
  String get switcherChatEnded => 'Conversa direta · não são mais amigos';

  @override
  String get switcherChatOnline => 'Conversa direta · online';

  @override
  String get switcherChatAway => 'Conversa direta · ausente';

  @override
  String get switcherChat => 'Conversa direta';

  @override
  String switcherFriend(String presence) {
    return 'Amigo · $presence';
  }

  @override
  String switcherVoiceRoom(String group) {
    return 'Sala de voz em $group';
  }

  @override
  String switcherVoiceRoomPeople(String group, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Sala de voz em $group · $n pessoas',
      one: 'Sala de voz em $group · $n pessoa',
    );
    return '$_temp0';
  }

  @override
  String switcherGroup(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Grupo · $n membros',
      one: 'Grupo · 1 membro',
      zero: 'Grupo · $n membros',
    );
    return '$_temp0';
  }

  @override
  String get switcherPlaceholder =>
      'Ir para uma conversa, grupo ou sala de voz';

  @override
  String get switcherGoTo => 'Ir para';

  @override
  String get switcherEmpty => 'Nenhum grupo, amigo ou conversa ainda.';

  @override
  String switcherNoMatch(String query) {
    return 'Nada corresponde a “$query”.';
  }

  @override
  String get switcherHintChoose => 'para escolher';

  @override
  String get switcherHintGo => 'para ir';

  @override
  String get switcherHintClose => 'para fechar';

  @override
  String switcherTooltip(String shortcut) {
    return 'Ir para uma conversa, grupo ou sala de voz ($shortcut)';
  }

  @override
  String get friendsIntro =>
      'Cada amizade existe só nos dois dispositivos. Sem diretório, sem servidor.';

  @override
  String get friendsTabAll => 'Todos';

  @override
  String get friendsTabOnline => 'Online';

  @override
  String get friendsTabRequests => 'Pedidos';

  @override
  String get friendsSearch => 'Buscar amigos';

  @override
  String get friendsNone =>
      'Nenhum amigo ainda. Compartilhe seu QR, link ou código, ou use o de outra pessoa.';

  @override
  String get friendsNoneOnline => 'Nenhum amigo está online agora.';

  @override
  String get friendsNoMatch => 'Ninguém corresponde à busca.';

  @override
  String get friendsInARoom => 'Em uma sala';

  @override
  String friendsInRoomOf(String group) {
    return 'Em uma sala · $group';
  }

  @override
  String get friendsOnline => 'Online';

  @override
  String get friendsAway => 'Ausente';

  @override
  String get friendsNotSeenYet => 'Ainda não apareceu online';

  @override
  String friendsSeen(String when) {
    return 'Visto $when';
  }

  @override
  String friendsShowCard(String name) {
    return 'Mostrar o cartão de $name';
  }

  @override
  String friendsMissedCall(String when) {
    return 'Chamada perdida · $when';
  }

  @override
  String get friendsMessage => 'Mensagem';

  @override
  String friendsOfflineNoCall(String name) {
    return '$name está offline: as chamadas só tocam enquanto o amigo está online';
  }

  @override
  String friendsHangUpFirst(String name) {
    return 'Desligue primeiro para ligar para $name';
  }

  @override
  String friendsBothInRoom(String room) {
    return 'Vocês dois estão em $room.';
  }

  @override
  String friendsOptions(String name) {
    return 'Opções para $name';
  }

  @override
  String friendsRemoveTitle(String name) {
    return 'Remover $name dos seus amigos?';
  }

  @override
  String get friendsRemoveText =>
      'Vocês somem da lista de amigos um do outro, sem aviso. Vocês continuam juntos nos grupos em comum. Para voltar a ser amigos, é preciso outro convite.';

  @override
  String get friendsRemove => 'Remover';

  @override
  String friendsAddToGroup(String group) {
    return 'Adicionar a “$group”';
  }

  @override
  String get friendsRemoveMenu => 'Remover dos amigos…';

  @override
  String get friendsNoRequests => 'Nenhum pedido pendente.';

  @override
  String friendsRequestIncoming(String when) {
    return 'Quer ser seu amigo · $when';
  }

  @override
  String friendsRequestOutgoing(String when) {
    return 'Enviado $when · esperando resposta';
  }

  @override
  String get friendsPendingRequest => 'Pedido pendente';

  @override
  String friendsCheckFingerprint(String name) {
    return 'Confira se a impressão digital é a mesma que $name vê na tela.';
  }

  @override
  String get friendsAddTitle => 'Adicionar um amigo';

  @override
  String get friendsAddNote => 'Seus dispositivos trocam chaves diretamente.';

  @override
  String get friendsTabCode => 'Código';

  @override
  String get friendsTabLink => 'Link';

  @override
  String get friendsTabQr => 'QR';

  @override
  String get friendsMakeInvitation => 'Criar um convite';

  @override
  String get friendsInvitationExpired => 'Este convite expirou.';

  @override
  String get friendsMakeAnother => 'Criar outro';

  @override
  String friendsHintQr(String time) {
    return 'Escaneie pelo Fogata · expira em $time';
  }

  @override
  String friendsHintLink(String time) {
    return 'Envie para uma pessoa · expira em $time';
  }

  @override
  String friendsHintCode(String time) {
    return 'Leia em voz alta para uma pessoa · expira em $time';
  }

  @override
  String get friendsHiddenCode => 'Oculto caso você compartilhe a tela.';

  @override
  String get friendsHiddenLink =>
      'Oculto caso você compartilhe a tela. Ele contém os endereços de rede deste computador: compartilhe só em privado.';

  @override
  String get friendsGotLink =>
      'Recebeu um link de amizade ou de grupo, ou um código?';

  @override
  String get friendsPastePlaceholder => 'Cole um link ou código';

  @override
  String get friendsLinkOrCode => 'Link ou código';

  @override
  String get friendsAdd => 'Adicionar';

  @override
  String get friendsQrLabel => 'Código QR do seu convite';

  @override
  String friendsAddedToGroup(String name) {
    return '$name recebe o grupo na próxima vez que vocês dois estiverem online.';
  }

  @override
  String get directToday => 'Hoje';

  @override
  String get directYesterday => 'Ontem';

  @override
  String directTimerOff(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Você',
      'other': '$name',
    });
    return '$_temp0 desativou as mensagens temporárias';
  }

  @override
  String directTimerFirst(String time) {
    return 'Mensagens temporárias ativadas: $time';
  }

  @override
  String directTimerOn(String name, String mine, String time) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Você',
      'other': '$name',
    });
    return '$_temp0 ativou as mensagens temporárias: $time';
  }

  @override
  String directPinnedLine(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Você',
      'other': '$name',
    });
    return '$_temp0 fixou uma mensagem';
  }

  @override
  String get directCallFewSeconds => 'alguns segundos';

  @override
  String directCallMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minutos',
      one: '1 minuto',
      zero: '$n minutos',
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
      zero: '$n horas',
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
      zero: '$hours horas',
    );
    String _temp1 = intl.Intl.pluralLogic(
      minutes,
      locale: localeName,
      other: '$minutes minutos',
      one: '1 minuto',
      zero: '$minutes minutos',
    );
    return '$_temp0 e $_temp1';
  }

  @override
  String directCallLine(String name, String mine, String length) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Você',
      'other': '$name',
    });
    return '$_temp0 iniciou uma chamada que durou $length';
  }

  @override
  String get directBlockedOffline =>
      'Você está aparecendo offline. Fique online para escrever.';

  @override
  String directBlockedFriendOffline(String name) {
    return '$name está offline. Vocês podem escrever quando os dois estiverem online.';
  }

  @override
  String get directOnline => 'Online';

  @override
  String get directAway => 'Ausente';

  @override
  String get directOffline => 'Offline';

  @override
  String directOfflineSeen(String when) {
    return 'Offline · visto $when';
  }

  @override
  String get directHeading => 'MENSAGENS DIRETAS';

  @override
  String directPreviewMine(String text) {
    return 'Você: $text';
  }

  @override
  String directOpenChat(String name) {
    return 'Abrir a conversa com $name';
  }

  @override
  String directShowCard(String name) {
    return 'Mostrar o cartão de $name';
  }

  @override
  String get directNoLongerFriend => 'Vocês não são mais amigos';

  @override
  String directCallOffline(String name) {
    return '$name está offline: as chamadas só tocam enquanto o amigo está online';
  }

  @override
  String directHangUpFirst(String name) {
    return 'Desligue primeiro para ligar para $name';
  }

  @override
  String directPinnedCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n mensagens fixadas',
      one: '1 mensagem fixada',
      zero: '$n mensagens fixadas',
    );
    return '$_temp0';
  }

  @override
  String get directPinnedTitle => 'Mensagens fixadas';

  @override
  String directPinnedNote(int max) {
    return 'Fixadas para vocês dois, e não somem. Até $max.';
  }

  @override
  String get directNothingPinned => 'Nada fixado ainda';

  @override
  String get directNothingPinnedNote =>
      'Passe o mouse sobre uma mensagem e clique no alfinete para mantê-la aqui.';

  @override
  String directDayTime(String day, String time) {
    return '$day, $time';
  }

  @override
  String directFilesCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n arquivos',
    );
    return '$_temp0';
  }

  @override
  String directShowMessageFrom(String name) {
    return 'Mostrar a mensagem de $name';
  }

  @override
  String get directUnpin => 'Desafixar';

  @override
  String get directPin => 'Fixar';

  @override
  String get directDeleteMessage => 'Apagar para mim';

  @override
  String get directDeleteMessageTitle => 'Apagar esta mensagem?';

  @override
  String directDeleteMessageNote(String name) {
    return 'Ela é apagada deste computador, com as suas imagens e arquivos. $name mantém a própria cópia.';
  }

  @override
  String get directDeleteMessageAction => 'Apagar';

  @override
  String get directDeleteForBoth => 'Apagar para os dois';

  @override
  String directDeleteOwnNote(String name) {
    return 'Apagar para os dois também a tira da conversa de $name, assim que o Fogata dela a receber. Apagar para mim a deixa lá.';
  }

  @override
  String directTimerLabel(String time) {
    return 'Mensagens temporárias: $time';
  }

  @override
  String get directTimerTitle => 'Mensagens temporárias';

  @override
  String get directCloseMenu => 'Fechar o menu';

  @override
  String get directTimerNote =>
      'As mensagens novas somem nos dois dispositivos, com as imagens e os arquivos delas.';

  @override
  String get directEmptyNote =>
      'O que você escreve fica guardado só nos computadores de vocês dois.';

  @override
  String directPinLineLabel(String line) {
    return '$line: mostrar';
  }

  @override
  String get directShowMessage => 'Mostrar a mensagem';

  @override
  String get directPinnedStays => 'Fixada: fica até ser desafixada';

  @override
  String get directPinned => 'Fixada';

  @override
  String directPinsFull(int max) {
    return 'Dá para fixar até $max mensagens: desafixe uma primeiro';
  }

  @override
  String get directDelivered => 'Entregue';

  @override
  String get directNotDelivered => 'Não entregue';

  @override
  String get directSending => 'Enviando…';

  @override
  String directWaiting(String name) {
    return 'Esperando $name ficar online · será enviada quando vocês dois estiverem';
  }

  @override
  String get directAvailableWhenYouOnline =>
      'disponível quando você ficar online';

  @override
  String directAvailableWhenOnline(String name) {
    return 'disponível quando $name estiver online';
  }

  @override
  String get directDownloadedByItself => 'baixada automaticamente';

  @override
  String directOpenFile(String name) {
    return 'Abrir $name';
  }

  @override
  String directPlayFile(String name) {
    return 'Reproduzir $name';
  }

  @override
  String directDownloadSize(String size) {
    return 'Baixar · $size';
  }

  @override
  String directVideoPlaysWhenHere(String name, String length) {
    return '$name · $length · é reproduzido quando chegar';
  }

  @override
  String directVideoNotHere(String name, String length, String when) {
    return '$name · $length · $when';
  }

  @override
  String directGettingIt(int percent) {
    return 'Recebendo · $percent%';
  }

  @override
  String directFileNotHere(String size, String when) {
    return '$size · $when';
  }

  @override
  String get directSaveToDownloads => 'Salvar em Downloads';

  @override
  String get directRemovedForSpace => 'Removido daqui para liberar espaço';

  @override
  String get directAskAgain => 'Pedir de novo';

  @override
  String get directClosePicture => 'Fechar a imagem';

  @override
  String get directCloseVideo => 'Fechar o vídeo';

  @override
  String get directCannotPlay =>
      'Este computador não consegue reproduzir este vídeo.';

  @override
  String get directPause => 'Pausar';

  @override
  String get directPlay => 'Reproduzir';

  @override
  String directRemoveFilesTitle(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Remover os $n arquivos?',
      one: 'Remover o arquivo?',
      zero: 'Remover os $n arquivos?',
    );
    return '$_temp0';
  }

  @override
  String get directRemoveFilesText =>
      'Eles saem desta mensagem. Os arquivos no seu computador continuam como estão.';

  @override
  String get directDraftKept =>
      'O que você estava escrevendo fica aqui até você poder enviar.';

  @override
  String directMessagePlaceholder(String name) {
    return 'Mensagem para $name';
  }

  @override
  String get directAddMessageOptional => 'Adicionar uma mensagem (opcional)';

  @override
  String get directAutoDownloadNote =>
      'Imagens de até 10 MB são baixadas automaticamente; vídeos e arquivos, quando você os abre.';

  @override
  String get directVoiceRecord => 'Gravar uma mensagem de voz';

  @override
  String get directVoiceInCall => 'Você não pode gravar durante uma chamada';

  @override
  String directVoiceWaitingElsewhere(String name) {
    return 'Primeiro envie ou exclua a mensagem de voz da sua conversa com $name.';
  }

  @override
  String get directVoiceDelete => 'Excluir';

  @override
  String get directVoiceTooShort =>
      'Curto demais para enviar: uma mensagem de voz precisa de pelo menos meio segundo.';

  @override
  String directVoiceLeft(String time) {
    return 'Faltam $time';
  }

  @override
  String directVoiceRecordingFrom(String microphone) {
    return 'Gravando de $microphone';
  }

  @override
  String get directVoiceRecordingDefault =>
      'O microfone escolhido não está disponível: gravando com o padrão';

  @override
  String directVoiceKeys(String esc, String enter) {
    return '$esc exclui, $enter envia';
  }

  @override
  String directVoiceKeySend(String enter) {
    return '$enter envia';
  }

  @override
  String directVoicePreview(String length) {
    return 'Mensagem de voz · $length';
  }

  @override
  String get directVoiceMessage => 'Mensagem de voz';

  @override
  String get directVoiceNoMic => 'O Fogata não consegue usar o microfone.';

  @override
  String get directVoiceMicStopped =>
      'O microfone parou. Você ainda pode enviar o que foi gravado.';

  @override
  String directVoiceNoMicCheck(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Verifique se há um conectado, escolhido em Voz e vídeo e permitido em Ajustes do Sistema › Privacidade e Segurança.',
      'other': 'Verifique se há um conectado e escolhido em Voz e vídeo.',
    });
    return '$_temp0';
  }

  @override
  String get directVoiceSpeed => 'Velocidade de reprodução';

  @override
  String get directVoiceNotPlayed => 'Ainda não ouvida';

  @override
  String directRemoveFile(String name) {
    return 'Remover $name';
  }

  @override
  String get directAltered =>
      'O histórico desta conversa neste computador mudou desde a última vez que o Fogata o salvou. Algumas mensagens podem estar faltando.';

  @override
  String directEnded(String name) {
    return 'Você e $name não são mais amigos. Esta conversa só pode ser lida.';
  }

  @override
  String get directDeleteChat => 'Excluir conversa';

  @override
  String directDeleteTitle(String name) {
    return 'Excluir a conversa com $name?';
  }

  @override
  String directDeleteText(String name) {
    return 'As mensagens, imagens, vídeos e arquivos dela saem deste computador. $name continua com a própria cópia.';
  }

  @override
  String get directDelete => 'Excluir';

  @override
  String get directUsageMessages => 'Mensagens';

  @override
  String get directUsagePictures => 'Imagens';

  @override
  String get directUsageVideos => 'Vídeos';

  @override
  String get directUsageFiles => 'Arquivos';

  @override
  String get directUsageHeading => 'ESTA CONVERSA NESTE DISPOSITIVO';

  @override
  String get directFreeUpSpace => 'Liberar espaço…';

  @override
  String get directAddFilesTooltip => 'Adicionar imagens, vídeos ou arquivos';

  @override
  String get directAddFiles => 'Adicionar arquivos';

  @override
  String get accountMoveLessThanMinuteLeft => 'falta menos de um minuto';

  @override
  String get accountMoveAboutMinuteLeft => 'falta cerca de um minuto';

  @override
  String accountMoveAboutMinutesLeft(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'faltam cerca de $n minutos',
      one: 'falta cerca de um minuto',
      zero: 'faltam cerca de $n minutos',
    );
    return '$_temp0';
  }

  @override
  String accountMoveNoRoom(String need, String free) {
    return 'Este computador precisa de $need livres para a conta e tem $free.';
  }

  @override
  String accountMoveTooLarge(String size) {
    return 'O outro computador ofereceu $size, muito mais do que uma conta ocupa.';
  }

  @override
  String get accountMoveBusy =>
      'Este computador está concluindo outra transferência: conclua ou descarte essa primeiro.';

  @override
  String get accountMoveNothingToFinish =>
      'Este computador não tem nenhuma transferência daquele computador para concluir.';

  @override
  String get accountMoveNetworkStopped => 'A rede parou.';

  @override
  String get accountMoveConnectionLost => 'A conexão caiu.';

  @override
  String get accountMovePeerNoRoom =>
      'O novo computador não tem espaço livre suficiente.';

  @override
  String get accountMovePeerTooLarge =>
      'A conta é grande demais para ser transferida.';

  @override
  String get accountMovePeerBusy =>
      'O novo computador está concluindo outra transferência.';

  @override
  String get accountMovePeerCouldNotKeep =>
      'O novo computador não conseguiu guardar os arquivos.';

  @override
  String get accountMovePeerCancelled =>
      'A transferência foi cancelada no outro computador.';

  @override
  String get accountMovePeerCouldNotFinish =>
      'O novo computador ainda não conseguiu concluir a transferência.';

  @override
  String get accountMovePeerStopped =>
      'A transferência foi interrompida no outro computador.';

  @override
  String get accountMovePeerNothingToFinish =>
      'Aquele computador não tem nenhuma transferência deste para concluir.';

  @override
  String get accountMovePeerNotProven =>
      'Aquele computador não provou que tem esta transferência.';

  @override
  String get accountMoveWrongCode =>
      'O outro computador não aceitou o código, então nada foi enviado.';

  @override
  String get accountMoveExpired => 'O código expirou antes de ser usado.';

  @override
  String get accountMoveTooManyTries =>
      'Vários códigos errados foram tentados; o código parou de funcionar.';

  @override
  String get accountMoveNoContact =>
      'Os dois computadores não conseguiram se conectar.';

  @override
  String get accountMoveUnconfirmed =>
      'O final da transferência não foi confirmado.';

  @override
  String get accountMoveCancelled => 'Cancelada.';

  @override
  String get accountMoveInterrupted =>
      'A transferência parou antes do fim; nada mudou.';

  @override
  String accountMoveInterruptedWhy(String why) {
    return 'A transferência parou antes do fim; nada mudou. $why';
  }

  @override
  String get accountMoveDeclined =>
      'O outro computador interrompeu a transferência.';

  @override
  String accountMoveDeclinedWhy(String why) {
    return 'O outro computador interrompeu a transferência. $why';
  }

  @override
  String get accountMoveLater =>
      'A conta chegou e será instalada na próxima vez que o Fogata iniciar aqui.';

  @override
  String accountMoveLaterWhy(String why) {
    return 'A conta chegou e será instalada na próxima vez que o Fogata iniciar aqui. $why';
  }

  @override
  String get accountMoveDidNotFinish => 'Não foi concluída.';

  @override
  String get accountMoveAnotherComputer => 'outro computador';

  @override
  String get accountMoveTheOtherComputer => 'o outro computador';

  @override
  String get moveTooLateBeingPlaced =>
      'Tarde demais para parar: a conta está sendo instalada.';

  @override
  String get moveTooLateDiscard =>
      'Tarde demais: a conta está sendo instalada.';

  @override
  String get moveTooLateOtherHasIt =>
      'Tarde demais para parar: o outro computador já tem a conta.';

  @override
  String moveFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n amigos',
      one: '1 amigo',
      zero: '$n amigos',
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
      zero: '$n grupos',
    );
    return '$_temp0';
  }

  @override
  String moveGroupsWithKeys(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n grupos, com as chaves',
      one: '1 grupo, com as chaves',
      zero: '$n grupos, com as chaves',
    );
    return '$_temp0';
  }

  @override
  String moveChats(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n conversas',
      one: '1 conversa',
      zero: '$n conversas',
    );
    return '$_temp0';
  }

  @override
  String moveInAll(String size) {
    return '$size no total';
  }

  @override
  String get moveYourSettings => 'Suas configurações';

  @override
  String get moveNotDevices => 'sem o microfone e a câmera escolhidos';

  @override
  String get moveFinishingBoth => 'Concluindo nos dois computadores…';

  @override
  String movePercent(int percent) {
    return '$percent%';
  }

  @override
  String movePercentLeft(int percent, String left) {
    return '$percent% · $left';
  }

  @override
  String get moveStepIdentity => 'Identidade e perfil';

  @override
  String get moveStepFriends => 'Amigos';

  @override
  String get moveStepGroups => 'Grupos e suas chaves';

  @override
  String get moveStepChats => 'Conversas, imagens e arquivos';

  @override
  String moveBytesOf(String done, String total) {
    return '$done de $total';
  }

  @override
  String get moveStepFinishing => 'Concluindo nos dois computadores';

  @override
  String moveBringingNamed(String name) {
    return 'Trazendo a conta de $name';
  }

  @override
  String get moveBringingYours => 'Trazendo sua conta';

  @override
  String get moveKeepBothOn =>
      'Mantenha os dois computadores ligados e o Fogata aberto.';

  @override
  String get moveKeepOpenNote =>
      'Mantenha o Fogata aberto nos dois computadores. É mais rápido na mesma rede; senão, eles se conectam pela internet, diretamente.';

  @override
  String moveFinishBringingNamed(String name) {
    return 'Terminar de trazer a conta de $name';
  }

  @override
  String get moveFinishBringingYours => 'Terminar de trazer sua conta';

  @override
  String get moveEverythingCame =>
      'Tudo chegou. Digite este código no outro computador para concluir.';

  @override
  String moveEverythingCameFrom(String computer) {
    return 'Tudo chegou. Digite este código em “$computer” para concluir.';
  }

  @override
  String get moveBringHereTitle => 'Trazer sua conta para cá';

  @override
  String get moveBringHereSub =>
      'Digite este código no computador que tem sua conta.';

  @override
  String get moveHideCode => 'Ocultar o código';

  @override
  String get moveShowCode => 'Mostrar o código';

  @override
  String moveExpiresIn(String time) {
    return 'expira em $time';
  }

  @override
  String moveStepFogataShows(String what) {
    return 'Naquele computador, o Fogata mostra $what.';
  }

  @override
  String get moveWaitingToConfirm => 'Esperando confirmação';

  @override
  String get moveStepOpenThere => 'Naquele computador, abra o Fogata.';

  @override
  String moveStepTypeUnder(String where) {
    return 'Digite o código em $where.';
  }

  @override
  String get moveFinishTheMove => 'Concluir a transferência';

  @override
  String moveStepGoTo(String where) {
    return 'Vá para $where.';
  }

  @override
  String get moveSettingsPath => 'Configurações → Transferir conta';

  @override
  String get moveToAnotherComputer => 'Transferir para outro computador';

  @override
  String get moveWaitingForOther => 'Esperando seu outro computador…';

  @override
  String moveWrongCodesTyped(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Códigos errados foram digitados $n vezes em outros computadores.',
      one: 'Um código errado foi digitado em outro computador.',
      zero: 'Códigos errados foram digitados $n vezes em outros computadores.',
    );
    return '$_temp0';
  }

  @override
  String get moveThrowAwayInstead => 'Descartar em vez disso…';

  @override
  String get moveFoundOther => 'Seu outro computador foi encontrado';

  @override
  String get moveFoundSameNetwork => 'Mesma rede · criptografado entre os dois';

  @override
  String get moveFoundInternet => 'Pela internet · criptografado entre os dois';

  @override
  String get moveConfirmThere => 'Confirme no outro computador para começar';

  @override
  String get moveSameAccountNote =>
      'Esta é a conta que já está neste computador. Nada se perde ao trazê-la para cá.';

  @override
  String get moveTakeYes => 'Trazer para cá';

  @override
  String get moveWelcomeBack => 'Boas-vindas de volta';

  @override
  String moveWelcomeBackNamed(String name) {
    return 'Boas-vindas de volta, $name';
  }

  @override
  String get moveDoneSub =>
      'Sua conta agora está neste computador. O outro computador parou de usá-la.';

  @override
  String get moveOpenFogata => 'Abrir o Fogata';

  @override
  String get moveCodeExpiredLabel => 'expirado';

  @override
  String get moveCodeStoppedLabel => 'parou de funcionar';

  @override
  String get moveCodeExpiredNote =>
      'Este código expirou antes de o seu outro computador usá-lo. Os códigos duram 10 minutos.';

  @override
  String get moveCodeStoppedNote =>
      'Códigos errados foram digitados várias vezes em outros computadores, então este código parou de funcionar.';

  @override
  String get moveNewCode => 'Novo código';

  @override
  String get moveAlmostThere => 'Quase lá';

  @override
  String get moveLaterSub =>
      'Sua conta chegou inteira, mas ainda não pôde ser instalada. Isso acontece na próxima vez que o Fogata iniciar aqui.';

  @override
  String get moveStartAgain => 'Reiniciar o Fogata';

  @override
  String get moveUnconfirmedSub =>
      'Tudo chegou, mas o outro computador não confirmou o fim. Mostre um código e digite-o lá para concluir a transferência.';

  @override
  String get moveStoppedThereTitle =>
      'A transferência foi interrompida no outro computador';

  @override
  String get moveStoppedThereSub =>
      'Nada mudou: sua conta continua no outro computador, e nada dela ficou aqui.';

  @override
  String get moveCouldNotStartTitle => 'A transferência não pôde começar';

  @override
  String get moveSomethingWrongHere => 'Algo deu errado neste computador.';

  @override
  String get moveHalfwayTitle => 'A transferência parou no meio';

  @override
  String get moveHalfwaySub =>
      'A conexão com seu outro computador caiu. Nada mudou: sua conta continua no outro computador, e nada dela ficou aqui.';

  @override
  String get moveShowCodeToFinish => 'Mostrar um código para concluir';

  @override
  String get moveTryNewCode => 'Tentar de novo com um novo código';

  @override
  String moveBackToNamed(String name) {
    return 'Voltar para $name';
  }

  @override
  String get moveBackToThisProfile => 'Voltar para este perfil';

  @override
  String get moveNewProfileInstead => 'Criar um perfil novo em vez disso';

  @override
  String get moveDiscardTitle => 'Descartar a conta que chegou?';

  @override
  String get moveDiscardText =>
      'Faça isso só se o outro computador continuar usando a conta. Aquele computador espera para confirmar até a pessoa escolher continuar usando a conta lá.';

  @override
  String get moveKeepIt => 'Manter';

  @override
  String get moveThrowAway => 'Descartar';

  @override
  String get moveBringFromAnother => 'Trazer sua conta de outro computador';

  @override
  String get moveAlreadyOnFogata => 'Já usa o Fogata?';

  @override
  String get moveCodeTriedBefore =>
      'Esse código já falhou uma vez. Gere um novo código no novo computador.';

  @override
  String moveMovingTo(String computer) {
    return 'Transferindo sua conta para “$computer”';
  }

  @override
  String get moveKeepBothOnOffline =>
      'Mantenha os dois computadores ligados e o Fogata aberto. Enquanto isso, o Fogata fica offline aqui.';

  @override
  String get moveCodeFieldLabel => 'O código mostrado no novo computador';

  @override
  String get moveToAnotherSub =>
      'Sua identidade, amigos, grupos, conversas, arquivos e configurações vão para o outro computador, direto deste e com criptografia. Depois, este computador para de usar sua conta, então nunca existe um segundo você.';

  @override
  String moveStepChoose(String choice) {
    return 'No novo computador, abra o Fogata e escolha $choice.';
  }

  @override
  String get moveStepTypeItShows => 'Digite o código que ele mostra:';

  @override
  String get moveWaitingForAnswer => 'Esperando sua resposta…';

  @override
  String get moveLookingForCode =>
      'Procurando o computador que mostra este código…';

  @override
  String get moveOfflineWhileMoving =>
      'O Fogata fica offline neste computador durante a transferência.';

  @override
  String get moveWrongCodeNote =>
      'O novo computador não aceitou esse código, então nada foi enviado. Confira e digite de novo. Se ainda falhar, gere um novo código lá.';

  @override
  String get moveCantReachNew => 'Não foi possível alcançar o novo computador.';

  @override
  String get moveCantReachNewNote =>
      'Os dois se procuraram por 3 minutos. Coloque-os na mesma rede (o mesmo Wi-Fi ou um cabo) e tente de novo. Sua conta continua aqui e o Fogata está online de novo.';

  @override
  String get moveDeclinedNote =>
      'A transferência foi interrompida no novo computador. Nada foi transferido: sua conta continua aqui e o Fogata está online de novo.';

  @override
  String get moveCouldNotStart => 'A transferência não pôde começar.';

  @override
  String get moveHalfwayNote =>
      'A transferência parou no meio: a conexão com o novo computador caiu. Nada mudou: sua conta continua aqui e o Fogata está online de novo.';

  @override
  String get moveThisProfileWords => 'este perfil';

  @override
  String get moveBringCardTitle => 'Trazer uma conta para cá';

  @override
  String get moveBringCardSub =>
      'Mostre um código neste computador e digite-o no computador que tem a conta.';

  @override
  String get moveBringCardButton => 'Trazer uma conta para cá…';

  @override
  String moveReplacesHere(String name) {
    return 'Substitui $name neste computador.';
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
      zero: '$friends amigos',
    );
    String _temp1 = intl.Intl.pluralLogic(
      groups,
      locale: localeName,
      other: '$groups grupos',
      one: '1 grupo',
      zero: '$groups grupos',
    );
    String _temp2 = intl.Intl.pluralLogic(
      chats,
      locale: localeName,
      other: '$chats conversas',
      one: '1 conversa',
      zero: '$chats conversas',
    );
    return '$bold, o perfil atual deste computador, será substituído, e $_temp0, $_temp1 e $_temp2 dele serão excluídos daqui. Para manter $name, transfira esse perfil para outro computador primeiro.';
  }

  @override
  String moveReplaceAndShow(String name) {
    return 'Substituir $name e mostrar um código';
  }

  @override
  String moveConfirmTitle(String computer) {
    return 'Transferir sua conta para “$computer”?';
  }

  @override
  String get moveConfirmSameNetwork =>
      'Encontrado na mesma rede. Tudo vai criptografado, direto para ele.';

  @override
  String get moveConfirmInternet =>
      'Encontrado pela internet. Tudo vai criptografado, direto para ele.';

  @override
  String moveConfirmReplaces(String name) {
    return 'A conta substitui o perfil $name lá.';
  }

  @override
  String get moveConfirmNote =>
      'Quando terminar, este computador para de usar sua conta e não pode usá-la de novo. Depois, você pode excluir o que sobrar aqui.';

  @override
  String get moveConfirmYes => 'Transferir minha conta';

  @override
  String get moveKeepTitle => 'Continuar usando sua conta aqui?';

  @override
  String get moveKeepText =>
      'Só se o novo computador não tiver a conta: abra o Fogata lá primeiro. Se os dois a usassem, seus amigos e grupos veriam duas cópias de você, e as mensagens chegariam a só uma.';

  @override
  String get moveKeepYes => 'Continuar usando aqui';

  @override
  String moveWaitingFor(String computer) {
    return 'Esperando “$computer” confirmar';
  }

  @override
  String get moveWaitingText =>
      'Sua conta foi enviada, mas a conexão caiu antes de o novo computador dizer que está pronto. Enquanto isso, este computador não vai usar a conta.';

  @override
  String get moveFinishText =>
      'Abra o Fogata no novo computador: ele mostra um código. Digite-o aqui.';

  @override
  String get moveFinish => 'Concluir';

  @override
  String get moveFinishWrongCode =>
      'O novo computador não aceitou esse código. Confira e digite de novo.';

  @override
  String get moveFinishNoContact =>
      'Não foi possível alcançar o novo computador. Coloque os dois na mesma rede e tente de novo.';

  @override
  String get moveKeepThisOne =>
      'O novo computador não tem a conta: continuar usando este…';

  @override
  String moveMovedYours(String computer) {
    return 'Sua conta foi transferida para “$computer”';
  }

  @override
  String moveMovedNamed(String name, String computer) {
    return 'A conta de $name foi transferida para “$computer”';
  }

  @override
  String get moveMovedNoteYou =>
      'Este computador não a usa mais: amigos e grupos falam com você no outro.';

  @override
  String moveMovedNoteNamed(String name) {
    return 'Este computador não a usa mais: amigos e grupos falam com $name no outro.';
  }

  @override
  String moveMovedTodayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'Hoje à $time.',
      'other': 'Hoje às $time.',
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
  String get moveLeftHere => 'O que sobrou aqui';

  @override
  String get moveLeftHereText =>
      'Conversas, imagens e arquivos de antes da transferência, criptografados. O outro computador tem a própria cópia.';

  @override
  String get moveDeleteData => 'Excluir os dados do Fogata neste computador';

  @override
  String get moveAfterDeleting =>
      'Depois de excluir, o Fogata aqui começa do zero.';

  @override
  String get moveCodeNotACode =>
      'Um código tem dez letras e números, como XXXXX-XXXXX.';

  @override
  String get moveCodeMistyped =>
      'Esse código não está certo: confira as letras e os números.';

  @override
  String get welcomeHeadline => 'Converse como se\nestivesse na mesma sala.';

  @override
  String get welcomeIntro =>
      'Voz, vídeo e chat com amigos, família ou sua equipe, direto do seu computador para o deles.';

  @override
  String get welcomeNoAccount => 'Sem conta';

  @override
  String get welcomeEncrypted => 'Criptografado';

  @override
  String get welcomeEphemeral => 'Salas efêmeras';

  @override
  String get welcomeCharacter =>
      'O personagem do Fogata, uma pequena chama com fones de ouvido fazendo o sinal de paz';

  @override
  String get welcomeImageUnreadable => 'Não foi possível ler essa imagem.';

  @override
  String get welcomeKeychainTitle =>
      'Seus dados ainda não podem ser criptografados';

  @override
  String welcomeKeychainText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' No Linux, o chaveiro é um Secret Service em execução, como o KWallet ou o GNOME Keyring: inicie ou desbloqueie um e tente de novo.',
      'other': ' Desbloqueie ou repare o chaveiro e tente de novo.',
    });
    return 'O Fogata criptografa sua identidade, seus amigos e suas mensagens diretas com uma chave que guarda no chaveiro do sistema, e o chaveiro não a guardou.$_temp0';
  }

  @override
  String get welcomeKeychainRetry => 'Tentar de novo';

  @override
  String get welcomeKeychainPlain => 'Continuar sem criptografia';

  @override
  String get welcomeKeychainPlainNote =>
      'Sem criptografia, qualquer pessoa que copie os arquivos deste computador pode lê-los. As configurações continuarão avisando, e o Fogata criptografa tudo na primeira vez que abrir com um chaveiro.';

  @override
  String get welcomeWriteName => 'Escreva seu nome.';

  @override
  String get welcomeCardTitle => 'Como você quer aparecer para o seu pessoal?';

  @override
  String get welcomeCardSub => 'Um nome e uma imagem. Mude quando quiser.';

  @override
  String get welcomeNoServer =>
      'Nada vai para um servidor do Fogata: não existe nenhum. A rede pública IPFS é usada só como lista telefônica para encontrar seus amigos.';

  @override
  String get welcomeYourName => 'Seu nome';

  @override
  String welcomePictureLabel(String name) {
    return 'Imagem: $name';
  }

  @override
  String get welcomeUploadPicture => 'Enviar uma imagem';

  @override
  String get welcomeNameLabel => 'Nome';

  @override
  String get welcomeCreate => 'Criar';

  @override
  String get welcomeReady => 'Pronto';

  @override
  String get welcomeCreateLabel => 'Criar identidade';

  @override
  String welcomeKeyLabel(String key) {
    return 'Chave criada neste computador: $key';
  }

  @override
  String get welcomeNewKey => 'Criar outra chave';

  @override
  String get stoppedTitle => 'O Fogata parou de funcionar.';

  @override
  String get stoppedStartingAgain => 'Reiniciando…';

  @override
  String get stoppedRestartToConnect => 'Reinicie para conectar de novo.';

  @override
  String get stoppedRestartNow => 'Reiniciar agora';

  @override
  String get settingsSectionProfile => 'Meu perfil';

  @override
  String get settingsSectionMove => 'Transferir conta';

  @override
  String get settingsSectionAppearance => 'Aparência';

  @override
  String get settingsSectionLanguageTime => 'Idioma e hora';

  @override
  String get settingsSectionVoice => 'Voz e vídeo';

  @override
  String get settingsSectionSounds => 'Sons';

  @override
  String get settingsSectionBehaviour => 'Comportamento';

  @override
  String get settingsSectionNetwork => 'Rede';

  @override
  String get settingsSectionStorage => 'Armazenamento e downloads';

  @override
  String get settingsHeadingUser => 'Usuário';

  @override
  String get settingsHeadingApp => 'App';

  @override
  String get settingsTitle => 'Configurações';

  @override
  String get settingsAll => 'Todas as configurações';

  @override
  String get settingsCloseTooltip => 'Fechar configurações';

  @override
  String get settingsUnsavedSaveTitle => 'Salvar suas alterações?';

  @override
  String get settingsUnsavedDiscardTitle => 'Descartar suas alterações?';

  @override
  String get settingsUnsavedSaveText =>
      'Seu perfil tem alterações que ainda não foram salvas.';

  @override
  String get settingsUnsavedDiscardText =>
      'Seu perfil tem alterações que não podem ser salvas como estão.';

  @override
  String get settingsProfileSaved =>
      'Perfil salvo. Amigos e grupos online já veem; os outros, quando se conectarem.';

  @override
  String get settingsBioSaved =>
      'Bio salva. Amigos online já veem; os outros, quando se conectarem.';

  @override
  String settingsSwitchOn(String what) {
    return '$what ativado';
  }

  @override
  String settingsSwitchOff(String what) {
    return '$what desativado';
  }

  @override
  String get settingsOn => 'Ativado';

  @override
  String get settingsOff => 'Desativado';

  @override
  String get settingsIdentity => 'Sua identidade';

  @override
  String get settingsIdentityNote =>
      'Seu nome e sua imagem chegam assinados aos seus amigos e grupos; sua bio, aos seus amigos.';

  @override
  String get settingsPicture => 'Imagem';

  @override
  String get settingsUploadPicture => 'Enviar uma imagem ou um GIF';

  @override
  String get settingsName => 'Nome';

  @override
  String get settingsBio => 'Bio';

  @override
  String get settingsBioHint => 'Algumas palavras sobre você';

  @override
  String get settingsNoBio => 'Seu cartão não mostra bio.';

  @override
  String settingsBioWhere(int lines) {
    return 'Seus amigos veem no seu cartão, em até $lines linhas.';
  }

  @override
  String get settingsBannerTitle => 'Fundo do painel';

  @override
  String get settingsBannerNote =>
      'O topo do seu cartão, para você e seus amigos.';

  @override
  String get settingsBannerAnother => 'Outra imagem';

  @override
  String get settingsBannerUpload => 'Usar uma imagem deste computador';

  @override
  String get settingsFingerprint => 'Impressão digital deste computador';

  @override
  String get settingsTheme => 'Tema';

  @override
  String get settingsThemeNote =>
      'As cores da janela. Sistema acompanha o computador entre Escuro e Claro.';

  @override
  String get settingsMicSpeaker => 'Microfone e alto-falante';

  @override
  String get settingsMicSpeakerNote =>
      'Com o que as chamadas ouvem você e por onde tocam. Também dá para trocar no meio de uma chamada.';

  @override
  String get settingsCamera => 'Câmera';

  @override
  String get settingsCameraNote =>
      'O que as chamadas mostram de você quando sua câmera está ligada. Teste aqui antes de uma chamada.';

  @override
  String get settingsSoundEffects => 'Efeitos sonoros';

  @override
  String get settingsSoundEffectsNote =>
      'Entrar e sair de uma sala, pessoas chegando e saindo, mensagens do chat, o microfone e o compartilhamento.';

  @override
  String get settingsSounds => 'Sons';

  @override
  String settingsChangeFailed(String reason) {
    return 'O Fogata não conseguiu mudar isso: $reason';
  }

  @override
  String get settingsAutostartApprove =>
      'Permita o Fogata em Ajustes do Sistema › Geral › Itens de Início.';

  @override
  String get settingsAutostartOldMac => 'Requer o macOS 13 ou posterior.';

  @override
  String get settingsAutostartUnsupported =>
      'Esta cópia do Fogata não consegue abrir sozinha.';

  @override
  String get settingsAutostartUnread =>
      'O Fogata não conseguiu ler esta configuração do seu sistema.';

  @override
  String get settingsAutostart => 'Abrir ao iniciar';

  @override
  String get settingsAutostartNote =>
      'Abrir o Fogata quando seu computador iniciar, depois que você entrar na sessão.';

  @override
  String get settingsKeepRunning => 'Continuar em execução';

  @override
  String get settingsKeepRunningNote =>
      'Quando a janela é fechada, o Fogata fica na bandeja do sistema, para seus amigos ainda poderem ligar para você.';

  @override
  String get settingsNoTray =>
      'Esta área de trabalho não mostra ícones na bandeja, então fechar a janela encerra o Fogata.';

  @override
  String get settingsHelping => 'Ajudar membros';

  @override
  String get settingsHelpingNote =>
      'Como este computador ajuda os membros dos seus grupos.';

  @override
  String get settingsHelpRelay => 'Ajudar a conectar e retransmitir';

  @override
  String get settingsHelpConnect => 'Só ajudar a conectar';

  @override
  String get settingsHelpRelayNote =>
      'Quando dois membros não conseguem se conectar, este computador passa o tráfego criptografado deles (sem vê-lo), até 2 GB por chamada. Isso usa o seu upload.';

  @override
  String get settingsHelpConnectNote =>
      'Este computador ajuda dois membros a se encontrarem, mas não passa o tráfego deles. Se eles não conseguirem se conectar, não vão se ouvir.';

  @override
  String get settingsRestartTray =>
      'A mudança vale depois que você sair do Fogata pelo ícone da bandeja e abri-lo de novo.';

  @override
  String get settingsRestart =>
      'A mudança vale depois que você sair do Fogata e abri-lo de novo.';

  @override
  String get settingsNetworkCheck => 'Verificação de rede';

  @override
  String get settingsNetworkCheckNote =>
      'Como este computador vê a rede e o que isso significa para alcançar seus amigos. Nada sai daqui: só olha o que a conexão já sabe.';

  @override
  String get settingsCheckNetwork => 'Verificar a rede';

  @override
  String get settingsCheckAgain => 'Verificar de novo';

  @override
  String get settingsDownloads => 'Downloads';

  @override
  String get settingsDownloadsNote =>
      'Onde os arquivos baixados de uma sala são salvos.';

  @override
  String settingsVersionData(String version, String path) {
    return 'Fogata $version · Dados em $path';
  }

  @override
  String get settingsNotEncrypted => 'Sem criptografia neste computador';

  @override
  String settingsNotEncryptedText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' No Linux, o chaveiro é um Secret Service em execução, como KWallet ou GNOME Keyring: inicie ou instale um, e o Fogata criptografa seus dados na próxima vez que abrir.',
      'other': ' O Fogata tenta de novo na próxima vez que abrir.',
    });
    return 'Sua identidade, seus amigos e suas mensagens diretas estão guardados neste computador sem criptografia, porque não foi encontrado nenhum chaveiro do sistema para guardar a chave.$_temp0';
  }

  @override
  String get settingsNotAllEncrypted =>
      'Nem tudo está criptografado neste computador';

  @override
  String get settingsNotAllEncryptedText =>
      'Parte dos dados deste computador (sua identidade, seus amigos e suas mensagens diretas) ainda não está criptografada: o Fogata ainda está criptografando ou não conseguiu terminar, como quando o disco está cheio. Ele continua toda vez que o Fogata abre.';

  @override
  String settingsDamagedSecret(String file) {
    return '$file não abre com a chave deste computador: foi danificado no disco. Enquanto estiver lá, a criptografia não pode terminar, e uma cópia de antes da criptografia continua sem criptografia. Separe-o para terminar: ele é guardado como está. Se for a sua identidade ou os seus amigos, o Fogata recupera essa cópia e a criptografa; o arquivo de um grupo ou de uma conversa se perde neste computador: o grupo terá de deixar você entrar de novo, e as mensagens anteriores da conversa deixarão de aparecer.';
  }

  @override
  String get settingsSetDamagedAside => 'Separar e reiniciar';

  @override
  String get storageTitle => 'Armazenamento';

  @override
  String get storageNote =>
      'Suas mensagens diretas existem só nos seus dispositivos. Os chats das salas somem quando a sala fecha, então só elas crescem.';

  @override
  String get storageDirect => 'Mensagens diretas';

  @override
  String get storageOfNoLimit => 'sem limite';

  @override
  String storageOfLimit(String limit) {
    return 'de $limit';
  }

  @override
  String storageLegendPictures(String size) {
    return 'Imagens $size';
  }

  @override
  String storageLegendVideos(String size) {
    return 'Vídeos $size';
  }

  @override
  String storageLegendFiles(String size) {
    return 'Arquivos $size';
  }

  @override
  String storageLegendMessages(String size) {
    return 'Mensagens $size';
  }

  @override
  String get storageRoom => 'Espaço para mensagens diretas';

  @override
  String get storageNoLimitNote => 'Nada é removido automaticamente.';

  @override
  String storageOverNote(String limit) {
    return 'Acima do limite: as imagens, vídeos e arquivos mais antigos saem, até chegar a $limit. As mensagens sempre ficam.';
  }

  @override
  String get storageLimitNote =>
      'Passado o limite, as imagens, vídeos e arquivos mais antigos saem primeiro. As mensagens sempre ficam; no lugar deles aparece “Removido daqui para liberar espaço”.';

  @override
  String get storageAutoPictures => 'Baixar imagens automaticamente';

  @override
  String get storageAutoPicturesNote => 'Até 10 MB cada, assim que chegam.';

  @override
  String get storageAutoFiles => 'Baixar vídeos e arquivos automaticamente';

  @override
  String get storageAutoFilesNote =>
      'Desativado: eles chegam quando você os abrir, se quem enviou estiver online.';

  @override
  String get storageTimer => 'Mensagens temporárias em novas conversas';

  @override
  String get storageTimerNote =>
      'Cada conversa pode mudar isso; as duas pessoas sempre veem o mesmo tempo.';

  @override
  String get storageTimer24h => '24 h';

  @override
  String get storageByChat => 'Por conversa';

  @override
  String get storageByChatNote =>
      'Limpar remove as imagens, vídeos e arquivos daqui. As mensagens ficam, e a outra pessoa continua com a própria cópia.';

  @override
  String get storageMessagesOnly => 'Só mensagens';

  @override
  String get storageClearFiles => 'Limpar arquivos';

  @override
  String storageClearFilesTooltip(String name) {
    return 'Limpar os arquivos da conversa com $name';
  }

  @override
  String get storageCleared => 'Limpo';

  @override
  String storageConfirmTitle(String limit) {
    return 'Manter $limit de imagens, vídeos e arquivos?';
  }

  @override
  String storageConfirmText(String limit) {
    return 'Os mais antigos saem agora, até chegar a $limit; os das mensagens fixadas, por último. As mensagens sempre ficam, e seus amigos continuam com as cópias deles.';
  }

  @override
  String get storageRemoveOldest => 'Remover os mais antigos';

  @override
  String get themePickerSystem =>
      'Tema do sistema, como o computador estiver configurado';

  @override
  String themePickerTheme(String name) {
    return 'Tema $name';
  }

  @override
  String get themeDark => 'Escuro';

  @override
  String get themeMidnight => 'Meia-noite';

  @override
  String get themeDim => 'Penumbra';

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
  String get themeBannerClay => 'Argila';

  @override
  String get themeBannerRaspberry => 'Framboesa';

  @override
  String get themeBannerMustard => 'Mostarda';

  @override
  String get themeBannerSlate => 'Ardósia';

  @override
  String get themeBannerNight => 'Noite';

  @override
  String widgetsCopyWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Copiar código QR',
      'link': 'Copiar link',
      'code': 'Copiar código',
      'other': 'Copiar',
    });
    return '$_temp0';
  }

  @override
  String widgetsShowWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Mostrar código QR',
      'link': 'Mostrar link',
      'code': 'Mostrar código',
      'other': 'Mostrar',
    });
    return '$_temp0';
  }

  @override
  String get widgetsHide => 'Ocultar';

  @override
  String revealFailed(String path) {
    return 'Não foi possível abrir a pasta. O arquivo está em $path';
  }

  @override
  String get emojiGroupPeople => 'Pessoas';

  @override
  String get emojiGroupNature => 'Natureza';

  @override
  String get emojiGroupFood => 'Comidas e bebidas';

  @override
  String get emojiGroupActivities => 'Atividades';

  @override
  String get emojiGroupTravel => 'Viagens e lugares';

  @override
  String get emojiGroupObjects => 'Objetos';

  @override
  String get emojiGroupSymbols => 'Símbolos';

  @override
  String get emojiGroupFlags => 'Bandeiras';

  @override
  String get emojiPickerLimit => 'A mensagem chegou ao limite.';

  @override
  String get emojiPickerButton => 'Emoji';

  @override
  String get emojiPickerResults => 'Resultados';

  @override
  String get emojiPickerRecent => 'Usados recentemente';

  @override
  String get emojiPickerSearch => 'Buscar emoji (em inglês)';

  @override
  String get emojiPickerSkinTone => 'Tom de pele';

  @override
  String get emojiPickerNone => 'Nenhum emoji encontrado';

  @override
  String get emojiPickerNoFont =>
      'Este computador não tem uma fonte de emoji colorida, então os emojis podem aparecer simples ou vazios. Instalar a Noto Color Emoji resolve.';

  @override
  String get emojiPickerDefaultTone => 'Tom de pele padrão';

  @override
  String emojiPickerTone(int n) {
    return 'Tom de pele $n';
  }

  @override
  String get iconsGroupMountain => 'Montanha';

  @override
  String get iconsGroupStar => 'Estrela';

  @override
  String get iconsGroupGameController => 'Controle de videogame';

  @override
  String get iconsGroupMusic => 'Música';

  @override
  String get iconsGroupBook => 'Livro';

  @override
  String get iconsGroupWaves => 'Ondas';

  @override
  String get iconsGroupCoffee => 'Café';

  @override
  String get iconsGroupHeart => 'Coração';

  @override
  String get iconsGroupFilm => 'Filme';

  @override
  String get newGroupPictureUnreadable => 'Não foi possível ler essa imagem.';

  @override
  String newGroupMade(String name) {
    return 'Grupo “$name” criado. Os membros recebem o grupo quando ficarem online.';
  }

  @override
  String get newGroupTitle => 'Novo grupo';

  @override
  String get newGroupSubtitle => 'O grupo é permanente. As salas dele, não.';

  @override
  String get newGroupName => 'Nome do grupo';

  @override
  String get newGroupNameExample => 'Pedal de domingo';

  @override
  String get newGroupIconOrPicture => 'Ícone ou imagem';

  @override
  String get newGroupChooseFromComputer => 'Escolher do computador';

  @override
  String get newGroupBannerOptional => 'Banner (opcional)';

  @override
  String get newGroupMembers => 'Membros';

  @override
  String newGroupSelected(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n selecionados',
      one: '1 selecionado',
      zero: '$n selecionados',
    );
    return '$_temp0';
  }

  @override
  String get newGroupSearchPlaceholder => 'Buscar seus amigos';

  @override
  String get newGroupSearchLabel => 'Buscar amigos';

  @override
  String get newGroupNoFriends =>
      'Nenhum amigo ainda. Você pode criar o grupo e adicionar amigos depois, ou convidar pessoas com um link.';

  @override
  String get newGroupKeptOnDevices => 'Guardado no dispositivo de cada membro.';

  @override
  String get newGroupCreate => 'Criar grupo';

  @override
  String get groupBannerSettings => 'Configurações do grupo';

  @override
  String get groupBannerPictureUnreadable =>
      'Não foi possível ler essa imagem.';

  @override
  String get groupBannerChoose => 'Escolher um banner';

  @override
  String get groupBannerAnother => 'Outro banner';

  @override
  String get groupBannerNone => 'Sem banner';

  @override
  String get groupBannerNote =>
      'Aparece no topo da coluna do grupo, abaixo do nome. PNG, JPEG, ou um GIF ou WebP animado de até 550 KB.';

  @override
  String get homeTitle => 'Início';

  @override
  String get homeSubtitle =>
      'As salas se acendem quando alguém entra e se apagam quando todos saem.';

  @override
  String get homeAddFriend => 'Adicionar um amigo';

  @override
  String get homeNewGroup => 'Novo grupo';

  @override
  String get homeLive => 'Ao vivo';

  @override
  String get homeNoRoom => 'Sem sala';

  @override
  String get homeNoRoomNote => 'Abra a sala de um grupo para acendê-la';

  @override
  String homeInviteBy(String name, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n membros',
      one: '1 membro',
      zero: '$n membros',
    );
    return '$name adicionou você · $_temp0';
  }

  @override
  String get homeInviteNote =>
      'Até você entrar, este computador não se conecta a eles nem envia nada a eles.';

  @override
  String get homeInviteJoin => 'Entrar';

  @override
  String get homeNoGroups => 'Nenhum grupo ainda';

  @override
  String get homeNoGroupsNote =>
      'Adicione seus amigos e crie um grupo com eles. O grupo é permanente; as salas dele, não.';

  @override
  String homePersonYou(String name) {
    return '$name (você)';
  }

  @override
  String homeMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n membros',
      one: '1 membro',
      zero: '$n membros',
    );
    return '$_temp0';
  }

  @override
  String get homeYouAreIn => 'Você está aqui';

  @override
  String get homeMuteMic => 'Silenciar microfone';

  @override
  String get homeUnmuteMic => 'Ativar microfone';

  @override
  String get homeCameraOff => 'Desligar a câmera';

  @override
  String get homeCameraOn => 'Ligar a câmera';

  @override
  String get homeMembersTooltip => 'Membros';

  @override
  String get homeReturnToRoom => 'Voltar à sala';

  @override
  String get homeJoinRoom => 'Entrar';

  @override
  String homeMembersOnline(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membros',
      one: '1 membro',
      zero: '$members membros',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'ninguém mais online',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get homeNoRoomsYet => 'Nenhuma sala ainda';

  @override
  String homeLastRoom(String when) {
    return 'Última sala $when';
  }

  @override
  String get homeOpenRoom => 'Abrir sala';

  @override
  String get membersGroupGone => 'Este grupo não está mais aqui.';

  @override
  String membersSummary(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membros',
      one: '1 membro',
      zero: '$members membros',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'ninguém mais online',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String membersSummaryLastRoom(int members, int online, String when) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members membros',
      one: '1 membro',
      zero: '$members membros',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'ninguém mais online',
    );
    return '$_temp0 · $_temp1 · última sala $when';
  }

  @override
  String get membersCopyInviteLink => 'Copiar link de convite';

  @override
  String membersInviteLinkTooltip(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'alguém do grupo',
      'first': 'o primeiro do grupo',
      'other': 'um dos $n primeiros do grupo',
    });
    return 'Funciona uma vez, por 24 horas, e $_temp0 deixa a pessoa entrar';
  }

  @override
  String membersInviteLinkCopied(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'alguém do grupo',
      'first': 'o primeiro do grupo',
      'other': 'um dos $n primeiros do grupo',
    });
    return 'Link de convite copiado. Ele funciona uma vez, por 24 horas, e $_temp0 deixa a pessoa entrar.';
  }

  @override
  String get membersInviteFriends => 'Convidar amigos';

  @override
  String get membersBackToRoom => 'Voltar para a sala';

  @override
  String get membersOpenRoom => 'Abrir sala';

  @override
  String get membersEnterRoom => 'Entrar na sala';

  @override
  String get membersTitle => 'Membros';

  @override
  String get membersOrderNote =>
      'Em ordem: cada um atua sobre quem está abaixo';

  @override
  String get membersGroupSettings => 'Configurações do grupo';

  @override
  String membersManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Só os $n primeiros podem mudar o grupo.',
      one: 'Só o primeiro pode mudar o grupo.',
      zero: 'Só os $n primeiros podem mudar o grupo.',
    );
    return '$_temp0';
  }

  @override
  String get membersLeaveGroup => 'Sair do grupo';

  @override
  String membersInviteTitle(String group) {
    return 'Convidar amigos para “$group”';
  }

  @override
  String get membersInviteNote =>
      'Eles recebem um convite e escolhem se querem entrar.';

  @override
  String get membersPresenceYou => 'você';

  @override
  String membersPresenceOnline(String how) {
    return 'online · $how';
  }

  @override
  String membersPresenceAway(String how) {
    return 'ausente · $how';
  }

  @override
  String get membersPresenceOffline => 'offline agora';

  @override
  String membersYou(String name) {
    return '$name (você)';
  }

  @override
  String get membersWaitingForKey => 'esperando a chave';

  @override
  String membersFingerprint(String fingerprint) {
    return 'Impressão digital $fingerprint';
  }

  @override
  String membersOptionsFor(String name) {
    return 'Opções para $name';
  }

  @override
  String get membersMoveUp => 'Subir para logo abaixo de mim';

  @override
  String get membersMoveToEnd => 'Mover para o fim';

  @override
  String get membersTimeOut => 'Suspender de uma sala…';

  @override
  String get membersRemoveFromGroup => 'Remover do grupo';

  @override
  String membersRemoveTitle(String name) {
    return 'Remover $name?';
  }

  @override
  String membersRemoveText(String name) {
    return '$name sai do grupo e o grupo cria uma nova chave. O que a pessoa já viu continua com ela.';
  }

  @override
  String get membersFirst => 'Primeiro';

  @override
  String membersTimeOutTitle(String name) {
    return 'Suspender $name';
  }

  @override
  String membersTimeOutText(String name) {
    return '$name sai da sala e só pode voltar quando o tempo acabar ou quando alguém acima na ordem liberar a volta.';
  }

  @override
  String get membersUntilLetBack => 'Até liberar';

  @override
  String membersLeaveTitle(String group) {
    return 'Sair de “$group”?';
  }

  @override
  String get membersLeaveText =>
      'Você deixa de ver as salas do grupo e os outros deixam de ver você nele. O grupo cria uma nova chave sem você. Para voltar, alguém do grupo precisa deixar você entrar de novo.';

  @override
  String membersLeaveTextFirst(String name) {
    return 'Você deixa de ver as salas do grupo e os outros deixam de ver você nele. O grupo cria uma nova chave sem você. Para voltar, alguém do grupo precisa deixar você entrar de novo. $name passa a ser o primeiro.';
  }

  @override
  String get groupSettingsTabGeneral => 'Geral';

  @override
  String get groupSettingsTabVoiceRooms => 'Salas de voz';

  @override
  String get groupSettingsTabMembers => 'Membros';

  @override
  String get groupSettingsTabInvite => 'Convidar';

  @override
  String get groupSettingsTabLimits => 'Limites';

  @override
  String get groupSettingsTabActivity => 'Atividade';

  @override
  String get groupSettingsFirstHere => 'Primeiro neste grupo';

  @override
  String groupSettingsPlaceHere(String place) {
    return '$place neste grupo';
  }

  @override
  String get groupSettingsLeave => 'Sair do grupo';

  @override
  String get groupSettingsInviteTitle => 'Convidar pessoas';

  @override
  String get groupSettingsClose => 'Fechar configurações do grupo';

  @override
  String get groupSettingsPicture => 'Imagem';

  @override
  String get groupSettingsChooseFromComputer => 'Escolher do computador';

  @override
  String get groupSettingsUseIcon => 'Usar um ícone';

  @override
  String get groupSettingsPictureNote =>
      'PNG, JPEG ou WebP, cortada em quadrado e reduzida como as fotos de perfil, ou um GIF ou WebP animado de até 550 KB.';

  @override
  String get groupSettingsOrIcon => 'Ou um ícone';

  @override
  String get groupSettingsBanner => 'Banner';

  @override
  String get groupSettingsName => 'Nome';

  @override
  String get groupSettingsNameField => 'Nome do grupo';

  @override
  String get groupSettingsNameNote =>
      'Todos no grupo veem o novo nome e a nova imagem.';

  @override
  String get groupSettingsRoomsIntro =>
      'Cada sala de voz tem suas próprias pessoas, chat, arquivos e chamadas: quem está em uma não se conecta com quem está em outra. Cada pessoa fica em uma sala por vez.';

  @override
  String get groupSettingsRoomName => 'Nome da sala';

  @override
  String get groupSettingsAccessEveryone => 'Todos';

  @override
  String get groupSettingsAccessSome => 'Só alguns';

  @override
  String groupSettingsWhoCanEnter(String room) {
    return 'Quem pode entrar em $room';
  }

  @override
  String get groupSettingsFirstRoomStays =>
      'A primeira sala não pode ser excluída';

  @override
  String get groupSettingsPeopleInside => 'Há pessoas na sala';

  @override
  String get groupSettingsDeleteRoom => 'Excluir sala';

  @override
  String get groupSettingsAddRoom => 'Adicionar sala de voz';

  @override
  String groupSettingsNewRoomName(int n) {
    return 'Sala $n';
  }

  @override
  String groupSettingsRoomsNote(int rooms, int people) {
    return 'Até $rooms salas, com $people pessoas em cada (Limites). Uma sala com pessoas dentro não pode ser excluída.';
  }

  @override
  String get groupSettingsKeptOut => 'Suspensos';

  @override
  String get groupSettingsAccessEveryoneLong => 'Todos no grupo';

  @override
  String get groupSettingsAccessChosen => 'Só os membros que eu escolher';

  @override
  String get groupSettingsAccessNote =>
      'Você e quem está acima de você sempre podem entrar. Quem ficar de fora vê a sala com um cadeado.';

  @override
  String groupSettingsYou(String name) {
    return '$name (você)';
  }

  @override
  String groupSettingsKeptOutBy(String room, String by) {
    return ' · $room · por $by';
  }

  @override
  String groupSettingsTimeLeft(String time) {
    return 'Faltam $time';
  }

  @override
  String get groupSettingsUntilLetBack => 'Até liberar';

  @override
  String get groupSettingsLetBack => 'Liberar';

  @override
  String groupSettingsManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Só os $n primeiros podem mudar o grupo: nome, salas, convites e limites. Você atua sobre quem está abaixo de você.',
      one: 'Só o primeiro pode mudar o grupo: nome, salas, convites e limites. Você atua sobre quem está abaixo de você.',
      zero:
          'Só os $n primeiros podem mudar o grupo: nome, salas, convites e limites. Você atua sobre quem está abaixo de você.',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsMembersCount(int n, int max) {
    String _temp0 = intl.Intl.pluralLogic(
      max,
      locale: localeName,
      other: '$max membros',
      one: '1 membro',
      zero: '$max membros',
    );
    return '$n de $_temp0, em ordem';
  }

  @override
  String get groupSettingsMembersNote =>
      'Cada um atua sobre quem está abaixo: sobe a pessoa para logo abaixo de si ou a move para o fim, suspende de uma sala ou remove do grupo. Quem entra começa no fim.';

  @override
  String get groupSettingsWhoManages => 'Quem administra este grupo';

  @override
  String get groupSettingsManagersEveryone => 'Todos';

  @override
  String groupSettingsManagersFirstN(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Os $n primeiros',
      one: 'O primeiro',
      zero: 'Os $n primeiros',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsManagersMore(int n, int members) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: 'os $members membros',
      one: 'o 1 membro',
      zero: 'os $members membros',
    );
    return 'Definido para os $n primeiros, mais do que $_temp0 do grupo agora: por enquanto, são todos.';
  }

  @override
  String get groupSettingsManagersEveryoneNote =>
      'Quem administra muda o nome, as imagens, as salas e os limites, e deixa pessoas entrarem. Com todos administrando, o grupo continua mesmo que alguém perca o dispositivo.';

  @override
  String groupSettingsManagersFirstNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Só os $n primeiros mudam o nome, as imagens, as salas e os limites, e deixam pessoas entrarem. Se todos perderem os dispositivos, ninguém mais pode administrar o grupo: o Fogata não tem servidor que possa resolver isso.',
      one: 'Só o primeiro muda o nome, as imagens, as salas e os limites, e deixa pessoas entrarem. Se esse dispositivo for perdido, ninguém mais pode administrar o grupo: o Fogata não tem servidor que possa resolver isso.',
      zero:
          'Só os $n primeiros mudam o nome, as imagens, as salas e os limites, e deixam pessoas entrarem. Se todos perderem os dispositivos, ninguém mais pode administrar o grupo: o Fogata não tem servidor que possa resolver isso.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsMoveUp => 'Subir';

  @override
  String groupSettingsMoveUpTooltip(String name) {
    return 'Subir $name para logo abaixo de você';
  }

  @override
  String get groupSettingsToEnd => 'Para o fim';

  @override
  String groupSettingsToEndTooltip(String name) {
    return 'Mover $name para o fim';
  }

  @override
  String get groupSettingsTimeOut => 'Suspender de uma sala';

  @override
  String get groupSettingsRemove => 'Remover do grupo';

  @override
  String get groupSettingsAskingToJoin => 'Pedindo para entrar';

  @override
  String get groupSettingsInviteFriend => 'Convidar um amigo';

  @override
  String get groupSettingsInviteFriendNote =>
      'Eles recebem um convite no Fogata e escolhem se querem entrar. Só seus amigos aparecem aqui.';

  @override
  String get groupSettingsInviteLink => 'Link de convite';

  @override
  String groupSettingsInviteLinkNote(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'alguém do grupo',
      'first': 'o primeiro do grupo',
      'other': 'um dos $n primeiros do grupo',
    });
    return 'Para alguém que ainda não é seu amigo. Funciona uma vez e por 24 horas, e $_temp0 deixa a pessoa entrar: se chegar a outra pessoa, você vê quem está pedindo e pode recusar.';
  }

  @override
  String get groupSettingsCopyLink => 'Copiar link de convite';

  @override
  String groupSettingsOpenLinks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n links ainda não usados.',
      one: '1 link ainda não usado.',
      zero: 'Nenhum link esperando para ser usado.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsCancelLinks => 'Cancelar todos';

  @override
  String get groupSettingsNoFriends =>
      'Nenhum amigo ainda: adicione em Amigos.';

  @override
  String get groupSettingsAllFriendsIn =>
      'Todos os seus amigos estão no grupo.';

  @override
  String get groupSettingsFriendOnline => 'Online';

  @override
  String get groupSettingsFriendOffline => 'Offline agora';

  @override
  String get groupSettingsInviteButton => 'Convidar';

  @override
  String get groupSettingsGroupFull => 'O grupo está cheio';

  @override
  String get groupSettingsSayNo => 'Recusar';

  @override
  String get groupSettingsLetIn => 'Deixar entrar';

  @override
  String get groupSettingsLimitGroup => 'Pessoas no grupo';

  @override
  String groupSettingsLimitGroupNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'das $n pessoas',
      one: 'da 1 pessoa',
      zero: 'das $n pessoas',
    );
    return 'De 2 a 20. Quando estiver cheio, os convites e links param de funcionar. Não pode ficar abaixo $_temp0 que já estão nele.';
  }

  @override
  String get groupSettingsLimitRoom => 'Pessoas em cada sala de voz';

  @override
  String get groupSettingsLimitRoomNote =>
      'De 2 a 8. Voz e vídeo vão direto entre todos na sala, então 8 é o máximo com som bom; com câmeras ligadas, menos funciona melhor. Quem chegar com a sala cheia vê que ela está cheia.';

  @override
  String get shellChipCallMany =>
      'Direto · criptografado · só quem está na chamada';

  @override
  String get shellChipCallTwo => 'Direto · criptografado · só vocês dois';

  @override
  String get shellGroupMembers => 'Membros do grupo';

  @override
  String get shellMembers => 'Membros';

  @override
  String get shellDirectMessages => 'Mensagens diretas';

  @override
  String get shellPathDirect => 'Direto · criptografado';

  @override
  String shellPathVia(String name) {
    return 'Via $name · criptografado';
  }

  @override
  String get shellPathViaSomeone => 'Via um membro · criptografado';

  @override
  String shellCallWith(String names) {
    return 'Chamada com $names';
  }

  @override
  String get shellRinging => 'Tocando · ';

  @override
  String get shellInCall => 'Em chamada · ';

  @override
  String get shellCallEnded => 'Chamada encerrada';

  @override
  String get shellNewGroup => 'Novo grupo';

  @override
  String get shellSectionCall => 'Chamada';

  @override
  String shellCallRinging(String name) {
    return '$name · tocando';
  }

  @override
  String get shellSectionLiveNow => 'Ao vivo agora';

  @override
  String get shellSectionInCall => 'Na chamada';

  @override
  String get shellChat => 'Chat';

  @override
  String get shellFiles => 'Arquivos';

  @override
  String get shellSectionGroup => 'Grupo';

  @override
  String get shellInvite => 'Convidar';

  @override
  String get shellSectionVoiceRooms => 'Salas de voz';

  @override
  String shellVoiceRoomYouAreIn(String room) {
    return '$room · aqui';
  }

  @override
  String get shellNotForYou => 'Fechada para você';

  @override
  String get shellOnlySomeMembers => 'Só alguns membros';

  @override
  String shellPersonYou(String name) {
    return '$name (você)';
  }

  @override
  String get shellLive => 'AO VIVO';

  @override
  String get shellYourProfile => 'Seu perfil';

  @override
  String shellMuteMic(String keys) {
    return 'Silenciar microfone ($keys)';
  }

  @override
  String shellUnmuteMic(String keys) {
    return 'Ativar microfone ($keys)';
  }

  @override
  String get shellDeafen => 'Parar de ouvir os outros';

  @override
  String get shellUndeafen => 'Ouvir os outros de novo';

  @override
  String get shellStateCalling => 'Ligando…';

  @override
  String get shellStateInCall => 'Em chamada';

  @override
  String get shellStateInRoom => 'Na sala';

  @override
  String get shellStateOffline => 'Offline';

  @override
  String get shellStateOnline => 'Online';

  @override
  String get shellStateAway => 'Ausente';

  @override
  String get shellShowInFolder => 'Mostrar na pasta';

  @override
  String get shellCopyPath => 'Copiar caminho';

  @override
  String shellVolumePercent(int n) {
    return '$n%';
  }

  @override
  String get shellMuteKeys => 'Ctrl+Shift+M';

  @override
  String get settingsSectionDonate => 'Doar para o Fogata';

  @override
  String get settingsSectionReport => 'Relatar um problema';

  @override
  String get settingsHeadingHelp => 'Ajuda';

  @override
  String get settingsPreRelease => 'Pré-lançamento';

  @override
  String get donateTitle => 'Mantenha o Fogata independente';

  @override
  String get donateText =>
      'O Fogata é gratuito, não tem anúncios nem servidores vigiando você. Sua doação mantém tudo assim.';

  @override
  String get donateButton => 'Doar';

  @override
  String get donateNote =>
      'Abre o Ko-fi no seu navegador. O Fogata nunca vê o seu pagamento.';

  @override
  String get reportWhatTitle => 'O que aconteceu?';

  @override
  String get reportWhatNote =>
      'O que você estava fazendo, o que esperava e o que viu no lugar.';

  @override
  String get reportIncludeLog =>
      'Incluir o registro técnico dos últimos 2 dias';

  @override
  String get reportLogNote =>
      'Versões, erros e etapas de conexão. Nunca suas mensagens, chaves, contatos ou endereços IP.';

  @override
  String get reportSeeSent => 'Ver o que é enviado';

  @override
  String get reportHideSent => 'Ocultar a lista';

  @override
  String get reportPreview => 'Ler o relatório inteiro antes de enviar';

  @override
  String get reportHidePreview => 'Ocultar o relatório';

  @override
  String get reportPreviewNote =>
      'Isto é exatamente o que o arquivo contém, e é tudo o que vai. Nomes, pastas, nomes de arquivos, endereços e chaves já foram removidos do registro.';

  @override
  String get reportPreviewWorking => 'Preparando o relatório…';

  @override
  String get reportSentWhat => 'O que você escreveu acima.';

  @override
  String get reportSentVersion =>
      'A versão e a compilação do Fogata, seu sistema e a versão dele, e o idioma em que o Fogata está.';

  @override
  String get reportSentLog =>
      'O registro dos últimos 2 dias: quando o Fogata iniciou, os erros dele e como ele chegou a outros dispositivos. Nomes, mensagens, pastas, nomes de arquivos, links, códigos, chaves e endereços IP são removidos, e cada dispositivo aparece só como um número que não significa nada fora do relatório.';

  @override
  String get reportEmail => 'Enviar por e-mail para a equipe do Fogata';

  @override
  String get reportSave => 'Salvar em um arquivo';

  @override
  String reportHowEmail(String address) {
    return 'Seu app de e-mail abre com a mensagem pronta para $address, e a pasta com o arquivo do relatório abre ao lado: anexe o arquivo e envie. Nada sai do seu computador até você fazer isso.';
  }

  @override
  String get reportReadyTitle => 'Seu e-mail está pronto';

  @override
  String reportReadyText(String file, String address) {
    return 'Anexe $file da pasta que abriu e envie para $address.';
  }

  @override
  String get reportShowFile => 'Mostrar o arquivo';

  @override
  String get reportCopyAddress => 'Copiar o endereço';

  @override
  String get reportAddressCopied => 'Endereço copiado';

  @override
  String reportSaved(String file) {
    return 'Relatório salvo: $file';
  }

  @override
  String reportNoMailApp(String address) {
    return 'Nenhum app de e-mail abriu. Envie o arquivo para $address pelo e-mail que você usa.';
  }

  @override
  String reportFailed(String reason) {
    return 'Não foi possível gravar o relatório: $reason';
  }

  @override
  String get splashStarting => 'Iniciando';

  @override
  String get splashConnecting => 'Conectando';

  @override
  String get splashChecking => 'Procurando atualizações';

  @override
  String get splashDownloading => 'Baixando atualização';

  @override
  String get splashInstalling => 'Instalando atualização';

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
    return 'Atualização pronta · $version';
  }

  @override
  String updateRequired(String version) {
    return 'Atualização necessária · $version';
  }

  @override
  String get updateRestart => 'Reiniciar';

  @override
  String updateReadyTip(String version) {
    return 'O Fogata $version chegou. Reinicie para usá-lo.';
  }

  @override
  String get updateRequiredTip =>
      'Esta versão não funciona mais com a versão mais nova dos seus amigos. Reinicie para atualizar.';

  @override
  String updateFailed(String reason) {
    return 'Não foi possível instalar a atualização: $reason';
  }

  @override
  String get errJoinInviteGone =>
      'Esse convite não funciona mais ou o grupo está cheio.';

  @override
  String get errJoinNoKeyYet =>
      'Este dispositivo ainda não tem a chave do grupo. O novo membro entra assim que ela chegar.';

  @override
  String get errJoinWroteTooMuch =>
      'Este dispositivo fez mudanças que os outros membros ainda não receberam. O novo membro pode entrar quando outro membro estiver online.';

  @override
  String get errJoinAskAgain =>
      'Esse pedido para entrar foi feito sobre um estado anterior do grupo. A pessoa precisa pedir de novo.';

  @override
  String get errNoSpaceForFile =>
      'Não há espaço livre suficiente para este arquivo. Libere espaço e tente de novo.';

  @override
  String get recoveryTitle => 'Cópia de recuperação';

  @override
  String get recoveryNote =>
      'Um arquivo que traz esta conta de volta se o chaveiro ou o disco deste computador se perder. Só a sua chave de recuperação abre esse arquivo.';

  @override
  String get recoveryOff => 'Desativada';

  @override
  String recoveryOn(String id) {
    return 'Ativada · chave $id';
  }

  @override
  String get recoveryTurnOn => 'Ativar…';

  @override
  String get recoveryChangeKey => 'Trocar a chave…';

  @override
  String get recoveryMakeCopy => 'Fazer uma cópia…';

  @override
  String get recoveryReplacedKept =>
      'Este computador ainda guarda uma conta que uma restauração substituiu.';

  @override
  String get recoveryDeleteReplaced => 'Excluir…';

  @override
  String get recoveryChangeTitle => 'Criar uma nova chave de recuperação?';

  @override
  String get recoveryChangeText =>
      'As cópias novas só vão abrir com a chave nova. As que você fez antes continuam abrindo só com a antiga: guarde-a enquanto guardar essas cópias.';

  @override
  String get recoveryChangeConfirm => 'Criar uma chave nova';

  @override
  String get recoveryKeyTitle => 'Sua chave de recuperação';

  @override
  String get recoveryKeyText =>
      'Anote ou imprima a chave e guarde longe das suas cópias. Ela só aparece desta vez.';

  @override
  String recoveryKeyName(String id) {
    return 'Nome curto: $id';
  }

  @override
  String get recoveryKeyWarning =>
      'O Fogata não guarda esta chave em lugar nenhum: sem ela, nenhuma cópia abre. Quem tiver uma cópia e esta chave tem a sua conta.';

  @override
  String get recoveryKeyWritten => 'Já anotei';

  @override
  String get recoveryCheckTitle => 'Confira o que você anotou';

  @override
  String get recoveryCheckText =>
      'Digite estes dois grupos da chave, do jeito que você anotou.';

  @override
  String recoveryCheckGroup(int n) {
    return 'Grupo $n';
  }

  @override
  String get recoveryCheckWrong =>
      'Não é esse: olhe de novo o que você anotou.';

  @override
  String get recoveryTurnOnButton => 'Ativar as cópias';

  @override
  String recoveryTurnedOn(String id) {
    return 'As cópias estão ativadas, com a chave $id.';
  }

  @override
  String get recoveryCopyTitle => 'Fazer uma cópia de recuperação';

  @override
  String get recoveryCopyText =>
      'O Fogata fica offline enquanto faz a cópia, por mais tempo se incluir os arquivos das conversas. Guarde o arquivo longe deste computador, num pendrive, por exemplo.';

  @override
  String get recoveryCopyAttachments =>
      'Incluir os arquivos enviados nas conversas';

  @override
  String recoveryCopyAttachmentsNote(String size) {
    return 'Imagens, vídeos e arquivos: mais $size.';
  }

  @override
  String get recoveryCopySave => 'Escolher onde…';

  @override
  String get recoveryCopyInDataFolder =>
      'Uma cópia não pode ficar na pasta de dados do Fogata. Escolha outro lugar.';

  @override
  String recoveryCopyMade(String file, String size) {
    return 'Cópia salva: $file ($size).';
  }

  @override
  String recoveryCopyFailed(String why) {
    return 'Não foi possível fazer a cópia. $why';
  }

  @override
  String get recoveryDeleteTitle => 'Excluir a conta substituída?';

  @override
  String get recoveryDeleteText =>
      'Ela foi deixada de lado quando uma cópia foi restaurada no lugar dela. Depois de excluída, não dá para trazê-la de volta.';

  @override
  String get recoveryDelete => 'Excluir';

  @override
  String get recoveryDeleted => 'A conta substituída foi excluída.';

  @override
  String get recoveryFileType => 'Cópia do Fogata';

  @override
  String get recoveryLostComputer => 'Perdeu seu computador?';

  @override
  String get recoveryRestoreLink => 'Restaurar de uma cópia';

  @override
  String get recoveryRestoreTitle => 'Restaurar de uma cópia';

  @override
  String get recoveryRestoreSub =>
      'Escolha a cópia de recuperação que você salvou e depois digite sua chave de recuperação.';

  @override
  String get recoveryChooseCopy => 'Escolher a cópia…';

  @override
  String recoveryMadeOn(String date) {
    return 'Uma cópia feita em $date.';
  }

  @override
  String get recoveryOtherKey =>
      'Ela foi feita para outra chave de recuperação, não a deste computador. Digite a chave para a qual ela foi feita.';

  @override
  String get recoveryKeyField => 'Chave de recuperação';

  @override
  String get recoveryKeyNotAKey =>
      'Uma chave de recuperação tem 53 letras e números, em grupos de quatro.';

  @override
  String get recoveryKeyMistyped =>
      'Uma letra ou um número está errado: confira.';

  @override
  String get recoveryProblemNotACopy =>
      'Isto não é uma cópia de uma conta do Fogata.';

  @override
  String get recoveryProblemNewer =>
      'Esta cópia foi feita por uma versão mais nova do Fogata. Atualize o Fogata para abri-la.';

  @override
  String get recoveryProblemOtherKey =>
      'Esta cópia foi feita para outra chave de recuperação.';

  @override
  String get recoveryProblemDamaged =>
      'Esta cópia está danificada ou foi alterada depois de feita.';

  @override
  String get recoveryProblemUnproven =>
      'Esta cópia não foi feita pela conta que ela contém, por isso não será aberta.';

  @override
  String recoveryWhose(String name) {
    return 'Conta de $name';
  }

  @override
  String recoveryHolds(int files, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files arquivos',
      one: '$files arquivo',
      zero: '$files arquivos',
    );
    return '$_temp0, $size no total';
  }

  @override
  String get recoveryWithAttachments =>
      'Com os arquivos enviados nas conversas.';

  @override
  String get recoveryWithoutAttachments =>
      'Sem os arquivos enviados nas conversas.';

  @override
  String get recoveryOnlyIf =>
      'Use uma cópia só se nenhum outro computador usar mais esta conta. Se algum ainda usar, transfira a conta a partir dele: uma conta não pode estar em dois computadores.';

  @override
  String get recoveryMoveInstead => 'Transferir em vez disso';

  @override
  String recoveryHereNamed(String name) {
    return 'Este computador tem a conta de $name.';
  }

  @override
  String recoveryHereDevice(String fingerprint) {
    return 'Este computador tem uma conta sem perfil (dispositivo $fingerprint).';
  }

  @override
  String get recoveryHereUnknown =>
      'Este computador tem parte de uma conta que não abre.';

  @override
  String get recoveryHereReplaced =>
      'Restaurar substitui essa conta. Ela fica guardada à parte, sem ser excluída, até você excluí-la em Configurações › Armazenamento e downloads.';

  @override
  String get recoveryRestore => 'Restaurar';

  @override
  String get recoveryReplace => 'Substituir e restaurar';

  @override
  String recoveryRestoring(String name) {
    return 'Restaurando a conta de $name…';
  }

  @override
  String recoveryRestored(String name) {
    return 'A conta de $name foi restaurada.';
  }

  @override
  String get recoveryRefusedAccountHere =>
      'Agora este computador tem uma conta. Escolha a cópia de novo para ver o que ela substitui.';

  @override
  String get recoveryRefusedReplacedKept =>
      'Este computador ainda guarda uma conta que uma restauração anterior substituiu. Exclua-a para restaurar esta cópia.';

  @override
  String recoveryRefusedNoRoom(String need, String free) {
    return 'A cópia precisa de $need neste disco, e há $free livres. Nada mudou.';
  }

  @override
  String get recoveryRefusedInFolder =>
      'A cópia está dentro da pasta de dados do Fogata. Mova-a para outro lugar e escolha de novo.';

  @override
  String get recoveryBusy =>
      'O Fogata está ocupado com uma cópia de recuperação ou uma transferência. Tente de novo quando terminar.';

  @override
  String get moveBusyWithCopy =>
      'O Fogata está ocupado com uma cópia de recuperação. Transfira a conta quando terminar.';

  @override
  String get recoveryDeleteKeptAside => 'Excluir a conta guardada à parte…';

  @override
  String get reachOffline =>
      'Daqui o Fogata não alcança ninguém na internet. Confira a conexão, ou talvez um firewall esteja bloqueando o Fogata.';

  @override
  String get reachFirewall =>
      'O firewall deste computador não deixa nada entrar para o Fogata. O comando que libera o Fogata está nas notificações.';

  @override
  String get reachUdpBlocked =>
      'Esta rede ou este computador bloqueia o UDP. As chamadas e as conexões diretas através de roteadores precisam dele; até ele ser liberado, os outros só alcançam você através de um membro de um grupo ou a partir de um endereço público.';

  @override
  String get reachSymmetricNat =>
      'Seu roteador dá uma porta pública diferente para cada destino (um NAT simétrico), então não dá para abrir um caminho direto através dele. Amigos atrás de outro roteador só alcançam você através de um membro de um grupo em comum.';

  @override
  String get reachNoIpv6 =>
      'Também não há IPv6 funcionando, que permitiria contornar o roteador.';

  @override
  String reachFineFriend(String name) {
    return 'Sua rede parece estar boa para o Fogata, então o que separa vocês provavelmente está do lado de $name (offline, ou a rede dessa pessoa).';
  }

  @override
  String get reachFineOthers =>
      'Sua rede parece estar boa para o Fogata, então o que separa vocês provavelmente está do outro lado.';

  @override
  String directDropTitle(String name) {
    return 'Solte para enviar para $name';
  }

  @override
  String get directDropNote =>
      'Eles esperam na caixa de mensagem, onde você pode adicionar uma mensagem e conferir os arquivos antes de enviá-los.';

  @override
  String directPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n arquivos ocultos ou privados ficaram de fora: arquivos assim não são adicionados arrastando, porque uma página da web pode iniciar um arraste. Para enviá-los, escolha-os com o botão +.',
      one: 'Um arquivo oculto ou privado ficou de fora: arquivos assim não são adicionados arrastando, porque uma página da web pode iniciar um arraste. Para enviá-lo, escolha-o com o botão +.',
      zero:
          '$n arquivos ocultos ou privados ficaram de fora: arquivos assim não são adicionados arrastando, porque uma página da web pode iniciar um arraste. Para enviá-los, escolha-os com o botão +.',
    );
    return '$_temp0';
  }
}
