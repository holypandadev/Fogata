// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Russian (`ru`).
class L10nRu extends L10n {
  L10nRu([String locale = 'ru']) : super(locale);

  @override
  String get commonCancel => 'Отмена';

  @override
  String get commonSave => 'Сохранить';

  @override
  String get commonClose => 'Закрыть';

  @override
  String commonNewMessageFrom(String name, String text) {
    return 'Новое сообщение от $name: $text';
  }

  @override
  String commonNewMessagesFrom(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n новых сообщения от $name',
      many: '$n новых сообщений от $name',
      few: '$n новых сообщения от $name',
      one: '$n новое сообщение от $name',
    );
    return '$_temp0';
  }

  @override
  String commonNewMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n новых сообщения',
      many: '$n новых сообщений',
      few: '$n новых сообщения',
      one: '$n новое сообщение',
    );
    return '$_temp0';
  }

  @override
  String get commonAccept => 'Принять';

  @override
  String get commonDecline => 'Отклонить';

  @override
  String get commonSend => 'Отправить';

  @override
  String get commonTryAgain => 'Повторить';

  @override
  String get commonCopy => 'Копировать';

  @override
  String get commonCopied => 'Скопировано';

  @override
  String get commonBack => 'Назад';

  @override
  String get commonDiscard => 'Не сохранять';

  @override
  String get commonKeepEditing => 'Продолжить правку';

  @override
  String get commonNotNow => 'Не сейчас';

  @override
  String get commonDownload => 'Скачать';

  @override
  String get commonSettings => 'Настройки';

  @override
  String get commonHome => 'Главная';

  @override
  String get commonFriends => 'Друзья';

  @override
  String get commonContinue => 'Продолжить';

  @override
  String get commonDone => 'Готово';

  @override
  String get commonRemove => 'Удалить';

  @override
  String get commonYou => 'Вы';

  @override
  String timeYesterdayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'вчера в $time',
      'other': 'вчера в $time',
    });
    return '$_temp0';
  }

  @override
  String timeDayAt(String date, String time) {
    return '$date, $time';
  }

  @override
  String get timeNow => 'сейчас';

  @override
  String timeMinutes(int n) {
    return '$n мин';
  }

  @override
  String timeHours(int n) {
    return '$n ч';
  }

  @override
  String timeHoursMinutes(int hours, int minutes) {
    return '$hours ч $minutes мин';
  }

  @override
  String timeDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n дня',
      many: '$n дней',
      few: '$n дня',
      one: '$n день',
    );
    return '$_temp0';
  }

  @override
  String spanMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n минуты',
      many: '$n минут',
      few: '$n минуты',
      one: '$n минуту',
    );
    return '$_temp0';
  }

  @override
  String spanHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n часа',
      many: '$n часов',
      few: '$n часа',
      one: '$n час',
    );
    return '$_temp0';
  }

  @override
  String spanDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n дня',
      many: '$n дней',
      few: '$n дня',
      one: '$n день',
    );
    return '$_temp0';
  }

  @override
  String spanWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n недели',
      many: '$n недель',
      few: '$n недели',
      one: '$n неделю',
    );
    return '$_temp0';
  }

  @override
  String get agoMoment => 'только что';

  @override
  String agoMinutes(int n) {
    return '$n мин назад';
  }

  @override
  String agoHours(int n) {
    return '$n ч назад';
  }

  @override
  String get agoYesterday => 'вчера';

  @override
  String agoDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n дня назад',
      many: '$n дней назад',
      few: '$n дня назад',
      one: '$n день назад',
    );
    return '$_temp0';
  }

  @override
  String agoWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n недели назад',
      many: '$n недель назад',
      few: '$n недели назад',
      one: '$n неделю назад',
    );
    return '$_temp0';
  }

  @override
  String agoMonths(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n месяца назад',
      many: '$n месяцев назад',
      few: '$n месяца назад',
      one: '$n месяц назад',
    );
    return '$_temp0';
  }

  @override
  String get agoOverYear => 'больше года назад';

  @override
  String sizeBytes(int n) {
    return '$n Б';
  }

  @override
  String sizeKB(String n) {
    return '$n КБ';
  }

  @override
  String sizeMB(String n) {
    return '$n МБ';
  }

  @override
  String sizeGB(String n) {
    return '$n ГБ';
  }

  @override
  String sizeTB(String n) {
    return '$n ТБ';
  }

  @override
  String listTwo(String first, String second) {
    return '$first и $second';
  }

  @override
  String listMore(String rest, String last) {
    return '$rest и $last';
  }

  @override
  String get listSeparator => ', ';

  @override
  String ordinal(int n) {
    return '№ $n';
  }

  @override
  String get errLinkExpired => 'Срок действия ссылки истёк. Попросите новую.';

  @override
  String get errGroupInvite => 'Это приглашение в группу.';

  @override
  String get errDeviceNotFound =>
      'Другое устройство не найдено. Убедитесь, что вы оба в сети.';

  @override
  String get errNotFriendCode => 'Это не похоже на ссылку или код Fogata.';

  @override
  String get errAlreadyInGroup => 'Вы уже состоите в этой группе.';

  @override
  String get errJoinRoomFirst => 'Сначала войдите в комнату.';

  @override
  String get errNewerVersion =>
      'Ваш профиль сохранён более новой версией Fogata. Обновите Fogata, чтобы открыть его.';

  @override
  String get errKeychainUnreachable =>
      'Fogata не удалось получить ключ к вашему профилю из связки ключей этого компьютера. Разблокируйте связку ключей или разрешите Fogata её использовать, затем повторите попытку.';

  @override
  String get errKeychainNoKey =>
      'В связке ключей этого компьютера больше нет ключа к вашему профилю, поэтому Fogata не может его открыть.';

  @override
  String get errKeychainWrongKey =>
      'Ключ из связки ключей этого компьютера не открывает ваш профиль.';

  @override
  String get errNoGroupKey =>
      'Эта группа ещё не готова на этом компьютере. Повторите попытку чуть позже.';

  @override
  String get errAlreadyRunning =>
      'Fogata уже открыта с этим профилем. Сначала закройте другое окно.';

  @override
  String get errFileGone => 'Этого файла больше нет в комнате.';

  @override
  String get errNobodyHasFile => 'Сейчас ни у кого в сети нет этого файла.';

  @override
  String get errStillStarting => 'Fogata ещё запускается.';

  @override
  String get errIdentityExists =>
      'На этом компьютере уже есть аккаунт; его ключ нельзя изменить.';

  @override
  String get errNameRule => 'Введите имя длиной от 1 до 40 видимых символов.';

  @override
  String get errBioRule =>
      'Описание — не больше 190 символов в 4 строках, без скрытых символов.';

  @override
  String get errPictureRule =>
      'Изображение должно быть в формате PNG или JPEG и весить не больше 64 КБ.';

  @override
  String get errNotFriendLink =>
      'Это не ссылка для добавления в друзья в Fogata.';

  @override
  String get errLinkDamaged => 'Ссылка неполная или повреждена.';

  @override
  String get errLinkNeedsNewer =>
      'Для этой ссылки нужна более новая версия Fogata.';

  @override
  String get errCodeShape => 'Код выглядит так: FGT-XXXXX-XXXXX.';

  @override
  String get errCodeWrong => 'Код неверный или больше не действует.';

  @override
  String get errNotProven =>
      'Другое устройство не смогло подтвердить свою подлинность.';

  @override
  String get errCallGone => 'Этот звонок уже не активен.';

  @override
  String get errAlreadyInCall =>
      'Вы уже участвуете в звонке. Сначала завершите его.';

  @override
  String get errAlreadyFriends => 'Вы уже друзья.';

  @override
  String get errMemberNotConnected =>
      'Этот участник сейчас не подключён. Попробуйте снова, когда он будет в сети.';

  @override
  String get errMemberTooOld =>
      'Версия Fogata у этого участника слишком старая, чтобы принимать запросы в друзья из группы. Отправьте ему ссылку для добавления в друзья.';

  @override
  String get errMemberAskLost =>
      'Запрос в друзья не дошёл до этого участника. Попробуйте снова.';

  @override
  String get errOwnLink =>
      'Это ваша собственная ссылка: отправьте её другу, которого хотите добавить.';

  @override
  String get errLinkUsed =>
      'Эта ссылка уже использована или устарела. Попросите новую.';

  @override
  String get errLinkExpiredUnreached =>
      'Срок действия ссылки истёк, прежде чем удалось связаться с её устройством. Попросите новую.';

  @override
  String get errCodeUnanswered =>
      'Ни одно устройство не ответило на код. Проверьте код и то, что другое устройство всё ещё его показывает.';

  @override
  String get errRefused =>
      'Другое устройство отказало. Возможно, код уже был использован.';

  @override
  String get errNobodyLetIn =>
      'Никто вас не впустил. Повторите попытку, когда кто-нибудь из группы будет в сети.';

  @override
  String get errNotBelow =>
      'Это можно сделать только с теми, кто ниже вас в группе.';

  @override
  String get errNotManager => 'Это могут только те, кто управляет группой.';

  @override
  String get errLastManager =>
      'Вы не можете исключить себя из тех, кто управляет группой.';

  @override
  String get errRoomNotEmpty =>
      'В этой голосовой комнате есть люди: удалите её, когда она опустеет.';

  @override
  String get errTooManyFiles =>
      'Вы уже делитесь максимальным числом файлов для одного человека в комнате. Сначала уберите некоторые.';

  @override
  String get errDiskFull =>
      'Диск заполнен. Освободите место и повторите попытку.';

  @override
  String get errTooManyChanges =>
      'Это устройство внесло в группу слишком много изменений, которые ещё не получил ни один другой участник. Повторите попытку, когда кто-нибудь ещё в группе что-нибудь изменит.';

  @override
  String get errSomething => 'Что-то пошло не так.';

  @override
  String errRoomFull(int n) {
    return 'Эта голосовая комната заполнена (мест: $n).';
  }

  @override
  String get findingOffline =>
      'Ни один узел публичной сети не ответил: похоже, этот компьютер не выходит в интернет или брандмауэр блокирует Fogata.';

  @override
  String get findingUdpWorks =>
      'UDP работает. Он нужен для звонков и для соединений через роутеры.';

  @override
  String get findingUdpBlocked =>
      'Похоже, эта сеть или этот компьютер блокирует UDP (узлы, ответившие по TCP, не ответили по QUIC). Чат по-прежнему может идти через других участников, но прямые соединения через роутеры и звонки в основном не будут работать. Проверьте брандмауэр или правила сети.';

  @override
  String get findingCarrierNat =>
      'У этого компьютера адрес за NAT провайдера (100.64.0.0/10): провайдер делит один публичный адрес между многими клиентами.';

  @override
  String get findingPublicIpv4 =>
      'У этого компьютера публичный IPv4-адрес: другие могут подключаться к нему напрямую.';

  @override
  String get findingHardNat =>
      'За роутером, который выдаёт разный публичный порт для каждого адресата (жёсткий NAT).';

  @override
  String get findingEasyNat =>
      'За роутером, который сохраняет один публичный порт для всех адресатов (мягкий NAT): его можно открыть с обеих сторон одновременно.';

  @override
  String get findingUnknownRouter =>
      'Слишком мало узлов видели публичный адрес этого компьютера, чтобы определить тип роутера.';

  @override
  String get findingNoIpv6 =>
      'Нет публичного IPv6-адреса. Fogata он не обязателен, но с ним прямые соединения проще.';

  @override
  String get findingIpv6Works =>
      'Публичный IPv6 работает: друзья с IPv6 подключаются без роутера на пути.';

  @override
  String get findingIpv6Unreached =>
      'У этого компьютера есть публичный IPv6-адрес, но ни один узел ещё не подключился к нему по IPv6.';

  @override
  String get findingSummaryNoUdp =>
      'Итог: пока UDP не разрешён, с прямыми соединениями будут проблемы.';

  @override
  String get findingSummaryPublic =>
      'Итог: ваши друзья должны подключаться к этому компьютеру напрямую.';

  @override
  String get findingSummaryEasyNat =>
      'Итог: с большинством друзей прямые соединения должны работать — оба роутера открываются одновременно; с другом за жёстким NAT нужен участник группы в качестве посредника.';

  @override
  String get findingSummaryHardNatWithIpv6 =>
      'Итог: друзья с IPv6 или публичным адресом подключаются напрямую; с остальными нужен участник группы в качестве посредника.';

  @override
  String get findingSummaryHardNat =>
      'Итог: напрямую подключаются только друзья с публичным адресом; с остальными нужен участник группы в качестве посредника.';

  @override
  String get findingSummaryUnknown =>
      'Итог: недостаточно данных; проверьте ещё раз через минуту.';

  @override
  String findingInternet(int n) {
    return 'Интернет: этот компьютер выходит в публичную сеть (известно узлов: $n).';
  }

  @override
  String findingDns(String detail) {
    return 'DNS: не удалось полностью прочитать системные настройки ($detail)';
  }

  @override
  String findingLanDevices(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n устройства с Fogata',
      many: '$n устройств с Fogata',
      few: '$n устройства с Fogata',
      one: '$n устройство с Fogata',
    );
    return '$_temp0 в этой локальной сети: друзья здесь подключаются без выхода в интернет.';
  }

  @override
  String findingFirewall(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'в PowerShell от имени администратора',
      'macos': 'в Терминале',
      'other': 'в терминале',
    });
    return 'Брандмауэр этого компьютера ничего не пропускает к Fogata, поэтому друг рядом с вами, но за другим роутером (например, если дома два роутера), не сможет к нему подключиться. Выполните эту команду один раз $_temp0, чтобы впустить Fogata:';
  }

  @override
  String get settingsLanguage => 'Язык';

  @override
  String get settingsLanguageNote =>
      'Язык интерфейса Fogata. Сообщения остаются такими, какими их написали.';

  @override
  String get settingsLanguageChoose => 'Выберите язык';

  @override
  String get settingsTime => 'Время';

  @override
  String get settingsTimeNote =>
      'Как показывается время суток в чате, звонках и списках.';

  @override
  String settingsTime24(String example) {
    return '24 часа · $example';
  }

  @override
  String settingsTime12(String example) {
    return '12 часов · $example';
  }

  @override
  String firewallNoticeText(String system, String command) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'в PowerShell от имени администратора',
      'macos': 'в Терминале',
      'other': 'в терминале',
    });
    return 'Друг рядом с вами, но за другим роутером, не может подключиться к этому компьютеру. Выполните один раз $_temp0: $command';
  }

  @override
  String firewallCopied(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'в PowerShell от имени администратора',
      'macos': 'в Терминале',
      'other': 'в терминале',
    });
    return 'Скопировано. Вставьте и выполните $_temp0.';
  }

  @override
  String get appRestarted => 'Fogata перестала работать и запустилась снова.';

  @override
  String appVoiceRoomInGroup(String room, String group) {
    return '$room в группе $group';
  }

  @override
  String appBothIn(String room) {
    return 'Вы оба в комнате $room.';
  }

  @override
  String appHungUp(String name) {
    return '$name: звонок завершён';
  }

  @override
  String get appYourFriend => 'Ваш друг';

  @override
  String get appCallEndedAlone => 'Никто не вернулся, поэтому звонок завершён';

  @override
  String get appAnsweredElsewhere => 'Звонок принят на другом компьютере';

  @override
  String appVoiceRoomDeleted(String room) {
    return 'Комната «$room» удалена.';
  }

  @override
  String get appSomeone => 'Кто-то';

  @override
  String get appAGroup => 'без названия';

  @override
  String appFriendRequest(String name) {
    return '$name хочет добавить вас в друзья.';
  }

  @override
  String appRequestDelivered(String name) {
    return 'Запрос доставлен пользователю $name. Ожидается ответ.';
  }

  @override
  String appFriendsNow(String name) {
    return 'Теперь вы и $name — друзья.';
  }

  @override
  String appJoinedGroup(String group) {
    return 'Теперь вы в группе «$group».';
  }

  @override
  String appRemovedFromGroup(String group) {
    return 'Вы больше не в группе «$group».';
  }

  @override
  String appAddedToGroup(String name, String group) {
    return '$name приглашает вас в группу «$group». Вступать или нет — решать вам.';
  }

  @override
  String appAsksToJoin(String name, String group) {
    return '$name просит впустить в группу «$group».';
  }

  @override
  String appJoinWaiting(String group) {
    return 'Ждём, когда кто-нибудь из группы «$group» впустит вас.';
  }

  @override
  String appJoinDeclined(String group) {
    return 'Кто-то из группы «$group» отказал. Это приглашение больше не действует.';
  }

  @override
  String appDownloadedTo(String path) {
    return 'Скачано: $path';
  }

  @override
  String appDownloadFailed(String why) {
    return 'Не удалось скачать: $why';
  }

  @override
  String get appOtherSession =>
      'Этот аккаунт открыт и на другом компьютере. Закройте там Fogata: аккаунт работает только на одном компьютере одновременно.';

  @override
  String get appFriendsRestored =>
      'Ваш список друзей на этом компьютере был повреждён и восстановлен из копии, сделанной перед последним изменением. Друга, добавленного или удалённого незадолго до этого, возможно, придётся добавить или удалить снова.';

  @override
  String appFilesRefused(String folder) {
    return 'Некоторые файлы Fogata на этом компьютере оказались незашифрованными там, где они хранятся зашифрованными, поэтому они не использовались. Ничего не удалено: они хранятся в $folder.';
  }

  @override
  String appInvitationRanOut(String group) {
    return 'Приглашение в группу «$group» истекло, прежде чем кто-нибудь из группы впустил вас. Попросите новое.';
  }

  @override
  String appTakenOut(String who, String room) {
    return 'Участник $who удалил вас из комнаты $room.';
  }

  @override
  String appRoomForSome(String room) {
    return 'Комната $room теперь только для некоторых участников.';
  }

  @override
  String appTakenOutUntilLetBack(String who, String room) {
    return 'Участник $who удалил вас из комнаты $room, пока кто-нибудь выше вас не разрешит вернуться.';
  }

  @override
  String appTakenOutFor(String who, String room, String span) {
    return 'Участник $who удалил вас из комнаты $room на $span.';
  }

  @override
  String appTakenOutWhile(String who, String room) {
    return 'Участник $who удалил вас из комнаты $room на время.';
  }

  @override
  String appComeBackIn(String room, String wait) {
    return 'Вы сможете вернуться в комнату $room через $wait.';
  }

  @override
  String appRoomNotForYou(String room) {
    return 'Комната $room сейчас недоступна для вас.';
  }

  @override
  String appFoldersNotShared(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Делиться можно только файлами, не папками: пропущено $n папки.',
      many: 'Делиться можно только файлами, не папками: пропущено $n папок.',
      few: 'Делиться можно только файлами, не папками: пропущено $n папки.',
      one: 'Делиться можно только файлами, не папками: пропущена $n папка.',
    );
    return '$_temp0';
  }

  @override
  String appSharingOne(String file) {
    return 'Вы делитесь файлом $file с этого компьютера. Он перестанет быть доступен, когда вы выйдете из комнаты.';
  }

  @override
  String appSharingMany(int n) {
    return 'Вы делитесь файлами с этого компьютера (файлов: $n). Они перестанут быть доступны, когда вы выйдете из комнаты.';
  }

  @override
  String appSavedTo(String path) {
    return 'Сохранено: $path';
  }

  @override
  String appAskingToJoin(String group) {
    return 'Запрос на вступление в группу «$group» отправлен. Кто-нибудь из группы впустит вас, когда увидит его.';
  }

  @override
  String get appLookingForDevice =>
      'Ищем другое устройство, чтобы отправить запрос…';

  @override
  String callStateNoAnswer(String name) {
    return '$name не отвечает';
  }

  @override
  String callStateDeclined(String name) {
    return '$name сейчас не может говорить';
  }

  @override
  String callStateBusy(String name) {
    return '$name в другом звонке';
  }

  @override
  String callStateOutdated(String name) {
    return '$name: для звонка нужна более новая версия Fogata';
  }

  @override
  String callStateUnreached(String name) {
    return 'Не удалось дозвониться: $name';
  }

  @override
  String get callStateReturn => 'Вернуться';

  @override
  String get callStateCallBack => 'Перезвонить';

  @override
  String get callStateCall => 'Позвонить';

  @override
  String noticesDeclined(String name, String group) {
    return '$name: приглашение в группу «$group» отклонено';
  }

  @override
  String noticesDeclinedToast(String name, String group) {
    return '$name: приглашение в группу «$group» отклонено.';
  }

  @override
  String noticesMissedInCall(String name) {
    return '$name: пропущенный звонок, пока вы были в другом звонке';
  }

  @override
  String noticesMissedOffline(String name) {
    return '$name: пропущенный звонок, пока вы были в режиме невидимки';
  }

  @override
  String noticesMissedCall(String name) {
    return 'Пропущенный звонок: $name';
  }

  @override
  String get contactsGeneral => 'Общая';

  @override
  String get directStateTimerOff => 'Выкл.';

  @override
  String get directStateTimer24h => '24 часа';

  @override
  String directStateTimerDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n дня',
      many: '$n дней',
      few: '$n дня',
      one: '$n день',
    );
    return '$_temp0';
  }

  @override
  String directStateDisappearsMinutes(int n) {
    return 'Исчезнет через $n мин';
  }

  @override
  String directStateDisappearsHours(int n) {
    return 'Исчезнет через $n ч';
  }

  @override
  String directStateDisappearsDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Исчезнет через $n дня',
      many: 'Исчезнет через $n дней',
      few: 'Исчезнет через $n дня',
      one: 'Исчезнет через $n день',
    );
    return '$_temp0';
  }

  @override
  String get directStateNoLimit => 'Без ограничений';

  @override
  String get directStateMessageGone => 'Этого сообщения больше нет.';

  @override
  String directStateFoldersNotSent(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Отправлять можно только файлы, не папки: пропущено $n папки.',
      many: 'Отправлять можно только файлы, не папки: пропущено $n папок.',
      few: 'Отправлять можно только файлы, не папки: пропущено $n папки.',
      one: 'Отправлять можно только файлы, не папки: пропущена $n папка.',
    );
    return '$_temp0';
  }

  @override
  String directStateFileFailed(String why) {
    return 'Не удалось получить файл: $why';
  }

  @override
  String movingTooLarge(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Анимированный баннер может быть не больше $most пикселей; этот — $size.',
      'other':
          'Анимированное изображение может быть не больше $most пикселей; это — $size.',
    });
    return '$_temp0';
  }

  @override
  String movingTooLong(String what, String most, String count) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'В анимированном баннере может быть до $most кадров; в этом $count.',
      'other':
          'В анимированном изображении может быть до $most кадров; в этом $count.',
    });
    return '$_temp0';
  }

  @override
  String movingTooHeavy(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Анимированный баннер может весить до $most; этот весит $size.',
      'other':
          'Анимированное изображение может весить до $most; это весит $size.',
    });
    return '$_temp0';
  }

  @override
  String get namesHidden =>
      'В этом имени есть скрытые символы. Введите его заново без них.';

  @override
  String get namesBlank =>
      'В этом имени нет видимых символов. Используйте буквы, цифры или эмодзи.';

  @override
  String get namesDefaultBio => 'Привет! Я в Fogata.';

  @override
  String get namesBioHidden =>
      'В этом описании есть скрытые символы. Введите его заново без них.';

  @override
  String namesBioLines(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'В описании может быть не больше $n строки.',
      many: 'В описании может быть не больше $n строк.',
      few: 'В описании может быть не больше $n строк.',
      one: 'В описании может быть не больше $n строки.',
    );
    return '$_temp0';
  }

  @override
  String namesBioChars(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'В описании может быть не больше $n символа.',
      many: 'В описании может быть не больше $n символов.',
      few: 'В описании может быть не больше $n символов.',
      one: 'В описании может быть не больше $n символа.',
    );
    return '$_temp0';
  }

  @override
  String get restartFailed =>
      'Fogata не удалось запуститься снова. Закройте её и откройте вручную.';

  @override
  String typingOne(String name) {
    return '$name печатает…';
  }

  @override
  String typingTwo(String first, String second) {
    return '$first и $second печатают…';
  }

  @override
  String get typingSeveral => 'Несколько человек печатают…';

  @override
  String get outgoingTheRoom => 'комната';

  @override
  String outgoingDropTitle(String group) {
    return 'Отпустите, чтобы поделиться: $group';
  }

  @override
  String get outgoingDropNote =>
      'Можно добавить сообщение и проверить файлы перед отправкой. Остальные скачивают их с этого компьютера, пока вы в комнате.';

  @override
  String outgoingPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n скрытых или личных файла не добавлены: такие файлы не добавляются перетаскиванием, ведь перетаскивание может начать веб-страница. Чтобы поделиться ими, выберите их скрепкой.',
      many:
          '$n скрытых или личных файлов не добавлены: такие файлы не добавляются перетаскиванием, ведь перетаскивание может начать веб-страница. Чтобы поделиться ими, выберите их скрепкой.',
      few:
          '$n скрытых или личных файла не добавлены: такие файлы не добавляются перетаскиванием, ведь перетаскивание может начать веб-страница. Чтобы поделиться ими, выберите их скрепкой.',
      one:
          '$n скрытый или личный файл не добавлен: такие файлы не добавляются перетаскиванием, ведь перетаскивание может начать веб-страница. Чтобы поделиться им, выберите его скрепкой.',
    );
    return '$_temp0';
  }

  @override
  String outgoingToShare(int n, String count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$count файла к отправке · $size',
      many: '$count файлов к отправке · $size',
      few: '$count файла к отправке · $size',
      one: '$count файл к отправке · $size',
    );
    return '$_temp0';
  }

  @override
  String get outgoingRemoveAll => 'Убрать все';

  @override
  String outgoingRemoveFile(String file) {
    return 'Убрать $file';
  }

  @override
  String get windowTrayOpen => 'Открыть Fogata';

  @override
  String get windowTrayQuit => 'Выйти из Fogata';

  @override
  String get windowMinimize => 'Свернуть';

  @override
  String get windowRestore => 'Свернуть в окно';

  @override
  String get windowMaximize => 'Развернуть';

  @override
  String get autostartReason => 'Открывать Fogata при входе в систему.';

  @override
  String get devicesMicrophone => 'Микрофон';

  @override
  String get devicesSpeaker => 'Динамик';

  @override
  String get devicesCamera => 'Камера';

  @override
  String get devicesChooseMicrophone => 'Выбрать микрофон';

  @override
  String get devicesChooseSpeaker => 'Выбрать динамик';

  @override
  String get devicesChooseCamera => 'Выбрать камеру';

  @override
  String get devicesNoCamera => 'Камера не подключена.';

  @override
  String get devicesCameraInCall => 'Камера уже используется в звонке.';

  @override
  String devicesCameraFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Не удалось использовать камеру. Разрешите доступ в разделе «Системные настройки › Конфиденциальность и безопасность».',
      'other': 'Не удалось использовать камеру.',
    });
    return '$_temp0';
  }

  @override
  String get devicesCameraDenied =>
      'Система не разрешает Fogata использовать камеру. Разрешите это в настройках системы и попробуйте снова.';

  @override
  String get devicesCameraUnavailable =>
      'Нет доступа к камере: в этой системе нет портала камеры.';

  @override
  String get devicesPreviewOnlyYou => 'Предпросмотр видите только вы.';

  @override
  String get devicesStopVideo => 'Остановить видео';

  @override
  String get devicesTestVideo => 'Проверить видео';

  @override
  String get devicesNoiseOn => 'Шумоподавление включено';

  @override
  String get devicesNoiseOff => 'Шумоподавление выключено';

  @override
  String get devicesNoise => 'Шумоподавление';

  @override
  String get devicesNoiseNote =>
      'Снижает постоянный фоновый шум, например гул вентилятора, в том, что слышат другие.';

  @override
  String get devicesNoiseShort => 'Убирает гул вентилятора и фон.';

  @override
  String get devicesMirror => 'Зеркалить мою камеру';

  @override
  String get devicesMirrorNote =>
      'Меняет только то, как вы видите себя здесь. Другие всегда видят вашу камеру как есть.';

  @override
  String get devicesMirrorOn => 'Зеркальная камера включена';

  @override
  String get devicesMirrorOff => 'Зеркальная камера выключена';

  @override
  String get devicesTestMic => 'Проверить микрофон';

  @override
  String get devicesStopMicTest => 'Остановить проверку';

  @override
  String get devicesMicLevel => 'Уровень микрофона';

  @override
  String get devicesHearMyself => 'Слышать себя';

  @override
  String get devicesHearMyselfNote =>
      'Воспроизводит микрофон через динамик. Используйте наушники, иначе звук динамика может снова попасть в микрофон.';

  @override
  String get devicesHearMyselfOn => 'Слышать себя: включено';

  @override
  String get devicesHearMyselfOff => 'Слышать себя: выключено';

  @override
  String get devicesMicTestOnlyYou =>
      'Ничего из проверки никому не отправляется.';

  @override
  String get devicesMicInCall => 'Микрофон занят вашим звонком.';

  @override
  String get devicesNoMic => 'Микрофон не подключён.';

  @override
  String get devicesSystemDefault => 'По умолчанию';

  @override
  String get devicesNotConnected =>
      'Не подключено: используется устройство по умолчанию';

  @override
  String get devicesNoCameraShort => 'Камера не подключена';

  @override
  String get devicesMicOptions => 'Настройки микрофона';

  @override
  String get devicesSpeakerOptions => 'Настройки динамика';

  @override
  String get devicesCameraOptions => 'Настройки камеры';

  @override
  String get devicesInputDevice => 'Устройство ввода';

  @override
  String get devicesOutputDevice => 'Устройство вывода';

  @override
  String get devicesChosenMissing =>
      'Выбранное устройство не подключено: используется устройство по умолчанию.';

  @override
  String get devicesVoiceSettings => 'Настройки голоса и видео';

  @override
  String get photoImages => 'Изображения';

  @override
  String photoTooLarge(int n) {
    return 'Изображение слишком большое: максимум $n Мп.';
  }

  @override
  String get photoUnreadable => 'Не удалось прочитать изображение.';

  @override
  String get callSource => 'Исходное';

  @override
  String callQuality(String resolution, int fps) {
    return '$resolution · $fps кадр/с';
  }

  @override
  String get callSetupFailed => 'Не удалось начать звонок.';

  @override
  String get callNoSoundSystem =>
      'Не удалось подключиться к звуковой системе этого компьютера, поэтому звонок не может начаться.';

  @override
  String callMicGone(String name) {
    return 'Ваш микрофон отключён: сейчас используется $name.';
  }

  @override
  String get callMicBack => 'Микрофон снова подключён.';

  @override
  String callMicSilent(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Не удалось использовать микрофон. Проверьте, что он подключён и разрешён в разделе «Системные настройки › Конфиденциальность и безопасность».',
      'windows': 'Не удалось использовать микрофон. Проверьте, что он подключён и разрешён в разделе «Параметры › Конфиденциальность и защита › Микрофон».',
      'other': 'Не удалось использовать микрофон. Проверьте, что он подключён.',
    });
    return '$_temp0';
  }

  @override
  String get callNoMicrophone => 'Микрофон не подключён.';

  @override
  String callMicFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Не удалось использовать микрофон. Разрешите доступ в разделе «Системные настройки › Конфиденциальность и безопасность».',
      'windows': 'Не удалось использовать микрофон. Разрешите доступ в разделе «Параметры › Конфиденциальность и защита › Микрофон».',
      'other': 'Не удалось использовать микрофон.',
    });
    return '$_temp0';
  }

  @override
  String callScreenFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Не удалось показать экран. Разрешите запись экрана в разделе «Системные настройки › Конфиденциальность и безопасность».',
      'other': 'Не удалось показать экран.',
    });
    return '$_temp0';
  }

  @override
  String get callShareLost =>
      'Показ остановлен: то, что вы показывали, больше нельзя захватить.';

  @override
  String callKeptOut(String name) {
    return 'Связь потеряна: $name.';
  }

  @override
  String get qualityGood => 'Хорошее';

  @override
  String get qualityOkay => 'Среднее';

  @override
  String get qualityPoor => 'Плохое';

  @override
  String get qualityLost => 'Потеряно';

  @override
  String qualityDelay(int ms) {
    return '$ms мс';
  }

  @override
  String qualityPercent(String value) {
    return '$value%';
  }

  @override
  String get qualityAnotherMember => 'другого участника';

  @override
  String qualityLostHint(String name, int seconds) {
    return '$name: ничего не приходит уже $seconds с. Fogata продолжает поиск; связь восстановится сама, как только заработает сеть собеседника.';
  }

  @override
  String qualityBothPoor(String name) {
    return 'Сейчас обе сети работают плохо: вы и $name можете слышать друг друга с прерываниями.';
  }

  @override
  String qualityTheirSide(String name) {
    return '$name: проблема на той стороне, все слышат этого человека с прерываниями. Там сейчас плохо работает Wi-Fi или мобильный интернет.';
  }

  @override
  String qualityYourSideWith(String name) {
    return 'Проблема на вашей стороне: $name и остальные могут слышать вас с прерываниями. Ваш Wi-Fi или мобильный интернет сейчас работает плохо.';
  }

  @override
  String qualityTheirsSlow(String name) {
    return '$name: сеть сейчас немного медленная, все могут иногда слышать этого человека с прерываниями.';
  }

  @override
  String qualityAllGoodWith(String name) {
    return 'Вы и $name хорошо слышите друг друга.';
  }

  @override
  String qualityMineSlowWith(String name) {
    return 'Ваша сеть сейчас немного медленная: $name может иногда слышать вас с прерываниями.';
  }

  @override
  String qualityWayThrough(String name, String carrier) {
    return '$name: собственная сеть в порядке; связь между вами медленнее, потому что идёт через $carrier. Вы можете иногда слышать этого человека с прерываниями, остальные слышат хорошо.';
  }

  @override
  String qualityWaySlowFine(String name) {
    return '$name: собственная сеть в порядке; связь между вами сейчас медленная. Вы можете иногда слышать этого человека с прерываниями, остальные слышат хорошо.';
  }

  @override
  String qualityWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          '$name: связь между вами сейчас немного медленная, дело может быть в любой из сетей. Вы можете иногда слышать друг друга с прерываниями.',
      'other':
          '$name: связь между вами сейчас плохая, дело может быть в любой из сетей. Вы можете слышать друг друга с прерываниями.',
    });
    return '$_temp0';
  }

  @override
  String qualityWayVia(String level, String name, String carrier) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          '$name: связь между вами сейчас немного медленная, дело может быть в любой из сетей. Вы можете иногда слышать друг друга с прерываниями. Связь идёт через $carrier.',
      'other':
          '$name: связь между вами сейчас плохая, дело может быть в любой из сетей. Вы можете слышать друг друга с прерываниями. Связь идёт через $carrier.',
    });
    return '$_temp0';
  }

  @override
  String get qualityYourSide =>
      'Проблема на вашей стороне: остальные могут слышать вас с прерываниями. Ваш Wi-Fi или мобильный интернет сейчас работает плохо.';

  @override
  String get qualityMineSlow =>
      'Ваша сеть сейчас немного медленная: остальные могут иногда слышать вас с прерываниями.';

  @override
  String qualityLostShort(String name, int seconds) {
    return '$name: ничего не приходит уже $seconds с. Fogata продолжает поиск.';
  }

  @override
  String qualityAllWellWith(String name) {
    return 'Всё в порядке: вы и $name хорошо слышите друг друга.';
  }

  @override
  String qualityNoteDropped(String name) {
    return '$name: связь прервалась, Fogata ищет снова';
  }

  @override
  String qualityNoteTheirsPoor(String name) {
    return '$name: сеть плохая, поэтому все слышат с прерываниями';
  }

  @override
  String qualityNoteTheirsSlow(String name) {
    return '$name: сеть немного медленная';
  }

  @override
  String qualityNoteThrough(String name, String carrier) {
    return '$name: связь идёт через $carrier';
  }

  @override
  String qualityNoteWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': '$name: связь сейчас немного медленная',
      'other': '$name: связь сейчас плохая',
    });
    return '$_temp0';
  }

  @override
  String get qualityNoteSeparator => '; ';

  @override
  String get qualityAllWell =>
      'Всё в порядке: вы слышите всех, и все слышат вас.';

  @override
  String qualityAllWellMine(String notes) {
    return 'У вас всё в порядке. $notes.';
  }

  @override
  String qualityYourDetails(String level) {
    return 'Ваше соединение: $level. Показать подробности';
  }

  @override
  String qualityTheirDetails(String name, String level) {
    return 'Соединение ($name): $level. Показать подробности';
  }

  @override
  String get qualityReconnecting => 'Переподключение…';

  @override
  String get qualityReconnectingLabel => 'Переподключение';

  @override
  String qualityLevelConnection(String level) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'good': 'Хорошее соединение',
      'okay': 'Среднее соединение',
      'weak': 'Плохое соединение',
      'other': 'Соединение потеряно',
    });
    return '$_temp0';
  }

  @override
  String get qualityPoorConnection => 'Плохое соединение';

  @override
  String get qualityMinePoorBanner =>
      'Ваша сеть работает плохо: остальные могут слышать вас с прерываниями';

  @override
  String get qualityYourNetwork => 'Ваша сеть';

  @override
  String qualityTo(String name) {
    return 'Вы → $name';
  }

  @override
  String qualityBetween(String name) {
    return 'Вы ↔ $name';
  }

  @override
  String qualityTheirNetwork(String name) {
    return '$name: собственная сеть';
  }

  @override
  String get qualityLastHeard => 'Последний сигнал';

  @override
  String qualitySecondsAgo(int n) {
    return '$n с назад';
  }

  @override
  String get qualityPath => 'Маршрут';

  @override
  String get qualityLookingAgain => 'Повторный поиск';

  @override
  String get qualityDelayLabel => 'Задержка';

  @override
  String get qualityLostPackets => 'Потерянные пакеты';

  @override
  String get qualityYourConnection => 'Ваше соединение';

  @override
  String qualityTheirConnection(String name) {
    return 'Соединение: $name';
  }

  @override
  String roomCountFullLabel(int count, int max) {
    return 'Заполнено, занято мест: $count из $max';
  }

  @override
  String roomCountLabel(int count, int max) {
    return 'Занято мест: $count из $max';
  }

  @override
  String get roomCountFullPrefix => 'Заполнено · ';

  @override
  String get roomCountFullWord => 'Заполнено ';

  @override
  String get volumeVoice => 'Громкость голоса';

  @override
  String get volumeOnlyYouHear => 'Эти изменения слышите только вы';

  @override
  String get volumeScreenSound => 'Звук экрана';

  @override
  String volumePercent(int n) {
    return '$n%';
  }

  @override
  String get volumeMuteVoiceForMe => 'Заглушить голос для меня';

  @override
  String get volumeMuteForMe => 'Заглушить для меня';

  @override
  String get volumeMuteScreenForMe => 'Заглушить звук экрана для меня';

  @override
  String get volumeBackTo100 => 'Вернуть 100%';

  @override
  String get volumeBelowYou => 'НИЖЕ ВАС';

  @override
  String volumeRemoveFrom(String room) {
    return 'Удалить из комнаты $room';
  }

  @override
  String volumeRemoveTitle(String name, String room) {
    return 'Удалить участника $name из комнаты $room?';
  }

  @override
  String volumeRemoveText(String name, String room) {
    return '$name сразу покинет комнату $room, но сможет тут же вернуться. Тайм-аут не пускает человека обратно какое-то время.';
  }

  @override
  String volumeTimeOutOf(String room) {
    return 'Временно отстранить от комнаты $room';
  }

  @override
  String volumeTimeOutText(String name, String room) {
    return '$name покинет комнату $room и не сможет вернуться:';
  }

  @override
  String get volumeUntilLetBack => 'Пока кто-нибудь выше не разрешит вернуться';

  @override
  String get pipYourScreen => 'Ваш экран';

  @override
  String pipScreenOf(String name) {
    return 'Экран: $name';
  }

  @override
  String get pipYourCamera => 'Ваша камера';

  @override
  String get pipBackToCall => 'Вернуться к звонку';

  @override
  String get pipBackToRoom => 'Вернуться в комнату';

  @override
  String get pipStopWatching => 'Перестать смотреть';

  @override
  String get addToCallTitle => 'Добавить в звонок';

  @override
  String addToCallNote(int n) {
    return 'Друзья в сети. Максимум участников звонка: $n.';
  }

  @override
  String get addToCallSearch => 'Поиск друзей';

  @override
  String get addToCallNoFriends => 'Друзей пока нет.';

  @override
  String get addToCallNoMatch => 'Нет друзей с таким именем.';

  @override
  String get addToCallInCall => 'В звонке';

  @override
  String get addToCallCalling => 'Вызов…';

  @override
  String get addToCallOffline => 'Не в сети';

  @override
  String get addToCallCall => 'Позвонить';

  @override
  String get streamQualityTitle => 'Качество трансляции';

  @override
  String get streamQualityNoteSharing =>
      'Как ваш экран видят другие. Изменения применяются сразу.';

  @override
  String get streamQualityNote =>
      'Как ваш экран увидят другие, когда вы его покажете.';

  @override
  String get streamQualityResolution => 'Разрешение';

  @override
  String streamQualitySourceLines(int lines) {
    return 'Исходное (${lines}p)';
  }

  @override
  String get streamQualitySource => 'Исходное';

  @override
  String get streamQualityFrameRate => 'Частота кадров';

  @override
  String streamQualityFps(int n) {
    return '$n кадр/с';
  }

  @override
  String get streamQualityHigherNote =>
      'Чем выше, тем больше нагрузка на исходящий канал и компьютер.';

  @override
  String get streamQualitySomeone => 'Кто-то';

  @override
  String streamQualityLoweredOne(String name, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Остальные получают выбранное вами качество.',
      'other': '',
    });
    return '$name: соединение сейчас не тянет больше, поэтому изображение уменьшено.$_temp0';
  }

  @override
  String streamQualityLoweredMany(String names, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' Остальные получают выбранное вами качество.',
      'other': '',
    });
    return 'Некоторые соединения сейчас не тянут больше: $names получают уменьшенное изображение.$_temp0';
  }

  @override
  String get streamQualitySpared =>
      'Чтобы не перегружать ваш компьютер, сейчас все зрители получают изображение меньшего размера или с меньшей частотой кадров.';

  @override
  String get streamQualityGoingOut => 'СЕЙЧАС ОТПРАВЛЯЕТСЯ';

  @override
  String get streamQualityNobody => 'Пока никто не смотрит.';

  @override
  String streamQualityNow(int height, int fps) {
    return '${height}p · $fps кадр/с';
  }

  @override
  String callLayersIsCalling(String name) {
    return '$name звонит';
  }

  @override
  String get callLayersGoToCall => 'F6 — перейти к звонку';

  @override
  String callLayersWith(String names) {
    return 'В звонке: $names';
  }

  @override
  String get callLayersDirectCall => 'Личный звонок · ';

  @override
  String get callLayersAnswerMicOff => 'Ответить с выключенным микрофоном';

  @override
  String callLayersAnsweringLeaves(String room) {
    return 'Ответив, вы выйдете из комнаты $room';
  }

  @override
  String get callLayersLeaveAndAnswer => 'Выйти и ответить';

  @override
  String callLayersBothIn(String room) {
    return 'Вы оба в комнате $room.';
  }

  @override
  String get callLayersDismiss => 'Скрыть';

  @override
  String callLayersCallWith(String names) {
    return 'Звонок: $names';
  }

  @override
  String get callLayersRoom => 'Комната';

  @override
  String callLayersBackTo(String title) {
    return 'Вернуться: $title';
  }

  @override
  String callLayersRinging(String time) {
    return 'Вызов · $time';
  }

  @override
  String callLayersInCall(String time) {
    return 'В звонке · $time';
  }

  @override
  String callLayersOnlyYou(String time) {
    return 'Только вы · $time';
  }

  @override
  String callLayersInRoom(int n, String time) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n человека в комнате',
      many: '$n человек в комнате',
      few: '$n человека в комнате',
      one: '$n человек в комнате',
    );
    return '$_temp0 · $time';
  }

  @override
  String get callLayersMute => 'Выключить микрофон';

  @override
  String get callLayersUnmute => 'Включить микрофон';

  @override
  String get callLayersHangUp => 'Завершить звонок';

  @override
  String get callLayersLeaveRoom => 'Выйти из комнаты';

  @override
  String roomDeclineCallTitle(String name) {
    return 'Отклонить входящий звонок ($name)?';
  }

  @override
  String roomDeclineCallText(String caller, String callee) {
    return '$caller звонит вам. Одновременно можно быть только в одном звонке: если позвонить пользователю $callee, входящий звонок будет отклонён.';
  }

  @override
  String roomDeclineCallTextSomeone(String caller) {
    return '$caller звонит вам. Одновременно можно быть только в одном звонке: если позвонить кому-то другому, входящий звонок будет отклонён.';
  }

  @override
  String get roomDeclineAndCall => 'Отклонить и позвонить';

  @override
  String roomHangUpTitle(String name) {
    return 'Завершить звонок с пользователем $name?';
  }

  @override
  String get roomHangUpText =>
      'Одновременно можно быть только в одном звонке или комнате: если продолжить, звонок завершится.';

  @override
  String get roomHangUp => 'Завершить звонок';

  @override
  String get roomCloseTitle => 'Выйти и закрыть комнату?';

  @override
  String get roomCloseText =>
      'Кроме вас, здесь никого нет. Когда вы выйдете, комната закроется, а всё сказанное в ней исчезнет с этого компьютера; то, что скачали другие, останется у них.';

  @override
  String get roomCloseChatMessages => 'Сообщения чата';

  @override
  String get roomCloseFilesShared => 'Ваши файлы';

  @override
  String get roomStay => 'Остаться';

  @override
  String get roomLeaveAndClose => 'Выйти и закрыть';

  @override
  String get roomLivePrefix => 'В эфире · ';

  @override
  String get roomPillConnecting => 'Подключение';

  @override
  String get roomPillOnlyYou => 'Только вы';

  @override
  String get roomPillError => 'Ошибка';

  @override
  String get roomLastOne => 'Кроме вас, в комнате никого не осталось.';

  @override
  String get roomOnlyYouIn => 'В комнате только вы.';

  @override
  String get roomAloneWarning =>
      ' Когда вы выйдете, она закроется и всё в ней исчезнет.';

  @override
  String get roomEarlierPeople => 'Предыдущие участники';

  @override
  String get roomMorePeople => 'Следующие участники';

  @override
  String roomTileYou(String name) {
    return '$name (вы)';
  }

  @override
  String roomTileNoSound(String name) {
    return '$name · без звука';
  }

  @override
  String roomTileConnecting(String name) {
    return '$name · подключение';
  }

  @override
  String roomTileAway(String name) {
    return '$name · нет связи';
  }

  @override
  String roomTileMicOff(String name) {
    return '$name, микрофон выключен';
  }

  @override
  String roomTileMutedByYou(String said) {
    return '$said, без звука для вас';
  }

  @override
  String roomTileVolume(String said, String percent) {
    return '$said, громкость $percent';
  }

  @override
  String roomTileLeft(String name) {
    return '$name · вне звонка, может вернуться';
  }

  @override
  String roomIsSharing(String name) {
    return '$name показывает экран';
  }

  @override
  String get roomNothingSentUntilWatch =>
      'Пока вы не начнёте смотреть, вам ничего не передаётся';

  @override
  String get roomWatch => 'Смотреть';

  @override
  String get roomLive => 'ЭФИР';

  @override
  String get roomSharingYourScreen => 'Вы показываете свой экран';

  @override
  String get roomShowPreview => 'Показать предпросмотр';

  @override
  String get roomPreviewMirror =>
      'Если показать его, пока вы делитесь самим Fogata, получится бесконечное зеркало';

  @override
  String get roomNobodyWatching => 'Никто не смотрит';

  @override
  String roomPeopleWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n человека смотрят',
      many: '$n человек смотрят',
      few: '$n человека смотрят',
      one: '$n человек смотрит',
    );
    return '$_temp0';
  }

  @override
  String get roomExitFullScreen => 'Выйти из полноэкранного режима';

  @override
  String get roomFullScreen => 'Во весь экран';

  @override
  String get roomPlayScreenSound => 'Включить звук экрана';

  @override
  String get roomMuteScreenSound => 'Выключить звук экрана';

  @override
  String get roomQualityPoor => 'Плохо';

  @override
  String get roomQualityOkay => 'Средне';

  @override
  String get roomQualityGood => 'Хорошо';

  @override
  String get roomQualityPoorTitle => 'Плохое соединение';

  @override
  String get roomQualityOkayTitle => 'Соединение так себе';

  @override
  String get roomQualityGoodTitle => 'Хорошее соединение';

  @override
  String get roomQualityMeasuring => 'Замер';

  @override
  String get roomQualityDelay => 'Задержка';

  @override
  String roomQualityMs(String n) {
    return '$n мс';
  }

  @override
  String get roomQualityLost => 'Потерянные пакеты';

  @override
  String roomQualityPercent(String n) {
    return '$n%';
  }

  @override
  String get roomQualityPicture => 'Изображение';

  @override
  String get roomQualityPath => 'Маршрут';

  @override
  String get roomQualityMayFreeze =>
      'Изображение может зависать. Помогает, когда экран одновременно показывают меньше людей.';

  @override
  String get roomQualityMayShrink =>
      'Изображение может уменьшаться, чтобы не прерываться.';

  @override
  String roomCallingName(String name) {
    return 'Вызов: $name…';
  }

  @override
  String get roomRingsFor30 =>
      'Звонок на компьютере собеседника длится до 30 секунд.';

  @override
  String get roomReturnToCall => 'Вернуться в звонок';

  @override
  String get roomCallAgain => 'Позвонить снова';

  @override
  String get roomAloneHint =>
      'Когда кто-нибудь из группы войдёт, он появится здесь.';

  @override
  String roomAlreadyLeft(String names) {
    return 'Уже вышли: $names.';
  }

  @override
  String get roomGroupFound => 'Группа найдена в сети';

  @override
  String get roomLookingForRoute => 'поиск лучшего маршрута…';

  @override
  String get roomConnectingTitle => 'Подключение к комнате';

  @override
  String roomSeconds(String n) {
    return '$n с';
  }

  @override
  String roomReachedOf(int reached, int total, String time) {
    return '$reached из $total · $time';
  }

  @override
  String get roomPathLan => 'локальная сеть, с шифрованием';

  @override
  String get roomPathDirect => 'напрямую, с шифрованием';

  @override
  String get roomPathThroughMember => 'через участника, с шифрованием';

  @override
  String roomPathThrough(String name) {
    return 'через $name, с шифрованием';
  }

  @override
  String get roomCouldNotConnect => 'Не удалось подключиться к комнате';

  @override
  String get roomCouldNotConnectText =>
      'Никто не ответил вовремя. Возможно, ваша сеть блокирует прямые соединения между устройствами.';

  @override
  String get roomNoAnswer => 'нет ответа';

  @override
  String get roomSeeNetworkCheck => 'Открыть проверку сети →';

  @override
  String get roomGoesNothing => 'Комната закроется для всех';

  @override
  String roomGoesMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n сообщения исчезнут',
      many: '$n сообщений исчезнут',
      few: '$n сообщения исчезнут',
      one: '$n сообщение исчезнет',
    );
    return '$_temp0';
  }

  @override
  String roomGoesFiles(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n вашего файла исчезнут',
      many: '$n ваших файлов исчезнут',
      few: '$n ваших файла исчезнут',
      one: '$n ваш файл исчезнет',
    );
    return '$_temp0';
  }

  @override
  String roomGoesBoth(int messages, int files) {
    String _temp0 = intl.Intl.pluralLogic(
      messages,
      locale: localeName,
      other: '$messages сообщения',
      many: '$messages сообщений',
      few: '$messages сообщения',
      one: '$messages сообщение',
    );
    String _temp1 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files файла',
      many: '$files файлов',
      few: '$files файла',
      one: '$files файл',
    );
    return '$_temp0 и $_temp1 исчезнут';
  }

  @override
  String get roomMuteMicrophone => 'Выключить микрофон';

  @override
  String get roomUnmuteMicrophone => 'Включить микрофон';

  @override
  String get roomStopSharingScreen => 'Остановить показ экрана';

  @override
  String get roomShareScreen => 'Показать экран';

  @override
  String get roomUndeafen => 'Снова слышать других';

  @override
  String get roomDeafen => 'Выключить звук';

  @override
  String get roomCameraOff => 'Выключить камеру';

  @override
  String get roomCameraOn => 'Включить камеру';

  @override
  String get roomLeave => 'Выйти';

  @override
  String get roomSoundWaitsForMic =>
      'Звук экрана передаётся, только когда микрофон включён';

  @override
  String get roomStopSharingSound => 'Не передавать звук';

  @override
  String get roomShareSoundToo => 'Передавать и звук';

  @override
  String get roomSoundNeedsPipeWire =>
      'С этого компьютера нельзя передавать звук: для этого нужен PipeWire, а он здесь не запущен. Передаётся только изображение.';

  @override
  String get roomAddFriendToCall => 'Добавить друга в звонок';

  @override
  String get roomHideChat => 'Скрыть чат';

  @override
  String get roomShowChat => 'Показать чат';

  @override
  String get roomTabChat => 'Чат';

  @override
  String get roomTabFiles => 'Файлы';

  @override
  String roomNoteLeftCall(String name) {
    return '$name — выход из звонка · ';
  }

  @override
  String roomNoteJoinedCall(String name) {
    return '$name — вход в звонок · ';
  }

  @override
  String roomNoteLeftRoom(String name) {
    return '$name — выход из комнаты · ';
  }

  @override
  String roomNoteEnteredRoom(String name) {
    return '$name — вход в комнату · ';
  }

  @override
  String get roomSearchMessages => 'Поиск сообщений';

  @override
  String get roomClearSearch => 'Очистить поиск';

  @override
  String get roomChatGoneCall => 'Этот чат исчезнет, когда звонок закончится';

  @override
  String get roomChatGoneRoom => 'Этот чат исчезнет, когда комната закроется';

  @override
  String roomNoMatches(String text) {
    return 'Нет сообщений, содержащих «$text»';
  }

  @override
  String roomMatches(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n совпадения',
      many: '$n совпадений',
      few: '$n совпадения',
      one: '$n совпадение',
    );
    return '$_temp0';
  }

  @override
  String get roomReceivingMessages => 'Получение сообщений комнаты…';

  @override
  String roomReceivingMessagesFrom(String name) {
    return 'Получение сообщений комнаты с компьютера пользователя $name…';
  }

  @override
  String get roomSomeone => 'Кто-то';

  @override
  String get roomShareFiles => 'Поделиться файлами';

  @override
  String get roomAddMoreFiles => 'Добавить ещё файлы';

  @override
  String roomMessageTo(String name) {
    return 'Сообщение для $name';
  }

  @override
  String get roomMessageTheRoom => 'Сообщение в комнату';

  @override
  String get roomMessageTheCall => 'Сообщение в звонок';

  @override
  String get roomAddMessageOptional => 'Добавьте сообщение (необязательно)';

  @override
  String get roomMessageLabel => 'Сообщение';

  @override
  String roomEnterToSend(String key) {
    return '$key — отправить';
  }

  @override
  String roomEscToRemove(String key) {
    return '$key — убрать их';
  }

  @override
  String roomOrTwo(String first, String second) {
    return '$first или $second';
  }

  @override
  String roomOrMore(String rest, String last) {
    return '$rest или $last';
  }

  @override
  String roomFileNotAvailable(String name) {
    return 'Недоступно · $name нет в комнате';
  }

  @override
  String get roomSharingFromHere => 'Раздаётся с этого компьютера';

  @override
  String roomAvailableWhile(String names) {
    return 'Доступно, пока в комнате $names';
  }

  @override
  String roomSizeDownloads(String size, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n скачивания',
      many: '$n скачиваний',
      few: '$n скачивания',
      one: '$n скачивание',
    );
    return '$size · $_temp0';
  }

  @override
  String roomDownloadFile(String file) {
    return 'Скачать $file';
  }

  @override
  String roomShowInFolder(String file) {
    return 'Показать $file в папке';
  }

  @override
  String roomGettingFile(String file) {
    return 'Получение: $file…';
  }

  @override
  String get roomSaveToDownloads => 'Сохранить в «Загрузки»';

  @override
  String roomPictureKept(String name) {
    return '$name нет в комнате · изображение хранится здесь, пока вы не выйдете';
  }

  @override
  String get roomSaveFile => 'Сохранить';

  @override
  String get roomClosePicture => 'Закрыть изображение';

  @override
  String get roomFilesNote =>
      'Файлы никуда не загружаются: они передаются прямо с компьютера того, кто ими поделился, и только пока он в комнате.';

  @override
  String roomFilesAvailable(int n) {
    return 'Доступны · $n';
  }

  @override
  String roomFilesGone(int n) {
    return 'Больше недоступны · $n';
  }

  @override
  String roomPersonLeft(String name) {
    return '$name — выход из комнаты';
  }

  @override
  String get roomPickOrDrop => 'Выберите их здесь или перетащите в комнату';

  @override
  String get roomFileYours => 'Ваш';

  @override
  String roomFileFrom(String name) {
    return 'От: $name';
  }

  @override
  String roomDownloads(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n скачивания',
      many: '$n скачиваний',
      few: '$n скачивания',
      one: '$n скачивание',
    );
    return '$_temp0';
  }

  @override
  String get roomStopSharingFile => 'Стоп';

  @override
  String get roomWhatToShare => 'Что вы хотите показать?';

  @override
  String roomScreenSource(String name) {
    return 'Экран: $name';
  }

  @override
  String roomPlayVideo(String file) {
    return 'Воспроизвести $file';
  }

  @override
  String get roomCloseVideo => 'Закрыть видео';

  @override
  String get roomVideoGone => 'Этого видео больше нет в комнате.';

  @override
  String get roomVideoNotFetched => 'Не удалось получить это видео.';

  @override
  String get roomVideoCannotPlay =>
      'Этот компьютер не может воспроизвести это видео.';

  @override
  String roomGettingVideoPercent(int n) {
    return 'Получение видео · $n%';
  }

  @override
  String get roomGettingVideo => 'Получение видео…';

  @override
  String get roomPause => 'Пауза';

  @override
  String get roomPlay => 'Воспроизвести';

  @override
  String get roomSoundOn => 'Включить звук';

  @override
  String get roomMuteVideo => 'Выключить звук';

  @override
  String get roomLeaveFullScreen => 'Выйти из полноэкранного режима';

  @override
  String roomCallWaits(String time) {
    return 'Когда в звонке остаётся один человек, звонок ждёт остальных ещё $time';
  }

  @override
  String roomCallWaitsAlone(String time) {
    return 'Вы остались одни. Звонок ждёт остальных ещё $time';
  }

  @override
  String roomOpenFile(String file) {
    return 'Открыть $file';
  }

  @override
  String activityCreated(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы создали группу «$name»',
      'other': 'Участник $by создал группу «$name»',
    });
    return '$_temp0';
  }

  @override
  String activityRenamed(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы переименовали группу в «$name»',
      'other': 'Участник $by переименовал группу в «$name»',
    });
    return '$_temp0';
  }

  @override
  String activityIcon(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы изменили значок группы',
      'other': 'Участник $by изменил значок группы',
    });
    return '$_temp0';
  }

  @override
  String activityPictureRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы удалили изображение группы',
      'other': 'Участник $by удалил изображение группы',
    });
    return '$_temp0';
  }

  @override
  String activityPicture(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы изменили изображение группы',
      'other': 'Участник $by изменил изображение группы',
    });
    return '$_temp0';
  }

  @override
  String activityBannerRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы удалили баннер группы',
      'other': 'Участник $by удалил баннер группы',
    });
    return '$_temp0';
  }

  @override
  String activityBanner(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы изменили баннер группы',
      'other': 'Участник $by изменил баннер группы',
    });
    return '$_temp0';
  }

  @override
  String activityAdded(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы добавили',
      'other': 'Участник $by добавил',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'вас',
      'other': 'участника $member',
    });
    return '$_temp0 $_temp1';
  }

  @override
  String activityJoined(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы вступили',
      'other': 'Участник $by вступил',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'вашему приглашению',
      'other': 'приглашению участника $member',
    });
    return '$_temp0 по $_temp1';
  }

  @override
  String activityLetIn(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы впустили',
      'other': 'Участник $by впустил',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'вас',
      'other': 'участника $member',
    });
    return '$_temp0 $_temp1';
  }

  @override
  String activityRemoved(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы удалили',
      'other': 'Участник $by удалил',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'вас',
      'other': 'участника $member',
    });
    return '$_temp0 $_temp1';
  }

  @override
  String activityLeft(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы покинули группу',
      'other': 'Участник $by покинул группу',
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
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы переместили',
      'other': 'Участник $by переместил',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'вас',
      'other': 'участника $member',
    });
    return '$_temp0 $_temp1 выше, на позицию $place';
  }

  @override
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы переместили',
      'other': 'Участник $by переместил',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'вас',
      'other': 'участника $member',
    });
    return '$_temp0 $_temp1 в конец';
  }

  @override
  String activityManagersAll(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы разрешили',
      'other': 'Участник $by разрешил',
    });
    return '$_temp0 всем управлять группой';
  }

  @override
  String activityManagersFirst(String by, String byMe, int n) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы оставили',
      'other': 'Участник $by оставил',
    });
    String _temp1 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n',
      many: '$n',
      few: '$n',
      one: '$n',
    );
    return '$_temp0 управление группой только первым в списке ($_temp1)';
  }

  @override
  String activityLimits(String by, String byMe, int people, int perRoom) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы установили',
      'other': 'Участник $by установил',
    });
    return '$_temp0 максимум людей: в группе — $people, в каждой голосовой комнате — $perRoom';
  }

  @override
  String activityRoomOpened(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы открыли',
      'other': 'Участник $by открыл',
    });
    return '$_temp0 голосовую комнату «$room»';
  }

  @override
  String activityRoomRenamed(String by, String byMe, String from, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы переименовали',
      'other': 'Участник $by переименовал',
    });
    return '$_temp0 голосовую комнату «$from» в «$room»';
  }

  @override
  String activityRoomClosed(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы удалили',
      'other': 'Участник $by удалил',
    });
    return '$_temp0 голосовую комнату «$room»';
  }

  @override
  String activityInvited(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы создали',
      'other': 'Участник $by создал',
    });
    return '$_temp0 ссылку-приглашение';
  }

  @override
  String activityRevoked(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы отменили',
      'other': 'Участник $by отменил',
    });
    return '$_temp0 ссылку-приглашение';
  }

  @override
  String activityRoomOpenToAll(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы открыли',
      'other': 'Участник $by открыл',
    });
    return '$_temp0 комнату «$room» для всех';
  }

  @override
  String activityRoomForAbove(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы оставили',
      'other': 'Участник $by оставил',
    });
    return '$_temp0 комнату «$room» только для тех, кто выше';
  }

  @override
  String activityRoomOnly(String by, String byMe, String people, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {
      'yes': 'Вы разрешили',
      'other': 'Участник $by разрешил',
    });
    return '$_temp0 вход в комнату «$room» только: $people';
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
      'yes': 'Вы отстранили',
      'other': 'Участник $by отстранил',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'вас',
      'other': 'участника $member',
    });
    return '$_temp0 $_temp1 от комнаты «$room», пока не разрешат вернуться';
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
      'yes': 'Вы отстранили',
      'other': 'Участник $by отстранил',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'вас',
      'other': 'участника $member',
    });
    String _temp2 = intl.Intl.selectLogic(untilKind, {
      'today': 'до $until',
      'todayAtOne': 'до $until',
      'yesterday': 'до $until',
      'other': 'до $until',
    });
    return '$_temp0 $_temp1 от комнаты «$room» $_temp2';
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
      'yes': 'Вы разрешили',
      'other': 'Участник $by разрешил',
    });
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'вам',
      'other': 'участнику $member',
    });
    return '$_temp0 $_temp1 вернуться в комнату «$room»';
  }

  @override
  String get activityDefaultRoom => 'Общая';

  @override
  String get activityYouInList => 'вы';

  @override
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'вы были удалены',
      'other': 'участник $author был удалён',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' вами',
      'other': ' участником $by',
      'none': '',
    });
    return 'Отменено: в тот же момент $_temp0$_temp1';
  }

  @override
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'вы были перемещены',
      'other': 'участник $author был перемещён',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' вами',
      'other': ' участником $by',
      'none': '',
    });
    return 'Отменено: в тот же момент $_temp0 в конец$_temp1';
  }

  @override
  String activityUndoneLeft(String author, String authorMe) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'вы покинули',
      'other': 'участник $author покинул',
    });
    return 'Отменено: в тот же момент $_temp0 группу';
  }

  @override
  String get activityAboutList =>
      'Все в группе видят один и тот же список из собственной записи группы. Изменения изображения и баннера хранятся отдельно: здесь те, что видело это устройство.';

  @override
  String get activityAboutOrder =>
      'В порядке самой группы; время — по часам каждого автора.';

  @override
  String get activityNothingYet => 'Пока ничего.';

  @override
  String get activityLoading => 'Загрузка…';

  @override
  String get activityShowOlder => 'Показать более ранние';

  @override
  String activitySeenHere(String when) {
    return '$when · замечено на этом устройстве';
  }

  @override
  String get bellNotifications => 'Уведомления';

  @override
  String bellNewNotifications(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n нового уведомления',
      many: '$n новых уведомлений',
      few: '$n новых уведомления',
      one: '$n новое уведомление',
    );
    return '$_temp0';
  }

  @override
  String get bellClearAll => 'Очистить все';

  @override
  String get bellNothingNew => 'Ничего нового';

  @override
  String get bellFooter =>
      'Здесь остаются последние три. Пропущенные звонки и запросы также видны в разделе «Друзья».';

  @override
  String bellBothInRoom(String room) {
    return 'Вы оба в комнате $room.';
  }

  @override
  String get bellDismiss => 'Скрыть';

  @override
  String bellFriendRequest(String name) {
    return '$name хочет добавить вас в друзья';
  }

  @override
  String bellCheckFingerprint(String fingerprint) {
    return 'Проверьте, что отпечаток совпадает: $fingerprint';
  }

  @override
  String bellJoinAsk(String name, String group) {
    return '$name просит впустить в группу «$group»';
  }

  @override
  String bellJoinAskNote(String fingerprint) {
    return 'По одной из ссылок-приглашений вашей группы. Отпечаток: $fingerprint';
  }

  @override
  String get bellLetIn => 'Впустить';

  @override
  String get bellSayNo => 'Отказать';

  @override
  String bellGroupInvite(String name, String group) {
    return '$name приглашает вас в группу «$group»';
  }

  @override
  String bellMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n участника',
      many: '$n участников',
      few: '$n участника',
      one: '$n участник',
    );
    return '$_temp0';
  }

  @override
  String get bellJoin => 'Вступить';

  @override
  String bellNewMessages(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$name: $n нового сообщения',
      many: '$name: $n новых сообщений',
      few: '$name: $n новых сообщения',
      one: '$name: $n новое сообщение',
    );
    return '$_temp0';
  }

  @override
  String get bellOpen => 'Открыть';

  @override
  String get bellMarkRead => 'Прочитано';

  @override
  String get bellFirewallTitle => 'Брандмауэр не пускает некоторых друзей';

  @override
  String get bellCopyCommand => 'Копировать команду';

  @override
  String get bellOtherSessionTitle => 'Открыто и на другом компьютере';

  @override
  String get bellSettingsResetTitle => 'Некоторые настройки сброшены';

  @override
  String get bellSettingsResetText =>
      'Некоторые ваши настройки не удалось прочитать, и они вернулись к значениям по умолчанию: проверьте их в настройках. Старый файл сохранён рядом с новым как app.json.bad.';

  @override
  String get friendCardOnline => 'В сети';

  @override
  String get friendCardAway => 'Нет на месте';

  @override
  String get friendCardOffline => 'Не в сети';

  @override
  String get friendCardCallsOnlyOnline =>
      'Звонить можно, только когда друг в сети.';

  @override
  String friendCardHangUpFirst(String name) {
    return 'Чтобы позвонить пользователю $name, сначала завершите текущий звонок.';
  }

  @override
  String friendCardBothInRoom(String room) {
    return 'Вы оба в комнате $room.';
  }

  @override
  String get friendCardMessage => 'Написать';

  @override
  String friendCardMutualFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n общего друга',
      many: '$n общих друзей',
      few: '$n общих друга',
      one: '$n общий друг',
    );
    return '$_temp0';
  }

  @override
  String get memberCardSendRequest => 'Отправить запрос в друзья';

  @override
  String get memberCardRequestSent => 'Запрос в друзья отправлен';

  @override
  String get memberCardAccept => 'Принять запрос в друзья';

  @override
  String memberCardAsksYou(String name) {
    return '$name хочет добавить вас в друзья.';
  }

  @override
  String memberCardInGroup(String group) {
    return 'Участник группы $group';
  }

  @override
  String memberCardOnlyOnline(String name) {
    return 'Запросы в друзья доходят, только пока $name в сети.';
  }

  @override
  String get memberCardShowOwn => 'Показать вашу карточку';

  @override
  String get userPanelEditProfile => 'Изменить профиль';

  @override
  String get userPanelStatus => 'Статус';

  @override
  String get userPanelOnline => 'В сети';

  @override
  String get userPanelAway => 'Нет на месте';

  @override
  String get userPanelOffline => 'Невидимка';

  @override
  String get userPanelOfflineNote =>
      'Друзья и участники ваших групп видят вас не в сети, но заходить в комнаты вы по-прежнему можете.';

  @override
  String get userPanelOnlineNote =>
      'Друзья и участники ваших групп видят вас в сети, а после 10 мин бездействия — «нет на месте».';

  @override
  String get userPanelBio => 'О СЕБЕ';

  @override
  String get userPanelAppearingOffline => 'Режим невидимки';

  @override
  String get userPanelCopyLinkLabel => 'Копировать ссылку для друга';

  @override
  String userPanelLinkExpiresIn(String time) {
    return 'Истекает через $time';
  }

  @override
  String get userPanelLinkTerms => '1 человек · 10 мин';

  @override
  String get userPanelCopyLink => 'Копировать ссылку для друга';

  @override
  String get switcherChatEnded => 'Личный чат · больше не друзья';

  @override
  String get switcherChatOnline => 'Личный чат · в сети';

  @override
  String get switcherChatAway => 'Личный чат · нет на месте';

  @override
  String get switcherChat => 'Личный чат';

  @override
  String switcherFriend(String presence) {
    return 'Друг · $presence';
  }

  @override
  String switcherVoiceRoom(String group) {
    return 'Голосовая комната в группе $group';
  }

  @override
  String switcherVoiceRoomPeople(String group, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Голосовая комната в группе $group · сейчас $n человека',
      many: 'Голосовая комната в группе $group · сейчас $n человек',
      few: 'Голосовая комната в группе $group · сейчас $n человека',
      one: 'Голосовая комната в группе $group · сейчас $n человек',
    );
    return '$_temp0';
  }

  @override
  String switcherGroup(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Группа · $n участника',
      many: 'Группа · $n участников',
      few: 'Группа · $n участника',
      one: 'Группа · $n участник',
    );
    return '$_temp0';
  }

  @override
  String get switcherPlaceholder =>
      'Перейти в чат, группу или голосовую комнату';

  @override
  String get switcherGoTo => 'Перейти';

  @override
  String get switcherEmpty => 'Пока нет групп, друзей или чатов.';

  @override
  String switcherNoMatch(String query) {
    return 'Ничего не найдено по запросу «$query».';
  }

  @override
  String get switcherHintChoose => 'выбрать';

  @override
  String get switcherHintGo => 'перейти';

  @override
  String get switcherHintClose => 'закрыть';

  @override
  String switcherTooltip(String shortcut) {
    return 'Перейти в чат, группу или голосовую комнату ($shortcut)';
  }

  @override
  String get friendsIntro =>
      'Каждая дружба хранится только на двух устройствах. Без каталога, без сервера.';

  @override
  String get friendsTabAll => 'Все';

  @override
  String get friendsTabOnline => 'В сети';

  @override
  String get friendsTabRequests => 'Запросы';

  @override
  String get friendsSearch => 'Поиск друзей';

  @override
  String get friendsNone =>
      'Друзей пока нет. Поделитесь своим QR-кодом, ссылкой или кодом либо воспользуйтесь чужими.';

  @override
  String get friendsNoneOnline => 'Сейчас никого из друзей нет в сети.';

  @override
  String get friendsNoMatch => 'Никого не найдено.';

  @override
  String get friendsInARoom => 'В комнате';

  @override
  String friendsInRoomOf(String group) {
    return 'В комнате · $group';
  }

  @override
  String get friendsOnline => 'В сети';

  @override
  String get friendsAway => 'Нет на месте';

  @override
  String get friendsNotSeenYet => 'В сети пока не видели';

  @override
  String friendsSeen(String when) {
    return 'Последний раз в сети $when';
  }

  @override
  String friendsShowCard(String name) {
    return 'Показать карточку: $name';
  }

  @override
  String friendsMissedCall(String when) {
    return 'Пропущенный звонок · $when';
  }

  @override
  String get friendsMessage => 'Написать';

  @override
  String friendsOfflineNoCall(String name) {
    return '$name не в сети: звонить можно, только когда друг в сети';
  }

  @override
  String friendsHangUpFirst(String name) {
    return 'Сначала завершите звонок, чтобы позвонить пользователю $name';
  }

  @override
  String friendsBothInRoom(String room) {
    return 'Вы оба в комнате $room.';
  }

  @override
  String friendsOptions(String name) {
    return 'Действия: $name';
  }

  @override
  String friendsRemoveTitle(String name) {
    return 'Удалить пользователя $name из друзей?';
  }

  @override
  String get friendsRemoveText =>
      'Вы исчезнете из списков друзей друг друга, без уведомления. В общих группах вы останетесь вместе. Чтобы снова стать друзьями, понадобится новое приглашение.';

  @override
  String get friendsRemove => 'Удалить';

  @override
  String friendsAddToGroup(String group) {
    return 'Добавить в группу «$group»';
  }

  @override
  String get friendsRemoveMenu => 'Удалить из друзей…';

  @override
  String get friendsNoRequests => 'Нет ожидающих запросов.';

  @override
  String friendsRequestIncoming(String when) {
    return 'Хочет добавить вас в друзья · $when';
  }

  @override
  String friendsRequestOutgoing(String when) {
    return 'Отправлено $when · ожидается ответ';
  }

  @override
  String get friendsPendingRequest => 'Запрос в друзья';

  @override
  String friendsCheckFingerprint(String name) {
    return 'Проверьте, что отпечаток совпадает с тем, который $name видит на своём экране.';
  }

  @override
  String get friendsAddTitle => 'Добавить друга';

  @override
  String get friendsAddNote => 'Ваши устройства обмениваются ключами напрямую.';

  @override
  String get friendsTabCode => 'Код';

  @override
  String get friendsTabLink => 'Ссылка';

  @override
  String get friendsTabQr => 'QR';

  @override
  String get friendsMakeInvitation => 'Создать приглашение';

  @override
  String get friendsInvitationExpired =>
      'Срок действия этого приглашения истёк.';

  @override
  String get friendsMakeAnother => 'Создать новое';

  @override
  String friendsHintQr(String time) {
    return 'Отсканируйте в Fogata · истекает через $time';
  }

  @override
  String friendsHintLink(String time) {
    return 'Отправьте одному человеку · истекает через $time';
  }

  @override
  String friendsHintCode(String time) {
    return 'Продиктуйте одному человеку · истекает через $time';
  }

  @override
  String get friendsHiddenCode =>
      'Скрыто на случай, если вы показываете экран.';

  @override
  String get friendsHiddenLink =>
      'Скрыто на случай, если вы показываете экран. Содержит сетевые адреса этого компьютера: передавайте только в личной переписке.';

  @override
  String get friendsGotLink => 'Есть ссылка-приглашение или код?';

  @override
  String get friendsPastePlaceholder => 'Вставьте ссылку или код';

  @override
  String get friendsLinkOrCode => 'Ссылка или код';

  @override
  String get friendsAdd => 'Добавить';

  @override
  String get friendsQrLabel => 'QR-код вашего приглашения';

  @override
  String friendsAddedToGroup(String name) {
    return '$name получит группу, когда вы оба будете в сети.';
  }

  @override
  String get directToday => 'Сегодня';

  @override
  String get directYesterday => 'Вчера';

  @override
  String directTimerOff(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Вы выключили исчезающие сообщения',
      'other': 'Пользователь $name выключил исчезающие сообщения',
    });
    return '$_temp0';
  }

  @override
  String directTimerFirst(String time) {
    return 'Исчезающие сообщения: удаляются через $time';
  }

  @override
  String directTimerOn(String name, String mine, String time) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Вы включили исчезающие сообщения',
      'other': 'Пользователь $name включил исчезающие сообщения',
    });
    return '$_temp0: удаляются через $time';
  }

  @override
  String directPinnedLine(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Вы закрепили сообщение',
      'other': 'Пользователь $name закрепил сообщение',
    });
    return '$_temp0';
  }

  @override
  String get directCallFewSeconds => 'несколько секунд';

  @override
  String directCallMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n минуты',
      many: '$n минут',
      few: '$n минуты',
      one: '$n минуту',
    );
    return '$_temp0';
  }

  @override
  String directCallHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n часа',
      many: '$n часов',
      few: '$n часа',
      one: '$n час',
    );
    return '$_temp0';
  }

  @override
  String directCallHoursMinutes(int hours, int minutes) {
    String _temp0 = intl.Intl.pluralLogic(
      hours,
      locale: localeName,
      other: '$hours часа',
      many: '$hours часов',
      few: '$hours часа',
      one: '$hours час',
    );
    String _temp1 = intl.Intl.pluralLogic(
      minutes,
      locale: localeName,
      other: '$minutes минуты',
      many: '$minutes минут',
      few: '$minutes минуты',
      one: '$minutes минуту',
    );
    return '$_temp0 $_temp1';
  }

  @override
  String directCallLine(String name, String mine, String length) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'Вы начали звонок',
      'other': 'Пользователь $name начал звонок',
    });
    return '$_temp0, который длился $length';
  }

  @override
  String get directBlockedOffline =>
      'Вы в режиме невидимки. Чтобы писать, выйдите в сеть.';

  @override
  String directBlockedFriendOffline(String name) {
    return '$name не в сети. Писать можно, когда вы оба в сети.';
  }

  @override
  String get directOnline => 'В сети';

  @override
  String get directAway => 'Нет на месте';

  @override
  String get directOffline => 'Не в сети';

  @override
  String directOfflineSeen(String when) {
    return 'Не в сети · последний раз $when';
  }

  @override
  String get directHeading => 'ЛИЧНЫЕ СООБЩЕНИЯ';

  @override
  String directPreviewMine(String text) {
    return 'Вы: $text';
  }

  @override
  String directOpenChat(String name) {
    return 'Открыть чат: $name';
  }

  @override
  String directShowCard(String name) {
    return 'Показать карточку: $name';
  }

  @override
  String get directNoLongerFriend => 'Больше не в друзьях';

  @override
  String directCallOffline(String name) {
    return '$name не в сети: звонить можно, только когда друг в сети';
  }

  @override
  String directHangUpFirst(String name) {
    return 'Сначала завершите звонок, чтобы позвонить пользователю $name';
  }

  @override
  String directPinnedCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n закреплённого сообщения',
      many: '$n закреплённых сообщений',
      few: '$n закреплённых сообщения',
      one: '$n закреплённое сообщение',
    );
    return '$_temp0';
  }

  @override
  String get directPinnedTitle => 'Закреплённые сообщения';

  @override
  String directPinnedNote(int max) {
    return 'Закреплены для вас обоих и не исчезают. Максимум: $max.';
  }

  @override
  String get directNothingPinned => 'Пока ничего не закреплено';

  @override
  String get directNothingPinnedNote =>
      'Наведите курсор на сообщение и нажмите на булавку, чтобы закрепить его здесь.';

  @override
  String directDayTime(String day, String time) {
    return '$day, $time';
  }

  @override
  String directFilesCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n файла',
      many: '$n файлов',
      few: '$n файла',
      one: '$n файл',
    );
    return '$_temp0';
  }

  @override
  String directShowMessageFrom(String name) {
    return 'Показать сообщение (автор: $name)';
  }

  @override
  String get directUnpin => 'Открепить';

  @override
  String get directPin => 'Закрепить';

  @override
  String get directDeleteMessage => 'Удалить у себя';

  @override
  String get directDeleteMessageTitle => 'Удалить это сообщение?';

  @override
  String directDeleteMessageNote(String name) {
    return 'Оно удалится с этого компьютера вместе с изображениями и файлами. У $name останется своя копия.';
  }

  @override
  String get directDeleteMessageAction => 'Удалить';

  @override
  String get directDeleteForBoth => 'Удалить у обоих';

  @override
  String directDeleteOwnNote(String name) {
    return '«Удалить у обоих» уберёт его и из чата $name, как только его Fogata это получит. «Удалить у себя» оставит его там.';
  }

  @override
  String directTimerLabel(String time) {
    return 'Исчезающие сообщения: $time';
  }

  @override
  String get directTimerTitle => 'Исчезающие сообщения';

  @override
  String get directCloseMenu => 'Закрыть меню';

  @override
  String get directTimerNote =>
      'Новые сообщения исчезают на обоих устройствах вместе с изображениями и файлами.';

  @override
  String get directEmptyNote =>
      'То, что вы пишете, хранится только на ваших двух компьютерах.';

  @override
  String directPinLineLabel(String line) {
    return '$line: показать';
  }

  @override
  String get directShowMessage => 'Показать сообщение';

  @override
  String get directPinnedStays =>
      'Закреплено: не исчезнет, пока его не открепят';

  @override
  String get directPinned => 'Закреплено';

  @override
  String directPinsFull(int max) {
    return 'Достигнут предел закреплённых сообщений ($max): сначала открепите одно';
  }

  @override
  String get directDelivered => 'Доставлено';

  @override
  String get directNotDelivered => 'Не доставлено';

  @override
  String get directSending => 'Отправка…';

  @override
  String directWaiting(String name) {
    return 'Ждём, пока $name появится в сети · сообщение уйдёт, когда вы оба будете в сети';
  }

  @override
  String get directAvailableWhenYouOnline =>
      'будет доступно, когда вы выйдете в сеть';

  @override
  String directAvailableWhenOnline(String name) {
    return 'будет доступно, когда $name будет в сети';
  }

  @override
  String get directDownloadedByItself => 'скачано автоматически';

  @override
  String directOpenFile(String name) {
    return 'Открыть $name';
  }

  @override
  String directPlayFile(String name) {
    return 'Воспроизвести $name';
  }

  @override
  String directDownloadSize(String size) {
    return 'Скачать · $size';
  }

  @override
  String directVideoPlaysWhenHere(String name, String length) {
    return '$name · $length · воспроизведётся, когда загрузится';
  }

  @override
  String directVideoNotHere(String name, String length, String when) {
    return '$name · $length · $when';
  }

  @override
  String directGettingIt(int percent) {
    return 'Получение · $percent%';
  }

  @override
  String directFileNotHere(String size, String when) {
    return '$size · $when';
  }

  @override
  String get directSaveToDownloads => 'Сохранить в «Загрузки»';

  @override
  String get directRemovedForSpace => 'Удалено здесь, чтобы освободить место';

  @override
  String get directAskAgain => 'Запросить снова';

  @override
  String get directClosePicture => 'Закрыть изображение';

  @override
  String get directCloseVideo => 'Закрыть видео';

  @override
  String get directCannotPlay =>
      'Этот компьютер не может воспроизвести это видео.';

  @override
  String get directPause => 'Пауза';

  @override
  String get directPlay => 'Воспроизвести';

  @override
  String directRemoveFilesTitle(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Убрать $n файла?',
      many: 'Убрать $n файлов?',
      few: 'Убрать $n файла?',
      one: 'Убрать $n файл?',
    );
    return '$_temp0';
  }

  @override
  String get directRemoveFilesText =>
      'Они будут убраны из этого сообщения. Файлы на вашем компьютере останутся как есть.';

  @override
  String get directDraftKept =>
      'Набранный текст останется здесь, чтобы его можно было отправить позже.';

  @override
  String directMessagePlaceholder(String name) {
    return 'Сообщение для $name';
  }

  @override
  String get directAddMessageOptional => 'Добавьте сообщение (необязательно)';

  @override
  String get directAutoDownloadNote =>
      'Изображения до 10 МБ скачиваются автоматически, видео и файлы — когда вы их открываете.';

  @override
  String get directVoiceRecord => 'Записать голосовое сообщение';

  @override
  String get directVoiceInCall => 'Во время звонка нельзя записывать';

  @override
  String directVoiceWaitingElsewhere(String name) {
    return 'Сначала отправьте или удалите голосовое сообщение в чате с пользователем $name.';
  }

  @override
  String get directVoiceDelete => 'Удалить';

  @override
  String get directVoiceTooShort =>
      'Слишком коротко для отправки: голосовое сообщение должно длиться не меньше половины секунды.';

  @override
  String directVoiceLeft(String time) {
    return 'Осталось $time';
  }

  @override
  String directVoiceRecordingFrom(String microphone) {
    return 'Запись: $microphone';
  }

  @override
  String get directVoiceRecordingDefault =>
      'Выбранный микрофон недоступен: запись с микрофона по умолчанию';

  @override
  String directVoiceKeys(String esc, String enter) {
    return '$esc — удалить, $enter — отправить';
  }

  @override
  String directVoiceKeySend(String enter) {
    return '$enter — отправить';
  }

  @override
  String directVoicePreview(String length) {
    return 'Голосовое сообщение · $length';
  }

  @override
  String get directVoiceMessage => 'Голосовое сообщение';

  @override
  String get directVoiceNoMic => 'Fogata не может использовать микрофон.';

  @override
  String get directVoiceMicStopped =>
      'Микрофон перестал работать. Записанное можно отправить.';

  @override
  String directVoiceNoMicCheck(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Проверьте, что микрофон подключён, выбран в разделе «Голос и видео» и разрешён в разделе «Системные настройки › Конфиденциальность и безопасность».',
      'other': 'Проверьте, что микрофон подключён и выбран в разделе «Голос и видео».',
    });
    return '$_temp0';
  }

  @override
  String get directVoiceSpeed => 'Скорость воспроизведения';

  @override
  String get directVoiceNotPlayed => 'Ещё не прослушано';

  @override
  String directRemoveFile(String name) {
    return 'Убрать $name';
  }

  @override
  String get directAltered =>
      'История этого чата на этом компьютере изменилась после последнего сохранения в Fogata. Некоторых сообщений может не хватать.';

  @override
  String directEnded(String name) {
    return 'Вы и $name больше не друзья. Этот чат доступен только для чтения.';
  }

  @override
  String get directDeleteChat => 'Удалить чат';

  @override
  String directDeleteTitle(String name) {
    return 'Удалить чат с пользователем $name?';
  }

  @override
  String directDeleteText(String name) {
    return 'Сообщения, изображения, видео и файлы этого чата будут удалены с этого компьютера. У пользователя $name останется своя копия.';
  }

  @override
  String get directDelete => 'Удалить';

  @override
  String get directUsageMessages => 'Сообщения';

  @override
  String get directUsagePictures => 'Изображения';

  @override
  String get directUsageVideos => 'Видео';

  @override
  String get directUsageFiles => 'Файлы';

  @override
  String get directUsageHeading => 'ЭТОТ ЧАТ НА ЭТОМ УСТРОЙСТВЕ';

  @override
  String get directFreeUpSpace => 'Освободить место…';

  @override
  String get directAddFilesTooltip => 'Добавить изображения, видео или файлы';

  @override
  String get directAddFiles => 'Добавить файлы';

  @override
  String get accountMoveLessThanMinuteLeft => 'осталось меньше минуты';

  @override
  String get accountMoveAboutMinuteLeft => 'осталась примерно минута';

  @override
  String accountMoveAboutMinutesLeft(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'осталось примерно $n минуты',
      many: 'осталось примерно $n минут',
      few: 'осталось примерно $n минуты',
      one: 'осталась примерно $n минута',
    );
    return '$_temp0';
  }

  @override
  String accountMoveNoRoom(String need, String free) {
    return 'Этому компьютеру нужно $need свободного места для аккаунта, а свободно $free.';
  }

  @override
  String accountMoveTooLarge(String size) {
    return 'Другой компьютер предложил $size — намного больше, чем может занимать аккаунт.';
  }

  @override
  String get accountMoveBusy =>
      'Этот компьютер завершает другой перенос: сначала завершите или отмените его.';

  @override
  String get accountMoveNothingToFinish =>
      'На этом компьютере нет незавершённого переноса с того компьютера.';

  @override
  String get accountMoveNetworkStopped => 'Сеть перестала работать.';

  @override
  String get accountMoveConnectionLost => 'Соединение потеряно.';

  @override
  String get accountMovePeerNoRoom =>
      'На новом компьютере недостаточно свободного места.';

  @override
  String get accountMovePeerTooLarge => 'Аккаунт слишком большой для переноса.';

  @override
  String get accountMovePeerBusy => 'Новый компьютер завершает другой перенос.';

  @override
  String get accountMovePeerCouldNotKeep =>
      'Новый компьютер не смог сохранить файлы.';

  @override
  String get accountMovePeerCancelled =>
      'Перенос отменён на другом компьютере.';

  @override
  String get accountMovePeerCouldNotFinish =>
      'Новый компьютер пока не смог завершить перенос.';

  @override
  String get accountMovePeerStopped =>
      'Перенос остановлен на другом компьютере.';

  @override
  String get accountMovePeerNothingToFinish =>
      'На том компьютере нет незавершённого переноса с этого компьютера.';

  @override
  String get accountMovePeerNotProven =>
      'Тот компьютер не подтвердил, что получил этот перенос.';

  @override
  String get accountMoveWrongCode =>
      'Другой компьютер не принял код, поэтому ничего не отправлено.';

  @override
  String get accountMoveExpired =>
      'Срок действия кода истёк до того, как его ввели.';

  @override
  String get accountMoveTooManyTries =>
      'Было введено несколько неверных кодов; код перестал действовать.';

  @override
  String get accountMoveNoContact =>
      'Компьютерам не удалось связаться друг с другом.';

  @override
  String get accountMoveUnconfirmed =>
      'Самый конец переноса не был подтверждён.';

  @override
  String get accountMoveCancelled => 'Отменено.';

  @override
  String get accountMoveInterrupted =>
      'Перенос прервался, не дойдя до конца; ничего не изменилось.';

  @override
  String accountMoveInterruptedWhy(String why) {
    return 'Перенос прервался, не дойдя до конца; ничего не изменилось. $why';
  }

  @override
  String get accountMoveDeclined => 'Другой компьютер остановил перенос.';

  @override
  String accountMoveDeclinedWhy(String why) {
    return 'Другой компьютер остановил перенос. $why';
  }

  @override
  String get accountMoveLater =>
      'Аккаунт получен и будет установлен при следующем запуске Fogata на этом компьютере.';

  @override
  String accountMoveLaterWhy(String why) {
    return 'Аккаунт получен и будет установлен при следующем запуске Fogata на этом компьютере. $why';
  }

  @override
  String get accountMoveDidNotFinish => 'Перенос не завершился.';

  @override
  String get accountMoveAnotherComputer => 'другой компьютер';

  @override
  String get accountMoveTheOtherComputer => 'другой компьютер';

  @override
  String get moveTooLateBeingPlaced =>
      'Остановить уже нельзя: аккаунт устанавливается.';

  @override
  String get moveTooLateDiscard => 'Слишком поздно: аккаунт устанавливается.';

  @override
  String get moveTooLateOtherHasIt =>
      'Остановить уже нельзя: аккаунт теперь на другом компьютере.';

  @override
  String moveFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n друга',
      many: '$n друзей',
      few: '$n друга',
      one: '$n друг',
    );
    return '$_temp0';
  }

  @override
  String moveGroups(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n группы',
      many: '$n групп',
      few: '$n группы',
      one: '$n группа',
    );
    return '$_temp0';
  }

  @override
  String moveGroupsWithKeys(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n группы с ключами',
      many: '$n групп с ключами',
      few: '$n группы с ключами',
      one: '$n группа с ключами',
    );
    return '$_temp0';
  }

  @override
  String moveChats(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n чата',
      many: '$n чатов',
      few: '$n чата',
      one: '$n чат',
    );
    return '$_temp0';
  }

  @override
  String moveInAll(String size) {
    return 'всего $size';
  }

  @override
  String get moveYourSettings => 'Ваши настройки';

  @override
  String get moveNotDevices => 'кроме выбранных микрофона и камеры';

  @override
  String get moveFinishingBoth => 'Завершение на обоих компьютерах…';

  @override
  String movePercent(int percent) {
    return '$percent%';
  }

  @override
  String movePercentLeft(int percent, String left) {
    return '$percent% · $left';
  }

  @override
  String get moveStepIdentity => 'Аккаунт и профиль';

  @override
  String get moveStepFriends => 'Друзья';

  @override
  String get moveStepGroups => 'Группы и их ключи';

  @override
  String get moveStepChats => 'Чаты, изображения и файлы';

  @override
  String moveBytesOf(String done, String total) {
    return '$done из $total';
  }

  @override
  String get moveStepFinishing => 'Завершение на обоих компьютерах';

  @override
  String moveBringingNamed(String name) {
    return 'Перенос аккаунта «$name»';
  }

  @override
  String get moveBringingYours => 'Перенос вашего аккаунта';

  @override
  String get moveKeepBothOn =>
      'Не выключайте компьютеры и не закрывайте Fogata.';

  @override
  String get moveKeepOpenNote =>
      'Не закрывайте Fogata на обоих компьютерах. Быстрее всего в одной сети; иначе компьютеры соединятся через интернет напрямую.';

  @override
  String moveFinishBringingNamed(String name) {
    return 'Завершите перенос аккаунта «$name»';
  }

  @override
  String get moveFinishBringingYours => 'Завершите перенос вашего аккаунта';

  @override
  String get moveEverythingCame =>
      'Всё получено. Чтобы завершить, введите этот код на другом компьютере.';

  @override
  String moveEverythingCameFrom(String computer) {
    return 'Всё получено. Чтобы завершить, введите этот код на компьютере «$computer».';
  }

  @override
  String get moveBringHereTitle => 'Перенесите аккаунт сюда';

  @override
  String get moveBringHereSub =>
      'Введите этот код на компьютере, где сейчас ваш аккаунт.';

  @override
  String get moveHideCode => 'Скрыть код';

  @override
  String get moveShowCode => 'Показать код';

  @override
  String moveExpiresIn(String time) {
    return 'истекает через $time';
  }

  @override
  String moveStepFogataShows(String what) {
    return 'На том компьютере Fogata покажет $what.';
  }

  @override
  String get moveWaitingToConfirm => 'Ожидание подтверждения';

  @override
  String get moveStepOpenThere => 'Откройте Fogata на том компьютере.';

  @override
  String moveStepTypeUnder(String where) {
    return 'Введите код в разделе $where.';
  }

  @override
  String get moveFinishTheMove => 'Завершение переноса';

  @override
  String moveStepGoTo(String where) {
    return 'Откройте $where.';
  }

  @override
  String get moveSettingsPath => 'Настройки → Перенос аккаунта';

  @override
  String get moveToAnotherComputer => 'Перенос на другой компьютер';

  @override
  String get moveWaitingForOther => 'Ожидание другого компьютера…';

  @override
  String moveWrongCodesTyped(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Попыток ввода неверного кода на других компьютерах: $n.',
      many: 'Попыток ввода неверного кода на других компьютерах: $n.',
      few: 'Попыток ввода неверного кода на других компьютерах: $n.',
      one: 'Попыток ввода неверного кода на других компьютерах: $n.',
    );
    return '$_temp0';
  }

  @override
  String get moveThrowAwayInstead => 'Удалить полученный аккаунт…';

  @override
  String get moveFoundOther => 'Другой компьютер найден';

  @override
  String get moveFoundSameNetwork =>
      'Одна сеть · зашифровано между компьютерами';

  @override
  String get moveFoundInternet =>
      'Через интернет · зашифровано между компьютерами';

  @override
  String get moveConfirmThere =>
      'Подтвердите на другом компьютере, чтобы начать';

  @override
  String get moveSameAccountNote =>
      'Это аккаунт, который уже есть на этом компьютере. При переносе сюда ничего не теряется.';

  @override
  String get moveTakeYes => 'Перенести сюда';

  @override
  String get moveWelcomeBack => 'С возвращением';

  @override
  String moveWelcomeBackNamed(String name) {
    return 'С возвращением, $name';
  }

  @override
  String get moveDoneSub =>
      'Теперь ваш аккаунт на этом компьютере. Другой компьютер перестал его использовать.';

  @override
  String get moveOpenFogata => 'Открыть Fogata';

  @override
  String get moveCodeExpiredLabel => 'истёк';

  @override
  String get moveCodeStoppedLabel => 'перестал действовать';

  @override
  String get moveCodeExpiredNote =>
      'Срок действия этого кода истёк до того, как его использовал ваш другой компьютер. Коды действуют 10 минут.';

  @override
  String get moveCodeStoppedNote =>
      'На других компьютерах несколько раз вводили неверные коды, поэтому этот код перестал действовать.';

  @override
  String get moveNewCode => 'Новый код';

  @override
  String get moveAlmostThere => 'Почти готово';

  @override
  String get moveLaterSub =>
      'Ваш аккаунт получен целиком, но пока не может быть установлен. Это произойдёт при следующем запуске Fogata на этом компьютере.';

  @override
  String get moveStartAgain => 'Перезапустить Fogata';

  @override
  String get moveUnconfirmedSub =>
      'Всё получено, но другой компьютер не подтвердил завершение. Покажите код и введите его там, чтобы завершить перенос.';

  @override
  String get moveStoppedThereTitle => 'Перенос остановлен на другом компьютере';

  @override
  String get moveStoppedThereSub =>
      'Ничего не изменилось: ваш аккаунт всё ещё на другом компьютере, и здесь от него ничего не сохранено.';

  @override
  String get moveCouldNotStartTitle => 'Не удалось начать перенос';

  @override
  String get moveSomethingWrongHere =>
      'На этом компьютере что-то пошло не так.';

  @override
  String get moveHalfwayTitle => 'Перенос остановился на полпути';

  @override
  String get moveHalfwaySub =>
      'Соединение с вашим другим компьютером потеряно. Ничего не изменилось: ваш аккаунт всё ещё на другом компьютере, и здесь от него ничего не сохранено.';

  @override
  String get moveShowCodeToFinish => 'Показать код для завершения';

  @override
  String get moveTryNewCode => 'Повторить с новым кодом';

  @override
  String moveBackToNamed(String name) {
    return 'Вернуться к профилю «$name»';
  }

  @override
  String get moveBackToThisProfile => 'Вернуться к этому профилю';

  @override
  String get moveNewProfileInstead => 'Создать новый профиль';

  @override
  String get moveDiscardTitle => 'Удалить полученный аккаунт?';

  @override
  String get moveDiscardText =>
      'Делайте это, только если другой компьютер продолжает его использовать. Тот компьютер ждёт подтверждения, пока его владелец не решит продолжить пользоваться аккаунтом там.';

  @override
  String get moveKeepIt => 'Оставить';

  @override
  String get moveThrowAway => 'Удалить';

  @override
  String get moveBringFromAnother => 'Перенести аккаунт с другого компьютера';

  @override
  String get moveAlreadyOnFogata => 'Уже пользуетесь Fogata?';

  @override
  String get moveCodeTriedBefore =>
      'Этот код уже не совпал один раз. Получите новый код на новом компьютере.';

  @override
  String moveMovingTo(String computer) {
    return 'Перенос аккаунта на компьютер «$computer»';
  }

  @override
  String get moveKeepBothOnOffline =>
      'Не выключайте компьютеры и не закрывайте Fogata. Пока идёт перенос, Fogata здесь не в сети.';

  @override
  String get moveCodeFieldLabel => 'Код, показанный на новом компьютере';

  @override
  String get moveToAnotherSub =>
      'Ваш аккаунт, друзья, группы, чаты, файлы и настройки перейдут на другой компьютер напрямую с этого, в зашифрованном виде. Затем этот компьютер перестанет использовать ваш аккаунт, так что второго «вас» никогда не будет.';

  @override
  String moveStepChoose(String choice) {
    return 'На новом компьютере откройте Fogata и выберите $choice.';
  }

  @override
  String get moveStepTypeItShows => 'Введите код, который он показывает:';

  @override
  String get moveWaitingForAnswer => 'Ожидание вашего ответа…';

  @override
  String get moveLookingForCode => 'Поиск компьютера с этим кодом…';

  @override
  String get moveOfflineWhileMoving =>
      'Во время переноса Fogata на этом компьютере не в сети.';

  @override
  String get moveWrongCodeNote =>
      'Новый компьютер не принял этот код, поэтому ничего не отправлено. Проверьте его и введите снова. Если не получится, получите там новый код.';

  @override
  String get moveUpdateBoth =>
      'На другом компьютере более старая версия Fogata. Обновите оба компьютера до одной и той же версии и попробуйте снова. Ничего не перенесено.';

  @override
  String get moveCantReachNew => 'Не удаётся связаться с новым компьютером.';

  @override
  String get moveCantReachNewNote =>
      'Компьютеры искали друг друга 3 минуты. Подключите их к одной сети (к одному Wi-Fi или кабелем) и повторите попытку. Ваш аккаунт по-прежнему здесь, и Fogata снова в сети.';

  @override
  String get moveDeclinedNote =>
      'Перенос остановлен на новом компьютере. Ничего не перенесено: ваш аккаунт по-прежнему здесь, и Fogata снова в сети.';

  @override
  String get moveCouldNotStart => 'Не удалось начать перенос.';

  @override
  String get moveHalfwayNote =>
      'Перенос остановился на полпути: соединение с новым компьютером потеряно. Ничего не изменилось: ваш аккаунт по-прежнему здесь, и Fogata снова в сети.';

  @override
  String get moveThisProfileWords => 'этот профиль';

  @override
  String get moveBringCardTitle => 'Перенести аккаунт сюда';

  @override
  String get moveBringCardSub =>
      'Покажите код на этом компьютере и введите его на том, где сейчас аккаунт.';

  @override
  String get moveBringCardButton => 'Перенести аккаунт сюда…';

  @override
  String moveReplacesHere(String name) {
    return 'Заменит $name на этом компьютере.';
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
      other: '$friends друга',
      many: '$friends друзей',
      few: '$friends друга',
      one: '$friends друг',
    );
    String _temp1 = intl.Intl.pluralLogic(
      groups,
      locale: localeName,
      other: '$groups группы',
      many: '$groups групп',
      few: '$groups группы',
      one: '$groups группа',
    );
    String _temp2 = intl.Intl.pluralLogic(
      chats,
      locale: localeName,
      other: '$chats чата',
      many: '$chats чатов',
      few: '$chats чата',
      one: '$chats чат',
    );
    return '$bold (профиль, который сейчас на этом компьютере) будет заменён, а его $_temp0, $_temp1 и $_temp2 будут удалены здесь. Чтобы сохранить $name, сначала перенесите его на другой компьютер.';
  }

  @override
  String moveReplaceAndShow(String name) {
    return 'Заменить $name и показать код';
  }

  @override
  String moveConfirmTitle(String computer) {
    return 'Перенести ваш аккаунт на компьютер «$computer»?';
  }

  @override
  String get moveConfirmSameNetwork =>
      'Найден в той же сети. Всё передаётся напрямую, в зашифрованном виде.';

  @override
  String get moveConfirmInternet =>
      'Найден через интернет. Всё передаётся напрямую, в зашифрованном виде.';

  @override
  String moveConfirmReplaces(String name) {
    return 'Там он заменит профиль $name.';
  }

  @override
  String get moveConfirmNote =>
      'Когда перенос завершится, этот компьютер перестанет использовать ваш аккаунт и больше не сможет его использовать. Затем вы сможете удалить то, что здесь осталось.';

  @override
  String get moveConfirmYes => 'Перенести мой аккаунт';

  @override
  String get moveKeepTitle => 'Продолжить пользоваться аккаунтом здесь?';

  @override
  String get moveKeepText =>
      'Только если на новом компьютере его нет: сначала откройте там Fogata. Если аккаунтом будут пользоваться оба компьютера, друзья и группы увидят двух «вас», а сообщения будут доходить только до одного.';

  @override
  String get moveKeepYes => 'Продолжить здесь';

  @override
  String moveWaitingFor(String computer) {
    return 'Ожидание подтверждения от компьютера «$computer»';
  }

  @override
  String get moveWaitingText =>
      'Ваш аккаунт отправлен, но соединение прервалось до того, как новый компьютер сообщил о готовности. Пока что этот компьютер не будет использовать аккаунт.';

  @override
  String get moveFinishText =>
      'Откройте Fogata на новом компьютере: там появится код. Введите его здесь.';

  @override
  String get moveFinish => 'Завершить';

  @override
  String get moveFinishWrongCode =>
      'Новый компьютер не принял этот код. Проверьте его и введите снова.';

  @override
  String get moveFinishNoContact =>
      'Не удаётся связаться с новым компьютером. Подключите оба компьютера к одной сети и повторите попытку.';

  @override
  String get moveKeepThisOne =>
      'На новом компьютере его нет: продолжить здесь…';

  @override
  String moveMovedYours(String computer) {
    return 'Ваш аккаунт перенесён на компьютер «$computer»';
  }

  @override
  String moveMovedNamed(String name, String computer) {
    return 'Аккаунт «$name» перенесён на компьютер «$computer»';
  }

  @override
  String get moveMovedNoteYou =>
      'Этот компьютер больше его не использует: друзья и группы связываются с вами на другом.';

  @override
  String moveMovedNoteNamed(String name) {
    return 'Этот компьютер больше его не использует: для друзей и групп $name теперь на другом компьютере.';
  }

  @override
  String moveMovedTodayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'Сегодня в $time.',
      'other': 'Сегодня в $time.',
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
  String get moveLeftHere => 'Что осталось здесь';

  @override
  String get moveLeftHereText =>
      'Чаты, изображения и файлы, полученные до переноса, в зашифрованном виде. У другого компьютера есть своя копия.';

  @override
  String get moveDeleteData => 'Удалить данные Fogata на этом компьютере';

  @override
  String get moveAfterDeleting =>
      'После удаления Fogata здесь запустится как новая.';

  @override
  String get moveCodeNotACode =>
      'Код состоит из десяти букв и цифр, например XXXXX-XXXXX.';

  @override
  String get moveCodeMistyped =>
      'Код введён с ошибкой: проверьте буквы и цифры.';

  @override
  String get welcomeHeadline => 'Общайтесь так, будто\nвы в одной комнате.';

  @override
  String get welcomeIntro =>
      'Голос, видео и чат с друзьями, семьёй или командой — напрямую с вашего компьютера на их компьютеры.';

  @override
  String get welcomeNoAccount => 'Без регистрации';

  @override
  String get welcomeEncrypted => 'Шифрование';

  @override
  String get welcomeEphemeral => 'Временные комнаты';

  @override
  String get welcomeCharacter =>
      'Персонаж Fogata — маленький огонёк в наушниках, показывающий знак мира';

  @override
  String get welcomeImageUnreadable => 'Не удалось прочитать это изображение.';

  @override
  String get welcomeKeychainTitle => 'Ваши данные пока нельзя зашифровать';

  @override
  String welcomeKeychainText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' В Linux связка ключей — это запущенная служба Secret Service, например KWallet или GNOME Keyring: запустите или разблокируйте её и повторите попытку.',
      'other':
          ' Разблокируйте или восстановите связку ключей и повторите попытку.',
    });
    return 'Fogata шифрует ваш аккаунт, друзей и личные сообщения ключом, который хранит в системной связке ключей, но связка ключей его не сохранила.$_temp0';
  }

  @override
  String get welcomeKeychainRetry => 'Повторить';

  @override
  String get welcomeKeychainPlain => 'Продолжить без шифрования';

  @override
  String get welcomeKeychainPlainNote =>
      'Без шифрования их может прочитать любой, кто скопирует файлы этого компьютера. В настройках останется предупреждение, а Fogata всё зашифрует при первом запуске со связкой ключей.';

  @override
  String get welcomeProfileKeptTitle => 'Не удалось прочитать ваш профиль';

  @override
  String welcomeProfileKeptText(String folder) {
    return 'Не удалось прочитать профиль и друзей, созданных на этом компьютере, поэтому Fogata открывается как новый. Ничего не удалено: файлы хранятся в $folder.';
  }

  @override
  String get welcomeProfileKeptConfirm =>
      'Новый профиль начинается без друзей. Файлы старого профиля остаются на месте, и Fogata их не использует.';

  @override
  String get welcomeProfileKeptMake => 'Создать новый профиль';

  @override
  String get welcomeWriteName => 'Введите имя.';

  @override
  String get welcomeCardTitle => 'Как вас будут видеть друзья?';

  @override
  String get welcomeCardSub =>
      'Имя и изображение. Их можно изменить когда угодно.';

  @override
  String get welcomeNoServer =>
      'Ничего не отправляется на сервер Fogata: его просто нет. Публичная сеть IPFS служит только телефонной книгой, чтобы находить ваших друзей.';

  @override
  String get welcomeYourName => 'Ваше имя';

  @override
  String welcomePictureLabel(String name) {
    return 'Изображение: $name';
  }

  @override
  String get welcomeUploadPicture => 'Загрузить изображение';

  @override
  String get welcomeNameLabel => 'Имя';

  @override
  String get welcomeCreate => 'Создать';

  @override
  String get welcomeReady => 'Готово';

  @override
  String get welcomeCreateLabel => 'Создать аккаунт';

  @override
  String welcomeKeyLabel(String key) {
    return 'Ключ, созданный на этом компьютере: $key';
  }

  @override
  String get welcomeNewKey => 'Создать другой ключ';

  @override
  String get stoppedTitle => 'Fogata перестала работать.';

  @override
  String get stoppedStartingAgain => 'Перезапуск…';

  @override
  String get stoppedRestartToConnect =>
      'Перезапустите её, чтобы снова подключиться.';

  @override
  String get stoppedRestartNow => 'Перезапустить';

  @override
  String get settingsSectionProfile => 'Мой профиль';

  @override
  String get settingsSectionMove => 'Перенос аккаунта';

  @override
  String get settingsSectionAppearance => 'Внешний вид';

  @override
  String get settingsSectionLanguageTime => 'Язык и время';

  @override
  String get settingsSectionVoice => 'Голос и видео';

  @override
  String get settingsSectionSounds => 'Звуки';

  @override
  String get settingsSectionBehaviour => 'Поведение';

  @override
  String get settingsSectionNetwork => 'Сеть';

  @override
  String get settingsSectionStorage => 'Хранилище и загрузки';

  @override
  String get settingsHeadingUser => 'Пользователь';

  @override
  String get settingsHeadingApp => 'Приложение';

  @override
  String get settingsTitle => 'Настройки';

  @override
  String get settingsAll => 'Все настройки';

  @override
  String get settingsCloseTooltip => 'Закрыть настройки';

  @override
  String get settingsUnsavedSaveTitle => 'Сохранить изменения?';

  @override
  String get settingsUnsavedDiscardTitle => 'Отменить изменения?';

  @override
  String get settingsUnsavedSaveText =>
      'В профиле есть несохранённые изменения.';

  @override
  String get settingsUnsavedDiscardText =>
      'В профиле есть изменения, которые нельзя сохранить в текущем виде.';

  @override
  String get settingsProfileSaved =>
      'Профиль сохранён. Друзья и группы в сети видят его уже сейчас, остальные — когда подключатся.';

  @override
  String get settingsBioSaved =>
      'Описание сохранено. Друзья в сети видят его уже сейчас, остальные — когда подключатся.';

  @override
  String settingsSwitchOn(String what) {
    return '$what: вкл.';
  }

  @override
  String settingsSwitchOff(String what) {
    return '$what: выкл.';
  }

  @override
  String get settingsOn => 'Вкл.';

  @override
  String get settingsOff => 'Выкл.';

  @override
  String get settingsIdentity => 'Ваш профиль';

  @override
  String get settingsIdentityNote =>
      'Ваше имя и изображение передаются друзьям и группам с цифровой подписью, описание — только друзьям.';

  @override
  String get settingsPicture => 'Изображение';

  @override
  String get settingsUploadPicture => 'Загрузить изображение или GIF';

  @override
  String get settingsName => 'Имя';

  @override
  String get settingsBio => 'О себе';

  @override
  String get settingsBioHint => 'Пара слов о вас';

  @override
  String get settingsNoBio => 'На вашей карточке нет описания.';

  @override
  String settingsBioWhere(int lines) {
    return 'Друзья видят его на вашей карточке (до $lines строк).';
  }

  @override
  String get settingsBannerTitle => 'Фон панели';

  @override
  String get settingsBannerNote =>
      'Верх вашей карточки — для вас и ваших друзей.';

  @override
  String get settingsBannerAnother => 'Другое изображение';

  @override
  String get settingsBannerUpload => 'Выбрать изображение на этом компьютере';

  @override
  String get settingsFingerprint => 'Отпечаток этого компьютера';

  @override
  String get settingsTheme => 'Тема';

  @override
  String get settingsThemeNote =>
      'Цвета окна. Тема «Системная» следует настройке компьютера: «Тёмная» или «Светлая».';

  @override
  String get settingsMicSpeaker => 'Микрофон и динамик';

  @override
  String get settingsMicSpeakerNote =>
      'Через что вас слышат в звонках и через что звучат другие. Их можно поменять и во время звонка.';

  @override
  String get settingsCamera => 'Камера';

  @override
  String get settingsCameraNote =>
      'Что видят другие в звонках, когда ваша камера включена. Проверьте её здесь перед звонком.';

  @override
  String get settingsSoundEffects => 'Звуковые эффекты';

  @override
  String get settingsSoundEffectsNote =>
      'Вход в комнату и выход из неё, приход и уход людей, сообщения чата, микрофон и показ экрана.';

  @override
  String get settingsSounds => 'Звуки';

  @override
  String settingsChangeFailed(String reason) {
    return 'Fogata не удалось это изменить: $reason';
  }

  @override
  String get settingsAutostartApprove =>
      'Разрешите Fogata в разделе «Системные настройки › Основные › Объекты входа».';

  @override
  String get settingsAutostartOldMac => 'Для этого нужна macOS 13 или новее.';

  @override
  String get settingsAutostartUnsupported =>
      'Эта копия Fogata не может открываться сама.';

  @override
  String get settingsAutostartUnread =>
      'Fogata не удалось прочитать эту настройку из системы.';

  @override
  String get settingsAutostart => 'Открывать при запуске';

  @override
  String get settingsAutostartNote =>
      'Открывать Fogata при включении компьютера, после входа в систему.';

  @override
  String get settingsKeepRunning => 'Работать в фоне';

  @override
  String get settingsKeepRunningNote =>
      'Когда окно закрыто, Fogata остаётся в трее, и друзья по-прежнему могут вам звонить.';

  @override
  String get settingsNoTray =>
      'Эта среда рабочего стола не показывает значки в трее, поэтому закрытие окна завершает Fogata.';

  @override
  String get settingsHelping => 'Помощь участникам';

  @override
  String get settingsHelpingNote =>
      'Как этот компьютер помогает участникам ваших групп.';

  @override
  String get settingsHelpRelay => 'Соединять и передавать';

  @override
  String get settingsHelpConnect => 'Только соединять';

  @override
  String get settingsHelpRelayNote =>
      'Когда два участника не могут подключиться друг к другу, этот компьютер передаёт их зашифрованный трафик (не видя его), до 2 ГБ за звонок. Для этого используется ваш исходящий канал.';

  @override
  String get settingsHelpConnectNote =>
      'Этот компьютер помогает двум участникам найти друг друга, но не передаёт их трафик. Если они не смогут подключиться, то не услышат друг друга.';

  @override
  String get settingsRestartTray =>
      'Изменение вступит в силу, когда вы выйдете из Fogata через значок в трее и откроете её снова.';

  @override
  String get settingsRestart =>
      'Изменение вступит в силу, когда вы закроете Fogata и откроете её снова.';

  @override
  String get settingsNetworkCheck => 'Проверка сети';

  @override
  String get settingsNetworkCheckNote =>
      'Как этот компьютер видит сеть и что это значит для связи с друзьями. Ничего никуда не отправляется: проверка смотрит только на то, что уже известно соединению.';

  @override
  String get settingsCheckNetwork => 'Проверить сеть';

  @override
  String get settingsCheckAgain => 'Проверить снова';

  @override
  String get settingsDownloads => 'Загрузки';

  @override
  String get settingsDownloadsNote =>
      'Куда сохраняются файлы, скачанные из комнаты.';

  @override
  String settingsVersionData(String version, String path) {
    return 'Fogata $version · Данные в $path';
  }

  @override
  String get settingsNotEncrypted => 'Не зашифровано на этом компьютере';

  @override
  String settingsNotEncryptedText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' В Linux связка ключей — это запущенная служба Secret Service, например KWallet или GNOME Keyring: запустите или установите её, и Fogata зашифрует данные при следующем запуске.',
      'other': ' Fogata попробует снова при следующем запуске.',
    });
    return 'Ваш аккаунт, друзья и личные сообщения хранятся на этом компьютере без шифрования, потому что не найдена системная связка ключей для хранения ключа.$_temp0';
  }

  @override
  String get settingsNotAllEncrypted => 'Не всё зашифровано на этом компьютере';

  @override
  String get settingsNotAllEncryptedText =>
      'Часть данных на этом компьютере (ваш аккаунт, друзья и личные сообщения) ещё не зашифрована: Fogata ещё шифрует её или не смогла закончить, например потому что диск заполнен. Шифрование продолжится при каждом запуске Fogata.';

  @override
  String settingsDamagedSecret(String file) {
    return '$file не открывается ключом этого компьютера: файл повреждён на диске. Пока он там, шифрование не может завершиться, а копия, сделанная до шифрования, остаётся незашифрованной. Отложите его, чтобы завершить: он сохранится как есть. Если это ваш аккаунт или список друзей, Fogata вернёт эту копию и зашифрует её; файл группы или чата на этом компьютере будет потерян: группе придётся снова принять вас, а прежние сообщения чата перестанут отображаться.';
  }

  @override
  String get settingsSetDamagedAside => 'Отложить и перезапустить';

  @override
  String get storageTitle => 'Хранилище';

  @override
  String get storageNote =>
      'Ваши личные сообщения хранятся только на ваших устройствах. Чаты комнат исчезают, когда комната закрывается, поэтому растут только личные сообщения.';

  @override
  String get storageDirect => 'Личные сообщения';

  @override
  String get storageOfNoLimit => 'без ограничения';

  @override
  String storageOfLimit(String limit) {
    return 'из $limit';
  }

  @override
  String storageLegendPictures(String size) {
    return 'Изображения $size';
  }

  @override
  String storageLegendVideos(String size) {
    return 'Видео $size';
  }

  @override
  String storageLegendFiles(String size) {
    return 'Файлы $size';
  }

  @override
  String storageLegendMessages(String size) {
    return 'Сообщения $size';
  }

  @override
  String get storageRoom => 'Место для личных сообщений';

  @override
  String get storageNoLimitNote => 'Ничего не удаляется автоматически.';

  @override
  String storageOverNote(String limit) {
    return 'Лимит превышен: самые старые изображения, видео и файлы удаляются, пока не останется $limit. Сообщения остаются всегда.';
  }

  @override
  String get storageLimitNote =>
      'При превышении лимита первыми удаляются самые старые изображения, видео и файлы. Сообщения остаются всегда; на месте файла появится надпись «Удалено здесь, чтобы освободить место».';

  @override
  String get storageAutoPictures => 'Скачивать изображения автоматически';

  @override
  String get storageAutoPicturesNote => 'До 10 МБ каждое, по мере поступления.';

  @override
  String get storageAutoFiles => 'Скачивать видео и файлы автоматически';

  @override
  String get storageAutoFilesNote =>
      'Если выключено: они скачиваются, когда вы их открываете, пока отправитель в сети.';

  @override
  String get storageTimer => 'Исчезающие сообщения в новых чатах';

  @override
  String get storageTimerNote =>
      'В каждом чате это можно изменить; оба собеседника всегда видят одинаковый таймер.';

  @override
  String get storageTimer24h => '24 ч';

  @override
  String get storageByChat => 'По чатам';

  @override
  String get storageByChatNote =>
      'Очистка удаляет здесь изображения, видео и файлы. Сообщения остаются, а у собеседника остаётся своя копия.';

  @override
  String get storageMessagesOnly => 'Только сообщения';

  @override
  String get storageClearFiles => 'Очистить файлы';

  @override
  String storageClearFilesTooltip(String name) {
    return 'Очистить файлы чата с пользователем $name';
  }

  @override
  String get storageCleared => 'Очищено';

  @override
  String storageConfirmTitle(String limit) {
    return 'Оставить $limit изображений, видео и файлов?';
  }

  @override
  String storageConfirmText(String limit) {
    return 'Самые старые будут удалены сейчас, пока не останется $limit; файлы закреплённых сообщений — в последнюю очередь. Сообщения остаются всегда, а у друзей остаются свои копии.';
  }

  @override
  String get storageRemoveOldest => 'Удалить самые старые';

  @override
  String get themePickerSystem => 'Системная тема, как настроено на компьютере';

  @override
  String themePickerTheme(String name) {
    return 'Тема «$name»';
  }

  @override
  String get themeDark => 'Тёмная';

  @override
  String get themeMidnight => 'Полночь';

  @override
  String get themeDim => 'Приглушённая';

  @override
  String get themeEmber => 'Угли';

  @override
  String get themeContrast => 'Контрастная';

  @override
  String get themeLight => 'Светлая';

  @override
  String get themeSystem => 'Системная';

  @override
  String get themeBannerViolet => 'Фиолетовый';

  @override
  String get themeBannerBlue => 'Синий';

  @override
  String get themeBannerGreen => 'Зелёный';

  @override
  String get themeBannerClay => 'Терракотовый';

  @override
  String get themeBannerRaspberry => 'Малиновый';

  @override
  String get themeBannerMustard => 'Горчичный';

  @override
  String get themeBannerSlate => 'Грифельный';

  @override
  String get themeBannerNight => 'Ночной';

  @override
  String widgetsCopyWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Копировать QR-код',
      'link': 'Копировать ссылку',
      'code': 'Копировать код',
      'other': 'Копировать',
    });
    return '$_temp0';
  }

  @override
  String widgetsShowWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Показать QR-код',
      'link': 'Показать ссылку',
      'code': 'Показать код',
      'key': 'Показать ключ',
      'other': 'Показать',
    });
    return '$_temp0';
  }

  @override
  String get widgetsHide => 'Скрыть';

  @override
  String get widgetsShowMore => 'Показать полностью';

  @override
  String get widgetsShowLess => 'Свернуть';

  @override
  String revealFailed(String path) {
    return 'Не удалось открыть папку. Файл находится здесь: $path';
  }

  @override
  String get emojiGroupPeople => 'Люди';

  @override
  String get emojiGroupNature => 'Природа';

  @override
  String get emojiGroupFood => 'Еда и напитки';

  @override
  String get emojiGroupActivities => 'Занятия';

  @override
  String get emojiGroupTravel => 'Путешествия и места';

  @override
  String get emojiGroupObjects => 'Предметы';

  @override
  String get emojiGroupSymbols => 'Символы';

  @override
  String get emojiGroupFlags => 'Флаги';

  @override
  String get emojiPickerLimit => 'Сообщение достигло максимальной длины.';

  @override
  String get emojiPickerButton => 'Эмодзи';

  @override
  String get emojiPickerResults => 'Результаты';

  @override
  String get emojiPickerRecent => 'Недавние';

  @override
  String get emojiPickerSearch => 'Поиск эмодзи (на английском)';

  @override
  String get emojiPickerSkinTone => 'Оттенок кожи';

  @override
  String get emojiPickerNone => 'Эмодзи не найдены';

  @override
  String get emojiPickerNoFont =>
      'На этом компьютере нет цветного шрифта эмодзи, поэтому эмодзи могут выглядеть простыми или пустыми. Это исправит установка Noto Color Emoji.';

  @override
  String get emojiPickerDefaultTone => 'Оттенок кожи по умолчанию';

  @override
  String emojiPickerTone(int n) {
    return 'Оттенок кожи $n';
  }

  @override
  String get iconsGroupMountain => 'Гора';

  @override
  String get iconsGroupStar => 'Звезда';

  @override
  String get iconsGroupGameController => 'Геймпад';

  @override
  String get iconsGroupMusic => 'Музыка';

  @override
  String get iconsGroupBook => 'Книга';

  @override
  String get iconsGroupWaves => 'Волны';

  @override
  String get iconsGroupCoffee => 'Кофе';

  @override
  String get iconsGroupHeart => 'Сердце';

  @override
  String get iconsGroupFilm => 'Кино';

  @override
  String get newGroupPictureUnreadable =>
      'Не удалось прочитать это изображение.';

  @override
  String newGroupMade(String name) {
    return 'Группа «$name» создана. Участники получат её, когда появятся в сети.';
  }

  @override
  String get newGroupTitle => 'Новая группа';

  @override
  String get newGroupSubtitle => 'Группа остаётся навсегда. Её комнаты — нет.';

  @override
  String get newGroupName => 'Название группы';

  @override
  String get newGroupNameExample => 'Воскресный велозаезд';

  @override
  String get newGroupIconOrPicture => 'Значок или изображение';

  @override
  String get newGroupChooseFromComputer => 'Выбрать на компьютере';

  @override
  String get newGroupBannerOptional => 'Баннер (необязательно)';

  @override
  String get newGroupMembers => 'Участники';

  @override
  String newGroupSelected(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Выбрано: $n',
      many: 'Выбрано: $n',
      few: 'Выбрано: $n',
      one: 'Выбрано: $n',
    );
    return '$_temp0';
  }

  @override
  String get newGroupSearchPlaceholder => 'Поиск среди друзей';

  @override
  String get newGroupSearchLabel => 'Поиск друзей';

  @override
  String get newGroupNoFriends =>
      'Друзей пока нет. Можно создать группу и добавить их позже или пригласить людей по ссылке.';

  @override
  String get newGroupKeptOnDevices =>
      'Хранится на устройствах всех участников.';

  @override
  String get newGroupCreate => 'Создать группу';

  @override
  String get groupBannerSettings => 'Настройки группы';

  @override
  String get groupBannerPictureUnreadable =>
      'Не удалось прочитать это изображение.';

  @override
  String get groupBannerChoose => 'Выбрать баннер';

  @override
  String get groupBannerAnother => 'Другой баннер';

  @override
  String get groupBannerNone => 'Без баннера';

  @override
  String get groupBannerNote =>
      'Показывается вверху колонки группы, под её названием. PNG, JPEG или анимированный GIF либо WebP до 550 КБ.';

  @override
  String get homeTitle => 'Главная';

  @override
  String get homeSubtitle =>
      'Комнаты загораются, когда кто-то входит, и гаснут, когда все уходят.';

  @override
  String get homeAddFriend => 'Добавить друга';

  @override
  String get homeNewGroup => 'Новая группа';

  @override
  String get homeLive => 'Горят';

  @override
  String get homeNoRoom => 'Не горят';

  @override
  String get homeNoRoomNote => 'Откройте комнату, и она загорится';

  @override
  String homeInviteBy(String name, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n участника',
      many: '$n участников',
      few: '$n участника',
      one: '$n участник',
    );
    return '$name приглашает вас · $_temp0';
  }

  @override
  String get homeInviteNote =>
      'Пока вы не вступите, этот компьютер не подключается к участникам и ничего им не отправляет.';

  @override
  String get homeInviteJoin => 'Вступить';

  @override
  String get homeNoGroups => 'Групп пока нет';

  @override
  String get homeNoGroupsNote =>
      'Добавьте друзей и создайте с ними группу. Группа остаётся навсегда, её комнаты — нет.';

  @override
  String homePersonYou(String name) {
    return '$name (вы)';
  }

  @override
  String homeMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n участника',
      many: '$n участников',
      few: '$n участника',
      one: '$n участник',
    );
    return '$_temp0';
  }

  @override
  String get homeYouAreIn => 'Вы здесь';

  @override
  String get homeMuteMic => 'Выключить микрофон';

  @override
  String get homeUnmuteMic => 'Включить микрофон';

  @override
  String get homeCameraOff => 'Выключить камеру';

  @override
  String get homeCameraOn => 'Включить камеру';

  @override
  String get homeMembersTooltip => 'Участники';

  @override
  String get homeReturnToRoom => 'Вернуться';

  @override
  String get homeJoinRoom => 'Войти';

  @override
  String homeMembersOnline(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members участника',
      many: '$members участников',
      few: '$members участника',
      one: '$members участник',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online в сети',
      many: '$online в сети',
      few: '$online в сети',
      one: '$online в сети',
      zero: 'больше никого в сети',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get homeNoRoomsYet => 'Комнат ещё не было';

  @override
  String homeLastRoom(String when) {
    return 'Последняя комната: $when';
  }

  @override
  String get homeOpenRoom => 'Открыть комнату';

  @override
  String get membersGroupGone => 'Этой группы здесь больше нет.';

  @override
  String membersSummary(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members участника',
      many: '$members участников',
      few: '$members участника',
      one: '$members участник',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online в сети',
      many: '$online в сети',
      few: '$online в сети',
      one: '$online в сети',
      zero: 'больше никого в сети',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String membersSummaryLastRoom(int members, int online, String when) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members участника',
      many: '$members участников',
      few: '$members участника',
      one: '$members участник',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online в сети',
      many: '$online в сети',
      few: '$online в сети',
      one: '$online в сети',
      zero: 'больше никого в сети',
    );
    return '$_temp0 · $_temp1 · последняя комната: $when';
  }

  @override
  String get membersCopyInviteLink => 'Копировать ссылку-приглашение';

  @override
  String membersInviteLinkTooltip(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'кто-нибудь из группы',
      'first': 'первый участник группы',
      'other': 'один из первых $n участников группы',
    });
    return 'Действует один раз в течение 24 часов, и $_temp0 впускает человека';
  }

  @override
  String membersInviteLinkCopied(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'кто-нибудь из группы',
      'first': 'первый участник группы',
      'other': 'один из первых $n участников группы',
    });
    return 'Ссылка-приглашение скопирована. Она действует один раз в течение 24 часов, и $_temp0 впускает человека.';
  }

  @override
  String get membersInviteFriends => 'Пригласить друзей';

  @override
  String get membersBackToRoom => 'Вернуться в комнату';

  @override
  String get membersOpenRoom => 'Открыть комнату';

  @override
  String get membersEnterRoom => 'Войти в комнату';

  @override
  String get membersTitle => 'Участники';

  @override
  String get membersOrderNote => 'По порядку: каждый управляет теми, кто ниже';

  @override
  String get membersGroupSettings => 'Настройки группы';

  @override
  String membersManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Менять группу могут только первые в списке: $n.',
      many: 'Менять группу могут только первые в списке: $n.',
      few: 'Менять группу могут только первые в списке: $n.',
      one: 'Менять группу могут только первые в списке: $n.',
    );
    return '$_temp0';
  }

  @override
  String get membersLeaveGroup => 'Покинуть группу';

  @override
  String membersInviteTitle(String group) {
    return 'Пригласить друзей в группу «$group»';
  }

  @override
  String get membersInviteNote =>
      'Они получат приглашение и сами решат, вступать ли.';

  @override
  String get membersPresenceYou => 'вы';

  @override
  String membersPresenceOnline(String how) {
    return 'в сети · $how';
  }

  @override
  String membersPresenceAway(String how) {
    return 'нет на месте · $how';
  }

  @override
  String get membersPresenceOffline => 'сейчас не в сети';

  @override
  String membersYou(String name) {
    return '$name (вы)';
  }

  @override
  String get membersWaitingForKey => 'ожидает ключ';

  @override
  String membersFingerprint(String fingerprint) {
    return 'Отпечаток $fingerprint';
  }

  @override
  String membersOptionsFor(String name) {
    return 'Действия: $name';
  }

  @override
  String get membersMoveUp => 'Переместить сразу под себя';

  @override
  String get membersMoveToEnd => 'Переместить в конец';

  @override
  String get membersTimeOut => 'Временно отстранить от комнаты…';

  @override
  String get membersRemoveFromGroup => 'Удалить из группы';

  @override
  String membersRemoveTitle(String name) {
    return 'Удалить участника $name?';
  }

  @override
  String membersRemoveText(String name) {
    return '$name покинет группу, а группа создаст новый ключ. То, что этот участник уже видел, останется у него.';
  }

  @override
  String get membersFirst => 'Первый';

  @override
  String membersTimeOutTitle(String name) {
    return 'Тайм-аут: $name';
  }

  @override
  String membersTimeOutText(String name) {
    return '$name покинет комнату и не сможет вернуться, пока не истечёт время или кто-нибудь выше не разрешит вернуться.';
  }

  @override
  String get membersUntilLetBack => 'До разрешения';

  @override
  String membersLeaveTitle(String group) {
    return 'Покинуть группу «$group»?';
  }

  @override
  String get membersLeaveText =>
      'Вы перестанете видеть её комнаты, а остальные — видеть вас в группе. Группа создаст новый ключ без вас. Чтобы вернуться, кто-нибудь из группы должен будет снова вас впустить.';

  @override
  String membersLeaveTextFirst(String name) {
    return 'Вы перестанете видеть её комнаты, а остальные — видеть вас в группе. Группа создаст новый ключ без вас. Чтобы вернуться, кто-нибудь из группы должен будет снова вас впустить. Первое место займёт $name.';
  }

  @override
  String get groupSettingsTabGeneral => 'Общие';

  @override
  String get groupSettingsTabVoiceRooms => 'Голосовые комнаты';

  @override
  String get groupSettingsTabMembers => 'Участники';

  @override
  String get groupSettingsTabInvite => 'Приглашения';

  @override
  String get groupSettingsTabLimits => 'Ограничения';

  @override
  String get groupSettingsTabActivity => 'Журнал';

  @override
  String get groupSettingsFirstHere => 'Первое место в этой группе';

  @override
  String groupSettingsPlaceHere(String place) {
    return '$place в этой группе';
  }

  @override
  String get groupSettingsLeave => 'Покинуть группу';

  @override
  String get groupSettingsInviteTitle => 'Пригласить людей';

  @override
  String get groupSettingsClose => 'Закрыть настройки группы';

  @override
  String get groupSettingsPicture => 'Изображение';

  @override
  String get groupSettingsChooseFromComputer => 'Выбрать на компьютере';

  @override
  String get groupSettingsUseIcon => 'Выбрать значок';

  @override
  String get groupSettingsPictureNote =>
      'PNG, JPEG или WebP — обрезается до квадрата и уменьшается, как фото профиля, — или анимированный GIF или WebP до 550 КБ.';

  @override
  String get groupSettingsOrIcon => 'Или значок';

  @override
  String get groupSettingsBanner => 'Баннер';

  @override
  String get groupSettingsName => 'Название';

  @override
  String get groupSettingsNameField => 'Название группы';

  @override
  String get groupSettingsNameNote =>
      'Новое название и изображение увидят все в группе.';

  @override
  String get groupSettingsRoomsIntro =>
      'У каждой голосовой комнаты свои люди, чат, файлы и звонки: те, кто в одной комнате, не связаны с теми, кто в другой. Одновременно можно быть только в одной комнате.';

  @override
  String get groupSettingsRoomName => 'Название комнаты';

  @override
  String get groupSettingsAccessEveryone => 'Все';

  @override
  String get groupSettingsAccessSome => 'Некоторые';

  @override
  String groupSettingsWhoCanEnter(String room) {
    return 'Кто может входить в комнату $room';
  }

  @override
  String get groupSettingsFirstRoomStays => 'Первую комнату нельзя удалить';

  @override
  String get groupSettingsPeopleInside => 'В комнате есть люди';

  @override
  String get groupSettingsDeleteRoom => 'Удалить комнату';

  @override
  String get groupSettingsAddRoom => 'Добавить голосовую комнату';

  @override
  String groupSettingsNewRoomName(int n) {
    return 'Комната $n';
  }

  @override
  String groupSettingsRoomsNote(int rooms, int people) {
    return 'До $rooms комнат, в каждой до $people человек («Ограничения»). Комнату, в которой есть люди, нельзя удалить.';
  }

  @override
  String get groupSettingsKeptOut => 'Отстранены';

  @override
  String get groupSettingsAccessEveryoneLong => 'Все в группе';

  @override
  String get groupSettingsAccessChosen => 'Только выбранные мной участники';

  @override
  String get groupSettingsAccessNote =>
      'Вы и те, кто выше вас, всегда можете войти. Те, кого не выбрали, видят комнату с замком.';

  @override
  String groupSettingsYou(String name) {
    return '$name (вы)';
  }

  @override
  String groupSettingsKeptOutBy(String room, String by) {
    return ' · $room · кто отстранил: $by';
  }

  @override
  String groupSettingsTimeLeft(String time) {
    return 'Осталось $time';
  }

  @override
  String get groupSettingsUntilLetBack => 'До разрешения';

  @override
  String get groupSettingsLetBack => 'Вернуть';

  @override
  String groupSettingsManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Менять группу (название, комнаты, приглашения и ограничения) могут только первые в списке: $n. Вы управляете теми, кто ниже вас.',
      many:
          'Менять группу (название, комнаты, приглашения и ограничения) могут только первые в списке: $n. Вы управляете теми, кто ниже вас.',
      few:
          'Менять группу (название, комнаты, приглашения и ограничения) могут только первые в списке: $n. Вы управляете теми, кто ниже вас.',
      one:
          'Менять группу (название, комнаты, приглашения и ограничения) могут только первые в списке: $n. Вы управляете теми, кто ниже вас.',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsMembersCount(int n, int max) {
    String _temp0 = intl.Intl.pluralLogic(
      max,
      locale: localeName,
      other: '$max участника',
      many: '$max участников',
      few: '$max участников',
      one: '$max участника',
    );
    return '$n из $_temp0, по порядку';
  }

  @override
  String get groupSettingsMembersNote =>
      'Каждый управляет теми, кто ниже: перемещает их сразу под себя или в конец, временно отстраняет от комнаты или удаляет. Новички начинают с конца.';

  @override
  String get groupSettingsWhoManages => 'Кто управляет группой';

  @override
  String get groupSettingsManagersEveryone => 'Все';

  @override
  String groupSettingsManagersFirstN(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Первые в списке: $n',
      many: 'Первые в списке: $n',
      few: 'Первые в списке: $n',
      one: 'Первые в списке: $n',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsManagersMore(int n, int members) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members участника',
      many: '$members участников',
      few: '$members участника',
      one: '$members участник',
    );
    return 'Выбрано первых $n — больше, чем сейчас в группе ($_temp0): пока это все.';
  }

  @override
  String get groupSettingsManagersEveryoneNote =>
      'Управляющие меняют название, изображения, комнаты и ограничения и впускают людей. Когда управляют все, группа продолжит жить, кто бы ни потерял своё устройство.';

  @override
  String groupSettingsManagersFirstNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Только первые в списке ($n) меняют название, изображения, комнаты и ограничения и впускают людей. Если их устройства будут потеряны, группой больше никто не сможет управлять: у Fogata нет сервера, который мог бы это исправить.',
      many:
          'Только первые в списке ($n) меняют название, изображения, комнаты и ограничения и впускают людей. Если их устройства будут потеряны, группой больше никто не сможет управлять: у Fogata нет сервера, который мог бы это исправить.',
      few:
          'Только первые в списке ($n) меняют название, изображения, комнаты и ограничения и впускают людей. Если их устройства будут потеряны, группой больше никто не сможет управлять: у Fogata нет сервера, который мог бы это исправить.',
      one:
          'Только первые в списке ($n) меняют название, изображения, комнаты и ограничения и впускают людей. Если их устройства будут потеряны, группой больше никто не сможет управлять: у Fogata нет сервера, который мог бы это исправить.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsMoveUp => 'Выше';

  @override
  String groupSettingsMoveUpTooltip(String name) {
    return 'Переместить участника $name сразу под вас';
  }

  @override
  String get groupSettingsToEnd => 'В конец';

  @override
  String groupSettingsToEndTooltip(String name) {
    return 'Переместить участника $name в конец';
  }

  @override
  String get groupSettingsTimeOut => 'Временно отстранить от комнаты';

  @override
  String get groupSettingsRemove => 'Удалить из группы';

  @override
  String get groupSettingsAskingToJoin => 'Просят впустить';

  @override
  String get groupSettingsInviteFriend => 'Пригласить друга';

  @override
  String get groupSettingsInviteFriendNote =>
      'Они получат приглашение в Fogata и сами решат, вступать ли. Здесь показаны только ваши друзья.';

  @override
  String get groupSettingsInviteLink => 'Ссылка-приглашение';

  @override
  String groupSettingsInviteLinkNote(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'кто-нибудь из группы',
      'first': 'первый участник группы',
      'other': 'один из первых $n участников группы',
    });
    return 'Для того, кто ещё не ваш друг. Ссылка действует один раз в течение 24 часов, и $_temp0 впускает человека: если она попадёт к кому-то другому, вы увидите, кто просит, и сможете отказать.';
  }

  @override
  String get groupSettingsCopyLink => 'Копировать ссылку-приглашение';

  @override
  String groupSettingsOpenLinks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n ссылки ещё не использовано.',
      many: '$n ссылок ещё не использовано.',
      few: '$n ссылки ещё не использованы.',
      one: '$n ссылка ещё не использована.',
      zero: 'Нет ссылок, ожидающих использования.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsCancelLinks => 'Отменить их';

  @override
  String get groupSettingsNoFriends =>
      'Друзей пока нет: добавьте их в разделе «Друзья».';

  @override
  String get groupSettingsAllFriendsIn => 'Все ваши друзья уже в группе.';

  @override
  String get groupSettingsFriendOnline => 'В сети';

  @override
  String get groupSettingsFriendOffline => 'Сейчас не в сети';

  @override
  String get groupSettingsInviteButton => 'Пригласить';

  @override
  String get groupSettingsGroupFull => 'Группа заполнена';

  @override
  String get groupSettingsSayNo => 'Отказать';

  @override
  String get groupSettingsLetIn => 'Впустить';

  @override
  String get groupSettingsLimitGroup => 'Людей в группе';

  @override
  String groupSettingsLimitGroupNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n человека',
      many: '$n человек',
      few: '$n человека',
      one: '$n человек',
    );
    return 'От 2 до 20. Когда группа заполнена, приглашения и ссылки перестают работать. Нельзя выбрать меньше, чем сейчас в группе: $_temp0.';
  }

  @override
  String get groupSettingsLimitRoom => 'Людей в каждой голосовой комнате';

  @override
  String get groupSettingsLimitRoomNote =>
      'От 2 до 8. Голос и видео идут напрямую между всеми в комнате, поэтому 8 — максимум, при котором звук хороший; с включёнными камерами лучше меньше. Тот, кто придёт, когда комната заполнена, увидит, что мест нет.';

  @override
  String get shellChipCallMany =>
      'Напрямую · зашифровано · только участники звонка';

  @override
  String get shellChipCallTwo => 'Напрямую · зашифровано · только вы двое';

  @override
  String get shellGroupMembers => 'Участники группы';

  @override
  String get shellMembers => 'Участники';

  @override
  String get shellDirectMessages => 'Личные сообщения';

  @override
  String get shellPathDirect => 'Напрямую · зашифровано';

  @override
  String shellPathVia(String name) {
    return 'Через $name · зашифровано';
  }

  @override
  String get shellPathViaSomeone => 'Через участника · зашифровано';

  @override
  String shellCallWith(String names) {
    return 'Звонок: $names';
  }

  @override
  String get shellRinging => 'Вызов · ';

  @override
  String get shellInCall => 'В звонке · ';

  @override
  String get shellCallEnded => 'Звонок завершён';

  @override
  String get shellNewGroup => 'Новая группа';

  @override
  String get shellSectionCall => 'Звонок';

  @override
  String shellCallRinging(String name) {
    return '$name · вызов';
  }

  @override
  String get shellSectionLiveNow => 'Сейчас горят';

  @override
  String get shellSectionInCall => 'В звонке';

  @override
  String get shellChat => 'Чат';

  @override
  String get shellFiles => 'Файлы';

  @override
  String get shellSectionGroup => 'Группа';

  @override
  String get shellInvite => 'Пригласить';

  @override
  String get shellSectionVoiceRooms => 'Голосовые комнаты';

  @override
  String shellVoiceRoomYouAreIn(String room) {
    return '$room · вы здесь';
  }

  @override
  String get shellNotForYou => 'Недоступно';

  @override
  String get shellOnlySomeMembers => 'Только для некоторых участников';

  @override
  String shellPersonYou(String name) {
    return '$name (вы)';
  }

  @override
  String get shellLive => 'ЭФИР';

  @override
  String get shellYourProfile => 'Ваш профиль';

  @override
  String shellMuteMic(String keys) {
    return 'Выключить микрофон ($keys)';
  }

  @override
  String shellUnmuteMic(String keys) {
    return 'Включить микрофон ($keys)';
  }

  @override
  String get shellDeafen => 'Выключить звук';

  @override
  String get shellUndeafen => 'Снова слышать других';

  @override
  String get shellStateCalling => 'Вызов…';

  @override
  String get shellStateInCall => 'В звонке';

  @override
  String get shellStateInRoom => 'В комнате';

  @override
  String get shellStateOffline => 'Невидимка';

  @override
  String get shellStateOnline => 'В сети';

  @override
  String get shellStateAway => 'Нет на месте';

  @override
  String get shellShowInFolder => 'Показать в папке';

  @override
  String get shellCopyPath => 'Копировать путь';

  @override
  String shellVolumePercent(int n) {
    return '$n%';
  }

  @override
  String get shellMuteKeys => 'Ctrl+Shift+M';

  @override
  String get settingsSectionDonate => 'Поддержать Fogata';

  @override
  String get settingsSectionReport => 'Сообщить о проблеме';

  @override
  String get settingsHeadingHelp => 'Помощь';

  @override
  String get settingsPreRelease => 'Предрелиз';

  @override
  String get donateTitle => 'Помогите Fogata оставаться независимой';

  @override
  String get donateText =>
      'Fogata бесплатна, в ней нет рекламы и серверов, которые за вами следят. Ваше пожертвование помогает ей такой оставаться.';

  @override
  String get donateButton => 'Пожертвовать';

  @override
  String get donateNote =>
      'Ko-fi откроется в браузере. Fogata никогда не видит ваш платёж.';

  @override
  String get reportWhatTitle => 'Что случилось?';

  @override
  String get reportWhatNote =>
      'Что вы делали, чего ожидали и что увидели вместо этого.';

  @override
  String get reportIncludeLog =>
      'Приложить технический журнал за последние 2 дня';

  @override
  String get reportLogNote =>
      'Версии, ошибки и шаги подключения. Никогда — ваши сообщения, ключи, контакты или IP-адреса.';

  @override
  String get reportSeeSent => 'Что отправляется';

  @override
  String get reportHideSent => 'Скрыть список';

  @override
  String get reportPreview => 'Прочитать весь отчёт перед отправкой';

  @override
  String get reportHidePreview => 'Скрыть отчёт';

  @override
  String get reportPreviewNote =>
      'Это ровно то, что лежит в файле, и больше ничего не отправляется. Имена, папки, имена файлов, адреса и ключи уже удалены из журнала.';

  @override
  String get reportPreviewWorking => 'Подготовка отчёта…';

  @override
  String get reportSentWhat => 'То, что вы написали выше.';

  @override
  String get reportSentVersion =>
      'Версия и сборка Fogata, ваша система и её версия, а также язык Fogata.';

  @override
  String get reportSentLog =>
      'Журнал за последние 2 дня: когда запускалась Fogata, её ошибки и как она связывалась с другими устройствами. Имена, сообщения, папки, имена файлов, ссылки, коды, ключи и IP-адреса удаляются, а каждое устройство показано только номером, который ничего не значит вне отчёта.';

  @override
  String get reportEmail => 'Отправить письмом команде Fogata';

  @override
  String get reportSave => 'Сохранить в файл';

  @override
  String reportHowEmail(String address) {
    return 'Почтовое приложение откроется с готовым письмом на $address, а рядом — папка с файлом отчёта: приложите файл и отправьте. Ничего не покинет ваш компьютер, пока вы этого не сделаете.';
  }

  @override
  String get reportReadyTitle => 'Письмо готово';

  @override
  String reportReadyText(String file, String address) {
    return 'Приложите $file из открывшейся папки и отправьте на $address.';
  }

  @override
  String get reportShowFile => 'Показать файл';

  @override
  String get reportCopyAddress => 'Копировать адрес';

  @override
  String get reportAddressCopied => 'Адрес скопирован';

  @override
  String reportSaved(String file) {
    return 'Отчёт сохранён: $file';
  }

  @override
  String reportNoMailApp(String address) {
    return 'Почтовое приложение не открылось. Отправьте файл на $address из своей почты.';
  }

  @override
  String reportFailed(String reason) {
    return 'Не удалось записать отчёт: $reason';
  }

  @override
  String get splashStarting => 'Запуск';

  @override
  String get splashConnecting => 'Подключение';

  @override
  String get splashChecking => 'Проверка обновлений';

  @override
  String get splashDownloading => 'Загрузка обновления';

  @override
  String get splashInstalling => 'Установка обновления';

  @override
  String get splashRestarting => 'Перезапуск';

  @override
  String splashProgress(String done, String total) {
    return '$done из $total МБ';
  }

  @override
  String splashProgressFrom(String done, String total, String name) {
    return '$done из $total МБ · от: $name';
  }

  @override
  String updateReady(String version) {
    return 'Обновление готово · $version';
  }

  @override
  String updateRequired(String version) {
    return 'Нужно обновление · $version';
  }

  @override
  String get updateRestart => 'Перезапустить';

  @override
  String updateReadyTip(String version) {
    return 'Fogata $version готова. Перезапустите, чтобы её использовать.';
  }

  @override
  String get updateRequiredTip =>
      'Эта версия больше не работает с более новой версией у ваших друзей. Перезапустите, чтобы обновиться.';

  @override
  String updateFailed(String reason) {
    return 'Не удалось установить обновление: $reason';
  }

  @override
  String get errJoinInviteGone =>
      'Это приглашение больше не действует, или в группе нет мест.';

  @override
  String get errJoinNoKeyYet =>
      'У этого устройства ещё нет ключа группы. Новый участник войдёт, как только ключ появится.';

  @override
  String get errJoinWroteTooMuch =>
      'Это устройство внесло изменения, которые другие участники ещё не получили. Нового участника можно будет впустить, когда кто-то ещё из группы будет в сети.';

  @override
  String get errJoinAskAgain =>
      'Этот запрос на вступление относится к прежнему состоянию группы. Человеку нужно отправить запрос заново.';

  @override
  String get errNoSpaceForFile =>
      'Для этого файла недостаточно свободного места. Освободите место и повторите попытку.';

  @override
  String get recoveryTitle => 'Резервная копия';

  @override
  String get recoveryNote =>
      'Файл, который вернёт этот аккаунт, если связка ключей или диск этого компьютера пропадут. Открыть его можно только вашим ключом восстановления.';

  @override
  String get recoveryOff => 'Выключена';

  @override
  String recoveryOn(String id) {
    return 'Включена · ключ $id';
  }

  @override
  String get recoveryTurnOn => 'Включить…';

  @override
  String get recoveryChangeKey => 'Сменить ключ…';

  @override
  String get recoveryMakeCopy => 'Сделать копию…';

  @override
  String get recoveryReplacedKept =>
      'На этом компьютере всё ещё хранится аккаунт, который заменило восстановление.';

  @override
  String get recoveryDeleteReplaced => 'Удалить…';

  @override
  String get recoveryChangeTitle => 'Создать новый ключ восстановления?';

  @override
  String get recoveryChangeText =>
      'Новые копии будут открываться только новым ключом. Сделанные раньше по-прежнему открываются только старым: храните его, пока храните эти копии.';

  @override
  String get recoveryChangeConfirm => 'Создать новый ключ';

  @override
  String get recoveryKeyTitle => 'Ваш ключ восстановления';

  @override
  String get recoveryKeyText =>
      'Запишите или распечатайте его и храните отдельно от копий. Он показывается только сейчас.';

  @override
  String recoveryKeyName(String id) {
    return 'Его короткое имя: $id';
  }

  @override
  String get recoveryKeyWarning =>
      'Fogata нигде не хранит этот ключ: без него ни одна копия не откроется. Тот, у кого есть копия и этот ключ, получит ваш аккаунт.';

  @override
  String get recoveryKeyWritten => 'Ключ записан';

  @override
  String get recoveryCheckTitle => 'Проверьте, что записали';

  @override
  String get recoveryCheckText =>
      'Введите эти две группы ключа так, как вы их записали.';

  @override
  String recoveryCheckGroup(int n) {
    return 'Группа $n';
  }

  @override
  String get recoveryCheckWrong =>
      'Не совпадает: посмотрите ещё раз, что вы записали.';

  @override
  String get recoveryTurnOnButton => 'Включить копии';

  @override
  String recoveryTurnedOn(String id) {
    return 'Копии включены (ключ $id).';
  }

  @override
  String get recoveryCopyTitle => 'Сделать резервную копию';

  @override
  String get recoveryCopyText =>
      'Пока делается копия, Fogata не в сети, а с файлами из чатов — дольше. Храните файл не на этом компьютере, например на флешке.';

  @override
  String get recoveryCopyAttachments => 'Добавить файлы, отправленные в чатах';

  @override
  String recoveryCopyAttachmentsNote(String size) {
    return 'Изображения, видео и файлы: ещё $size.';
  }

  @override
  String get recoveryCopySave => 'Выбрать место…';

  @override
  String get recoveryCopyInDataFolder =>
      'Копию нельзя хранить в папке данных Fogata. Выберите другое место.';

  @override
  String recoveryCopyMade(String file, String size) {
    return 'Копия сохранена: $file ($size).';
  }

  @override
  String recoveryCopyFailed(String why) {
    return 'Не удалось сделать копию. $why';
  }

  @override
  String get recoveryDeleteTitle => 'Удалить заменённый аккаунт?';

  @override
  String get recoveryDeleteText =>
      'Его отложили, когда на его место восстановили копию. После удаления его не вернуть.';

  @override
  String get recoveryDelete => 'Удалить';

  @override
  String get recoveryDeleted => 'Заменённый аккаунт удалён.';

  @override
  String get recoveryFileType => 'Копия Fogata';

  @override
  String get recoveryLostComputer => 'Потеряли компьютер?';

  @override
  String get recoveryRestoreLink => 'Восстановить из копии';

  @override
  String get recoveryRestoreTitle => 'Восстановить из копии';

  @override
  String get recoveryRestoreSub =>
      'Выберите сохранённую резервную копию, затем введите ключ восстановления.';

  @override
  String get recoveryChooseCopy => 'Выбрать копию…';

  @override
  String recoveryMadeOn(String date) {
    return 'Копия от $date.';
  }

  @override
  String get recoveryOtherKey =>
      'Она сделана для другого ключа восстановления, не того, что на этом компьютере. Введите ключ, для которого она сделана.';

  @override
  String get recoveryKeyField => 'Ключ восстановления';

  @override
  String get recoveryKeyNotAKey =>
      'В ключе восстановления 53 буквы и цифры, группами по четыре.';

  @override
  String get recoveryKeyMistyped =>
      'Где-то неверная буква или цифра: проверьте ввод.';

  @override
  String get recoveryProblemNotACopy => 'Это не копия аккаунта Fogata.';

  @override
  String get recoveryProblemNewer =>
      'Эта копия сделана более новой версией Fogata. Обновите Fogata, чтобы открыть её.';

  @override
  String get recoveryProblemOtherKey =>
      'Эта копия сделана для другого ключа восстановления.';

  @override
  String get recoveryProblemDamaged =>
      'Эта копия повреждена или изменена после создания.';

  @override
  String get recoveryProblemUnproven =>
      'Эту копию сделал не тот аккаунт, который в ней, поэтому она не открывается.';

  @override
  String recoveryWhose(String name) {
    return 'Аккаунт: $name';
  }

  @override
  String recoveryHolds(int files, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files файла',
      many: '$files файлов',
      few: '$files файла',
      one: '$files файл',
    );
    return '$_temp0, всего $size';
  }

  @override
  String get recoveryWithAttachments => 'С файлами, отправленными в чатах.';

  @override
  String get recoveryWithoutAttachments => 'Без файлов, отправленных в чатах.';

  @override
  String get recoveryOnlyIf =>
      'Используйте копию, только если этот аккаунт больше не используется ни на одном другом компьютере. Если используется, лучше перенесите аккаунт оттуда: один аккаунт не может быть на двух компьютерах.';

  @override
  String get recoveryMoveInstead => 'Лучше перенести';

  @override
  String recoveryHereNamed(String name) {
    return 'На этом компьютере есть аккаунт: $name.';
  }

  @override
  String recoveryHereDevice(String fingerprint) {
    return 'На этом компьютере есть аккаунт без профиля (устройство $fingerprint).';
  }

  @override
  String get recoveryHereUnknown =>
      'На этом компьютере есть часть аккаунта, которая не открывается.';

  @override
  String get recoveryHereReplaced =>
      'Восстановление заменит его. Он будет отложен, а не удалён, пока вы не удалите его в разделе «Настройки › Хранилище и загрузки».';

  @override
  String get recoveryRestore => 'Восстановить';

  @override
  String get recoveryReplace => 'Заменить и восстановить';

  @override
  String recoveryRestoring(String name) {
    return 'Восстанавливается аккаунт: $name…';
  }

  @override
  String recoveryRestored(String name) {
    return 'Аккаунт восстановлен: $name.';
  }

  @override
  String get recoveryRefusedAccountHere =>
      'Теперь на этом компьютере есть аккаунт. Выберите копию ещё раз, чтобы увидеть, что она заменит.';

  @override
  String get recoveryRefusedReplacedKept =>
      'На этом компьютере всё ещё хранится аккаунт, который заменило прошлое восстановление. Удалите его, чтобы восстановить эту копию.';

  @override
  String recoveryRefusedNoRoom(String need, String free) {
    return 'Копии нужно $need на этом диске, а свободно $free. Ничего не изменилось.';
  }

  @override
  String get recoveryRefusedInFolder =>
      'Копия лежит в папке данных Fogata. Переместите её в другое место и выберите снова.';

  @override
  String get recoveryBusy =>
      'Fogata занята резервной копией или переносом. Повторите попытку, когда она закончит.';

  @override
  String get moveBusyWithCopy =>
      'Fogata занята резервной копией. Перенесите аккаунт, когда она закончит.';

  @override
  String get recoveryDeleteKeptAside => 'Удалить отложенный аккаунт…';

  @override
  String get reachOffline =>
      'Отсюда Fogata не может связаться ни с кем в интернете. Проверьте подключение: возможно, Fogata блокирует брандмауэр.';

  @override
  String get reachFirewall =>
      'Брандмауэр этого компьютера ничего не пропускает к Fogata. Команда, которая впустит Fogata, есть в уведомлениях.';

  @override
  String get reachUdpBlocked =>
      'Эта сеть или этот компьютер блокирует UDP. Он нужен для звонков и прямых соединений через роутеры; пока он не разрешён, другие могут связаться с вами только через участника группы или с публичного адреса.';

  @override
  String get reachSymmetricNat =>
      'Ваш роутер выдаёт разный публичный порт для каждого адресата (симметричный NAT), поэтому прямой путь через него открыть нельзя. Друзья за другим роутером могут связаться с вами только через участника общей группы.';

  @override
  String get reachNoIpv6 =>
      'Работающего IPv6, с которым можно было бы обойти роутер, тоже нет.';

  @override
  String reachFineFriend(String name) {
    return 'С вашей сетью для Fogata, похоже, всё в порядке, так что помеха, скорее всего, на стороне пользователя $name (не в сети или проблемы с сетью).';
  }

  @override
  String get reachFineOthers =>
      'С вашей сетью для Fogata, похоже, всё в порядке, так что помеха, скорее всего, на другой стороне.';

  @override
  String directDropTitle(String name) {
    return 'Отпустите, чтобы отправить: $name';
  }

  @override
  String get directDropNote =>
      'Они ждут в поле сообщения: там можно добавить сообщение и проверить файлы перед отправкой.';

  @override
  String directPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n скрытых или личных файла не добавлены: такие файлы не добавляются перетаскиванием, ведь перетаскивание может начать веб-страница. Чтобы отправить их, выберите их кнопкой +.',
      many:
          '$n скрытых или личных файлов не добавлены: такие файлы не добавляются перетаскиванием, ведь перетаскивание может начать веб-страница. Чтобы отправить их, выберите их кнопкой +.',
      few:
          '$n скрытых или личных файла не добавлены: такие файлы не добавляются перетаскиванием, ведь перетаскивание может начать веб-страница. Чтобы отправить их, выберите их кнопкой +.',
      one:
          '$n скрытый или личный файл не добавлен: такие файлы не добавляются перетаскиванием, ведь перетаскивание может начать веб-страница. Чтобы отправить его, выберите его кнопкой +.',
    );
    return '$_temp0';
  }

  @override
  String appMustUpdateToJoin(String name, String group) {
    return '$name нужно обновить Fogata, прежде чем вступить в «$group». После обновления всё получится как обычно.';
  }

  @override
  String appUpdateToJoin(String group) {
    return 'Для «$group» нужна более новая версия Fogata. Обновите Fogata, чтобы вступить.';
  }

  @override
  String get errMustUpdateToJoin =>
      'Вашему другу нужно обновить Fogata, прежде чем вступить в эту группу.';

  @override
  String groupSettingsLevelWaiting(String names) {
    return 'Эта группа ждёт, чтобы включить более сильную защиту. Она включится сама, когда все обновят Fogata. Ещё не обновили: $names';
  }
}
