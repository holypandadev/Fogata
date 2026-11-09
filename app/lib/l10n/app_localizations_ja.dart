// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Japanese (`ja`).
class L10nJa extends L10n {
  L10nJa([String locale = 'ja']) : super(locale);

  @override
  String get commonCancel => 'キャンセル';

  @override
  String get commonSave => '保存';

  @override
  String get commonClose => '閉じる';

  @override
  String commonNewMessageFrom(String name, String text) {
    return '$nameからの新しいメッセージ：$text';
  }

  @override
  String commonNewMessagesFrom(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$nameからの新しいメッセージ$n件',
    );
    return '$_temp0';
  }

  @override
  String commonNewMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '新しいメッセージ$n件',
    );
    return '$_temp0';
  }

  @override
  String get commonAccept => '承認';

  @override
  String get commonDecline => '拒否';

  @override
  String get commonSend => '送信';

  @override
  String get commonTryAgain => '再試行';

  @override
  String get commonCopy => 'コピー';

  @override
  String get commonCopied => 'コピー済み';

  @override
  String get commonBack => '戻る';

  @override
  String get commonDiscard => '破棄';

  @override
  String get commonKeepEditing => '編集を続ける';

  @override
  String get commonNotNow => '後で';

  @override
  String get commonDownload => 'ダウンロード';

  @override
  String get commonSettings => '設定';

  @override
  String get commonHome => 'ホーム';

  @override
  String get commonFriends => 'フレンド';

  @override
  String get commonContinue => '続ける';

  @override
  String get commonDone => '完了';

  @override
  String get commonRemove => '削除';

  @override
  String get commonYou => 'あなた';

  @override
  String timeYesterdayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': '昨日 $time',
      'other': '昨日 $time',
    });
    return '$_temp0';
  }

  @override
  String timeDayAt(String date, String time) {
    return '$date $time';
  }

  @override
  String get timeNow => 'たった今';

  @override
  String timeMinutes(int n) {
    return '$n分';
  }

  @override
  String timeHours(int n) {
    return '$n時間';
  }

  @override
  String timeHoursMinutes(int hours, int minutes) {
    return '$hours時間$minutes分';
  }

  @override
  String timeDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n日');
    return '$_temp0';
  }

  @override
  String spanMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n分間');
    return '$_temp0';
  }

  @override
  String spanHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n時間');
    return '$_temp0';
  }

  @override
  String spanDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n日間');
    return '$_temp0';
  }

  @override
  String spanWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n週間');
    return '$_temp0';
  }

  @override
  String get agoMoment => 'たった今';

  @override
  String agoMinutes(int n) {
    return '$n分前';
  }

  @override
  String agoHours(int n) {
    return '$n時間前';
  }

  @override
  String get agoYesterday => '昨日';

  @override
  String agoDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n日前');
    return '$_temp0';
  }

  @override
  String agoWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n週間前',
    );
    return '$_temp0';
  }

  @override
  String agoMonths(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$nか月前',
    );
    return '$_temp0';
  }

  @override
  String get agoOverYear => '1年以上前';

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
    return '$firstと$second';
  }

  @override
  String listMore(String rest, String last) {
    return '$restと$last';
  }

  @override
  String get listSeparator => '、';

  @override
  String ordinal(int n) {
    return '$n番目';
  }

  @override
  String get errLinkExpired => 'リンクの有効期限が切れています。新しいリンクを送ってもらってください。';

  @override
  String get errGroupInvite => 'これはグループへの招待リンクです。';

  @override
  String get errDeviceNotFound => '相手のデバイスが見つかりませんでした。お互いにオンラインか確認してください。';

  @override
  String get errNotFriendCode => 'Fogataのリンクやコードではないようです。';

  @override
  String get errAlreadyInGroup => 'すでにそのグループに参加しています。';

  @override
  String get errJoinRoomFirst => '先にルームに入ってください。';

  @override
  String get errNewerVersion =>
      'このプロフィールは新しいバージョンのFogataで保存されています。開くにはFogataをアップデートしてください。';

  @override
  String get errKeychainUnreachable =>
      'このパソコンのキーチェーンからプロフィールの鍵を取得できませんでした。キーチェーンのロックを解除するか、Fogataに使用を許可してから、もう一度お試しください。';

  @override
  String get errKeychainNoKey =>
      'このパソコンのキーチェーンにプロフィールの鍵がもうないため、Fogataはプロフィールを開けません。';

  @override
  String get errKeychainWrongKey => 'このパソコンのキーチェーンにある鍵では、プロフィールを開けません。';

  @override
  String get errNoGroupKey => 'このグループはまだこのパソコンで準備ができていません。少ししてからもう一度お試しください。';

  @override
  String get errAlreadyRunning => 'このプロフィールでFogataがすでに開いています。先にもう一方を閉じてください。';

  @override
  String get errFileGone => 'そのファイルはもうルームにありません。';

  @override
  String get errNobodyHasFile => '今そのファイルを持っている人がオンラインにいません。';

  @override
  String get errStillStarting => 'Fogataはまだ起動中です。';

  @override
  String get errIdentityExists => 'このパソコンにはすでにIDがあります。その鍵は変更できません。';

  @override
  String get errNameRule => '表示される文字で、1〜40文字の名前を入力してください。';

  @override
  String get errBioRule => '自己紹介は4行・190文字までで、見えない文字は使えません。';

  @override
  String get errPictureRule => '画像は64 KB以下のPNGかJPEGにしてください。';

  @override
  String get errNotFriendLink => 'Fogataのフレンドリンクではありません。';

  @override
  String get errLinkDamaged => 'リンクが不完全か、壊れています。';

  @override
  String get errLinkNeedsNewer => 'このリンクには新しいバージョンのFogataが必要です。';

  @override
  String get errCodeShape => 'コードはFGT-XXXXX-XXXXXの形式です。';

  @override
  String get errCodeWrong => 'コードが間違っているか、もう使えません。';

  @override
  String get errNotProven => '相手のデバイスが本人であることを証明できませんでした。';

  @override
  String get errCallGone => 'その通話の呼び出しはもう終わっています。';

  @override
  String get errAlreadyInCall => 'すでに通話中です。先に通話を終了してください。';

  @override
  String get errAlreadyFriends => 'すでにフレンドです。';

  @override
  String get errMemberNotConnected =>
      'そのメンバーは今つながっていません。オンラインになってからもう一度お試しください。';

  @override
  String get errMemberTooOld =>
      'そのメンバーのFogataは古いため、グループからのフレンド申請を受け取れません。代わりにフレンドリンクを送ってください。';

  @override
  String get errMemberAskLost => 'フレンド申請がそのメンバーに届きませんでした。もう一度お試しください。';

  @override
  String get errOwnLink => 'それはあなた自身のリンクです。追加したいフレンドに送ってください。';

  @override
  String get errLinkUsed => 'そのリンクは使用済みか、有効期限が切れています。新しいリンクを送ってもらってください。';

  @override
  String get errLinkExpiredUnreached =>
      '相手のデバイスに接続する前に、リンクの有効期限が切れました。新しいリンクを送ってもらってください。';

  @override
  String get errCodeUnanswered =>
      'コードに応答したデバイスがありません。コードと、相手のデバイスにまだ表示されているかを確認してください。';

  @override
  String get errRefused => '相手のデバイスに拒否されました。コードはすでに使われている可能性があります。';

  @override
  String get errNobodyLetIn => '誰も参加を承認しませんでした。グループの誰かがオンラインのときにもう一度お試しください。';

  @override
  String get errNotBelow => 'グループ内であなたより下の人にのみ実行できます。';

  @override
  String get errNotManager => 'グループの管理者のみ実行できます。';

  @override
  String get errLastManager => '自分自身をグループの管理者から外すことはできません。';

  @override
  String get errRoomNotEmpty => 'そのボイスルームには人がいます。空になってから削除してください。';

  @override
  String get errTooManyFiles =>
      'ルームで1人が共有できるファイル数の上限に達しています。先にいくつかの共有を停止してください。';

  @override
  String get errDiskFull => 'ディスクがいっぱいです。空き容量を確保してから、もう一度お試しください。';

  @override
  String get errTooManyChanges =>
      'このデバイスによるグループの変更で、他のメンバーにまだ取り込まれていないものが多すぎます。グループの他の誰かが変更を行ってから、もう一度お試しください。';

  @override
  String get errSomething => '問題が発生しました。';

  @override
  String errRoomFull(int n) {
    return 'そのボイスルームは満員です。最大$n人までです。';
  }

  @override
  String get findingOffline =>
      '公開ネットワークのピアから応答がありません。このパソコンがインターネットに接続できていないか、ファイアウォールがFogataをブロックしているようです。';

  @override
  String get findingUdpWorks => 'UDPは使えます。通話とルーター越しの接続に必要です。';

  @override
  String get findingUdpBlocked =>
      'このネットワークまたはこのパソコンがUDPをブロックしているようです（TCPで応答した相手がQUICでは応答しませんでした）。チャットは他のメンバー経由で届きますが、ルーター越しの直接接続や通話はほとんど失敗します。ファイアウォールやネットワークのルールを確認してください。';

  @override
  String get findingCarrierNat =>
      'このパソコンのアドレスはキャリアグレードNAT（100.64.0.0/10）です。プロバイダーが1つのグローバルアドレスを多くの契約者で共有しています。';

  @override
  String get findingPublicIpv4 => 'このパソコンにはグローバルIPv4アドレスがあります。他の人から直接接続できます。';

  @override
  String get findingHardNat => '宛先ごとに異なる公開ポートを割り当てるルーター（ハードNAT）の内側にあります。';

  @override
  String get findingEasyNat =>
      'どの宛先にも同じ公開ポートを使うルーター（イージーNAT）の内側にあります。両側から同時に開くことができます。';

  @override
  String get findingUnknownRouter =>
      'このパソコンのグローバルアドレスを確認したピアが少なすぎるため、ルーターの種類を判断できません。';

  @override
  String get findingNoIpv6 =>
      'グローバルIPv6アドレスがありません。Fogataに必須ではありませんが、あると直接接続しやすくなります。';

  @override
  String get findingIpv6Works =>
      'グローバルIPv6が使えます。IPv6を持つフレンドとは、ルーターを介さずに接続できます。';

  @override
  String get findingIpv6Unreached =>
      'このパソコンにはグローバルIPv6アドレスがありますが、IPv6で接続してきたピアはまだありません。';

  @override
  String get findingSummaryNoUdp => 'まとめ：UDPが許可されるまで、直接接続には問題が出そうです。';

  @override
  String get findingSummaryPublic => 'まとめ：フレンドはこのパソコンに直接接続できるはずです。';

  @override
  String get findingSummaryEasyNat =>
      'まとめ：ほとんどのフレンドとは、両方のルーターを同時に開いて直接接続できるはずです。ハードNATの内側にいるフレンドとは、間にグループのメンバーが必要です。';

  @override
  String get findingSummaryHardNatWithIpv6 =>
      'まとめ：IPv6かグローバルアドレスを持つフレンドとは直接接続できます。それ以外のフレンドとは、間にグループのメンバーが必要です。';

  @override
  String get findingSummaryHardNat =>
      'まとめ：直接接続できるのは、グローバルアドレスを持つフレンドだけです。それ以外のフレンドとは、間にグループのメンバーが必要です。';

  @override
  String get findingSummaryUnknown => 'まとめ：情報が足りません。1分後にもう一度確認してください。';

  @override
  String findingInternet(int n) {
    return 'インターネット：このパソコンは公開ネットワークに接続できています（既知のピア：$n）。';
  }

  @override
  String findingDns(String detail) {
    return 'DNS：システム設定の全体を読み取れませんでした（$detail）';
  }

  @override
  String findingLanDevices(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n台');
    return 'このローカルネットワーク上のFogataデバイス：$_temp0。ここにいるフレンドとは、インターネットを経由せずに接続できます。';
  }

  @override
  String findingFirewall(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': '管理者として実行したPowerShellで',
      'macos': 'ターミナルで',
      'other': 'ターミナルで',
    });
    return 'このパソコンのファイアウォールがFogataへの接続をすべてブロックしているため、同じ場所で別のルーターの内側にいるフレンド（たとえば自宅にルーターが2台ある場合）は接続できません。$_temp0次のコマンドを一度実行し、Fogataを許可してください：';
  }

  @override
  String get settingsLanguage => '言語';

  @override
  String get settingsLanguageNote => 'Fogata自体の表示言語です。メッセージは書かれたときのまま表示されます。';

  @override
  String get settingsLanguageChoose => '言語を選択';

  @override
  String get settingsTime => '時刻';

  @override
  String get settingsTimeNote => 'チャット、通話、リストでの時刻の表示方法です。';

  @override
  String settingsTime24(String example) {
    return '24時間 · $example';
  }

  @override
  String settingsTime12(String example) {
    return '12時間 · $example';
  }

  @override
  String firewallNoticeText(String system, String command) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': '管理者として実行したPowerShellで',
      'macos': 'ターミナルで',
      'other': 'ターミナルで',
    });
    return '同じ場所で別のルーターの内側にいるフレンドが、このパソコンに接続できません。$_temp0一度実行してください：$command';
  }

  @override
  String firewallCopied(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': '管理者として実行したPowerShellに',
      'macos': 'ターミナルに',
      'other': 'ターミナルに',
    });
    return 'コピーしました。$_temp0貼り付けて実行してください。';
  }

  @override
  String get appRestarted => 'Fogataが停止したため、再起動しました。';

  @override
  String appVoiceRoomInGroup(String room, String group) {
    return '$groupの$room';
  }

  @override
  String appBothIn(String room) {
    return '2人とも$roomにいます。';
  }

  @override
  String appHungUp(String name) {
    return '$nameが通話を終了しました';
  }

  @override
  String get appYourFriend => 'フレンド';

  @override
  String get appCallEndedAlone => '誰も戻らなかったため、通話が終了しました';

  @override
  String get appAnsweredElsewhere => '別のパソコンで応答しました';

  @override
  String appVoiceRoomDeleted(String room) {
    return '「$room」は削除されました。';
  }

  @override
  String get appSomeone => '誰か';

  @override
  String get appAGroup => 'あるグループ';

  @override
  String appFriendRequest(String name) {
    return '$nameからフレンド申請が届きました。';
  }

  @override
  String appRequestDelivered(String name) {
    return '$nameに申請が届きました。相手の返事を待っています。';
  }

  @override
  String appFriendsNow(String name) {
    return '$nameとフレンドになりました。';
  }

  @override
  String appJoinedGroup(String group) {
    return '「$group」に参加しました。';
  }

  @override
  String appRemovedFromGroup(String group) {
    return '「$group」のメンバーではなくなりました。';
  }

  @override
  String appAddedToGroup(String name, String group) {
    return '$nameがあなたを「$group」に追加しました。参加するかどうかはあなた次第です。';
  }

  @override
  String appAsksToJoin(String name, String group) {
    return '$nameが「$group」への参加を希望しています。';
  }

  @override
  String appJoinWaiting(String group) {
    return '「$group」の誰かが参加を承認するのを待っています。';
  }

  @override
  String appJoinDeclined(String group) {
    return '「$group」の誰かが参加を断りました。その招待はもう使えません。';
  }

  @override
  String appDownloadedTo(String path) {
    return '$pathにダウンロードしました';
  }

  @override
  String appDownloadFailed(String why) {
    return 'ダウンロードできませんでした：$why';
  }

  @override
  String get appOtherSession =>
      'このIDは別のパソコンでも開かれています。そちらのFogataを閉じてください。1つのIDは同時に1台のパソコンでしか使えません。';

  @override
  String get appFriendsRestored =>
      'このパソコンのフレンドリストが破損していたため、最後の変更前に作成されたコピーから復元しました。直前に追加または削除したフレンドは、もう一度追加または削除が必要な場合があります。';

  @override
  String appInvitationRanOut(String group) {
    return 'グループの誰かが承認する前に、「$group」への招待の有効期限が切れました。新しい招待をもらってください。';
  }

  @override
  String appTakenOut(String who, String room) {
    return '$whoがあなたを$roomから退出させました。';
  }

  @override
  String appRoomForSome(String room) {
    return '$roomは現在、一部のメンバー専用です。';
  }

  @override
  String appTakenOutUntilLetBack(String who, String room) {
    return '$whoがあなたを$roomから退出させました。あなたより上の人が許可するまで戻れません。';
  }

  @override
  String appTakenOutFor(String who, String room, String span) {
    return '$whoがあなたを$roomから退出させました。$spanは戻れません。';
  }

  @override
  String appTakenOutWhile(String who, String room) {
    return '$whoがあなたを$roomから退出させました。しばらくは戻れません。';
  }

  @override
  String appComeBackIn(String room, String wait) {
    return '$roomには$wait後に戻れます。';
  }

  @override
  String appRoomNotForYou(String room) {
    return '現在、$roomには入れません。';
  }

  @override
  String appFoldersNotShared(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '共有できるのはファイルだけです。$n個のフォルダーは除外されました。',
      one: '共有できるのはファイルだけです。フォルダーは除外されました。',
    );
    return '$_temp0';
  }

  @override
  String appSharingOne(String file) {
    return 'このパソコンから$fileを共有しています。ルームを退出すると利用できなくなります。';
  }

  @override
  String appSharingMany(int n) {
    return 'このパソコンから$n個のファイルを共有しています。ルームを退出すると利用できなくなります。';
  }

  @override
  String appSavedTo(String path) {
    return '$pathに保存しました';
  }

  @override
  String appAskingToJoin(String group) {
    return '「$group」への参加を申請しています。グループの誰かが確認して承認すると参加できます。';
  }

  @override
  String get appLookingForDevice => '申請を送るため、相手のデバイスを探しています…';

  @override
  String callStateNoAnswer(String name) {
    return '$nameは応答しませんでした';
  }

  @override
  String callStateDeclined(String name) {
    return '$nameは今は話せません';
  }

  @override
  String callStateBusy(String name) {
    return '$nameは別の通話中です';
  }

  @override
  String callStateOutdated(String name) {
    return '$nameが参加するには新しいFogataが必要です';
  }

  @override
  String callStateUnreached(String name) {
    return '$nameに接続できませんでした';
  }

  @override
  String get callStateReturn => '通話に戻る';

  @override
  String get callStateCallBack => '折り返す';

  @override
  String get callStateCall => '通話';

  @override
  String noticesDeclined(String name, String group) {
    return '$nameが「$group」への招待を断りました';
  }

  @override
  String noticesDeclinedToast(String name, String group) {
    return '$nameが「$group」への招待を断りました。';
  }

  @override
  String noticesMissedInCall(String name) {
    return '通話中に$nameから着信がありました';
  }

  @override
  String noticesMissedOffline(String name) {
    return 'オフライン表示中に$nameから着信がありました';
  }

  @override
  String noticesMissedCall(String name) {
    return '$nameからの不在着信';
  }

  @override
  String get contactsGeneral => '一般';

  @override
  String get directStateTimerOff => 'オフ';

  @override
  String get directStateTimer24h => '24時間';

  @override
  String directStateTimerDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n日');
    return '$_temp0';
  }

  @override
  String directStateDisappearsMinutes(int n) {
    return 'あと$n分で消えます';
  }

  @override
  String directStateDisappearsHours(int n) {
    return 'あと$n時間で消えます';
  }

  @override
  String directStateDisappearsDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'あと$n日で消えます',
    );
    return '$_temp0';
  }

  @override
  String get directStateNoLimit => '制限なし';

  @override
  String get directStateMessageGone => 'そのメッセージはもうありません。';

  @override
  String directStateFoldersNotSent(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '送信できるのはファイルだけです。$n個のフォルダーは除外されました。',
      one: '送信できるのはファイルだけです。フォルダーは除外されました。',
    );
    return '$_temp0';
  }

  @override
  String directStateFileFailed(String why) {
    return 'ファイルを取得できませんでした：$why';
  }

  @override
  String movingTooLarge(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners': 'アニメーションバナーは$mostピクセルまでです。このファイルは$sizeです。',
      'other': 'アニメーション画像は$mostピクセルまでです。このファイルは$sizeです。',
    });
    return '$_temp0';
  }

  @override
  String movingTooLong(String what, String most, String count) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners': 'アニメーションバナーは$mostフレームまでです。このファイルは$countフレームです。',
      'other': 'アニメーション画像は$mostフレームまでです。このファイルは$countフレームです。',
    });
    return '$_temp0';
  }

  @override
  String movingTooHeavy(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners': 'アニメーションバナーは$mostまでです。このファイルは$sizeです。',
      'other': 'アニメーション画像は$mostまでです。このファイルは$sizeです。',
    });
    return '$_temp0';
  }

  @override
  String get namesHidden => 'この名前には見えない文字が含まれています。それらを除いて入力し直してください。';

  @override
  String get namesBlank => 'この名前には表示される文字がありません。文字、数字、絵文字を使ってください。';

  @override
  String get namesDefaultBio => 'こんにちは！Fogataを使っています。';

  @override
  String get namesBioHidden => 'この自己紹介には見えない文字が含まれています。それらを除いて入力し直してください。';

  @override
  String namesBioLines(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '自己紹介は$n行までです。',
    );
    return '$_temp0';
  }

  @override
  String namesBioChars(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '自己紹介は$n文字までです。',
    );
    return '$_temp0';
  }

  @override
  String get restartFailed => 'Fogataを再起動できませんでした。終了してから、手動で開き直してください。';

  @override
  String typingOne(String name) {
    return '$nameが入力中…';
  }

  @override
  String typingTwo(String first, String second) {
    return '$firstと$secondが入力中…';
  }

  @override
  String get typingSeveral => '複数の人が入力中…';

  @override
  String get outgoingTheRoom => 'ルーム';

  @override
  String outgoingDropTitle(String group) {
    return 'ドロップして$groupで共有';
  }

  @override
  String get outgoingDropNote =>
      '送る前にメッセージを添えたり、ファイルを確認したりできます。あなたがルームにいる間、他の人はこのパソコンからダウンロードします。';

  @override
  String outgoingPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '隠しファイルまたは非公開のファイル$n個を追加しませんでした。Webページがドラッグを始めることもあるため、このようなファイルはドラッグでは追加されません。共有するには、クリップボタンで選んでください。',
      one: '隠しファイルまたは非公開のファイルを追加しませんでした。Webページがドラッグを始めることもあるため、このようなファイルはドラッグでは追加されません。共有するには、クリップボタンで選んでください。',
    );
    return '$_temp0';
  }

  @override
  String outgoingToShare(int n, String count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '共有するファイル$count個 · $size',
    );
    return '$_temp0';
  }

  @override
  String get outgoingRemoveAll => 'すべて削除';

  @override
  String outgoingRemoveFile(String file) {
    return '$fileを取り除く';
  }

  @override
  String get windowTrayOpen => 'Fogataを開く';

  @override
  String get windowTrayQuit => 'Fogataを終了';

  @override
  String get windowMinimize => '最小化';

  @override
  String get windowRestore => '元に戻す';

  @override
  String get windowMaximize => '最大化';

  @override
  String get autostartReason => 'ログイン時にFogataを開きます。';

  @override
  String get devicesMicrophone => 'マイク';

  @override
  String get devicesSpeaker => 'スピーカー';

  @override
  String get devicesCamera => 'カメラ';

  @override
  String get devicesChooseMicrophone => 'マイクを選択';

  @override
  String get devicesChooseSpeaker => 'スピーカーを選択';

  @override
  String get devicesChooseCamera => 'カメラを選択';

  @override
  String get devicesNoCamera => 'カメラが接続されていません。';

  @override
  String get devicesCameraInCall => '通話でカメラを使用中です。';

  @override
  String devicesCameraFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'カメラを使用できませんでした。システム設定 › プライバシーとセキュリティで許可してください。',
      'other': 'カメラを使用できませんでした。',
    });
    return '$_temp0';
  }

  @override
  String get devicesCameraDenied =>
      'システムが Fogata にカメラの使用を許可していません。システム設定で許可してから、もう一度お試しください。';

  @override
  String get devicesCameraUnavailable =>
      'カメラにアクセスできません。このシステムにはカメラのポータルがありません。';

  @override
  String get devicesPreviewOnlyYou => 'プレビューはあなたにだけ表示されます。';

  @override
  String get devicesStopVideo => 'ビデオを停止';

  @override
  String get devicesTestVideo => 'ビデオをテスト';

  @override
  String get devicesNoiseOn => 'ノイズ抑制：オン';

  @override
  String get devicesNoiseOff => 'ノイズ抑制：オフ';

  @override
  String get devicesNoise => 'ノイズ抑制';

  @override
  String get devicesNoiseNote => '相手に聞こえる音から、ファンやハム音のような一定の背景ノイズを抑えます。';

  @override
  String get devicesNoiseShort => 'ファンなどの一定の雑音を抑えます。';

  @override
  String get devicesMirror => '自分のカメラを左右反転';

  @override
  String get devicesMirrorNote => 'ここでの自分の見え方だけが変わります。他の人には常にカメラの映像がそのまま届きます。';

  @override
  String get devicesMirrorOn => 'カメラの左右反転：オン';

  @override
  String get devicesMirrorOff => 'カメラの左右反転：オフ';

  @override
  String get devicesTestMic => 'マイクをテスト';

  @override
  String get devicesStopMicTest => 'テストを停止';

  @override
  String get devicesMicLevel => 'マイクの音量レベル';

  @override
  String get devicesHearMyself => '自分の声を聞く';

  @override
  String get devicesHearMyselfNote =>
      'マイクの音をスピーカーで再生します。ヘッドホンを使用してください。スピーカーの音が再びマイクに入ることがあります。';

  @override
  String get devicesHearMyselfOn => '自分の声を聞く：オン';

  @override
  String get devicesHearMyselfOff => '自分の声を聞く：オフ';

  @override
  String get devicesMicTestOnlyYou => 'テストの音は誰にも送信されません。';

  @override
  String get devicesMicInCall => 'マイクは通話で使用中です。';

  @override
  String get devicesNoMic => 'マイクが接続されていません。';

  @override
  String get devicesSystemDefault => 'システムのデフォルト';

  @override
  String get devicesNotConnected => '未接続：デフォルトを使用中';

  @override
  String get devicesNoCameraShort => 'カメラが接続されていません';

  @override
  String get devicesMicOptions => 'マイクのオプション';

  @override
  String get devicesSpeakerOptions => 'スピーカーのオプション';

  @override
  String get devicesCameraOptions => 'カメラのオプション';

  @override
  String get devicesInputDevice => '入力デバイス';

  @override
  String get devicesOutputDevice => '出力デバイス';

  @override
  String get devicesChosenMissing => '選択したデバイスが接続されていません。デフォルトを使用しています。';

  @override
  String get devicesVoiceSettings => '音声・ビデオの設定';

  @override
  String get photoImages => '画像';

  @override
  String photoTooLarge(int n) {
    return '画像が大きすぎます。最大$nメガピクセルです。';
  }

  @override
  String get photoUnreadable => 'その画像を読み込めませんでした。';

  @override
  String get callSource => 'ソース';

  @override
  String callQuality(String resolution, int fps) {
    return '$resolution · $fps fps';
  }

  @override
  String get callSetupFailed => '通話を準備できませんでした。';

  @override
  String get callNoSoundSystem => 'このパソコンのサウンドシステムに接続できないため、通話を開始できません。';

  @override
  String callMicGone(String name) {
    return 'マイクが取り外されました。代わりに$nameを使用しています。';
  }

  @override
  String get callMicBack => 'マイクが再び接続されました。';

  @override
  String callMicSilent(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos':
          'マイクを使用できませんでした。接続されていて、システム設定 › プライバシーとセキュリティで許可されているか確認してください。',
      'windows':
          'マイクを使用できませんでした。接続されていて、設定 › プライバシーとセキュリティ › マイクで許可されているか確認してください。',
      'other': 'マイクを使用できませんでした。接続されているか確認してください。',
    });
    return '$_temp0';
  }

  @override
  String get callNoMicrophone => 'マイクが接続されていません。';

  @override
  String callMicFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'マイクを使用できませんでした。システム設定 › プライバシーとセキュリティで許可してください。',
      'windows': 'マイクを使用できませんでした。設定 › プライバシーとセキュリティ › マイクで許可してください。',
      'other': 'マイクを使用できませんでした。',
    });
    return '$_temp0';
  }

  @override
  String callScreenFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': '画面を共有できませんでした。システム設定 › プライバシーとセキュリティで許可してください。',
      'other': '画面を共有できませんでした。',
    });
    return '$_temp0';
  }

  @override
  String get callShareLost => '共有を停止しました。共有していたものを取り込めなくなりました。';

  @override
  String callKeptOut(String name) {
    return '$nameとの接続が切れました。';
  }

  @override
  String get qualityGood => '良好';

  @override
  String get qualityOkay => '普通';

  @override
  String get qualityPoor => '不安定';

  @override
  String get qualityLost => '切断';

  @override
  String qualityDelay(int ms) {
    return '$ms ms';
  }

  @override
  String qualityPercent(String value) {
    return '$value%';
  }

  @override
  String get qualityAnotherMember => '別のメンバー';

  @override
  String qualityLostHint(String name, int seconds) {
    return '$nameから$seconds秒間何も届いていません。Fogataは引き続き探しています。相手のネットワークが戻れば、自動的に戻ります。';
  }

  @override
  String qualityBothPoor(String name) {
    return '現在、両方のネットワークが不安定です。あなたと$nameの声が途切れて聞こえることがあります。';
  }

  @override
  String qualityTheirSide(String name) {
    return '問題は$nameの側にあります。全員に$nameの声が途切れて聞こえます。現在、相手のWi-Fiまたはモバイルデータが不安定です。';
  }

  @override
  String qualityYourSideWith(String name) {
    return '問題はあなたの側にあります。$nameや他の人に、あなたの声が途切れて聞こえることがあります。現在、あなたのWi-Fiまたはモバイルデータが不安定です。';
  }

  @override
  String qualityTheirsSlow(String name) {
    return '現在、$nameのネットワークが少し遅くなっています。全員に$nameの声がときどき途切れて聞こえることがあります。';
  }

  @override
  String qualityAllGoodWith(String name) {
    return '$nameとは、お互いの声が問題なく聞こえています。';
  }

  @override
  String qualityMineSlowWith(String name) {
    return '現在、あなたのネットワークが少し遅くなっています。$nameにあなたの声がときどき途切れて聞こえることがあります。';
  }

  @override
  String qualityWayThrough(String name, String carrier) {
    return '$nameのネットワーク自体は問題ありませんが、$carrierを経由しているため、2人の間の経路が遅くなっています。あなたには$nameの声がときどき途切れて聞こえることがありますが、他の人にはよく聞こえています。';
  }

  @override
  String qualityWaySlowFine(String name) {
    return '$nameのネットワーク自体は問題ありませんが、現在、2人の間の経路が遅くなっています。あなたには$nameの声がときどき途切れて聞こえることがありますが、他の人にはよく聞こえています。';
  }

  @override
  String qualityWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          '現在、あなたと$nameの間の経路が少し遅くなっています。どちらかのネットワークが原因の可能性があります。お互いの声がときどき途切れて聞こえることがあります。',
      'other':
          '現在、あなたと$nameの間の経路が不安定です。どちらかのネットワークが原因の可能性があります。お互いの声が途切れて聞こえることがあります。',
    });
    return '$_temp0';
  }

  @override
  String qualityWayVia(String level, String name, String carrier) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          '現在、あなたと$nameの間の経路が少し遅くなっています。どちらかのネットワークが原因の可能性があります。お互いの声がときどき途切れて聞こえることがあります。$carrierを経由しています。',
      'other':
          '現在、あなたと$nameの間の経路が不安定です。どちらかのネットワークが原因の可能性があります。お互いの声が途切れて聞こえることがあります。$carrierを経由しています。',
    });
    return '$_temp0';
  }

  @override
  String get qualityYourSide =>
      '問題はあなたの側にあります。他の人に、あなたの声が途切れて聞こえることがあります。現在、あなたのWi-Fiまたはモバイルデータが不安定です。';

  @override
  String get qualityMineSlow =>
      '現在、あなたのネットワークが少し遅くなっています。他の人に、あなたの声がときどき途切れて聞こえることがあります。';

  @override
  String qualityLostShort(String name, int seconds) {
    return '$nameから$seconds秒間何も届いていません。Fogataは引き続き探しています。';
  }

  @override
  String qualityAllWellWith(String name) {
    return 'すべて良好です。あなたと$nameは、お互いの声が問題なく聞こえています。';
  }

  @override
  String qualityNoteDropped(String name) {
    return '$nameの接続が切れたため、Fogataが再び探しています';
  }

  @override
  String qualityNoteTheirsPoor(String name) {
    return '$nameのネットワークが不安定なため、全員に$nameの声が途切れて聞こえます';
  }

  @override
  String qualityNoteTheirsSlow(String name) {
    return '$nameのネットワークが少し遅くなっています';
  }

  @override
  String qualityNoteThrough(String name, String carrier) {
    return '$nameとは$carrierを経由して接続しています';
  }

  @override
  String qualityNoteWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': '現在、$nameへの経路が少し遅くなっています',
      'other': '現在、$nameへの経路が不安定です',
    });
    return '$_temp0';
  }

  @override
  String get qualityNoteSeparator => '。';

  @override
  String get qualityAllWell => 'すべて良好です。全員の声が聞こえ、全員にあなたの声が聞こえています。';

  @override
  String qualityAllWellMine(String notes) {
    return 'あなたの側は良好です。$notes。';
  }

  @override
  String qualityYourDetails(String level) {
    return 'あなたの接続：$level。詳細を表示';
  }

  @override
  String qualityTheirDetails(String name, String level) {
    return '$nameの接続：$level。詳細を表示';
  }

  @override
  String get qualityReconnecting => '再接続中…';

  @override
  String get qualityReconnectingLabel => '再接続中';

  @override
  String qualityLevelConnection(String level) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'good': '接続良好',
      'okay': '接続普通',
      'weak': '接続不安定',
      'other': '接続切断',
    });
    return '$_temp0';
  }

  @override
  String get qualityPoorConnection => '接続不安定';

  @override
  String get qualityMinePoorBanner => 'ネットワークが不安定です。他の人にあなたの声が途切れて聞こえることがあります';

  @override
  String get qualityYourNetwork => 'あなたのネットワーク';

  @override
  String qualityTo(String name) {
    return '$nameへ';
  }

  @override
  String qualityBetween(String name) {
    return 'あなたと$nameの間';
  }

  @override
  String qualityTheirNetwork(String name) {
    return '$nameのネットワーク';
  }

  @override
  String get qualityLastHeard => '最終受信';

  @override
  String qualitySecondsAgo(int n) {
    return '$n秒前';
  }

  @override
  String get qualityPath => '経路';

  @override
  String get qualityLookingAgain => '再び探しています';

  @override
  String get qualityDelayLabel => '遅延';

  @override
  String get qualityLostPackets => 'パケットロス';

  @override
  String get qualityYourConnection => 'あなたの接続';

  @override
  String qualityTheirConnection(String name) {
    return '$nameの接続';
  }

  @override
  String roomCountFullLabel(int count, int max) {
    return '満員、$max人中$count人';
  }

  @override
  String roomCountLabel(int count, int max) {
    return '$max人中$count人';
  }

  @override
  String get roomCountFullPrefix => '満員 · ';

  @override
  String get roomCountFullWord => '満員 ';

  @override
  String get volumeVoice => '声の音量';

  @override
  String get volumeOnlyYouHear => 'この変更はあなたにだけ反映されます';

  @override
  String get volumeScreenSound => '画面の音声';

  @override
  String volumePercent(int n) {
    return '$n%';
  }

  @override
  String get volumeMuteVoiceForMe => '声をミュート（自分のみ）';

  @override
  String get volumeMuteForMe => 'ミュート（自分のみ）';

  @override
  String get volumeMuteScreenForMe => '画面の音声をミュート（自分のみ）';

  @override
  String get volumeBackTo100 => '100%に戻す';

  @override
  String get volumeBelowYou => 'あなたより下';

  @override
  String volumeRemoveFrom(String room) {
    return '$roomから退出させる';
  }

  @override
  String volumeRemoveTitle(String name, String room) {
    return '$nameを$roomから退出させますか？';
  }

  @override
  String volumeRemoveText(String name, String room) {
    return '$nameはすぐに$roomから退出しますが、またすぐに戻れます。しばらく入れないようにするには、タイムアウトを使います。';
  }

  @override
  String volumeTimeOutOf(String room) {
    return '$roomからタイムアウト';
  }

  @override
  String volumeTimeOutText(String name, String room) {
    return '$nameは$roomから退出し、次の期間戻れません：';
  }

  @override
  String get volumeUntilLetBack => '上の人が許可するまで';

  @override
  String get pipYourScreen => 'あなたの画面';

  @override
  String pipScreenOf(String name) {
    return '$nameの画面';
  }

  @override
  String get pipYourCamera => 'あなたのカメラ';

  @override
  String get pipBackToCall => '通話に戻る';

  @override
  String get pipBackToRoom => 'ルームに戻る';

  @override
  String get pipStopWatching => '視聴をやめる';

  @override
  String get addToCallTitle => '通話に追加';

  @override
  String addToCallNote(int n) {
    return 'オンラインのフレンドです。通話は最大$n人までです。';
  }

  @override
  String get addToCallSearch => 'フレンドを検索';

  @override
  String get addToCallNoFriends => 'フレンドはまだいません。';

  @override
  String get addToCallNoMatch => 'その名前のフレンドはいません。';

  @override
  String get addToCallInCall => '通話中';

  @override
  String get addToCallCalling => '呼び出し中…';

  @override
  String get addToCallOffline => 'オフライン';

  @override
  String get addToCallCall => '呼び出す';

  @override
  String get streamQualityTitle => '配信画質';

  @override
  String get streamQualityNoteSharing => 'あなたの画面が他の人にどう届くかを設定します。変更はすぐに反映されます。';

  @override
  String get streamQualityNote => '画面を共有するとき、他の人にどう届くかを設定します。';

  @override
  String get streamQualityResolution => '解像度';

  @override
  String streamQualitySourceLines(int lines) {
    return 'ソース（${lines}p）';
  }

  @override
  String get streamQualitySource => 'ソース';

  @override
  String get streamQualityFrameRate => 'フレームレート';

  @override
  String streamQualityFps(int n) {
    return '$n fps';
  }

  @override
  String get streamQualityHigherNote => '高くするほど、上り回線とパソコンの負荷が増えます。';

  @override
  String get streamQualitySomeone => '誰か';

  @override
  String streamQualityLoweredOne(String name, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': '他の人には選んだ画質で届いています。',
      'other': '',
    });
    return '$nameの接続が現在これ以上受け取れないため、小さい映像で届いています。$_temp0';
  }

  @override
  String streamQualityLoweredMany(String names, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': '他の人には選んだ画質で届いています。',
      'other': '',
    });
    return '一部の接続が現在これ以上受け取れないため、$namesには小さい映像で届いています。$_temp0';
  }

  @override
  String get streamQualityGoingOut => '現在の送信状況';

  @override
  String get streamQualityNobody => 'まだ誰も視聴していません。';

  @override
  String streamQualityNow(int height, int fps) {
    return '${height}p · $fps fps';
  }

  @override
  String callLayersIsCalling(String name) {
    return '$nameから着信中';
  }

  @override
  String get callLayersGoToCall => 'F6キーで通話へ移動';

  @override
  String callLayersWith(String names) {
    return '$namesも参加中';
  }

  @override
  String get callLayersDirectCall => 'ダイレクト通話 · ';

  @override
  String get callLayersAnswerMicOff => 'マイクをオフにして応答';

  @override
  String callLayersAnsweringLeaves(String room) {
    return '応答すると$roomから退出します';
  }

  @override
  String get callLayersLeaveAndAnswer => '退出して応答';

  @override
  String callLayersBothIn(String room) {
    return '2人とも$roomにいます。';
  }

  @override
  String get callLayersDismiss => '閉じる';

  @override
  String callLayersCallWith(String names) {
    return '$namesと通話';
  }

  @override
  String get callLayersRoom => 'ルーム';

  @override
  String callLayersBackTo(String title) {
    return '$titleに戻る';
  }

  @override
  String callLayersRinging(String time) {
    return '呼び出し中 · $time';
  }

  @override
  String callLayersInCall(String time) {
    return '通話中 · $time';
  }

  @override
  String callLayersOnlyYou(String time) {
    return 'あなただけ · $time';
  }

  @override
  String callLayersInRoom(int n, String time) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'ルームに$n人',
    );
    return '$_temp0 · $time';
  }

  @override
  String get callLayersMute => 'ミュート';

  @override
  String get callLayersUnmute => 'ミュート解除';

  @override
  String get callLayersHangUp => '通話を終了';

  @override
  String get callLayersLeaveRoom => 'ルームから退出';

  @override
  String roomDeclineCallTitle(String name) {
    return '$nameからの着信を拒否しますか？';
  }

  @override
  String roomDeclineCallText(String caller, String callee) {
    return '$callerから着信があります。通話は同時に1つまでのため、$calleeに発信すると着信を拒否します。';
  }

  @override
  String roomDeclineCallTextSomeone(String caller) {
    return '$callerから着信があります。通話は同時に1つまでのため、他の人に発信すると着信を拒否します。';
  }

  @override
  String get roomDeclineAndCall => '拒否して発信';

  @override
  String roomHangUpTitle(String name) {
    return '$nameとの通話を終了しますか？';
  }

  @override
  String get roomHangUpText => '通話とルームは同時に1つまでです。続けると通話が終了します。';

  @override
  String get roomHangUp => '通話を終了';

  @override
  String get roomCloseTitle => '退出してルームを閉じますか？';

  @override
  String get roomCloseText =>
      'ここに残っているのはあなただけです。退出するとルームは閉じ、話した内容はこのパソコンから消えます。他の人がダウンロードしたものは、その人のパソコンに残ります。';

  @override
  String get roomCloseChatMessages => 'チャットメッセージ';

  @override
  String get roomCloseFilesShared => '共有したファイル';

  @override
  String get roomStay => '残る';

  @override
  String get roomLeaveAndClose => '退出して閉じる';

  @override
  String get roomLivePrefix => 'ライブ · ';

  @override
  String get roomPillConnecting => '接続中';

  @override
  String get roomPillOnlyYou => 'あなただけ';

  @override
  String get roomPillError => 'エラー';

  @override
  String get roomLastOne => 'ルームに残っているのはあなただけです。';

  @override
  String get roomOnlyYouIn => 'ルームにいるのはあなただけです。';

  @override
  String get roomAloneWarning => '退出するとルームは閉じ、中のものはすべて消えます。';

  @override
  String get roomEarlierPeople => '前の人を表示';

  @override
  String get roomMorePeople => '他の人を表示';

  @override
  String roomTileYou(String name) {
    return '$name（あなた）';
  }

  @override
  String roomTileNoSound(String name) {
    return '$name · 音声なし';
  }

  @override
  String roomTileConnecting(String name) {
    return '$name · 接続中';
  }

  @override
  String roomYourScreenWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'あなたの画面 · $n人が視聴中',
    );
    return '$_temp0';
  }

  @override
  String roomTileAway(String name) {
    return '$name · 一時切断中';
  }

  @override
  String roomTileMicOff(String name) {
    return '$name、マイクオフ';
  }

  @override
  String roomTileMutedByYou(String said) {
    return '$said、自分だけミュート中';
  }

  @override
  String roomTileVolume(String said, String percent) {
    return '$said、音量$percent';
  }

  @override
  String roomTileLeft(String name) {
    return '$nameが退出 · 戻る可能性あり';
  }

  @override
  String roomIsSharing(String name) {
    return '$nameが画面を共有しています';
  }

  @override
  String get roomNothingSentUntilWatch => '視聴するまで何も送られてきません';

  @override
  String get roomWatch => '視聴';

  @override
  String get roomLive => 'ライブ';

  @override
  String get roomSharingYourScreen => '画面を共有しています';

  @override
  String get roomShowPreview => 'プレビューを表示';

  @override
  String get roomPreviewMirror => 'Fogata 自体を共有中に表示すると、映像が無限に映り込みます';

  @override
  String get roomNobodyWatching => '視聴者なし';

  @override
  String roomPeopleWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n人が視聴中',
    );
    return '$_temp0';
  }

  @override
  String get roomExitFullScreen => '全画面表示を終了';

  @override
  String get roomFullScreen => '全画面表示';

  @override
  String get roomPlayScreenSound => '画面の音声を再生';

  @override
  String get roomMuteScreenSound => '画面の音声をミュート';

  @override
  String get roomQualityPoor => '不安定';

  @override
  String get roomQualityOkay => '普通';

  @override
  String get roomQualityGood => '良好';

  @override
  String get roomQualityPoorTitle => '接続が不安定です';

  @override
  String get roomQualityOkayTitle => '接続はまあまあです';

  @override
  String get roomQualityGoodTitle => '接続は良好です';

  @override
  String get roomQualityMeasuring => '測定中';

  @override
  String get roomQualityDelay => '遅延';

  @override
  String roomQualityMs(String n) {
    return '$n ms';
  }

  @override
  String get roomQualityLost => 'パケットロス';

  @override
  String roomQualityPercent(String n) {
    return '$n%';
  }

  @override
  String get roomQualityPicture => '映像';

  @override
  String get roomQualityPath => '経路';

  @override
  String get roomQualityMayFreeze => '映像が止まることがあります。同時に共有する人を減らすと改善します。';

  @override
  String get roomQualityMayShrink => '動きを保つため、映像が小さくなることがあります。';

  @override
  String roomCallingName(String name) {
    return '$nameを呼び出し中…';
  }

  @override
  String get roomRingsFor30 => '相手のパソコンで最大30秒間呼び出します。';

  @override
  String get roomReturnToCall => '通話に戻る';

  @override
  String get roomCallAgain => 'もう一度発信';

  @override
  String get roomAloneHint => 'グループの誰かが入ると、ここに表示されます。';

  @override
  String roomAlreadyLeft(String names) {
    return '$namesはすでに退出しました。';
  }

  @override
  String get roomGroupFound => 'ネットワーク上でグループが見つかりました';

  @override
  String get roomLookingForRoute => '最適な経路を探しています…';

  @override
  String get roomConnectingTitle => 'ルームに接続中';

  @override
  String roomSeconds(String n) {
    return '$n秒';
  }

  @override
  String roomReachedOf(int reached, int total, String time) {
    return '$total人中$reached人 · $time';
  }

  @override
  String get roomPathLan => 'ローカルネットワーク、暗号化';

  @override
  String get roomPathDirect => '直接、暗号化';

  @override
  String get roomPathThroughMember => 'メンバー経由、暗号化';

  @override
  String roomPathThrough(String name) {
    return '$name経由、暗号化';
  }

  @override
  String get roomCouldNotConnect => 'ルームに接続できませんでした';

  @override
  String get roomCouldNotConnectText =>
      '時間内に誰も応答しませんでした。ネットワークがデバイス間の直接接続をブロックしている可能性があります。';

  @override
  String get roomNoAnswer => '応答なし';

  @override
  String get roomSeeNetworkCheck => 'ネットワークチェックを見る →';

  @override
  String get roomGoesNothing => 'ルームは全員に対して閉じます';

  @override
  String roomGoesMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n件のメッセージが消えます',
    );
    return '$_temp0';
  }

  @override
  String roomGoesFiles(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '共有した$n個のファイルが消えます',
    );
    return '$_temp0';
  }

  @override
  String roomGoesBoth(int messages, int files) {
    String _temp0 = intl.Intl.pluralLogic(
      messages,
      locale: localeName,
      other: '$messages件のメッセージ',
    );
    String _temp1 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files個のファイル',
    );
    return '$_temp0と$_temp1が消えます';
  }

  @override
  String get roomMuteMicrophone => 'マイクをミュート';

  @override
  String get roomUnmuteMicrophone => 'マイクのミュートを解除';

  @override
  String get roomStopSharingScreen => '画面共有を停止';

  @override
  String get roomShareScreen => '画面を共有';

  @override
  String get roomUndeafen => 'スピーカーミュートを解除';

  @override
  String get roomDeafen => 'スピーカーミュート';

  @override
  String get roomCameraOff => 'カメラをオフ';

  @override
  String get roomCameraOn => 'カメラをオン';

  @override
  String get roomLeave => '退出';

  @override
  String get roomSoundWaitsForMic => '画面の音声はマイクがオンのときに送信されます';

  @override
  String get roomStopSharingSound => '音声の共有を停止';

  @override
  String get roomShareSoundToo => '音声も共有';

  @override
  String get roomAddFriendToCall => '通話にフレンドを追加';

  @override
  String get roomHideChat => 'チャットを非表示';

  @override
  String get roomShowChat => 'チャットを表示';

  @override
  String get roomTabChat => 'チャット';

  @override
  String get roomTabFiles => 'ファイル';

  @override
  String roomNoteLeftCall(String name) {
    return '$nameが通話から退出しました · ';
  }

  @override
  String roomNoteJoinedCall(String name) {
    return '$nameが通話に参加しました · ';
  }

  @override
  String roomNoteLeftRoom(String name) {
    return '$nameがルームから退出しました · ';
  }

  @override
  String roomNoteEnteredRoom(String name) {
    return '$nameがルームに入りました · ';
  }

  @override
  String get roomSearchMessages => 'メッセージを検索';

  @override
  String get roomClearSearch => '検索をクリア';

  @override
  String get roomChatGoneCall => 'このチャットは通話が終わると消えます';

  @override
  String get roomChatGoneRoom => 'このチャットはルームが閉じると消えます';

  @override
  String roomNoMatches(String text) {
    return '「$text」を含むメッセージはありません';
  }

  @override
  String roomMatches(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n件一致',
    );
    return '$_temp0';
  }

  @override
  String get roomReceivingMessages => 'ルームのメッセージを受信中…';

  @override
  String roomReceivingMessagesFrom(String name) {
    return '$nameのパソコンからルームのメッセージを受信中…';
  }

  @override
  String get roomSomeone => '誰か';

  @override
  String get roomShareFiles => 'ファイルを共有';

  @override
  String get roomAddMoreFiles => 'ファイルを追加';

  @override
  String roomMessageTo(String name) {
    return '$nameへメッセージを送信';
  }

  @override
  String get roomMessageTheRoom => 'ルームへメッセージを送信';

  @override
  String get roomMessageTheCall => '通話へメッセージを送信';

  @override
  String get roomAddMessageOptional => 'メッセージを追加（任意）';

  @override
  String get roomMessageLabel => 'メッセージ';

  @override
  String roomEnterToSend(String key) {
    return '$keyで送信';
  }

  @override
  String roomEscToRemove(String key) {
    return '$keyで取り除く';
  }

  @override
  String roomOrTwo(String first, String second) {
    return '$firstまたは$second';
  }

  @override
  String roomOrMore(String rest, String last) {
    return '$restまたは$last';
  }

  @override
  String roomFileNotAvailable(String name) {
    return '利用不可 · $nameが退出';
  }

  @override
  String get roomSharingFromHere => 'このパソコンから共有中';

  @override
  String roomAvailableWhile(String names) {
    return '$namesがルームにいる間は利用可能';
  }

  @override
  String roomSizeDownloads(String size, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n回ダウンロード',
    );
    return '$size · $_temp0';
  }

  @override
  String roomDownloadFile(String file) {
    return '$fileをダウンロード';
  }

  @override
  String roomShowInFolder(String file) {
    return '$fileをフォルダーで表示';
  }

  @override
  String roomGettingFile(String file) {
    return '$fileを取得中…';
  }

  @override
  String get roomSaveToDownloads => 'ダウンロードフォルダーに保存';

  @override
  String roomPictureKept(String name) {
    return '$nameが退出 · あなたが退出するまで画像はここに残ります';
  }

  @override
  String get roomSaveFile => '保存';

  @override
  String get roomClosePicture => '画像を閉じる';

  @override
  String get roomFilesNote =>
      'ファイルはどこにもアップロードされません。共有した人のパソコンから直接届き、その人がルームにいる間だけ使えます。';

  @override
  String roomFilesAvailable(int n) {
    return '利用可能 · $n';
  }

  @override
  String roomFilesGone(int n) {
    return '利用不可 · $n';
  }

  @override
  String roomPersonLeft(String name) {
    return '$nameが退出';
  }

  @override
  String get roomPickOrDrop => 'ここで選ぶか、ルームにドロップしてください';

  @override
  String get roomFileYours => 'あなた';

  @override
  String roomFileFrom(String name) {
    return '$nameから';
  }

  @override
  String roomDownloads(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n回ダウンロード',
    );
    return '$_temp0';
  }

  @override
  String get roomStopSharingFile => '停止';

  @override
  String get roomWhatToShare => '何を共有しますか？';

  @override
  String roomScreenSource(String name) {
    return '画面：$name';
  }

  @override
  String roomPlayVideo(String file) {
    return '$fileを再生';
  }

  @override
  String get roomCloseVideo => '動画を閉じる';

  @override
  String get roomVideoGone => 'この動画はもうルームにありません。';

  @override
  String get roomVideoNotFetched => 'この動画を取得できませんでした。';

  @override
  String get roomVideoCannotPlay => 'このパソコンではこの動画を再生できません。';

  @override
  String roomGettingVideoPercent(int n) {
    return '動画を取得中 · $n%';
  }

  @override
  String get roomGettingVideo => '動画を取得中…';

  @override
  String get roomPause => '一時停止';

  @override
  String get roomPlay => '再生';

  @override
  String get roomSoundOn => '音声をオン';

  @override
  String get roomMuteVideo => 'ミュート';

  @override
  String get roomLeaveFullScreen => '全画面表示を終了';

  @override
  String roomCallWaits(String time) {
    return '1人だけになると、通話は他の人が戻るのを$time待ちます';
  }

  @override
  String roomCallWaitsAlone(String time) {
    return '残っているのはあなただけです。通話は他の人が戻るのを$time待ちます';
  }

  @override
  String roomOpenFile(String file) {
    return '$fileを開く';
  }

  @override
  String activityCreated(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がグループを「$name」として作成しました';
  }

  @override
  String activityRenamed(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がグループ名を「$name」に変更しました';
  }

  @override
  String activityIcon(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がグループのアイコンを変更しました';
  }

  @override
  String activityPictureRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がグループの画像を削除しました';
  }

  @override
  String activityPicture(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がグループの画像を変更しました';
  }

  @override
  String activityBannerRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がグループのバナーを削除しました';
  }

  @override
  String activityBanner(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がグループのバナーを変更しました';
  }

  @override
  String activityAdded(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'あなた',
      'other': '$member',
    });
    return '$_temp0が$_temp1を追加しました';
  }

  @override
  String activityJoined(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'あなた',
      'other': '$member',
    });
    return '$_temp0が$_temp1の招待で参加しました';
  }

  @override
  String activityLetIn(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'あなた',
      'other': '$member',
    });
    return '$_temp0が$_temp1の参加を承認しました';
  }

  @override
  String activityRemoved(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'あなた',
      'other': '$member',
    });
    return '$_temp0が$_temp1をグループから外しました';
  }

  @override
  String activityLeft(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がグループから退出しました';
  }

  @override
  String activityMovedUp(
    String by,
    String byMe,
    String member,
    String memberMe,
    String place,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'あなた',
      'other': '$member',
    });
    return '$_temp0が$_temp1を$placeに上げました';
  }

  @override
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'あなた',
      'other': '$member',
    });
    return '$_temp0が$_temp1を最後に移動しました';
  }

  @override
  String activityManagersAll(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0が全員にグループの管理を任せました';
  }

  @override
  String activityManagersFirst(String by, String byMe, int n) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '最初の$n人',
      one: '最初の1人だけ',
    );
    return '$_temp0がグループの管理を$_temp1に任せました';
  }

  @override
  String activityLimits(String by, String byMe, int people, int perRoom) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がグループの上限を$people人、各ボイスルームを$perRoom人に設定しました';
  }

  @override
  String activityRoomOpened(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がボイスルーム「$room」を作成しました';
  }

  @override
  String activityRoomRenamed(String by, String byMe, String from, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がボイスルーム「$from」の名前を「$room」に変更しました';
  }

  @override
  String activityRoomClosed(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0がボイスルーム「$room」を削除しました';
  }

  @override
  String activityInvited(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0が招待リンクを作成しました';
  }

  @override
  String activityRevoked(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0が招待リンクを取り消しました';
  }

  @override
  String activityRoomOpenToAll(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0が「$room」を全員に開放しました';
  }

  @override
  String activityRoomForAbove(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0が「$room」を上位の人専用にしました';
  }

  @override
  String activityRoomOnly(String by, String byMe, String people, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    return '$_temp0が「$room」に入れる人を$peopleだけにしました';
  }

  @override
  String activityKeptOut(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'あなた',
      'other': '$member',
    });
    return '$_temp0が、許可するまで$_temp1を「$room」に入れないようにしました';
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
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'あなた',
      'other': '$member',
    });
    String _temp2 = intl.Intl.selectLogic(untilKind, {
      'today': '$untilまで',
      'todayAtOne': '$untilまで',
      'yesterday': '$untilまで',
      'other': '$untilまで',
    });
    return '$_temp0が$_temp1を「$room」から$_temp2タイムアウトしました';
  }

  @override
  String activityLetBack(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'あなた', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'あなた',
      'other': '$member',
    });
    return '$_temp0が$_temp1を「$room」に戻れるようにしました';
  }

  @override
  String get activityDefaultRoom => '一般';

  @override
  String get activityYouInList => 'あなた';

  @override
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'あなた',
      'other': '$author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': 'あなたによって',
      'other': '$byによって',
      'none': '',
    });
    return '取り消し：同時に$_temp0が$_temp1グループから外されていました';
  }

  @override
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'あなた',
      'other': '$author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': 'あなたによって',
      'other': '$byによって',
      'none': '',
    });
    return '取り消し：同時に$_temp0が$_temp1最後に移動されていました';
  }

  @override
  String activityUndoneLeft(String author, String authorMe) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'あなた',
      'other': '$author',
    });
    return '取り消し：同時に$_temp0が退出していました';
  }

  @override
  String get activityAboutList =>
      'このリストはグループ自体の記録に基づき、グループの全員に同じ内容が表示されます。画像とバナーの変更は別に記録され、このデバイスが確認したものだけが表示されます。';

  @override
  String get activityAboutOrder => 'グループ自体の順序で表示しています。時刻は各作成者の時計によるものです。';

  @override
  String get activityNothingYet => 'まだ何もありません。';

  @override
  String get activityLoading => '読み込み中…';

  @override
  String get activityShowOlder => '古い履歴を表示';

  @override
  String activitySeenHere(String when) {
    return '$when · このデバイスで確認';
  }

  @override
  String get bellNotifications => '通知';

  @override
  String bellNewNotifications(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n件の新しい通知',
    );
    return '$_temp0';
  }

  @override
  String get bellClearAll => 'すべて消去';

  @override
  String get bellNothingNew => '新しい通知はありません';

  @override
  String get bellFooter => '最新の3件がここに残ります。不在着信と申請は「フレンド」ページにも表示されます。';

  @override
  String bellBothInRoom(String room) {
    return '2人とも$roomにいます。';
  }

  @override
  String get bellDismiss => '閉じる';

  @override
  String bellFriendRequest(String name) {
    return '$nameからフレンド申請が届きました';
  }

  @override
  String bellCheckFingerprint(String fingerprint) {
    return 'フィンガープリントが一致するか確認してください：$fingerprint';
  }

  @override
  String bellJoinAsk(String name, String group) {
    return '$nameが「$group」への参加を希望しています';
  }

  @override
  String bellJoinAskNote(String fingerprint) {
    return 'グループの招待リンクを使用。フィンガープリント：$fingerprint';
  }

  @override
  String get bellLetIn => '承認';

  @override
  String get bellSayNo => '拒否';

  @override
  String bellGroupInvite(String name, String group) {
    return '$nameがあなたを「$group」に追加しました';
  }

  @override
  String bellMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'メンバー$n人',
    );
    return '$_temp0';
  }

  @override
  String get bellJoin => '参加';

  @override
  String bellNewMessages(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$nameから$n件の新しいメッセージ',
      one: '$nameから新しいメッセージ',
    );
    return '$_temp0';
  }

  @override
  String get bellOpen => '開く';

  @override
  String get bellMarkRead => '既読にする';

  @override
  String get bellFirewallTitle => 'ファイアウォールが一部のフレンドをブロックしています';

  @override
  String get bellCopyCommand => 'コマンドをコピー';

  @override
  String get bellOtherSessionTitle => '別のパソコンでも開かれています';

  @override
  String get bellSettingsResetTitle => '一部の設定がリセットされました';

  @override
  String get bellSettingsResetText =>
      '一部の設定を読み込めなかったため、デフォルトに戻りました。設定で確認してください。以前のファイルはapp.json.badとして新しいファイルの隣に残してあります。';

  @override
  String get friendCardOnline => 'オンライン';

  @override
  String get friendCardAway => '退席中';

  @override
  String get friendCardOffline => 'オフライン';

  @override
  String get friendCardCallsOnlyOnline => '通話の呼び出しは、フレンドがオンラインのときだけ届きます。';

  @override
  String friendCardHangUpFirst(String name) {
    return '$nameに発信するには、先に通話を終了してください。';
  }

  @override
  String friendCardBothInRoom(String room) {
    return '2人とも$roomにいます。';
  }

  @override
  String get friendCardMessage => 'メッセージ';

  @override
  String friendCardMutualFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '共通のフレンド$n人',
    );
    return '$_temp0';
  }

  @override
  String get memberCardSendRequest => 'フレンド申請を送る';

  @override
  String get memberCardRequestSent => 'フレンド申請を送りました';

  @override
  String get memberCardAccept => 'フレンド申請を承認';

  @override
  String memberCardAsksYou(String name) {
    return '$nameからフレンド申請が届いています。';
  }

  @override
  String memberCardInGroup(String group) {
    return '$groupのメンバー';
  }

  @override
  String memberCardOnlyOnline(String name) {
    return 'フレンド申請は$nameがオンラインのときだけ送れます。';
  }

  @override
  String get memberCardShowOwn => '自分のカードを表示';

  @override
  String get userPanelEditProfile => 'プロフィールを編集';

  @override
  String get userPanelStatus => 'ステータス';

  @override
  String get userPanelOnline => 'オンライン';

  @override
  String get userPanelAway => '退席中';

  @override
  String get userPanelOffline => 'オフライン';

  @override
  String get userPanelOfflineNote =>
      'フレンドとグループのメンバーにはオフラインと表示されます。ルームには引き続き入れます。';

  @override
  String get userPanelOnlineNote =>
      'フレンドとグループのメンバーにはオンラインと表示されます。このパソコンを10分使わないと退席中になります。';

  @override
  String get userPanelBio => '自己紹介';

  @override
  String get userPanelAppearingOffline => 'オフライン表示中';

  @override
  String get userPanelCopyLinkLabel => 'フレンドリンクをコピー';

  @override
  String userPanelLinkExpiresIn(String time) {
    return 'あと$timeで期限切れ';
  }

  @override
  String get userPanelLinkTerms => '1人 · 10分';

  @override
  String get userPanelCopyLink => 'フレンドリンクをコピー';

  @override
  String get switcherChatEnded => 'ダイレクトチャット · フレンドではありません';

  @override
  String get switcherChatOnline => 'ダイレクトチャット · オンライン';

  @override
  String get switcherChatAway => 'ダイレクトチャット · 退席中';

  @override
  String get switcherChat => 'ダイレクトチャット';

  @override
  String switcherFriend(String presence) {
    return 'フレンド · $presence';
  }

  @override
  String switcherVoiceRoom(String group) {
    return '$groupのボイスルーム';
  }

  @override
  String switcherVoiceRoomPeople(String group, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$groupのボイスルーム · $n人',
    );
    return '$_temp0';
  }

  @override
  String switcherGroup(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'グループ · メンバー$n人',
    );
    return '$_temp0';
  }

  @override
  String get switcherPlaceholder => 'チャット、グループ、ボイスルームに移動';

  @override
  String get switcherGoTo => '移動';

  @override
  String get switcherEmpty => 'グループ、フレンド、チャットはまだありません。';

  @override
  String switcherNoMatch(String query) {
    return '「$query」に一致するものはありません。';
  }

  @override
  String get switcherHintChoose => '選択';

  @override
  String get switcherHintGo => '移動';

  @override
  String get switcherHintClose => '閉じる';

  @override
  String switcherTooltip(String shortcut) {
    return 'チャット、グループ、ボイスルームに移動（$shortcut）';
  }

  @override
  String get friendsIntro => 'フレンド関係はお互いのデバイスにだけ保存されます。名簿もサーバーもありません。';

  @override
  String get friendsTabAll => 'すべて';

  @override
  String get friendsTabOnline => 'オンライン';

  @override
  String get friendsTabRequests => '申請';

  @override
  String get friendsSearch => 'フレンドを検索';

  @override
  String get friendsNone =>
      'フレンドはまだいません。あなたのQRコード、リンク、コードを共有するか、相手のものを使ってください。';

  @override
  String get friendsNoneOnline => '今オンラインのフレンドはいません。';

  @override
  String get friendsNoMatch => '検索に一致する人はいません。';

  @override
  String get friendsInARoom => 'ルームにいます';

  @override
  String friendsInRoomOf(String group) {
    return 'ルームにいます · $group';
  }

  @override
  String get friendsOnline => 'オンライン';

  @override
  String get friendsAway => '退席中';

  @override
  String get friendsNotSeenYet => 'まだオンラインになっていません';

  @override
  String friendsSeen(String when) {
    return '最終オンライン：$when';
  }

  @override
  String friendsShowCard(String name) {
    return '$nameのプロフィールを表示';
  }

  @override
  String friendsMissedCall(String when) {
    return '不在着信 · $when';
  }

  @override
  String get friendsMessage => 'メッセージ';

  @override
  String friendsOfflineNoCall(String name) {
    return '$nameはオフラインです。通話の呼び出しは、フレンドがオンラインのときだけ届きます';
  }

  @override
  String friendsHangUpFirst(String name) {
    return '$nameに発信するには、先に通話を終了してください';
  }

  @override
  String friendsBothInRoom(String room) {
    return '2人とも$roomにいます。';
  }

  @override
  String friendsOptions(String name) {
    return '$nameのオプション';
  }

  @override
  String friendsRemoveTitle(String name) {
    return '$nameをフレンドから削除しますか？';
  }

  @override
  String get friendsRemoveText =>
      '相手に通知されることなく、お互いのフレンドリストから消えます。共通のグループにはそのまま残ります。もう一度フレンドになるには、改めて招待が必要です。';

  @override
  String get friendsRemove => '削除';

  @override
  String friendsAddToGroup(String group) {
    return '「$group」に追加';
  }

  @override
  String get friendsRemoveMenu => 'フレンドから削除…';

  @override
  String get friendsNoRequests => '保留中の申請はありません。';

  @override
  String friendsRequestIncoming(String when) {
    return 'フレンド申請 · $when';
  }

  @override
  String friendsRequestOutgoing(String when) {
    return '$whenに送信 · 返事を待っています';
  }

  @override
  String get friendsPendingRequest => '保留中の申請';

  @override
  String friendsCheckFingerprint(String name) {
    return 'フィンガープリントが、$nameの画面に表示されているものと一致するか確認してください。';
  }

  @override
  String get friendsAddTitle => 'フレンドを追加';

  @override
  String get friendsAddNote => 'デバイス同士が直接鍵を交換します。';

  @override
  String get friendsTabCode => 'コード';

  @override
  String get friendsTabLink => 'リンク';

  @override
  String get friendsTabQr => 'QR';

  @override
  String get friendsMakeInvitation => '招待を作成';

  @override
  String get friendsInvitationExpired => 'この招待は有効期限が切れました。';

  @override
  String get friendsMakeAnother => '新しく作成';

  @override
  String friendsHintQr(String time) {
    return 'Fogataでスキャン · あと$timeで期限切れ';
  }

  @override
  String friendsHintLink(String time) {
    return '1人に送ってください · あと$timeで期限切れ';
  }

  @override
  String friendsHintCode(String time) {
    return '1人に読み上げてください · あと$timeで期限切れ';
  }

  @override
  String get friendsHiddenCode => '画面共有に備えて隠しています。';

  @override
  String get friendsHiddenLink =>
      '画面共有に備えて隠しています。このパソコンのネットワークアドレスが含まれるため、公開の場では共有しないでください。';

  @override
  String get friendsGotLink => 'フレンドやグループのリンク、コードを受け取りましたか？';

  @override
  String get friendsPastePlaceholder => 'リンクかコードを貼り付け';

  @override
  String get friendsLinkOrCode => 'リンクまたはコード';

  @override
  String get friendsAdd => '追加';

  @override
  String get friendsQrLabel => '招待のQRコード';

  @override
  String friendsAddedToGroup(String name) {
    return '次に2人ともオンラインになったとき、$nameにグループが届きます。';
  }

  @override
  String get directToday => '今日';

  @override
  String get directYesterday => '昨日';

  @override
  String directTimerOff(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'あなた',
      'other': '$name',
    });
    return '$_temp0が消えるメッセージをオフにしました';
  }

  @override
  String directTimerFirst(String time) {
    return '消えるメッセージ：オン（$time）';
  }

  @override
  String directTimerOn(String name, String mine, String time) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'あなた',
      'other': '$name',
    });
    return '$_temp0が消えるメッセージをオンにしました（$time）';
  }

  @override
  String directPinnedLine(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'あなた',
      'other': '$name',
    });
    return '$_temp0がメッセージをピン留めしました';
  }

  @override
  String get directCallFewSeconds => '数秒';

  @override
  String directCallMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n分');
    return '$_temp0';
  }

  @override
  String directCallHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n時間');
    return '$_temp0';
  }

  @override
  String directCallHoursMinutes(int hours, int minutes) {
    String _temp0 = intl.Intl.pluralLogic(
      hours,
      locale: localeName,
      other: '$hours時間',
    );
    String _temp1 = intl.Intl.pluralLogic(
      minutes,
      locale: localeName,
      other: '$minutes分',
    );
    return '$_temp0$_temp1';
  }

  @override
  String directCallLine(String name, String mine, String length) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'あなた',
      'other': '$name',
    });
    return '$_temp0が通話を開始しました（$length）';
  }

  @override
  String get directBlockedOffline => 'オフライン表示中です。メッセージを書くにはオンラインにしてください。';

  @override
  String directBlockedFriendOffline(String name) {
    return '$nameはオフラインです。2人ともオンラインのときに書けます。';
  }

  @override
  String get directOnline => 'オンライン';

  @override
  String get directAway => '退席中';

  @override
  String get directOffline => 'オフライン';

  @override
  String directOfflineSeen(String when) {
    return 'オフライン · 最終オンライン：$when';
  }

  @override
  String get directHeading => 'ダイレクトメッセージ';

  @override
  String directPreviewMine(String text) {
    return 'あなた：$text';
  }

  @override
  String directOpenChat(String name) {
    return '$nameとのチャットを開く';
  }

  @override
  String directShowCard(String name) {
    return '$nameのプロフィールを表示';
  }

  @override
  String get directNoLongerFriend => 'フレンドではありません';

  @override
  String directCallOffline(String name) {
    return '$nameはオフラインです。通話の呼び出しは、フレンドがオンラインのときだけ届きます';
  }

  @override
  String directHangUpFirst(String name) {
    return '$nameに発信するには、先に通話を終了してください';
  }

  @override
  String directPinnedCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'ピン留めされたメッセージ$n件',
    );
    return '$_temp0';
  }

  @override
  String get directPinnedTitle => 'ピン留めされたメッセージ';

  @override
  String directPinnedNote(int max) {
    return 'ピン留めは2人に共有され、消えることはありません。最大$max件までです。';
  }

  @override
  String get directNothingPinned => 'ピン留めはまだありません';

  @override
  String get directNothingPinnedNote => 'メッセージにカーソルを合わせてピンを選ぶと、ここに残せます。';

  @override
  String directDayTime(String day, String time) {
    return '$day $time';
  }

  @override
  String directFilesCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n個のファイル',
    );
    return '$_temp0';
  }

  @override
  String directShowMessageFrom(String name) {
    return '$nameのメッセージを表示';
  }

  @override
  String get directUnpin => 'ピン留めを解除';

  @override
  String get directPin => 'ピン留め';

  @override
  String get directDeleteMessage => '自分の画面から削除';

  @override
  String get directDeleteMessageTitle => 'このメッセージを削除しますか？';

  @override
  String directDeleteMessageNote(String name) {
    return '画像やファイルとともにこのコンピューターから削除されます。$nameさんの手元のコピーは残ります。';
  }

  @override
  String get directDeleteMessageAction => '削除';

  @override
  String get directDeleteForBoth => '両方から削除';

  @override
  String directDeleteOwnNote(String name) {
    return '「両方から削除」を選ぶと、$nameさんのFogataが受け取りしだい、相手のチャットからも消えます。「自分の画面から削除」では相手の画面に残ります。';
  }

  @override
  String directTimerLabel(String time) {
    return '消えるメッセージ：$time';
  }

  @override
  String get directTimerTitle => '消えるメッセージ';

  @override
  String get directCloseMenu => 'メニューを閉じる';

  @override
  String get directTimerNote => '新しいメッセージは、画像やファイルとともに両方のデバイスから消えます。';

  @override
  String get directEmptyNote => '書いた内容は2人のパソコンにだけ保存されます。';

  @override
  String directPinLineLabel(String line) {
    return '$line：表示';
  }

  @override
  String get directShowMessage => 'メッセージを表示';

  @override
  String get directPinnedStays => 'ピン留め：解除するまで残ります';

  @override
  String get directPinned => 'ピン留め';

  @override
  String directPinsFull(int max) {
    return 'ピン留めは最大$max件です。先にどれかを解除してください';
  }

  @override
  String get directDelivered => '配信済み';

  @override
  String get directNotDelivered => '未配信';

  @override
  String get directSending => '送信中…';

  @override
  String directWaiting(String name) {
    return '$nameがオンラインになるのを待っています · 2人ともオンラインになると送信されます';
  }

  @override
  String get directAvailableWhenYouOnline => 'オンラインにすると利用可能';

  @override
  String directAvailableWhenOnline(String name) {
    return '$nameがオンラインのときに利用可能';
  }

  @override
  String get directDownloadedByItself => '自動でダウンロード済み';

  @override
  String directOpenFile(String name) {
    return '$nameを開く';
  }

  @override
  String directPlayFile(String name) {
    return '$nameを再生';
  }

  @override
  String directDownloadSize(String size) {
    return 'ダウンロード · $size';
  }

  @override
  String directVideoPlaysWhenHere(String name, String length) {
    return '$name · $length · 届くと再生できます';
  }

  @override
  String directVideoNotHere(String name, String length, String when) {
    return '$name · $length · $when';
  }

  @override
  String directGettingIt(int percent) {
    return '取得中 · $percent%';
  }

  @override
  String directFileNotHere(String size, String when) {
    return '$size · $when';
  }

  @override
  String get directSaveToDownloads => 'ダウンロードフォルダーに保存';

  @override
  String get directRemovedForSpace => '空き容量確保のため、このパソコンから削除済み';

  @override
  String get directAskAgain => '再取得';

  @override
  String get directClosePicture => '画像を閉じる';

  @override
  String get directCloseVideo => '動画を閉じる';

  @override
  String get directCannotPlay => 'このパソコンではこの動画を再生できません。';

  @override
  String get directPause => '一時停止';

  @override
  String get directPlay => '再生';

  @override
  String get directSoundOn => '音声をオン';

  @override
  String get directMute => 'ミュート';

  @override
  String directRemoveFilesTitle(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n個のファイルを取り除きますか？',
      one: 'ファイルを取り除きますか？',
    );
    return '$_temp0';
  }

  @override
  String get directRemoveFilesText => 'このメッセージから取り除かれます。パソコン上のファイルはそのまま残ります。';

  @override
  String get directDraftKept => '書きかけの内容は、送信できるようになるまでここに残ります。';

  @override
  String directMessagePlaceholder(String name) {
    return '$nameへメッセージを送信';
  }

  @override
  String get directAddMessageOptional => 'メッセージを追加（任意）';

  @override
  String get directAutoDownloadNote =>
      '10 MBまでの画像は自動でダウンロードされます。動画とファイルは開いたときにダウンロードされます。';

  @override
  String get directVoiceRecord => 'ボイスメッセージを録音';

  @override
  String get directVoiceInCall => '通話中は録音できません';

  @override
  String directVoiceWaitingElsewhere(String name) {
    return '先に$nameとのチャットのボイスメッセージを送信するか削除してください。';
  }

  @override
  String get directVoiceDelete => '削除';

  @override
  String get directVoiceTooShort => '短すぎるため送信できません。ボイスメッセージは0.5秒以上必要です。';

  @override
  String directVoiceLeft(String time) {
    return '残り$time';
  }

  @override
  String directVoiceRecordingFrom(String microphone) {
    return '$microphoneで録音中';
  }

  @override
  String get directVoiceRecordingDefault => '選択したマイクは使用できません。デフォルトのマイクで録音中';

  @override
  String directVoiceKeys(String esc, String enter) {
    return '$escで削除、$enterで送信';
  }

  @override
  String directVoiceKeySend(String enter) {
    return '$enterで送信';
  }

  @override
  String directVoicePreview(String length) {
    return 'ボイスメッセージ · $length';
  }

  @override
  String get directVoiceMessage => 'ボイスメッセージ';

  @override
  String get directVoiceNoMic => 'Fogataがマイクを使用できません。';

  @override
  String get directVoiceMicStopped => 'マイクが止まりました。録音した部分は送信できます。';

  @override
  String directVoiceNoMicCheck(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos':
          'マイクが接続され、「音声・ビデオ」で選択され、システム設定 › プライバシーとセキュリティで許可されているか確認してください。',
      'other': 'マイクが接続され、「音声・ビデオ」で選択されているか確認してください。',
    });
    return '$_temp0';
  }

  @override
  String get directVoiceSpeed => '再生速度';

  @override
  String get directVoiceNotPlayed => '未再生';

  @override
  String directRemoveFile(String name) {
    return '$nameを取り除く';
  }

  @override
  String get directAltered =>
      'このパソコン上のチャット履歴が、Fogataが最後に保存したときから変わっています。一部のメッセージが失われている可能性があります。';

  @override
  String directEnded(String name) {
    return '$nameとはフレンドではなくなりました。このチャットは閲覧のみできます。';
  }

  @override
  String get directDeleteChat => 'チャットを削除';

  @override
  String directDeleteTitle(String name) {
    return '$nameとのチャットを削除しますか？';
  }

  @override
  String directDeleteText(String name) {
    return 'メッセージ、画像、動画、ファイルがこのパソコンから削除されます。$nameの手元のコピーは残ります。';
  }

  @override
  String get directDelete => '削除';

  @override
  String get directUsageMessages => 'メッセージ';

  @override
  String get directUsagePictures => '画像';

  @override
  String get directUsageVideos => '動画';

  @override
  String get directUsageFiles => 'ファイル';

  @override
  String get directUsageHeading => 'このデバイス上のこのチャット';

  @override
  String get directFreeUpSpace => '空き容量を確保…';

  @override
  String get directAddFilesTooltip => '画像、動画、ファイルを追加';

  @override
  String get directAddFiles => 'ファイルを追加';

  @override
  String get accountMoveLessThanMinuteLeft => '残り1分未満';

  @override
  String get accountMoveAboutMinuteLeft => '残り約1分';

  @override
  String accountMoveAboutMinutesLeft(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '残り約$n分',
    );
    return '$_temp0';
  }

  @override
  String accountMoveNoRoom(String need, String free) {
    return 'このパソコンにはアカウント用に$needの空きが必要ですが、空きは$freeです。';
  }

  @override
  String accountMoveTooLarge(String size) {
    return 'もう一方のパソコンが$sizeを送ろうとしました。アカウントとしては大きすぎます。';
  }

  @override
  String get accountMoveBusy => 'このパソコンは別の移行を処理中です。先にそれを完了するか、破棄してください。';

  @override
  String get accountMoveNothingToFinish => 'このパソコンには、そのパソコンからの完了待ちの移行がありません。';

  @override
  String get accountMoveNetworkStopped => 'ネットワークが停止しました。';

  @override
  String get accountMoveConnectionLost => '接続が切れました。';

  @override
  String get accountMovePeerNoRoom => '新しいパソコンの空き容量が足りません。';

  @override
  String get accountMovePeerTooLarge => 'アカウントが移行できるサイズを超えています。';

  @override
  String get accountMovePeerBusy => '新しいパソコンは別の移行を処理中です。';

  @override
  String get accountMovePeerCouldNotKeep => '新しいパソコンでファイルを保存できませんでした。';

  @override
  String get accountMovePeerCancelled => 'もう一方のパソコンで移行がキャンセルされました。';

  @override
  String get accountMovePeerCouldNotFinish => '新しいパソコンでは、まだ移行を完了できませんでした。';

  @override
  String get accountMovePeerStopped => 'もう一方のパソコンで移行が停止されました。';

  @override
  String get accountMovePeerNothingToFinish =>
      'そのパソコンには、このパソコンからの完了待ちの移行がありません。';

  @override
  String get accountMovePeerNotProven => 'そのパソコンは、この移行を受け取ったことを証明できませんでした。';

  @override
  String get accountMoveWrongCode => 'もう一方のパソコンがコードを受け入れなかったため、何も送信されませんでした。';

  @override
  String get accountMoveExpired => 'コードが使われる前に有効期限が切れました。';

  @override
  String get accountMoveTooManyTries => '間違ったコードが何度も試されたため、コードが無効になりました。';

  @override
  String get accountMoveNoContact => '2台のパソコンが互いに接続できませんでした。';

  @override
  String get accountMoveUnconfirmed => '移行の最後の確認が取れませんでした。';

  @override
  String get accountMoveCancelled => 'キャンセルしました。';

  @override
  String get accountMoveInterrupted => '移行が途中で止まりました。何も変更されていません。';

  @override
  String accountMoveInterruptedWhy(String why) {
    return '移行が途中で止まりました。何も変更されていません。$why';
  }

  @override
  String get accountMoveDeclined => 'もう一方のパソコンが移行を停止しました。';

  @override
  String accountMoveDeclinedWhy(String why) {
    return 'もう一方のパソコンが移行を停止しました。$why';
  }

  @override
  String get accountMoveLater => 'アカウントは届きました。次にこのパソコンでFogataを起動したときに反映されます。';

  @override
  String accountMoveLaterWhy(String why) {
    return 'アカウントは届きました。次にこのパソコンでFogataを起動したときに反映されます。$why';
  }

  @override
  String get accountMoveDidNotFinish => '完了しませんでした。';

  @override
  String get accountMoveAnotherComputer => '別のパソコン';

  @override
  String get accountMoveTheOtherComputer => 'もう一方のパソコン';

  @override
  String get moveTooLateBeingPlaced => 'もう停止できません。アカウントを反映しています。';

  @override
  String get moveTooLateDiscard => 'もう破棄できません。アカウントを反映しています。';

  @override
  String get moveTooLateOtherHasIt => 'もう停止できません。アカウントはすでにもう一方のパソコンにあります。';

  @override
  String moveFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'フレンド$n人',
    );
    return '$_temp0';
  }

  @override
  String moveGroups(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'グループ$n個',
    );
    return '$_temp0';
  }

  @override
  String moveGroupsWithKeys(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'グループ$n個（鍵を含む）',
    );
    return '$_temp0';
  }

  @override
  String moveChats(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'チャット$n件',
    );
    return '$_temp0';
  }

  @override
  String moveInAll(String size) {
    return '合計$size';
  }

  @override
  String get moveYourSettings => '設定';

  @override
  String get moveNotDevices => '選択したマイクとカメラを除く';

  @override
  String get moveFinishingBoth => '両方のパソコンで完了処理中…';

  @override
  String movePercent(int percent) {
    return '$percent%';
  }

  @override
  String movePercentLeft(int percent, String left) {
    return '$percent% · $left';
  }

  @override
  String get moveStepIdentity => 'IDとプロフィール';

  @override
  String get moveStepFriends => 'フレンド';

  @override
  String get moveStepGroups => 'グループとその鍵';

  @override
  String get moveStepChats => 'チャット、画像、ファイル';

  @override
  String moveBytesOf(String done, String total) {
    return '$done / $total';
  }

  @override
  String get moveStepFinishing => '両方のパソコンで完了処理';

  @override
  String moveBringingNamed(String name) {
    return '$nameのアカウントを移行中';
  }

  @override
  String get moveBringingYours => 'アカウントを移行中';

  @override
  String get moveKeepBothOn => '両方のパソコンの電源を入れたまま、Fogataを開いておいてください。';

  @override
  String get moveKeepOpenNote =>
      '両方のパソコンでFogataを開いたままにしてください。同じネットワーク上だと最も速く、そうでない場合はインターネット経由で直接接続します。';

  @override
  String moveFinishBringingNamed(String name) {
    return '$nameのアカウントの移行を完了';
  }

  @override
  String get moveFinishBringingYours => 'アカウントの移行を完了';

  @override
  String get moveEverythingCame => 'すべて届きました。完了するには、もう一方のパソコンでこのコードを入力してください。';

  @override
  String moveEverythingCameFrom(String computer) {
    return 'すべて届きました。完了するには、「$computer」でこのコードを入力してください。';
  }

  @override
  String get moveBringHereTitle => 'アカウントをここに移行';

  @override
  String get moveBringHereSub => 'アカウントがあるパソコンで、このコードを入力してください。';

  @override
  String get moveHideCode => 'コードを隠す';

  @override
  String get moveShowCode => 'コードを表示';

  @override
  String moveExpiresIn(String time) {
    return 'あと$timeで期限切れ';
  }

  @override
  String moveStepFogataShows(String what) {
    return 'そのパソコンのFogataに$whatと表示されます。';
  }

  @override
  String get moveWaitingToConfirm => '確認待ち';

  @override
  String get moveStepOpenThere => 'そのパソコンでFogataを開きます。';

  @override
  String moveStepTypeUnder(String where) {
    return '$whereの下にコードを入力します。';
  }

  @override
  String get moveFinishTheMove => '移行を完了';

  @override
  String moveStepGoTo(String where) {
    return '$whereに移動します。';
  }

  @override
  String get moveSettingsPath => '設定 → アカウントの移行';

  @override
  String get moveToAnotherComputer => '別のパソコンに移行';

  @override
  String get moveWaitingForOther => 'もう一方のパソコンを待っています…';

  @override
  String moveWrongCodesTyped(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '別のパソコンで間違ったコードが$n回入力されました。',
      one: '別のパソコンで間違ったコードが入力されました。',
    );
    return '$_temp0';
  }

  @override
  String get moveThrowAwayInstead => '代わりに破棄…';

  @override
  String get moveFoundOther => 'もう一方のパソコンが見つかりました';

  @override
  String get moveFoundSameNetwork => '同じネットワーク · 2台の間で暗号化';

  @override
  String get moveFoundInternet => 'インターネット経由 · 2台の間で暗号化';

  @override
  String get moveConfirmThere => '開始するには、もう一方のパソコンで確認してください';

  @override
  String get moveSameAccountNote =>
      'これはすでにこのパソコンにあるアカウントです。ここに移しても失われるものはありません。';

  @override
  String get moveTakeYes => 'ここに移行';

  @override
  String get moveWelcomeBack => 'おかえりなさい';

  @override
  String moveWelcomeBackNamed(String name) {
    return 'おかえりなさい、$name';
  }

  @override
  String get moveDoneSub => 'アカウントはこのパソコンに移りました。もう一方のパソコンでは使われなくなりました。';

  @override
  String get moveOpenFogata => 'Fogataを開く';

  @override
  String get moveCodeExpiredLabel => '期限切れ';

  @override
  String get moveCodeStoppedLabel => '無効';

  @override
  String get moveCodeExpiredNote =>
      'このコードは、もう一方のパソコンで使われる前に期限が切れました。コードの有効期間は10分です。';

  @override
  String get moveCodeStoppedNote => '別のパソコンで間違ったコードが何度も入力されたため、このコードは無効になりました。';

  @override
  String get moveNewCode => '新しいコード';

  @override
  String get moveAlmostThere => 'あと少しです';

  @override
  String get moveLaterSub =>
      'アカウントはすべて届きましたが、まだ反映できていません。次にこのパソコンでFogataを起動したときに反映されます。';

  @override
  String get moveStartAgain => 'Fogataを再起動';

  @override
  String get moveUnconfirmedSub =>
      'すべて届きましたが、もう一方のパソコンが完了を確認していません。コードを表示し、そちらで入力して移行を完了してください。';

  @override
  String get moveStoppedThereTitle => 'もう一方のパソコンで移行が停止されました';

  @override
  String get moveStoppedThereSub =>
      '何も変わっていません。アカウントはまだもう一方のパソコンにあり、ここには何も残っていません。';

  @override
  String get moveCouldNotStartTitle => '移行を開始できませんでした';

  @override
  String get moveSomethingWrongHere => 'このパソコンで問題が発生しました。';

  @override
  String get moveHalfwayTitle => '移行が途中で止まりました';

  @override
  String get moveHalfwaySub =>
      'もう一方のパソコンとの接続が切れました。何も変わっていません。アカウントはまだもう一方のパソコンにあり、ここには何も残っていません。';

  @override
  String get moveShowCodeToFinish => 'コードを表示して完了';

  @override
  String get moveTryNewCode => '新しいコードで再試行';

  @override
  String moveBackToNamed(String name) {
    return '$nameに戻る';
  }

  @override
  String get moveBackToThisProfile => 'このプロフィールに戻る';

  @override
  String get moveNewProfileInstead => '代わりに新しいプロフィールを作成';

  @override
  String get moveDiscardTitle => '届いたアカウントを破棄しますか？';

  @override
  String get moveDiscardText =>
      'もう一方のパソコンで使い続ける場合だけ、破棄してください。そのパソコンは、持ち主がそちらで使い続けると選ぶまで確認を待ちます。';

  @override
  String get moveKeepIt => '残す';

  @override
  String get moveThrowAway => '破棄';

  @override
  String get moveBringFromAnother => '別のパソコンからアカウントを移行';

  @override
  String get moveAlreadyOnFogata => 'すでにFogataを使っていますか？';

  @override
  String get moveCodeTriedBefore =>
      'そのコードはすでに一度試され、一致しませんでした。新しいパソコンで新しいコードを表示してください。';

  @override
  String moveMovingTo(String computer) {
    return 'アカウントを「$computer」に移行中';
  }

  @override
  String get moveKeepBothOnOffline =>
      '両方のパソコンの電源を入れたまま、Fogataを開いておいてください。その間、このパソコンのFogataはオフラインになります。';

  @override
  String get moveCodeFieldLabel => '新しいパソコンに表示されたコード';

  @override
  String get moveToAnotherSub =>
      'ID、フレンド、グループ、チャット、ファイル、設定が、このパソコンから暗号化されて直接もう一方のパソコンに移ります。その後このパソコンではアカウントが使われなくなるため、あなたが2人になることはありません。';

  @override
  String moveStepChoose(String choice) {
    return '新しいパソコンでFogataを開き、$choiceを選びます。';
  }

  @override
  String get moveStepTypeItShows => '表示されたコードを入力します：';

  @override
  String get moveWaitingForAnswer => 'あなたの返事を待っています…';

  @override
  String get moveLookingForCode => 'このコードを表示しているパソコンを探しています…';

  @override
  String get moveOfflineWhileMoving => '移行中、このパソコンのFogataはオフラインになります。';

  @override
  String get moveWrongCodeNote =>
      '新しいパソコンがそのコードを受け入れなかったため、何も送信されませんでした。確認して、もう一度入力してください。それでも失敗する場合は、そちらで新しいコードを表示してください。';

  @override
  String get moveCantReachNew => '新しいパソコンに接続できません。';

  @override
  String get moveCantReachNewNote =>
      '3分間お互いを探しました。同じネットワーク（同じWi-Fiまたはケーブル）に接続して、もう一度お試しください。アカウントはまだここにあり、Fogataはオンラインに戻りました。';

  @override
  String get moveDeclinedNote =>
      '新しいパソコンで移行が停止されました。何も移行されていません。アカウントはまだここにあり、Fogataはオンラインに戻りました。';

  @override
  String get moveCouldNotStart => '移行を開始できませんでした。';

  @override
  String get moveHalfwayNote =>
      '移行が途中で止まりました。新しいパソコンとの接続が切れました。何も変わっていません。アカウントはまだここにあり、Fogataはオンラインに戻りました。';

  @override
  String get moveThisProfileWords => 'このプロフィール';

  @override
  String get moveBringCardTitle => 'アカウントをここに移行';

  @override
  String get moveBringCardSub => 'このパソコンにコードを表示し、アカウントがあるパソコンで入力します。';

  @override
  String get moveBringCardButton => 'アカウントをここに移行…';

  @override
  String moveReplacesHere(String name) {
    return 'このパソコンの$nameを置き換えます。';
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
      other: 'フレンド$friends人',
    );
    String _temp1 = intl.Intl.pluralLogic(
      groups,
      locale: localeName,
      other: 'グループ$groups個',
    );
    String _temp2 = intl.Intl.pluralLogic(
      chats,
      locale: localeName,
      other: 'チャット$chats件',
    );
    return '現在このパソコンにあるプロフィール$boldは置き換えられ、その$_temp0、$_temp1、$_temp2はここから削除されます。$nameを残すには、先に別のパソコンに移行してください。';
  }

  @override
  String moveReplaceAndShow(String name) {
    return '$nameを置き換えてコードを表示';
  }

  @override
  String moveConfirmTitle(String computer) {
    return 'アカウントを「$computer」に移行しますか？';
  }

  @override
  String get moveConfirmSameNetwork => '同じネットワーク上で見つかりました。すべて暗号化され、直接送られます。';

  @override
  String get moveConfirmInternet => 'インターネット経由で見つかりました。すべて暗号化され、直接送られます。';

  @override
  String moveConfirmReplaces(String name) {
    return 'そのパソコンのプロフィール$nameは置き換えられます。';
  }

  @override
  String get moveConfirmNote =>
      '完了すると、このパソコンではアカウントが使われなくなり、二度と使えません。その後、ここに残ったものを削除できます。';

  @override
  String get moveConfirmYes => 'アカウントを移行';

  @override
  String get moveKeepTitle => 'このパソコンでアカウントを使い続けますか？';

  @override
  String get moveKeepText =>
      '新しいパソコンにアカウントがない場合だけにしてください。先にそちらでFogataを開いて確認してください。両方で使うと、フレンドやグループにはあなたが2人いるように見え、メッセージは片方にしか届きません。';

  @override
  String get moveKeepYes => 'ここで使い続ける';

  @override
  String moveWaitingFor(String computer) {
    return '「$computer」の確認を待っています';
  }

  @override
  String get moveWaitingText =>
      'アカウントは送信されましたが、新しいパソコンから準備完了の連絡が届く前に接続が切れました。その間、このパソコンではアカウントを使いません。';

  @override
  String get moveFinishText => '新しいパソコンでFogataを開くと、コードが表示されます。ここに入力してください。';

  @override
  String get moveFinish => '完了';

  @override
  String get moveFinishWrongCode =>
      '新しいパソコンがそのコードを受け入れませんでした。確認して、もう一度入力してください。';

  @override
  String get moveFinishNoContact =>
      '新しいパソコンに接続できません。両方を同じネットワークに接続して、もう一度お試しください。';

  @override
  String get moveKeepThisOne => '新しいパソコンにはありません：このパソコンで使い続ける…';

  @override
  String moveMovedYours(String computer) {
    return 'アカウントは「$computer」に移行しました';
  }

  @override
  String moveMovedNamed(String name, String computer) {
    return '$nameのアカウントは「$computer」に移行しました';
  }

  @override
  String get moveMovedNoteYou =>
      'このパソコンではもう使われていません。フレンドとグループは、もう一方のパソコンであなたとつながります。';

  @override
  String moveMovedNoteNamed(String name) {
    return 'このパソコンではもう使われていません。フレンドとグループは、もう一方のパソコンで$nameとつながります。';
  }

  @override
  String moveMovedTodayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': '今日 $time。',
      'other': '今日 $time。',
    });
    return '$_temp0';
  }

  @override
  String moveMovedOn(String when) {
    return '$when。';
  }

  @override
  String moveMovedWhenAndNote(String when, String note) {
    return '$when$note';
  }

  @override
  String get moveLeftHere => 'ここに残っているもの';

  @override
  String get moveLeftHereText =>
      '移行前のチャット、画像、ファイル（暗号化済み）。もう一方のパソコンにも独自のコピーがあります。';

  @override
  String get moveDeleteData => 'このパソコンのFogataのデータを削除';

  @override
  String get moveAfterDeleting => '削除すると、このパソコンのFogataは新規の状態で起動します。';

  @override
  String get moveCodeNotACode => 'コードはXXXXX-XXXXXのような10文字の英数字です。';

  @override
  String get moveCodeMistyped => 'コードが正しくありません。文字と数字を確認してください。';

  @override
  String get welcomeHeadline => '同じ部屋にいる\nみたいに話そう。';

  @override
  String get welcomeIntro =>
      'フレンド、家族、チームと音声・ビデオ・チャット。あなたのパソコンから相手のパソコンへ直接つながります。';

  @override
  String get welcomeNoAccount => 'アカウント不要';

  @override
  String get welcomeEncrypted => '暗号化';

  @override
  String get welcomeEphemeral => '一時的なルーム';

  @override
  String get welcomeCharacter => 'Fogataのキャラクター。ヘッドホンをつけてピースサインをしている小さな炎';

  @override
  String get welcomeImageUnreadable => 'その画像を読み込めませんでした。';

  @override
  String get welcomeKeychainTitle => 'データをまだ暗号化できません';

  @override
  String welcomeKeychainText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': 'Linuxでは、キーチェーンはKWalletやGNOME Keyringなど、実行中のSecret Serviceです。起動するかロックを解除してから、もう一度お試しください。',
      'other': 'キーチェーンのロックを解除するか修復してから、もう一度お試しください。',
    });
    return 'Fogataは、ID、フレンド、ダイレクトメッセージを、システムのキーチェーンに保存する鍵で暗号化しますが、キーチェーンに鍵を保存できませんでした。$_temp0';
  }

  @override
  String get welcomeKeychainRetry => '再試行';

  @override
  String get welcomeKeychainPlain => '暗号化せずに続ける';

  @override
  String get welcomeKeychainPlainNote =>
      '暗号化しないと、このパソコンのファイルをコピーした人は誰でも読めます。設定にはその旨が表示され続け、キーチェーンが使える状態で初めて開いたときに、Fogataがすべて暗号化します。';

  @override
  String get welcomeWriteName => '名前を入力してください。';

  @override
  String get welcomeCardTitle => 'みんなにどう表示されたいですか？';

  @override
  String get welcomeCardSub => '名前と画像です。いつでも変更できます。';

  @override
  String get welcomeNoServer =>
      'Fogataのサーバーには何も送られません。そもそもサーバーがありません。公開IPFSネットワークは、フレンドを見つけるための電話帳としてのみ使われます。';

  @override
  String get welcomeYourName => 'あなたの名前';

  @override
  String welcomePictureLabel(String name) {
    return '画像：$name';
  }

  @override
  String get welcomeUploadPicture => '画像をアップロード';

  @override
  String get welcomeNameLabel => '名前';

  @override
  String get welcomeCreate => '作成';

  @override
  String get welcomeReady => '完了';

  @override
  String get welcomeCreateLabel => 'IDを作成';

  @override
  String welcomeKeyLabel(String key) {
    return 'このパソコンで作成された鍵：$key';
  }

  @override
  String get welcomeNewKey => '別の鍵を作成';

  @override
  String get stoppedTitle => 'Fogataが停止しました。';

  @override
  String get stoppedStartingAgain => '再起動しています…';

  @override
  String get stoppedRestartToConnect => '再接続するには再起動してください。';

  @override
  String get stoppedRestartNow => '今すぐ再起動';

  @override
  String get settingsSectionProfile => 'マイプロフィール';

  @override
  String get settingsSectionMove => 'アカウントの移行';

  @override
  String get settingsSectionAppearance => '外観';

  @override
  String get settingsSectionLanguageTime => '言語と時刻';

  @override
  String get settingsSectionVoice => '音声・ビデオ';

  @override
  String get settingsSectionSounds => 'サウンド';

  @override
  String get settingsSectionBehaviour => '動作';

  @override
  String get settingsSectionNetwork => 'ネットワーク';

  @override
  String get settingsSectionStorage => 'ストレージとダ⁠ウ⁠ン⁠ロ⁠ー⁠ド';

  @override
  String get settingsHeadingUser => 'ユーザー';

  @override
  String get settingsHeadingApp => 'アプリ';

  @override
  String get settingsTitle => '設定';

  @override
  String get settingsAll => 'すべての設定';

  @override
  String get settingsCloseTooltip => '設定を閉じる';

  @override
  String get settingsUnsavedSaveTitle => '変更を保存しますか？';

  @override
  String get settingsUnsavedDiscardTitle => '変更を破棄しますか？';

  @override
  String get settingsUnsavedSaveText => 'プロフィールにまだ保存されていない変更があります。';

  @override
  String get settingsUnsavedDiscardText => 'プロフィールに、このままでは保存できない変更があります。';

  @override
  String get settingsProfileSaved =>
      'プロフィールを保存しました。オンラインのフレンドとグループには今すぐ、それ以外には接続したときに表示されます。';

  @override
  String get settingsBioSaved =>
      '自己紹介を保存しました。オンラインのフレンドには今すぐ、それ以外には接続したときに表示されます。';

  @override
  String settingsSwitchOn(String what) {
    return '$what：オン';
  }

  @override
  String settingsSwitchOff(String what) {
    return '$what：オフ';
  }

  @override
  String get settingsOn => 'オン';

  @override
  String get settingsOff => 'オフ';

  @override
  String get settingsIdentity => 'あなたのID';

  @override
  String get settingsIdentityNote => '名前と画像は署名付きでフレンドとグループに、自己紹介はフレンドに届きます。';

  @override
  String get settingsPicture => '画像';

  @override
  String get settingsUploadPicture => '画像またはGIFをアップロード';

  @override
  String get settingsName => '名前';

  @override
  String get settingsBio => '自己紹介';

  @override
  String get settingsBioHint => 'あなたについてひとこと';

  @override
  String get settingsNoBio => 'プロフィールに自己紹介は表示されません。';

  @override
  String settingsBioWhere(int lines) {
    return 'あなたのプロフィールでフレンドに表示されます（最大$lines行）。';
  }

  @override
  String get settingsBannerTitle => 'パネルの背景';

  @override
  String get settingsBannerNote => 'プロフィールの上部です。あなたとフレンドに表示されます。';

  @override
  String get settingsBannerAnother => '別の画像';

  @override
  String get settingsBannerUpload => 'このパソコンの画像を使用';

  @override
  String get settingsFingerprint => 'このパソコンのフィンガープリント';

  @override
  String get settingsTheme => 'テーマ';

  @override
  String get settingsThemeNote =>
      'ウィンドウの色です。「システム」はパソコンの設定に合わせて「ダーク」と「ライト」を切り替えます。';

  @override
  String get settingsMicSpeaker => 'マイクとスピーカー';

  @override
  String get settingsMicSpeakerNote => '通話で使うマイクと音声の出力先です。通話中にも変更できます。';

  @override
  String get settingsCamera => 'カメラ';

  @override
  String get settingsCameraNote => 'カメラがオンのとき、通話で映る映像です。通話の前にここで試せます。';

  @override
  String get settingsSoundEffects => '効果音';

  @override
  String get settingsSoundEffectsNote =>
      'ルームへの入退室、人の出入り、チャットメッセージ、マイク、共有のときに鳴ります。';

  @override
  String get settingsSounds => 'サウンド';

  @override
  String settingsChangeFailed(String reason) {
    return 'Fogataはその設定を変更できませんでした：$reason';
  }

  @override
  String get settingsAutostartApprove =>
      'システム設定 › 一般 › ログイン項目でFogataを許可してください。';

  @override
  String get settingsAutostartOldMac => 'macOS 13以降が必要です。';

  @override
  String get settingsAutostartUnsupported => 'このFogataは自動では起動できません。';

  @override
  String get settingsAutostartUnread => 'システムからこの設定を読み取れませんでした。';

  @override
  String get settingsAutostart => '起動時に開く';

  @override
  String get settingsAutostartNote => 'パソコンの起動後、ログインしたときにFogataを開きます。';

  @override
  String get settingsKeepRunning => 'バックグラウンドで実行';

  @override
  String get settingsKeepRunningNote =>
      'ウィンドウを閉じてもFogataはトレイに残るため、フレンドからの通話を受けられます。';

  @override
  String get settingsNoTray =>
      'このデスクトップにはトレイアイコンが表示されないため、ウィンドウを閉じるとFogataは終了します。';

  @override
  String get settingsHelping => 'メンバーのサポート';

  @override
  String get settingsHelpingNote => 'このパソコンがグループのメンバーをどう手助けするかを設定します。';

  @override
  String get settingsHelpRelay => '接続支援と中継';

  @override
  String get settingsHelpConnect => '接続支援のみ';

  @override
  String get settingsHelpRelayNote =>
      '2人のメンバーが互いに接続できない場合、このパソコンが暗号化された通信を中継します（中身は見えません）。通話1回につき最大2 GBまでで、あなたの上り回線を使います。';

  @override
  String get settingsHelpConnectNote =>
      'このパソコンは2人のメンバーがお互いを見つける手助けをしますが、通信は中継しません。接続できない場合、お互いの声は聞こえません。';

  @override
  String get settingsRestartTray => '変更は、トレイアイコンからFogataを終了して再び開いたときに反映されます。';

  @override
  String get settingsRestart => '変更は、Fogataを終了して再び開いたときに反映されます。';

  @override
  String get settingsNetworkCheck => 'ネットワークチェック';

  @override
  String get settingsNetworkCheckNote =>
      'このパソコンからネットワークがどう見えているか、それがフレンドとの接続にどう影響するかを表示します。外部には何も送信されず、接続がすでに把握している情報だけを確認します。';

  @override
  String get settingsCheckNetwork => 'ネットワークをチェック';

  @override
  String get settingsCheckAgain => '再チェック';

  @override
  String get settingsDownloads => 'ダウンロード';

  @override
  String get settingsDownloadsNote => 'ルームからダウンロードしたファイルの保存先です。';

  @override
  String settingsVersionData(String version, String path) {
    return 'Fogata $version · データ：$path';
  }

  @override
  String get settingsNotEncrypted => 'このパソコンでは暗号化されていません';

  @override
  String settingsNotEncryptedText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': 'Linuxでは、キーチェーンとはKWalletやGNOME KeyringなどのSecret Serviceのことです。起動またはインストールすると、次にFogataを開いたときにデータが暗号化されます。',
      'other': '次にFogataを開いたときに、もう一度試します。',
    });
    return '鍵を保管するシステムのキーチェーンが見つからなかったため、ID、フレンド、ダイレクトメッセージはこのパソコンに暗号化されずに保存されています。$_temp0';
  }

  @override
  String get settingsNotAllEncrypted => 'このパソコンでは一部が暗号化されていません';

  @override
  String get settingsNotAllEncryptedText =>
      'このパソコンのデータ（ID、フレンド、ダイレクトメッセージ）の一部はまだ暗号化されていません。Fogataが暗号化している途中か、ディスクがいっぱいなどの理由で完了できませんでした。Fogataを開くたびに続きを行います。';

  @override
  String settingsDamagedSecret(String file) {
    return '$fileはこのパソコンの鍵で開けません。ディスク上で破損しています。このままでは暗号化を完了できず、暗号化前のコピーが暗号化されないまま残ります。完了するには、このファイルを取り除いてください。ファイルはそのまま保管されます。IDまたはフレンドのファイルなら、Fogataはそのコピーを戻して暗号化します。グループやチャットのファイルはこのパソコンでは失われます。グループにもう一度参加を承認してもらう必要があり、チャットの以前のメッセージは表示されなくなります。';
  }

  @override
  String get settingsSetDamagedAside => '取り除いて再起動';

  @override
  String get storageTitle => 'ストレージ';

  @override
  String get storageNote =>
      'ダイレクトメッセージはあなたのデバイスにだけ保存されます。ルームのチャットはルームが閉じると消えるため、増えるのはダイレクトメッセージだけです。';

  @override
  String get storageDirect => 'ダイレクトメッセージ';

  @override
  String get storageOfNoLimit => '/ 制限なし';

  @override
  String storageOfLimit(String limit) {
    return '/ $limit';
  }

  @override
  String storageLegendPictures(String size) {
    return '画像 $size';
  }

  @override
  String storageLegendVideos(String size) {
    return '動画 $size';
  }

  @override
  String storageLegendFiles(String size) {
    return 'ファイル $size';
  }

  @override
  String storageLegendMessages(String size) {
    return 'メッセージ $size';
  }

  @override
  String get storageRoom => 'ダイレクトメッセージの容量';

  @override
  String get storageNoLimitNote => '自動では何も削除されません。';

  @override
  String storageOverNote(String limit) {
    return '上限を超えています。古い画像、動画、ファイルから順に、$limitになるまで削除されます。メッセージは常に残ります。';
  }

  @override
  String get storageLimitNote =>
      '上限を超えると、古い画像、動画、ファイルから削除されます。メッセージは常に残り、その場所には「空き容量確保のため、このパソコンから削除済み」と表示されます。';

  @override
  String get storageAutoPictures => '画像を自動でダウンロード';

  @override
  String get storageAutoPicturesNote => '1つ10 MBまで、届いたときにダウンロードします。';

  @override
  String get storageAutoFiles => '動画とファイルを自動でダウンロード';

  @override
  String get storageAutoFilesNote => 'オフ：送信者がオンラインのときに、開くとダウンロードされます。';

  @override
  String get storageTimer => '新しいチャットの消えるメッセージ';

  @override
  String get storageTimerNote => 'チャットごとに変更できます。2人には常に同じタイマーが表示されます。';

  @override
  String get storageTimer24h => '24時間';

  @override
  String get storageByChat => 'チャット別';

  @override
  String get storageByChatNote =>
      '消去すると、このパソコンから画像、動画、ファイルが削除されます。メッセージは残り、相手のコピーもそのままです。';

  @override
  String get storageMessagesOnly => 'メッセージのみ';

  @override
  String get storageClearFiles => 'ファイルを消去';

  @override
  String storageClearFilesTooltip(String name) {
    return '$nameとのチャットのファイルを消去';
  }

  @override
  String get storageCleared => '消去済み';

  @override
  String storageConfirmTitle(String limit) {
    return '画像・動画・ファイルを$limitまでにしますか？';
  }

  @override
  String storageConfirmText(String limit) {
    return '古いものから今すぐ$limitになるまで削除されます。ピン留めされたメッセージのものは最後に削除されます。メッセージは常に残り、フレンドのコピーもそのままです。';
  }

  @override
  String get storageRemoveOldest => '古いものを削除';

  @override
  String get themePickerSystem => 'システムのテーマ（パソコンの設定に従う）';

  @override
  String themePickerTheme(String name) {
    return '$nameテーマ';
  }

  @override
  String get themeDark => 'ダーク';

  @override
  String get themeMidnight => 'ミッドナイト';

  @override
  String get themeDim => 'ディム';

  @override
  String get themeEmber => '残り火';

  @override
  String get themeContrast => 'コントラスト';

  @override
  String get themeLight => 'ライト';

  @override
  String get themeSystem => 'システム';

  @override
  String get themeBannerViolet => 'バイオレット';

  @override
  String get themeBannerBlue => 'ブルー';

  @override
  String get themeBannerGreen => 'グリーン';

  @override
  String get themeBannerClay => 'クレイ';

  @override
  String get themeBannerRaspberry => 'ラズベリー';

  @override
  String get themeBannerMustard => 'マスタード';

  @override
  String get themeBannerSlate => 'スレート';

  @override
  String get themeBannerNight => 'ナイト';

  @override
  String widgetsCopyWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'QRコードをコピー',
      'link': 'リンクをコピー',
      'code': 'コードをコピー',
      'other': 'コピー',
    });
    return '$_temp0';
  }

  @override
  String widgetsShowWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'QRコードを表示',
      'link': 'リンクを表示',
      'code': 'コードを表示',
      'other': '表示',
    });
    return '$_temp0';
  }

  @override
  String get widgetsHide => '隠す';

  @override
  String revealFailed(String path) {
    return 'フォルダーを開けませんでした。ファイルの場所：$path';
  }

  @override
  String get emojiGroupPeople => '人々';

  @override
  String get emojiGroupNature => '自然';

  @override
  String get emojiGroupFood => '食べ物・飲み物';

  @override
  String get emojiGroupActivities => 'アクティビティ';

  @override
  String get emojiGroupTravel => '旅行・場所';

  @override
  String get emojiGroupObjects => 'オブジェクト';

  @override
  String get emojiGroupSymbols => '記号';

  @override
  String get emojiGroupFlags => '旗';

  @override
  String get emojiPickerLimit => 'メッセージが上限に達しています。';

  @override
  String get emojiPickerButton => '絵文字';

  @override
  String get emojiPickerResults => '検索結果';

  @override
  String get emojiPickerRecent => '最近使用';

  @override
  String get emojiPickerSearch => '絵文字を検索（英語名）';

  @override
  String get emojiPickerSkinTone => '肌の色';

  @override
  String get emojiPickerNone => '絵文字が見つかりません';

  @override
  String get emojiPickerNoFont =>
      'このパソコンにはカラー絵文字のフォントがないため、絵文字が単色や空白で表示されることがあります。Noto Color Emojiをインストールすると解決します。';

  @override
  String get emojiPickerDefaultTone => '標準の肌の色';

  @override
  String emojiPickerTone(int n) {
    return '肌の色$n';
  }

  @override
  String get iconsGroupMountain => '山';

  @override
  String get iconsGroupStar => '星';

  @override
  String get iconsGroupGameController => 'ゲームコントローラー';

  @override
  String get iconsGroupMusic => '音楽';

  @override
  String get iconsGroupBook => '本';

  @override
  String get iconsGroupWaves => '波';

  @override
  String get iconsGroupCoffee => 'コーヒー';

  @override
  String get iconsGroupHeart => 'ハート';

  @override
  String get iconsGroupFilm => '映画';

  @override
  String get newGroupPictureUnreadable => 'その画像を読み込めませんでした。';

  @override
  String newGroupMade(String name) {
    return 'グループ「$name」を作成しました。メンバーがオンラインになると届きます。';
  }

  @override
  String get newGroupTitle => '新しいグループ';

  @override
  String get newGroupSubtitle => 'グループはずっと残りますが、ルームは残りません。';

  @override
  String get newGroupName => 'グループ名';

  @override
  String get newGroupNameExample => '日曜サイクリング';

  @override
  String get newGroupIconOrPicture => 'アイコンまたは画像';

  @override
  String get newGroupChooseFromComputer => 'パソコンから選ぶ';

  @override
  String get newGroupBannerOptional => 'バナー（任意）';

  @override
  String get newGroupMembers => 'メンバー';

  @override
  String newGroupSelected(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n人選択中',
    );
    return '$_temp0';
  }

  @override
  String get newGroupSearchPlaceholder => 'フレンドを検索';

  @override
  String get newGroupSearchLabel => 'フレンドを検索';

  @override
  String get newGroupNoFriends => 'フレンドはまだいません。グループを作成して後から追加するか、リンクで招待できます。';

  @override
  String get newGroupKeptOnDevices => '各メンバーのデバイスに保存されます。';

  @override
  String get newGroupCreate => 'グループを作成';

  @override
  String get groupBannerSettings => 'グループ設定';

  @override
  String get groupBannerPictureUnreadable => 'その画像を読み込めませんでした。';

  @override
  String get groupBannerChoose => 'バナーを選択';

  @override
  String get groupBannerAnother => '別のバナー';

  @override
  String get groupBannerNone => 'バナーなし';

  @override
  String get groupBannerNote =>
      'グループの列の上部、名前の下に表示されます。PNG、JPEG、または550 KBまでの動くGIFかWebPに対応しています。';

  @override
  String get homeTitle => 'ホーム';

  @override
  String get homeSubtitle => 'ルームは誰かが入ると灯り、全員が退出すると消えます。';

  @override
  String get homeAddFriend => 'フレンドを追加';

  @override
  String get homeNewGroup => '新しいグループ';

  @override
  String get homeLive => 'ライブ';

  @override
  String get homeNoRoom => 'ルームなし';

  @override
  String get homeNoRoomNote => '開くとルームが灯ります';

  @override
  String homeInviteBy(String name, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'メンバー$n人',
    );
    return '$nameがあなたを追加 · $_temp0';
  }

  @override
  String get homeInviteNote => '参加するまで、このパソコンはメンバーに接続せず、何も送信しません。';

  @override
  String get homeInviteJoin => '参加';

  @override
  String get homeNoGroups => 'グループはまだありません';

  @override
  String get homeNoGroupsNote =>
      'フレンドを追加して、一緒にグループを作成しましょう。グループはずっと残りますが、ルームは残りません。';

  @override
  String homePersonYou(String name) {
    return '$name（あなた）';
  }

  @override
  String homeMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'メンバー$n人',
    );
    return '$_temp0';
  }

  @override
  String get homeYouAreIn => '参加中';

  @override
  String get homeMuteMic => 'マイクをミュート';

  @override
  String get homeUnmuteMic => 'マイクのミュートを解除';

  @override
  String get homeCameraOff => 'カメラをオフ';

  @override
  String get homeCameraOn => 'カメラをオン';

  @override
  String get homeMembersTooltip => 'メンバー';

  @override
  String get homeReturnToRoom => 'ルームに戻る';

  @override
  String get homeJoinRoom => '参加';

  @override
  String homeMembersOnline(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: 'メンバー$members人',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online人がオンライン',
      zero: '他にオンラインの人はいません',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get homeNoRoomsYet => 'まだルームはありません';

  @override
  String homeLastRoom(String when) {
    return '前回のルーム：$when';
  }

  @override
  String get homeOpenRoom => 'ルームを開く';

  @override
  String get membersGroupGone => 'このグループはもうありません。';

  @override
  String membersSummary(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: 'メンバー$members人',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online人がオンライン',
      zero: '他にオンラインの人はいません',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String membersSummaryLastRoom(int members, int online, String when) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: 'メンバー$members人',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online人がオンライン',
      zero: '他にオンラインの人はいません',
    );
    return '$_temp0 · $_temp1 · 前回のルーム：$when';
  }

  @override
  String get membersCopyInviteLink => '招待リンクをコピー';

  @override
  String membersInviteLinkTooltip(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'グループの誰か',
      'first': 'グループの1番目のメンバー',
      'other': 'グループの上位$n人のいずれか',
    });
    return '1回だけ、24時間有効です。$_temp0が参加を承認します';
  }

  @override
  String membersInviteLinkCopied(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'グループの誰か',
      'first': 'グループの1番目のメンバー',
      'other': 'グループの上位$n人のいずれか',
    });
    return '招待リンクをコピーしました。1回だけ、24時間有効です。$_temp0が参加を承認します。';
  }

  @override
  String get membersInviteFriends => 'フレンドを招待';

  @override
  String get membersBackToRoom => 'ルームに戻る';

  @override
  String get membersOpenRoom => 'ルームを開く';

  @override
  String get membersEnterRoom => 'ルームに入る';

  @override
  String get membersTitle => 'メンバー';

  @override
  String get membersOrderNote => '順位順：各メンバーは自分より下の人を管理できます';

  @override
  String get membersGroupSettings => 'グループ設定';

  @override
  String membersManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'グループを変更できるのは上位$n人だけです。',
      one: 'グループを変更できるのは1番目のメンバーだけです。',
    );
    return '$_temp0';
  }

  @override
  String get membersLeaveGroup => 'グループから退出';

  @override
  String membersInviteTitle(String group) {
    return '「$group」にフレンドを招待';
  }

  @override
  String get membersInviteNote => '招待が届き、参加するかどうかは相手が選びます。';

  @override
  String get membersPresenceYou => 'あなた';

  @override
  String membersPresenceOnline(String how) {
    return 'オンライン · $how';
  }

  @override
  String membersPresenceAway(String how) {
    return '退席中 · $how';
  }

  @override
  String get membersPresenceOffline => '現在オフライン';

  @override
  String membersYou(String name) {
    return '$name（あなた）';
  }

  @override
  String get membersWaitingForKey => '鍵を待っています';

  @override
  String membersFingerprint(String fingerprint) {
    return 'フィンガープリント：$fingerprint';
  }

  @override
  String membersOptionsFor(String name) {
    return '$nameのオプション';
  }

  @override
  String get membersMoveUp => '自分のすぐ下に上げる';

  @override
  String get membersMoveToEnd => '最後に移動';

  @override
  String get membersTimeOut => 'ルームからタイムアウト…';

  @override
  String get membersRemoveFromGroup => 'グループから外す';

  @override
  String membersRemoveTitle(String name) {
    return '$nameを外しますか？';
  }

  @override
  String membersRemoveText(String name) {
    return '$nameはグループから外れ、グループは新しい鍵を作成します。すでに見た内容は、$nameの手元に残ります。';
  }

  @override
  String get membersFirst => '1番目';

  @override
  String membersTimeOutTitle(String name) {
    return '$nameをタイムアウト';
  }

  @override
  String membersTimeOutText(String name) {
    return '$nameはルームから退出し、時間が過ぎるか、上の人が許可するまで戻れません。';
  }

  @override
  String get membersUntilLetBack => '許可するまで';

  @override
  String membersLeaveTitle(String group) {
    return '「$group」から退出しますか？';
  }

  @override
  String get membersLeaveText =>
      'グループのルームが見えなくなり、他の人からもグループ内であなたが見えなくなります。グループはあなた抜きで新しい鍵を作成します。戻るには、グループの誰かにもう一度承認してもらう必要があります。';

  @override
  String membersLeaveTextFirst(String name) {
    return 'グループのルームが見えなくなり、他の人からもグループ内であなたが見えなくなります。グループはあなた抜きで新しい鍵を作成します。戻るには、グループの誰かにもう一度承認してもらう必要があります。$nameが1番目になります。';
  }

  @override
  String get groupSettingsTabGeneral => '一般';

  @override
  String get groupSettingsTabVoiceRooms => 'ボイスルーム';

  @override
  String get groupSettingsTabMembers => 'メンバー';

  @override
  String get groupSettingsTabInvite => '招待';

  @override
  String get groupSettingsTabLimits => '上限';

  @override
  String get groupSettingsTabActivity => 'アクティビティ';

  @override
  String get groupSettingsFirstHere => 'このグループの1番目';

  @override
  String groupSettingsPlaceHere(String place) {
    return 'このグループで$place';
  }

  @override
  String get groupSettingsLeave => 'グループから退出';

  @override
  String get groupSettingsInviteTitle => 'メンバーを招待';

  @override
  String get groupSettingsClose => 'グループ設定を閉じる';

  @override
  String get groupSettingsPicture => '画像';

  @override
  String get groupSettingsChooseFromComputer => 'パソコンから選ぶ';

  @override
  String get groupSettingsUseIcon => 'アイコンを使う';

  @override
  String get groupSettingsPictureNote =>
      'PNG、JPEG、WebP（プロフィール写真のように正方形に切り抜かれ、縮小されます）、または550 KBまでのアニメーションGIF・WebPに対応しています。';

  @override
  String get groupSettingsOrIcon => 'またはアイコン';

  @override
  String get groupSettingsBanner => 'バナー';

  @override
  String get groupSettingsName => '名前';

  @override
  String get groupSettingsNameField => 'グループ名';

  @override
  String get groupSettingsNameNote => '新しい名前と画像は、グループの全員に表示されます。';

  @override
  String get groupSettingsRoomsIntro =>
      'ボイスルームごとに参加者、チャット、ファイル、通話が分かれており、別のルームの人とはつながりません。1人が入れるルームは同時に1つです。';

  @override
  String get groupSettingsRoomName => 'ルーム名';

  @override
  String get groupSettingsAccessEveryone => '全員';

  @override
  String get groupSettingsAccessSome => '一部のみ';

  @override
  String groupSettingsWhoCanEnter(String room) {
    return '$roomに入れる人';
  }

  @override
  String get groupSettingsFirstRoomStays => '最初のルームは削除できません';

  @override
  String get groupSettingsPeopleInside => '中に人がいます';

  @override
  String get groupSettingsDeleteRoom => 'ルームを削除';

  @override
  String get groupSettingsAddRoom => 'ボイスルームを追加';

  @override
  String groupSettingsNewRoomName(int n) {
    return 'ルーム$n';
  }

  @override
  String groupSettingsRoomsNote(int rooms, int people) {
    return 'ルームは最大$rooms個、各ルーム$people人まで（「上限」で設定）。人がいるルームは削除できません。';
  }

  @override
  String get groupSettingsKeptOut => 'タイムアウト中';

  @override
  String get groupSettingsAccessEveryoneLong => 'グループの全員';

  @override
  String get groupSettingsAccessChosen => '選んだメンバーのみ';

  @override
  String get groupSettingsAccessNote =>
      'あなたと、あなたより上の人はいつでも入れます。対象外の人には、ルームに鍵マークが表示されます。';

  @override
  String groupSettingsYou(String name) {
    return '$name（あなた）';
  }

  @override
  String groupSettingsKeptOutBy(String room, String by) {
    return ' · $room · $byによる';
  }

  @override
  String groupSettingsTimeLeft(String time) {
    return '残り$time';
  }

  @override
  String get groupSettingsUntilLetBack => '許可するまで';

  @override
  String get groupSettingsLetBack => '解除';

  @override
  String groupSettingsManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'グループの名前、ルーム、招待、上限を変更できるのは上位$n人だけです。あなたは自分より下の人を管理できます。',
      one: 'グループの名前、ルーム、招待、上限を変更できるのは1番目のメンバーだけです。あなたは自分より下の人を管理できます。',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsMembersCount(int n, int max) {
    String _temp0 = intl.Intl.pluralLogic(
      max,
      locale: localeName,
      other: '$max人',
    );
    return 'メンバー$n人 / 最大$_temp0（順位順）';
  }

  @override
  String get groupSettingsMembersNote =>
      '各メンバーは自分より下の人を、自分のすぐ下に上げる、最後に移動する、ルームからタイムアウトする、グループから外す、といった操作ができます。新しく入った人は最後から始まります。';

  @override
  String get groupSettingsWhoManages => 'このグループを管理する人';

  @override
  String get groupSettingsManagersEveryone => '全員';

  @override
  String groupSettingsManagersFirstN(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '上位$n人',
      one: '1番目のメンバー',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsManagersMore(int n, int members) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members人',
    );
    return '上位$n人に設定されていますが、現在のメンバーは$_temp0なので、今は全員が管理者です。';
  }

  @override
  String get groupSettingsManagersEveryoneNote =>
      '管理者は名前、画像、ルーム、上限を変更し、参加を承認します。全員が管理者なら、誰かがデバイスを失ってもグループは続きます。';

  @override
  String groupSettingsManagersFirstNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '名前、画像、ルーム、上限の変更と参加の承認ができるのは上位$n人だけです。全員がデバイスを失うと、誰もグループを管理できなくなります。Fogataには修復できるサーバーがありません。',
      one: '名前、画像、ルーム、上限の変更と参加の承認ができるのは1番目のメンバーだけです。そのデバイスが失われると、誰もグループを管理できなくなります。Fogataには修復できるサーバーがありません。',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsMoveUp => '上げる';

  @override
  String groupSettingsMoveUpTooltip(String name) {
    return '$nameを自分のすぐ下に上げる';
  }

  @override
  String get groupSettingsToEnd => '最後へ';

  @override
  String groupSettingsToEndTooltip(String name) {
    return '$nameを最後に移動';
  }

  @override
  String get groupSettingsTimeOut => 'ルームからタイムアウト';

  @override
  String get groupSettingsRemove => 'グループから外す';

  @override
  String get groupSettingsAskingToJoin => '参加申請';

  @override
  String get groupSettingsInviteFriend => 'フレンドを招待';

  @override
  String get groupSettingsInviteFriendNote =>
      'Fogataで招待が届き、参加するかどうかは相手が選びます。ここにはあなたのフレンドだけが表示されます。';

  @override
  String get groupSettingsInviteLink => '招待リンク';

  @override
  String groupSettingsInviteLinkNote(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'グループの誰か',
      'first': 'グループの1番目のメンバー',
      'other': 'グループの上位$n人のいずれか',
    });
    return 'まだフレンドではない人向けです。1回だけ、24時間有効で、$_temp0が参加を承認します。別の人に届いた場合も、申請者を確認して拒否できます。';
  }

  @override
  String get groupSettingsCopyLink => '招待リンクをコピー';

  @override
  String groupSettingsOpenLinks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '未使用のリンクが$n件あります。',
      zero: '未使用のリンクはありません。',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsCancelLinks => 'すべて無効にする';

  @override
  String get groupSettingsNoFriends => 'フレンドはまだいません。「フレンド」ページで追加してください。';

  @override
  String get groupSettingsAllFriendsIn => 'フレンドは全員グループに参加しています。';

  @override
  String get groupSettingsFriendOnline => 'オンライン';

  @override
  String get groupSettingsFriendOffline => '現在オフライン';

  @override
  String get groupSettingsInviteButton => '招待';

  @override
  String get groupSettingsGroupFull => 'グループは満員です';

  @override
  String get groupSettingsSayNo => '拒否';

  @override
  String get groupSettingsLetIn => '承認';

  @override
  String get groupSettingsLimitGroup => 'グループの人数';

  @override
  String groupSettingsLimitGroupNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n人');
    return '2〜20人。満員になると、招待とリンクは使えなくなります。すでにいる$_temp0より少なくはできません。';
  }

  @override
  String get groupSettingsLimitRoom => '各ボイスルームの人数';

  @override
  String get groupSettingsLimitRoomNote =>
      '2〜8人。音声とビデオはルーム内の全員の間で直接やり取りされるため、快適に聞こえるのは8人までです。カメラをオンにする場合は、少ないほうが安定します。満員のときに来た人には、ルームが満員だと表示されます。';

  @override
  String get shellChipCallMany => '直接 · 暗号化 · 通話中の人だけ';

  @override
  String get shellChipCallTwo => '直接 · 暗号化 · 2人だけ';

  @override
  String get shellGroupMembers => 'グループのメンバー';

  @override
  String get shellMembers => 'メンバー';

  @override
  String get shellDirectMessages => 'ダイレクトメッセージ';

  @override
  String get shellPathDirect => '直接 · 暗号化';

  @override
  String shellPathVia(String name) {
    return '$name経由 · 暗号化';
  }

  @override
  String get shellPathViaSomeone => 'メンバー経由 · 暗号化';

  @override
  String shellCallWith(String names) {
    return '$namesと通話';
  }

  @override
  String get shellRinging => '呼び出し中 · ';

  @override
  String get shellInCall => '通話中 · ';

  @override
  String get shellCallEnded => '通話終了';

  @override
  String get shellNewGroup => '新しいグループ';

  @override
  String get shellSectionCall => '通話';

  @override
  String shellCallRinging(String name) {
    return '$name · 呼び出し中';
  }

  @override
  String get shellSectionLiveNow => 'ライブ中';

  @override
  String get shellSectionInCall => '通話中';

  @override
  String get shellChat => 'チャット';

  @override
  String get shellFiles => 'ファイル';

  @override
  String get shellSectionGroup => 'グループ';

  @override
  String get shellInvite => '招待';

  @override
  String get shellSectionVoiceRooms => 'ボイスルーム';

  @override
  String shellVoiceRoomYouAreIn(String room) {
    return '$room · 参加中';
  }

  @override
  String get shellNotForYou => '入室不可';

  @override
  String get shellOnlySomeMembers => '一部のメンバーのみ';

  @override
  String shellPersonYou(String name) {
    return '$name（あなた）';
  }

  @override
  String get shellLive => 'ライブ';

  @override
  String get shellYourProfile => 'あなたのプロフィール';

  @override
  String shellMuteMic(String keys) {
    return 'マイクをミュート（$keys）';
  }

  @override
  String shellUnmuteMic(String keys) {
    return 'マイクのミュートを解除（$keys）';
  }

  @override
  String get shellDeafen => 'スピーカーミュート';

  @override
  String get shellUndeafen => 'スピーカーミュートを解除';

  @override
  String get shellStateCalling => '発信中…';

  @override
  String get shellStateInCall => '通話中';

  @override
  String get shellStateInRoom => 'ルーム内';

  @override
  String get shellStateOffline => 'オフライン';

  @override
  String get shellStateOnline => 'オンライン';

  @override
  String get shellStateAway => '退席中';

  @override
  String get shellShowInFolder => 'フォルダーで表示';

  @override
  String get shellCopyPath => 'パスをコピー';

  @override
  String shellVolumePercent(int n) {
    return '$n%';
  }

  @override
  String get shellMuteKeys => 'Ctrl+Shift+M';

  @override
  String get settingsSectionDonate => 'Fogataに寄付';

  @override
  String get settingsSectionReport => '問題を報告';

  @override
  String get settingsHeadingHelp => 'ヘルプ';

  @override
  String get settingsPreRelease => 'プレリリース';

  @override
  String get donateTitle => 'Fogataの独立を支える';

  @override
  String get donateText => 'Fogataは無料で、広告も、あなたを監視するサーバーもありません。寄付がそれを支えます。';

  @override
  String get donateButton => '寄付する';

  @override
  String get donateNote => 'ブラウザーでKo-fiが開きます。Fogataが支払い情報を見ることはありません。';

  @override
  String get reportWhatTitle => '何が起きましたか？';

  @override
  String get reportWhatNote => '何をしていたか、何を期待していたか、代わりに何が起きたか。';

  @override
  String get reportIncludeLog => '過去2日間の技術ログを含める';

  @override
  String get reportLogNote => 'バージョン、エラー、接続の手順。メッセージ、鍵、連絡先、IPアドレスは含まれません。';

  @override
  String get reportSeeSent => '送信内容を見る';

  @override
  String get reportHideSent => '一覧を隠す';

  @override
  String get reportPreview => '送信前にレポート全体を読む';

  @override
  String get reportHidePreview => 'レポートを隠す';

  @override
  String get reportPreviewNote =>
      'これがファイルの中身そのもので、送られるのはこれだけです。名前、フォルダー、ファイル名、アドレス、鍵はすでにログから取り除かれています。';

  @override
  String get reportPreviewWorking => 'レポートを準備しています…';

  @override
  String get reportSentWhat => '上に書いた内容。';

  @override
  String get reportSentVersion => 'Fogataのバージョンとビルド、システムとそのバージョン、Fogataの表示言語。';

  @override
  String get reportSentLog =>
      '過去2日間のログ：Fogataの起動時刻、エラー、他のデバイスへの接続方法。名前、メッセージ、フォルダー、ファイル名、リンク、コード、鍵、IPアドレスは取り除かれ、各デバイスはこのレポートの外では意味のない番号でのみ示されます。';

  @override
  String get reportEmail => 'Fogataチームにメールで送る';

  @override
  String get reportSave => 'ファイルに保存';

  @override
  String reportHowEmail(String address) {
    return 'メールアプリが$address宛てのメッセージを用意して開き、その横にレポートファイルのあるフォルダーが開きます。ファイルを添付して送信してください。送信するまで、何もパソコンの外に出ません。';
  }

  @override
  String get reportReadyTitle => 'メールの準備ができました';

  @override
  String reportReadyText(String file, String address) {
    return '開いたフォルダーから$fileを添付して、$addressに送信してください。';
  }

  @override
  String get reportShowFile => 'ファイルを表示';

  @override
  String get reportCopyAddress => 'アドレスをコピー';

  @override
  String get reportAddressCopied => 'アドレスをコピーしました';

  @override
  String reportSaved(String file) {
    return 'レポートを保存しました：$file';
  }

  @override
  String reportNoMailApp(String address) {
    return 'メールアプリが開きませんでした。お使いのメールから$addressにファイルを送ってください。';
  }

  @override
  String reportFailed(String reason) {
    return 'レポートを書き込めませんでした：$reason';
  }

  @override
  String get splashStarting => '起動中';

  @override
  String get splashConnecting => '接続中';

  @override
  String get splashChecking => 'アップデートを確認中';

  @override
  String get splashDownloading => 'アップデートをダウンロード中';

  @override
  String get splashInstalling => 'アップデートをインストール中';

  @override
  String get splashRestarting => '再起動中';

  @override
  String splashProgress(String done, String total) {
    return '$done / $total MB';
  }

  @override
  String splashProgressFrom(String done, String total, String name) {
    return '$done / $total MB · $nameから';
  }

  @override
  String updateReady(String version) {
    return 'アップデート準備完了 · $version';
  }

  @override
  String updateRequired(String version) {
    return 'アップデートが必要 · $version';
  }

  @override
  String get updateRestart => '再起動';

  @override
  String updateReadyTip(String version) {
    return 'Fogata $versionの準備ができました。再起動すると使えます。';
  }

  @override
  String get updateRequiredTip =>
      'このバージョンは、フレンドの新しいバージョンと通信できなくなりました。再起動してアップデートしてください。';

  @override
  String updateFailed(String reason) {
    return 'アップデートをインストールできませんでした：$reason';
  }

  @override
  String get errJoinInviteGone => 'その招待はもう使えないか、グループが満員です。';

  @override
  String get errJoinNoKeyYet =>
      'このデバイスにはまだグループの鍵がありません。鍵が届きしだい、新しいメンバーが参加できます。';

  @override
  String get errJoinWroteTooMuch =>
      'このデバイスの変更が、まだ他のメンバーに取り込まれていません。他のメンバーがオンラインになれば、新しいメンバーを参加させられます。';

  @override
  String get errJoinAskAgain =>
      'その参加申請は、以前の状態のグループに対して出されたものです。もう一度申請してもらう必要があります。';

  @override
  String get errNoSpaceForFile =>
      'このファイルを保存する空き容量が足りません。空き容量を確保してから、もう一度お試しください。';

  @override
  String get recoveryTitle => '復元用コピー';

  @override
  String get recoveryNote =>
      'このパソコンのキーチェーンやディスクが失われても、このアカウントを取り戻せるファイルです。開けるのはあなたの復元キーだけです。';

  @override
  String get recoveryOff => 'オフ';

  @override
  String recoveryOn(String id) {
    return 'オン · キー $id';
  }

  @override
  String get recoveryTurnOn => 'オンにする…';

  @override
  String get recoveryChangeKey => 'キーを変更…';

  @override
  String get recoveryMakeCopy => 'コピーを作成…';

  @override
  String get recoveryReplacedKept => '復元で置き換えられたアカウントが、このパソコンにまだ保管されています。';

  @override
  String get recoveryDeleteReplaced => '削除…';

  @override
  String get recoveryChangeTitle => '新しい復元キーを作成しますか？';

  @override
  String get recoveryChangeText =>
      '新しいコピーは新しいキーでしか開けません。以前のコピーは引き続き古いキーでしか開けないので、それらのコピーを持っている間は古いキーも保管してください。';

  @override
  String get recoveryChangeConfirm => '新しいキーを作成';

  @override
  String get recoveryKeyTitle => 'あなたの復元キー';

  @override
  String get recoveryKeyText => '書き留めるか印刷して、コピーとは別の場所に保管してください。表示されるのは今回だけです。';

  @override
  String recoveryKeyName(String id) {
    return '短い名前：$id';
  }

  @override
  String get recoveryKeyWarning =>
      'Fogataはこのキーをどこにも保存しません。キーがなければ、どのコピーも開けません。コピーとこのキーの両方を持つ人は、あなたのアカウントを使えます。';

  @override
  String get recoveryKeyWritten => '書き留めました';

  @override
  String get recoveryCheckTitle => '書き留めた内容を確認';

  @override
  String get recoveryCheckText => 'キーの次の2つのグループを、書き留めたとおりに入力してください。';

  @override
  String recoveryCheckGroup(int n) {
    return 'グループ$n';
  }

  @override
  String get recoveryCheckWrong => '違います。書き留めた内容をもう一度確認してください。';

  @override
  String get recoveryTurnOnButton => 'コピーをオンにする';

  @override
  String recoveryTurnedOn(String id) {
    return 'コピーをオンにしました（キー $id）。';
  }

  @override
  String get recoveryCopyTitle => '復元用コピーを作成';

  @override
  String get recoveryCopyText =>
      'コピーの作成中、Fogataはオフラインになります。チャットのファイルを含めると長くかかります。ファイルはUSBメモリなど、このパソコン以外の場所に保管してください。';

  @override
  String get recoveryCopyAttachments => 'チャットで送ったファイルも含める';

  @override
  String recoveryCopyAttachmentsNote(String size) {
    return '画像、動画、ファイル：さらに$size';
  }

  @override
  String get recoveryCopySave => '保存先を選ぶ…';

  @override
  String get recoveryCopyInDataFolder =>
      'コピーはFogataのデータフォルダーには保存できません。別の場所を選んでください。';

  @override
  String recoveryCopyMade(String file, String size) {
    return 'コピーを保存しました：$file（$size）';
  }

  @override
  String recoveryCopyFailed(String why) {
    return 'コピーを作成できませんでした。$why';
  }

  @override
  String get recoveryDeleteTitle => '置き換えられたアカウントを削除しますか？';

  @override
  String get recoveryDeleteText => 'コピーを復元したときに、別の場所に移されたアカウントです。削除すると元に戻せません。';

  @override
  String get recoveryDelete => '削除';

  @override
  String get recoveryDeleted => '置き換えられたアカウントを削除しました。';

  @override
  String get recoveryFileType => 'Fogataのコピー';

  @override
  String get recoveryLostComputer => 'パソコンをなくしましたか？';

  @override
  String get recoveryRestoreLink => 'コピーから復元';

  @override
  String get recoveryRestoreTitle => 'コピーから復元';

  @override
  String get recoveryRestoreSub => '保存した復元用コピーを選び、復元キーを入力してください。';

  @override
  String get recoveryChooseCopy => 'コピーを選ぶ…';

  @override
  String recoveryMadeOn(String date) {
    return '$dateに作成されたコピーです。';
  }

  @override
  String get recoveryOtherKey => 'このパソコンのものとは別の復元キー用に作成されています。作成時のキーを入力してください。';

  @override
  String get recoveryKeyField => '復元キー';

  @override
  String get recoveryKeyNotAKey => '復元キーは53文字の英数字で、4文字ずつのグループに分かれています。';

  @override
  String get recoveryKeyMistyped => '文字か数字が1つ違います。確認してください。';

  @override
  String get recoveryProblemNotACopy => 'これはFogataアカウントのコピーではありません。';

  @override
  String get recoveryProblemNewer =>
      'このコピーは新しいバージョンのFogataで作成されました。開くにはFogataをアップデートしてください。';

  @override
  String get recoveryProblemOtherKey => 'このコピーは別の復元キー用に作成されています。';

  @override
  String get recoveryProblemDamaged => 'このコピーは壊れているか、作成後に変更されています。';

  @override
  String get recoveryProblemUnproven => 'このコピーは中のアカウントによって作成されたものではないため、開けません。';

  @override
  String recoveryWhose(String name) {
    return '$nameのアカウント';
  }

  @override
  String recoveryHolds(int files, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files個のファイル',
    );
    return '$_temp0、合計$size';
  }

  @override
  String get recoveryWithAttachments => 'チャットで送ったファイルを含みます。';

  @override
  String get recoveryWithoutAttachments => 'チャットで送ったファイルは含みません。';

  @override
  String get recoveryOnlyIf =>
      'コピーを使うのは、他のどのパソコンもこのアカウントを使っていない場合だけにしてください。まだ使っているパソコンがあるなら、そこからアカウントを移行してください。1つのアカウントを2台のパソコンで使うことはできません。';

  @override
  String get recoveryMoveInstead => '代わりに移行する';

  @override
  String recoveryHereNamed(String name) {
    return 'このパソコンには$nameのアカウントがあります。';
  }

  @override
  String recoveryHereDevice(String fingerprint) {
    return 'このパソコンにはプロフィールのないアカウントがあります（デバイス $fingerprint）。';
  }

  @override
  String get recoveryHereUnknown => 'このパソコンには、開けないアカウントの一部があります。';

  @override
  String get recoveryHereReplaced =>
      '復元するとこれは置き換えられます。削除はされず、設定 › ストレージとダウンロードで削除するまで別の場所に保管されます。';

  @override
  String get recoveryRestore => '復元';

  @override
  String get recoveryReplace => '置き換えて復元';

  @override
  String recoveryRestoring(String name) {
    return '$nameのアカウントを復元しています…';
  }

  @override
  String recoveryRestored(String name) {
    return '$nameのアカウントを復元しました。';
  }

  @override
  String get recoveryRefusedAccountHere =>
      '現在、このパソコンにはアカウントがあります。何が置き換えられるか確認するため、もう一度コピーを選んでください。';

  @override
  String get recoveryRefusedReplacedKept =>
      '以前の復元で置き換えられたアカウントが、まだこのパソコンに保管されています。このコピーを復元するには、そのアカウントを削除してください。';

  @override
  String recoveryRefusedNoRoom(String need, String free) {
    return 'このコピーにはこのディスクの空きが$need必要ですが、空きは$freeです。何も変更されていません。';
  }

  @override
  String get recoveryRefusedInFolder =>
      'コピーがFogataのデータフォルダーの中にあります。別の場所に移してから、もう一度選んでください。';

  @override
  String get recoveryBusy => 'Fogataは復元用コピーか移行の処理中です。終わってからもう一度お試しください。';

  @override
  String get moveBusyWithCopy => 'Fogataは復元用コピーの処理中です。終わってからアカウントを移行してください。';

  @override
  String get recoveryDeleteKeptAside => '保管中のアカウントを削除…';

  @override
  String get reachOffline =>
      'Fogataはここからインターネット上の誰にも接続できません。接続を確認してください。ファイアウォールがFogataをブロックしている可能性もあります。';

  @override
  String get reachFirewall =>
      'このパソコンのファイアウォールがFogataへの接続をすべてブロックしています。許可するコマンドは通知にあります。';

  @override
  String get reachUdpBlocked =>
      'このネットワークまたはこのパソコンがUDPをブロックしています。通話やルーター越しの直接接続にはUDPが必要です。許可されるまでは、グループのメンバー経由か、公開アドレスからしかつながりません。';

  @override
  String get reachSymmetricNat =>
      'お使いのルーターは宛先ごとに異なる公開ポートを割り当てる（対称型NAT）ため、ルーター越しの直接の経路を開けません。別のルーターの内側にいるフレンドとは、共通のグループのメンバー経由でしかつながりません。';

  @override
  String get reachNoIpv6 => 'ルーターを回避できる、使えるIPv6もありません。';

  @override
  String reachFineFriend(String name) {
    return 'お使いのネットワークはFogataには問題なさそうです。つながらない原因はおそらく$nameの側（オフライン、または相手のネットワーク）にあります。';
  }

  @override
  String get reachFineOthers =>
      'お使いのネットワークはFogataには問題なさそうです。つながらない原因はおそらく相手側にあります。';
}
