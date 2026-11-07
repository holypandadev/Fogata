// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class L10nZh extends L10n {
  L10nZh([String locale = 'zh']) : super(locale);

  @override
  String get commonCancel => '取消';

  @override
  String get commonSave => '保存';

  @override
  String get commonClose => '关闭';

  @override
  String commonNewMessageFrom(String name, String text) {
    return '来自$name的新消息：$text';
  }

  @override
  String commonNewMessagesFrom(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '来自$name的$n条新消息',
    );
    return '$_temp0';
  }

  @override
  String commonNewMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n条新消息',
    );
    return '$_temp0';
  }

  @override
  String get commonAccept => '接受';

  @override
  String get commonDecline => '拒绝';

  @override
  String get commonSend => '发送';

  @override
  String get commonTryAgain => '重试';

  @override
  String get commonCopy => '复制';

  @override
  String get commonCopied => '已复制';

  @override
  String get commonBack => '返回';

  @override
  String get commonDiscard => '放弃';

  @override
  String get commonKeepEditing => '继续编辑';

  @override
  String get commonNotNow => '以后再说';

  @override
  String get commonDownload => '下载';

  @override
  String get commonSettings => '设置';

  @override
  String get commonHome => '主页';

  @override
  String get commonFriends => '好友';

  @override
  String get commonContinue => '继续';

  @override
  String get commonDone => '完成';

  @override
  String get commonRemove => '移除';

  @override
  String get commonYou => '你';

  @override
  String timeYesterdayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': '昨天 $time',
      'other': '昨天 $time',
    });
    return '$_temp0';
  }

  @override
  String timeDayAt(String date, String time) {
    return '$date $time';
  }

  @override
  String get timeNow => '刚刚';

  @override
  String timeMinutes(int n) {
    return '$n分钟';
  }

  @override
  String timeHours(int n) {
    return '$n小时';
  }

  @override
  String timeHoursMinutes(int hours, int minutes) {
    return '$hours小时$minutes分钟';
  }

  @override
  String timeDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n天');
    return '$_temp0';
  }

  @override
  String spanMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n分钟');
    return '$_temp0';
  }

  @override
  String spanHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n小时');
    return '$_temp0';
  }

  @override
  String spanDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n天');
    return '$_temp0';
  }

  @override
  String spanWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n周');
    return '$_temp0';
  }

  @override
  String get agoMoment => '刚刚';

  @override
  String agoMinutes(int n) {
    return '$n分钟前';
  }

  @override
  String agoHours(int n) {
    return '$n小时前';
  }

  @override
  String get agoYesterday => '昨天';

  @override
  String agoDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n天前');
    return '$_temp0';
  }

  @override
  String agoWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n周前');
    return '$_temp0';
  }

  @override
  String agoMonths(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n个月前',
    );
    return '$_temp0';
  }

  @override
  String get agoOverYear => '一年多以前';

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
    return '$first和$second';
  }

  @override
  String listMore(String rest, String last) {
    return '$rest和$last';
  }

  @override
  String get listSeparator => '、';

  @override
  String ordinal(int n) {
    return '第$n位';
  }

  @override
  String get errLinkExpired => '链接已过期。请向对方要一个新的。';

  @override
  String get errGroupInvite => '这是群组邀请链接。';

  @override
  String get errDeviceNotFound => '找不到对方的设备。请确认你们双方都在线。';

  @override
  String get errNotFriendCode => '这看起来不是Fogata链接或好友码。';

  @override
  String get errAlreadyInGroup => '你已经在这个群组里了。';

  @override
  String get errJoinRoomFirst => '请先进入房间。';

  @override
  String get errNewerVersion => '你的个人资料由更新版本的Fogata保存。请更新Fogata后再打开。';

  @override
  String get errKeychainUnreachable =>
      'Fogata无法从这台电脑的钥匙串中取得你个人资料的密钥。请解锁钥匙串或允许Fogata使用它，然后重试。';

  @override
  String get errKeychainNoKey => '这台电脑的钥匙串中已没有你个人资料的密钥，Fogata无法打开它。';

  @override
  String get errKeychainWrongKey => '这台电脑钥匙串中的密钥无法打开你的个人资料。';

  @override
  String get errNoGroupKey => '这个群组在这台电脑上还没准备好。请稍后重试。';

  @override
  String get errAlreadyRunning => 'Fogata已用这个个人资料打开。请先关闭另一个。';

  @override
  String get errFileGone => '这个文件已不在房间里。';

  @override
  String get errNobodyHasFile => '目前在线的人都没有这个文件。';

  @override
  String get errStillStarting => 'Fogata仍在启动中。';

  @override
  String get errIdentityExists => '这台电脑已有一个身份，其密钥无法更改。';

  @override
  String get errNameRule => '请输入1到40个可见字符的名字。';

  @override
  String get errBioRule => '个人简介最多190个字符、4行，且不能含隐藏字符。';

  @override
  String get errPictureRule => '图片必须是PNG或JPEG格式，最大64 KB。';

  @override
  String get errNotFriendLink => '这不是Fogata好友链接。';

  @override
  String get errLinkDamaged => '链接不完整或已损坏。';

  @override
  String get errLinkNeedsNewer => '这个链接需要更新版本的Fogata。';

  @override
  String get errCodeShape => '好友码的格式是FGT-XXXXX-XXXXX。';

  @override
  String get errCodeWrong => '好友码错误或已失效。';

  @override
  String get errNotProven => '对方设备无法证明自己的身份。';

  @override
  String get errCallGone => '这个来电已经停止响铃。';

  @override
  String get errAlreadyInCall => '你已在通话中。请先挂断。';

  @override
  String get errAlreadyFriends => '你们已经是好友了。';

  @override
  String get errOwnLink => '这是你自己的链接：请把它发给你想添加的好友。';

  @override
  String get errLinkUsed => '这个链接已被使用或已过期。请向对方要一个新的。';

  @override
  String get errLinkExpiredUnreached => '链接在联系上对方设备之前就过期了。请向对方要一个新的。';

  @override
  String get errCodeUnanswered => '没有设备回应这个好友码。请检查好友码，并确认对方设备仍在显示它。';

  @override
  String get errRefused => '对方设备拒绝了。这个好友码可能已被使用。';

  @override
  String get errNobodyLetIn => '没有人同意你加入。请在群组中有人在线时重试。';

  @override
  String get errNotBelow => '你只能对群组中排在你下面的人这样做。';

  @override
  String get errNotManager => '只有群组管理者才能这样做。';

  @override
  String get errLastManager => '你不能把自己从群组管理者中移除。';

  @override
  String get errRoomNotEmpty => '这个语音房间里还有人：请等房间空了再删除。';

  @override
  String get errTooManyFiles => '你共享的文件已达到房间对每人的上限。请先停止共享一些文件。';

  @override
  String get errDiskFull => '磁盘已满。请释放一些空间后重试。';

  @override
  String get errTooManyChanges => '这台设备对群组所做的更改过多，其他成员都还没有同步。请等群组中其他人做出更改后再试。';

  @override
  String get errSomething => '出错了。';

  @override
  String errRoomFull(int n) {
    return '这个语音房间已满：最多容纳$n人。';
  }

  @override
  String get findingOffline => '公共网络中没有节点回应：这台电脑似乎无法连接互联网，或者防火墙阻止了Fogata。';

  @override
  String get findingUdpWorks => 'UDP可用。通话和穿透路由器都需要它。';

  @override
  String get findingUdpBlocked =>
      '这个网络或这台电脑似乎阻止了UDP（通过TCP应答的节点没有通过QUIC应答）。聊天仍可通过其他成员转发，但穿透路由器的直接连接和通话大多会失败。请检查防火墙或网络规则。';

  @override
  String get findingCarrierNat =>
      '这台电脑使用运营商级NAT地址（100.64.0.0/10）：运营商让许多用户共用一个公网地址。';

  @override
  String get findingPublicIpv4 => '这台电脑有公网IPv4地址：其他人可以直接连接它。';

  @override
  String get findingHardNat => '位于对每个目标分配不同公网端口的路由器之后（严格型NAT）。';

  @override
  String get findingEasyNat => '位于对所有目标保持同一公网端口的路由器之后（宽松型NAT）：可以从双方同时打通。';

  @override
  String get findingUnknownRouter => '看到这台电脑公网地址的节点太少，无法判断路由器类型。';

  @override
  String get findingNoIpv6 => '没有公网IPv6地址。Fogata不需要它，但它能让直接连接更容易。';

  @override
  String get findingIpv6Works => '公网IPv6可用：有IPv6的好友可以绕开路由器直接连接。';

  @override
  String get findingIpv6Unreached => '这台电脑有公网IPv6地址，但还没有节点通过IPv6连接到它。';

  @override
  String get findingSummaryNoUdp => '总结：在允许UDP之前，直接连接可能会出问题。';

  @override
  String get findingSummaryPublic => '总结：你的好友应该可以直接连接这台电脑。';

  @override
  String get findingSummaryEasyNat =>
      '总结：与大多数好友的直接连接应该可行，双方路由器会同时打通；如果好友位于严格型NAT之后，需要一位群组成员在中间转发。';

  @override
  String get findingSummaryHardNatWithIpv6 =>
      '总结：有IPv6或公网地址的好友可以直接连接；与其他好友则需要一位群组成员在中间转发。';

  @override
  String get findingSummaryHardNat =>
      '总结：只有拥有公网地址的好友可以直接连接；与其他好友则需要一位群组成员在中间转发。';

  @override
  String get findingSummaryUnknown => '总结：信息不足，请过一分钟再检查。';

  @override
  String findingInternet(int n) {
    return '互联网：这台电脑可以连接公共网络（已知$n个节点）。';
  }

  @override
  String findingDns(String detail) {
    return 'DNS：无法完整读取系统设置（$detail）';
  }

  @override
  String findingLanDevices(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n台Fogata设备',
    );
    return '此局域网中有$_temp0：这里的好友无需经过互联网即可连接。';
  }

  @override
  String findingFirewall(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': '以管理员身份在PowerShell中',
      'macos': '在“终端”中',
      'other': '在终端中',
    });
    return '这台电脑的防火墙不允许任何连接进入Fogata，因此同一地点、位于另一台路由器之后的好友（例如家里有两台路由器）无法连接它。请$_temp0运行一次以下命令，让Fogata可以接收连接：';
  }

  @override
  String get settingsLanguage => '语言';

  @override
  String get settingsLanguageNote => 'Fogata自身界面的语言。消息内容保持原样。';

  @override
  String get settingsLanguageChoose => '选择语言';

  @override
  String get settingsTime => '时间';

  @override
  String get settingsTimeNote => '聊天、通话和列表中时间的显示方式。';

  @override
  String settingsTime24(String example) {
    return '24小时制 · $example';
  }

  @override
  String settingsTime12(String example) {
    return '12小时制 · $example';
  }

  @override
  String firewallNoticeText(String system, String command) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': '以管理员身份在PowerShell中',
      'macos': '在“终端”中',
      'other': '在终端中',
    });
    return '同一地点、位于另一台路由器之后的好友无法连接这台电脑。请$_temp0运行一次：$command';
  }

  @override
  String firewallCopied(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': '以管理员身份粘贴到PowerShell中',
      'macos': '粘贴到“终端”中',
      'other': '粘贴到终端中',
    });
    return '已复制。请$_temp0并运行。';
  }

  @override
  String get appRestarted => 'Fogata停止运行后已重新启动。';

  @override
  String appVoiceRoomInGroup(String room, String group) {
    return '$group的$room';
  }

  @override
  String appBothIn(String room) {
    return '你们都在$room里。';
  }

  @override
  String appHungUp(String name) {
    return '$name挂断了';
  }

  @override
  String get appYourFriend => '你的好友';

  @override
  String get appCallEndedAlone => '没有人回来，通话已结束';

  @override
  String get appAnsweredElsewhere => '已在另一台电脑上接听';

  @override
  String appVoiceRoomDeleted(String room) {
    return '“$room”已被删除。';
  }

  @override
  String get appSomeone => '有人';

  @override
  String get appAGroup => '某个群组';

  @override
  String appFriendRequest(String name) {
    return '$name想加你为好友。';
  }

  @override
  String appRequestDelivered(String name) {
    return '请求已送达$name，正在等待对方回复。';
  }

  @override
  String appFriendsNow(String name) {
    return '你和$name现在是好友了。';
  }

  @override
  String appJoinedGroup(String group) {
    return '你已加入“$group”。';
  }

  @override
  String appRemovedFromGroup(String group) {
    return '你已不在“$group”中。';
  }

  @override
  String appAddedToGroup(String name, String group) {
    return '$name把你添加到了“$group”。是否加入由你决定。';
  }

  @override
  String appAsksToJoin(String name, String group) {
    return '$name请求加入“$group”。';
  }

  @override
  String appJoinWaiting(String group) {
    return '正在等待“$group”中有人同意你加入。';
  }

  @override
  String appJoinDeclined(String group) {
    return '“$group”中有人拒绝了。这个邀请已失效。';
  }

  @override
  String appDownloadedTo(String path) {
    return '已下载到$path';
  }

  @override
  String appDownloadFailed(String why) {
    return '无法下载：$why';
  }

  @override
  String get appOtherSession =>
      '这个身份也在另一台电脑上打开了。请在那台电脑上关闭Fogata：一个身份同一时间只能在一台电脑上使用。';

  @override
  String get appFriendsRestored =>
      '你在这台电脑上的好友列表已损坏，已从上次更改前的备份中恢复。在那之前刚添加或移除的好友可能需要重新添加或移除。';

  @override
  String appInvitationRanOut(String group) {
    return '在群组中有人同意你加入之前，“$group”的邀请已过期。请向对方要一个新的。';
  }

  @override
  String appTakenOut(String who, String room) {
    return '$who把你移出了$room。';
  }

  @override
  String appRoomForSome(String room) {
    return '$room现在只对部分成员开放。';
  }

  @override
  String appTakenOutUntilLetBack(String who, String room) {
    return '$who把你移出了$room，需等排在你上面的人允许后才能回来。';
  }

  @override
  String appTakenOutFor(String who, String room, String span) {
    return '$who把你移出了$room，$span内不能回来。';
  }

  @override
  String appTakenOutWhile(String who, String room) {
    return '$who暂时把你移出了$room。';
  }

  @override
  String appComeBackIn(String room, String wait) {
    return '你可以在$wait后回到$room。';
  }

  @override
  String appRoomNotForYou(String room) {
    return '你现在不能进入$room。';
  }

  @override
  String appFoldersNotShared(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '无法共享文件夹，只能共享文件：已略过$n个文件夹。',
      one: '无法共享文件夹，只能共享文件：已略过该文件夹。',
    );
    return '$_temp0';
  }

  @override
  String appSharingOne(String file) {
    return '正在从这台电脑共享$file。你离开房间后，它将不再可供获取。';
  }

  @override
  String appSharingMany(int n) {
    return '正在从这台电脑共享$n个文件。你离开房间后，它们将不再可供获取。';
  }

  @override
  String appSavedTo(String path) {
    return '已保存到$path';
  }

  @override
  String appAskingToJoin(String group) {
    return '正在请求加入“$group”。群组中有人看到后会同意你加入。';
  }

  @override
  String get appLookingForDevice => '正在寻找对方设备以发送你的请求…';

  @override
  String callStateNoAnswer(String name) {
    return '$name没有接听';
  }

  @override
  String callStateDeclined(String name) {
    return '$name现在不方便通话';
  }

  @override
  String callStateBusy(String name) {
    return '$name正在另一个通话中';
  }

  @override
  String callStateOutdated(String name) {
    return '$name需要更新Fogata才能加入';
  }

  @override
  String callStateUnreached(String name) {
    return '无法联系到$name';
  }

  @override
  String get callStateReturn => '回到通话';

  @override
  String get callStateCallBack => '回拨';

  @override
  String get callStateCall => '呼叫';

  @override
  String noticesDeclined(String name, String group) {
    return '$name拒绝了加入“$group”的邀请';
  }

  @override
  String noticesDeclinedToast(String name, String group) {
    return '$name拒绝了加入“$group”的邀请。';
  }

  @override
  String noticesMissedInCall(String name) {
    return '$name在你通话时打来了电话';
  }

  @override
  String noticesMissedOffline(String name) {
    return '$name在你显示为离线时打来了电话';
  }

  @override
  String noticesMissedCall(String name) {
    return '$name的未接来电';
  }

  @override
  String get contactsGeneral => '常规';

  @override
  String get directStateTimerOff => '关闭';

  @override
  String get directStateTimer24h => '24小时';

  @override
  String directStateTimerDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n天');
    return '$_temp0';
  }

  @override
  String directStateDisappearsMinutes(int n) {
    return '$n分钟后消失';
  }

  @override
  String directStateDisappearsHours(int n) {
    return '$n小时后消失';
  }

  @override
  String directStateDisappearsDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n天后消失',
    );
    return '$_temp0';
  }

  @override
  String get directStateNoLimit => '无限制';

  @override
  String get directStateMessageGone => '这条消息已不在了。';

  @override
  String directStateFoldersNotSent(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '无法发送文件夹，只能发送文件：已略过$n个文件夹。',
      one: '无法发送文件夹，只能发送文件：已略过该文件夹。',
    );
    return '$_temp0';
  }

  @override
  String directStateFileFailed(String why) {
    return '无法获取文件：$why';
  }

  @override
  String movingTooLarge(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners': '动态横幅最大$most像素，这个是$size。',
      'other': '动态图片最大$most像素，这个是$size。',
    });
    return '$_temp0';
  }

  @override
  String movingTooLong(String what, String most, String count) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners': '动态横幅最多$most帧，这个有$count帧。',
      'other': '动态图片最多$most帧，这个有$count帧。',
    });
    return '$_temp0';
  }

  @override
  String movingTooHeavy(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners': '动态横幅最大$most，这个是$size。',
      'other': '动态图片最大$most，这个是$size。',
    });
    return '$_temp0';
  }

  @override
  String get namesHidden => '这个名字含有隐藏字符。请不带这些字符重新输入。';

  @override
  String get namesBlank => '这个名字里没有可见的内容。请使用文字、数字或表情。';

  @override
  String get namesDefaultBio => '嗨！我在用Fogata。';

  @override
  String get namesBioHidden => '这段简介含有隐藏字符。请不带这些字符重新输入。';

  @override
  String namesBioLines(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '个人简介最多$n行。',
    );
    return '$_temp0';
  }

  @override
  String namesBioChars(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '个人简介最多$n个字符。',
    );
    return '$_temp0';
  }

  @override
  String get restartFailed => 'Fogata无法重新启动。请退出后手动打开。';

  @override
  String typingOne(String name) {
    return '$name正在输入…';
  }

  @override
  String typingTwo(String first, String second) {
    return '$first和$second正在输入…';
  }

  @override
  String get typingSeveral => '多人正在输入…';

  @override
  String get outgoingTheRoom => '房间';

  @override
  String outgoingDropTitle(String group) {
    return '拖放到此处，在$group中共享';
  }

  @override
  String get outgoingDropNote => '发送前你可以添加消息并检查文件。你在房间里时，其他人会从这台电脑下载它们。';

  @override
  String outgoingPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n个隐藏或私密的文件未被添加：网页也能发起拖动，所以这类文件不能通过拖动添加。要共享它们，请用回形针按钮选择。',
      one: '一个隐藏或私密的文件未被添加：网页也能发起拖动，所以这类文件不能通过拖动添加。要共享它，请用回形针按钮选择。',
    );
    return '$_temp0';
  }

  @override
  String outgoingToShare(int n, String count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$count个文件待共享 · $size',
    );
    return '$_temp0';
  }

  @override
  String get outgoingRemoveAll => '全部移除';

  @override
  String outgoingRemoveFile(String file) {
    return '移除$file';
  }

  @override
  String get windowTrayOpen => '打开Fogata';

  @override
  String get windowTrayQuit => '退出Fogata';

  @override
  String get windowMinimize => '最小化';

  @override
  String get windowRestore => '还原';

  @override
  String get windowMaximize => '最大化';

  @override
  String get autostartReason => '登录时打开Fogata。';

  @override
  String get devicesMicrophone => '麦克风';

  @override
  String get devicesSpeaker => '扬声器';

  @override
  String get devicesCamera => '摄像头';

  @override
  String get devicesChooseMicrophone => '选择麦克风';

  @override
  String get devicesChooseSpeaker => '选择扬声器';

  @override
  String get devicesChooseCamera => '选择摄像头';

  @override
  String get devicesNoCamera => '未连接摄像头。';

  @override
  String get devicesCameraInCall => '摄像头正在你的通话中使用。';

  @override
  String devicesCameraFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': '无法使用摄像头。请在“系统设置 › 隐私与安全性”中允许。',
      'other': '无法使用摄像头。',
    });
    return '$_temp0';
  }

  @override
  String get devicesCameraDenied => '系统不允许 Fogata 使用摄像头。请在系统设置中允许后重试。';

  @override
  String get devicesCameraUnavailable => '无法访问摄像头：此系统没有摄像头门户。';

  @override
  String get devicesPreviewOnlyYou => '只有你能看到预览。';

  @override
  String get devicesStopVideo => '停止视频';

  @override
  String get devicesTestVideo => '测试视频';

  @override
  String get devicesNoiseOn => '噪音抑制已开启';

  @override
  String get devicesNoiseOff => '噪音抑制已关闭';

  @override
  String get devicesNoise => '噪音抑制';

  @override
  String get devicesNoiseNote => '减弱其他人听到的持续背景噪音，比如风扇声或嗡嗡声。';

  @override
  String get devicesNoiseShort => '减弱持续的风扇声或嗡嗡声。';

  @override
  String get devicesMirror => '镜像显示我的摄像头';

  @override
  String get devicesMirrorNote => '只改变你在这里看到的自己。其他人看到的始终是摄像头的原样画面。';

  @override
  String get devicesMirrorOn => '镜像显示摄像头已开启';

  @override
  String get devicesMirrorOff => '镜像显示摄像头已关闭';

  @override
  String get devicesTestMic => '测试麦克风';

  @override
  String get devicesStopMicTest => '停止测试';

  @override
  String get devicesMicLevel => '麦克风音量';

  @override
  String get devicesHearMyself => '听到自己的声音';

  @override
  String get devicesHearMyselfNote => '通过扬声器播放你的麦克风。请使用耳机，否则扬声器的声音可能再次进入麦克风。';

  @override
  String get devicesHearMyselfOn => '听到自己的声音已开启';

  @override
  String get devicesHearMyselfOff => '听到自己的声音已关闭';

  @override
  String get devicesMicTestOnlyYou => '测试的内容不会发送给任何人。';

  @override
  String get devicesMicInCall => '麦克风正在你的通话中使用。';

  @override
  String get devicesNoMic => '未连接麦克风。';

  @override
  String get devicesSystemDefault => '系统默认';

  @override
  String get devicesNotConnected => '未连接：使用默认设备';

  @override
  String get devicesNoCameraShort => '未连接摄像头';

  @override
  String get devicesMicOptions => '麦克风选项';

  @override
  String get devicesSpeakerOptions => '扬声器选项';

  @override
  String get devicesCameraOptions => '摄像头选项';

  @override
  String get devicesInputDevice => '输入设备';

  @override
  String get devicesOutputDevice => '输出设备';

  @override
  String get devicesChosenMissing => '所选设备未连接：使用默认设备。';

  @override
  String get devicesVoiceSettings => '语音和视频设置';

  @override
  String get photoImages => '图片';

  @override
  String photoTooLarge(int n) {
    return '这张图片太大：最多$n百万像素。';
  }

  @override
  String get photoUnreadable => '无法读取这张图片。';

  @override
  String get callSource => '原画';

  @override
  String callQuality(String resolution, int fps) {
    return '$resolution · $fps fps';
  }

  @override
  String get callSetupFailed => '无法建立通话。';

  @override
  String get callNoSoundSystem => '无法连接到这台电脑的声音系统，因此无法开始通话。';

  @override
  String callMicGone(String name) {
    return '你的麦克风已断开：现在使用$name。';
  }

  @override
  String get callMicBack => '你的麦克风已重新连接。';

  @override
  String callMicSilent(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': '无法使用麦克风。请检查它是否已连接，并在“系统设置 › 隐私与安全性”中允许。',
      'windows': '无法使用麦克风。请检查它是否已连接，并在“设置 › 隐私和安全性 › 麦克风”中允许。',
      'other': '无法使用麦克风。请检查它是否已连接。',
    });
    return '$_temp0';
  }

  @override
  String get callNoMicrophone => '未连接麦克风。';

  @override
  String callMicFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': '无法使用麦克风。请在“系统设置 › 隐私与安全性”中允许。',
      'windows': '无法使用麦克风。请在“设置 › 隐私和安全性 › 麦克风”中允许。',
      'other': '无法使用麦克风。',
    });
    return '$_temp0';
  }

  @override
  String callScreenFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': '无法共享屏幕。请在“系统设置 › 隐私与安全性”中允许。',
      'other': '无法共享屏幕。',
    });
    return '$_temp0';
  }

  @override
  String get callShareLost => '已停止共享：无法再捕获你正在共享的内容。';

  @override
  String callKeptOut(String name) {
    return '与$name的连接已断开。';
  }

  @override
  String get qualityGood => '良好';

  @override
  String get qualityOkay => '一般';

  @override
  String get qualityPoor => '较差';

  @override
  String get qualityLost => '中断';

  @override
  String qualityDelay(int ms) {
    return '$ms ms';
  }

  @override
  String qualityPercent(String value) {
    return '$value%';
  }

  @override
  String get qualityAnotherMember => '另一位成员';

  @override
  String qualityLostHint(String name, int seconds) {
    return '$seconds秒内没有收到$name的任何数据。Fogata会继续寻找对方；对方网络恢复后会自动回来。';
  }

  @override
  String qualityBothPoor(String name) {
    return '你们双方的网络现在都较差：你和$name可能会听到对方声音断断续续。';
  }

  @override
  String qualityTheirSide(String name) {
    return '问题出在$name那边：大家听到对方的声音都断断续续。对方的Wi-Fi或移动数据现在较差。';
  }

  @override
  String qualityYourSideWith(String name) {
    return '问题出在你这边：$name和其他人听到你的声音可能断断续续。你的Wi-Fi或移动数据现在较差。';
  }

  @override
  String qualityTheirsSlow(String name) {
    return '$name的网络现在有点慢：大家听到对方的声音可能偶尔断断续续。';
  }

  @override
  String qualityAllGoodWith(String name) {
    return '你和$name都能清楚地听到对方。';
  }

  @override
  String qualityMineSlowWith(String name) {
    return '你的网络现在有点慢：$name听到你的声音可能偶尔断断续续。';
  }

  @override
  String qualityWayThrough(String name, String carrier) {
    return '$name自己的网络正常；你们之间的线路较慢，因为它经过$carrier。你听到对方的声音可能偶尔断断续续，其他人听得很清楚。';
  }

  @override
  String qualityWaySlowFine(String name) {
    return '$name自己的网络正常；你们之间的线路现在较慢。你听到对方的声音可能偶尔断断续续，其他人听得很清楚。';
  }

  @override
  String qualityWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': '你和$name之间的线路现在有点慢，可能是任意一方的网络问题。你们可能偶尔听到对方声音断断续续。',
      'other': '你和$name之间的线路现在较差，可能是任意一方的网络问题。你们可能听到对方声音断断续续。',
    });
    return '$_temp0';
  }

  @override
  String qualityWayVia(String level, String name, String carrier) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': '你和$name之间的线路现在有点慢，可能是任意一方的网络问题。你们可能偶尔听到对方声音断断续续。线路经过$carrier。',
      'other': '你和$name之间的线路现在较差，可能是任意一方的网络问题。你们可能听到对方声音断断续续。线路经过$carrier。',
    });
    return '$_temp0';
  }

  @override
  String get qualityYourSide => '问题出在你这边：其他人听到你的声音可能断断续续。你的Wi-Fi或移动数据现在较差。';

  @override
  String get qualityMineSlow => '你的网络现在有点慢：其他人听到你的声音可能偶尔断断续续。';

  @override
  String qualityLostShort(String name, int seconds) {
    return '$seconds秒内没有收到$name的任何数据。Fogata会继续寻找对方。';
  }

  @override
  String qualityAllWellWith(String name) {
    return '一切正常：你和$name都能清楚地听到对方。';
  }

  @override
  String qualityNoteDropped(String name) {
    return '$name掉线了，Fogata正在重新寻找对方';
  }

  @override
  String qualityNoteTheirsPoor(String name) {
    return '$name的网络较差，大家听到对方的声音都断断续续';
  }

  @override
  String qualityNoteTheirsSlow(String name) {
    return '$name的网络有点慢';
  }

  @override
  String qualityNoteThrough(String name, String carrier) {
    return '$name经由$carrier连接到你';
  }

  @override
  String qualityNoteWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': '到$name的线路现在有点慢',
      'other': '到$name的线路现在较差',
    });
    return '$_temp0';
  }

  @override
  String get qualityNoteSeparator => '；';

  @override
  String get qualityAllWell => '一切正常：你能听到所有人，所有人也能听到你。';

  @override
  String qualityAllWellMine(String notes) {
    return '你这边一切正常。$notes。';
  }

  @override
  String qualityYourDetails(String level) {
    return '你的连接：$level。显示详情';
  }

  @override
  String qualityTheirDetails(String name, String level) {
    return '$name的连接：$level。显示详情';
  }

  @override
  String get qualityReconnecting => '正在重新连接…';

  @override
  String get qualityReconnectingLabel => '正在重新连接';

  @override
  String qualityLevelConnection(String level) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'good': '连接良好',
      'okay': '连接一般',
      'weak': '连接较差',
      'other': '连接中断',
    });
    return '$_temp0';
  }

  @override
  String get qualityPoorConnection => '连接较差';

  @override
  String get qualityMinePoorBanner => '你的网络较差：其他人听到你的声音可能断断续续';

  @override
  String get qualityYourNetwork => '你的网络';

  @override
  String qualityTo(String name) {
    return '到$name';
  }

  @override
  String qualityBetween(String name) {
    return '你和$name之间';
  }

  @override
  String qualityTheirNetwork(String name) {
    return '$name自己的网络';
  }

  @override
  String get qualityLastHeard => '最后收到';

  @override
  String qualitySecondsAgo(int n) {
    return '$n秒前';
  }

  @override
  String get qualityPath => '路径';

  @override
  String get qualityLookingAgain => '正在重新寻找对方';

  @override
  String get qualityDelayLabel => '延迟';

  @override
  String get qualityLostPackets => '丢包';

  @override
  String get qualityYourConnection => '你的连接';

  @override
  String qualityTheirConnection(String name) {
    return '$name的连接';
  }

  @override
  String roomCountFullLabel(int count, int max) {
    return '已满，$count人，上限$max人';
  }

  @override
  String roomCountLabel(int count, int max) {
    return '$count人，上限$max人';
  }

  @override
  String get roomCountFullPrefix => '已满 · ';

  @override
  String get roomCountFullWord => '已满 ';

  @override
  String get volumeVoice => '语音音量';

  @override
  String get volumeOnlyYouHear => '这些更改只有你能听到';

  @override
  String get volumeScreenSound => '屏幕声音';

  @override
  String volumePercent(int n) {
    return '$n%';
  }

  @override
  String get volumeMuteVoiceForMe => '仅对我静音语音';

  @override
  String get volumeMuteForMe => '仅对我静音';

  @override
  String get volumeMuteScreenForMe => '仅对我静音屏幕声音';

  @override
  String get volumeBackTo100 => '恢复到100%';

  @override
  String get volumeBelowYou => '排在你下面';

  @override
  String volumeRemoveFrom(String room) {
    return '移出$room';
  }

  @override
  String volumeRemoveTitle(String name, String room) {
    return '将$name移出$room？';
  }

  @override
  String volumeRemoveText(String name, String room) {
    return '$name会立即离开$room，但可以马上回来。暂时移出可以让对方一段时间内无法进入。';
  }

  @override
  String volumeTimeOutOf(String room) {
    return '暂时移出$room';
  }

  @override
  String volumeTimeOutText(String name, String room) {
    return '$name将离开$room，在以下时间内无法回来：';
  }

  @override
  String get volumeUntilLetBack => '直到排在其上面的人让其回来';

  @override
  String get pipYourScreen => '你的屏幕';

  @override
  String pipScreenOf(String name) {
    return '$name的屏幕';
  }

  @override
  String get pipYourCamera => '你的摄像头';

  @override
  String get pipBackToCall => '回到通话';

  @override
  String get pipBackToRoom => '回到房间';

  @override
  String get pipStopWatching => '停止观看';

  @override
  String get addToCallTitle => '添加到通话';

  @override
  String addToCallNote(int n) {
    return '在线的好友。一个通话最多$n人。';
  }

  @override
  String get addToCallSearch => '搜索好友';

  @override
  String get addToCallNoFriends => '还没有好友。';

  @override
  String get addToCallNoMatch => '没有叫这个名字的好友。';

  @override
  String get addToCallInCall => '通话中';

  @override
  String get addToCallCalling => '正在呼叫…';

  @override
  String get addToCallOffline => '离线';

  @override
  String get addToCallCall => '呼叫';

  @override
  String get streamQualityTitle => '共享画质';

  @override
  String get streamQualityNoteSharing => '你的屏幕传给其他人时的画质。更改立即生效。';

  @override
  String get streamQualityNote => '你共享屏幕时传给其他人的画质。';

  @override
  String get streamQualityResolution => '分辨率';

  @override
  String streamQualitySourceLines(int lines) {
    return '原画（${lines}p）';
  }

  @override
  String get streamQualitySource => '原画';

  @override
  String get streamQualityFrameRate => '帧率';

  @override
  String streamQualityFps(int n) {
    return '$n fps';
  }

  @override
  String get streamQualityHigherNote => '越高越占用你的上传带宽和电脑性能。';

  @override
  String get streamQualitySomeone => '有人';

  @override
  String streamQualityLoweredOne(String name, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': '其他人收到的是你选择的画质。',
      'other': '',
    });
    return '$name的连接目前承受不了更高画质，所以对方收到的画面较小。$_temp0';
  }

  @override
  String streamQualityLoweredMany(String names, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': '其他人收到的是你选择的画质。',
      'other': '',
    });
    return '部分连接目前承受不了更高画质：$names收到的画面较小。$_temp0';
  }

  @override
  String get streamQualityGoingOut => '当前发送';

  @override
  String get streamQualityNobody => '还没有人在观看。';

  @override
  String streamQualityNow(int height, int fps) {
    return '${height}p · $fps fps';
  }

  @override
  String callLayersIsCalling(String name) {
    return '$name来电';
  }

  @override
  String get callLayersGoToCall => '按F6转到通话';

  @override
  String callLayersWith(String names) {
    return '还有$names';
  }

  @override
  String get callLayersDirectCall => '私人通话 · ';

  @override
  String get callLayersAnswerMicOff => '接听时关闭麦克风';

  @override
  String callLayersAnsweringLeaves(String room) {
    return '接听将离开$room';
  }

  @override
  String get callLayersLeaveAndAnswer => '离开并接听';

  @override
  String callLayersBothIn(String room) {
    return '你们都在$room里。';
  }

  @override
  String get callLayersDismiss => '忽略';

  @override
  String callLayersCallWith(String names) {
    return '与$names通话';
  }

  @override
  String get callLayersRoom => '房间';

  @override
  String callLayersBackTo(String title) {
    return '回到$title';
  }

  @override
  String callLayersRinging(String time) {
    return '呼叫中 · $time';
  }

  @override
  String callLayersInCall(String time) {
    return '通话中 · $time';
  }

  @override
  String callLayersOnlyYou(String time) {
    return '只有你 · $time';
  }

  @override
  String callLayersInRoom(int n, String time) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '房间里$n人',
    );
    return '$_temp0 · $time';
  }

  @override
  String get callLayersMute => '静音';

  @override
  String get callLayersUnmute => '取消静音';

  @override
  String get callLayersHangUp => '挂断';

  @override
  String get callLayersLeaveRoom => '离开房间';

  @override
  String roomDeclineCallTitle(String name) {
    return '拒绝$name的来电？';
  }

  @override
  String roomDeclineCallText(String caller, String callee) {
    return '$caller正在呼叫你。你同一时间只能进行一个通话：呼叫$callee会拒绝这个来电。';
  }

  @override
  String roomDeclineCallTextSomeone(String caller) {
    return '$caller正在呼叫你。你同一时间只能进行一个通话：呼叫其他人会拒绝这个来电。';
  }

  @override
  String get roomDeclineAndCall => '拒绝并呼叫';

  @override
  String roomHangUpTitle(String name) {
    return '挂断与$name的通话？';
  }

  @override
  String get roomHangUpText => '你同一时间只能在一个通话或房间中：继续将结束这个通话。';

  @override
  String get roomHangUp => '挂断';

  @override
  String get roomCloseTitle => '离开并关闭房间？';

  @override
  String get roomCloseText =>
      '你是这里的最后一个人。你离开后房间会关闭，聊天内容会从这台电脑上消失；其他人已下载的内容会保留在他们的电脑上。';

  @override
  String get roomCloseChatMessages => '聊天消息';

  @override
  String get roomCloseFilesShared => '你共享的文件';

  @override
  String get roomStay => '留下';

  @override
  String get roomLeaveAndClose => '离开并关闭';

  @override
  String get roomLivePrefix => '进行中 · ';

  @override
  String get roomPillConnecting => '连接中';

  @override
  String get roomPillOnlyYou => '只有你';

  @override
  String get roomPillError => '出错';

  @override
  String get roomLastOne => '你是房间里的最后一个人。';

  @override
  String get roomOnlyYouIn => '房间里只有你。';

  @override
  String get roomAloneWarning => '你离开后，房间会关闭，里面的一切都会消失。';

  @override
  String get roomEarlierPeople => '前面的人';

  @override
  String get roomMorePeople => '更多人';

  @override
  String roomTileYou(String name) {
    return '$name（你）';
  }

  @override
  String roomTileNoSound(String name) {
    return '$name · 无声音';
  }

  @override
  String roomTileConnecting(String name) {
    return '$name · 连接中';
  }

  @override
  String roomYourScreenWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '你的屏幕 · $n人观看',
    );
    return '$_temp0';
  }

  @override
  String roomTileAway(String name) {
    return '$name · 连接中断';
  }

  @override
  String roomTileMicOff(String name) {
    return '$name，麦克风已关闭';
  }

  @override
  String roomTileMutedByYou(String said) {
    return '$said，已在你这边静音';
  }

  @override
  String roomTileVolume(String said, String percent) {
    return '$said，音量$percent';
  }

  @override
  String roomTileLeft(String name) {
    return '$name已离开 · 可能回来';
  }

  @override
  String roomIsSharing(String name) {
    return '$name正在共享屏幕';
  }

  @override
  String get roomNothingSentUntilWatch => '在你观看之前不会向你发送任何内容';

  @override
  String get roomWatch => '观看';

  @override
  String get roomLive => '直播';

  @override
  String get roomNobodyWatching => '无人观看';

  @override
  String roomPeopleWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n人观看',
    );
    return '$_temp0';
  }

  @override
  String get roomExitFullScreen => '退出全屏';

  @override
  String get roomFullScreen => '全屏';

  @override
  String get roomPlayScreenSound => '播放屏幕声音';

  @override
  String get roomMuteScreenSound => '静音屏幕声音';

  @override
  String get roomQualityPoor => '较差';

  @override
  String get roomQualityOkay => '一般';

  @override
  String get roomQualityGood => '良好';

  @override
  String get roomQualityPoorTitle => '连接较差';

  @override
  String get roomQualityOkayTitle => '连接一般';

  @override
  String get roomQualityGoodTitle => '连接良好';

  @override
  String get roomQualityMeasuring => '测量中';

  @override
  String get roomQualityDelay => '延迟';

  @override
  String roomQualityMs(String n) {
    return '$n ms';
  }

  @override
  String get roomQualityLost => '丢包';

  @override
  String roomQualityPercent(String n) {
    return '$n%';
  }

  @override
  String get roomQualityPicture => '画面';

  @override
  String get roomQualityPath => '路径';

  @override
  String get roomQualityMayFreeze => '画面可能会卡住。减少同时共享的人数会有帮助。';

  @override
  String get roomQualityMayShrink => '画面可能会变小以保持流畅。';

  @override
  String roomCallingName(String name) {
    return '正在呼叫$name…';
  }

  @override
  String get roomRingsFor30 => '对方电脑最多响铃30秒。';

  @override
  String get roomReturnToCall => '回到通话';

  @override
  String get roomCallAgain => '再次呼叫';

  @override
  String get roomAloneHint => '群组中有人进来时，会显示在这里。';

  @override
  String roomAlreadyLeft(String names) {
    return '$names已经离开了。';
  }

  @override
  String get roomGroupFound => '已在网络上找到群组';

  @override
  String get roomLookingForRoute => '正在寻找最佳路径…';

  @override
  String get roomConnectingTitle => '正在连接房间';

  @override
  String roomSeconds(String n) {
    return '$n秒';
  }

  @override
  String roomReachedOf(int reached, int total, String time) {
    return '$reached/$total · $time';
  }

  @override
  String get roomPathLan => '局域网，已加密';

  @override
  String get roomPathDirect => '直连，已加密';

  @override
  String get roomPathThroughMember => '经由成员中转，已加密';

  @override
  String roomPathThrough(String name) {
    return '经由$name中转，已加密';
  }

  @override
  String get roomCouldNotConnect => '无法连接房间';

  @override
  String get roomCouldNotConnectText => '没有人及时回应。你的网络可能阻止了设备之间的直接连接。';

  @override
  String get roomNoAnswer => '无回应';

  @override
  String get roomSeeNetworkCheck => '查看网络检测 →';

  @override
  String get roomGoesNothing => '房间将对所有人关闭';

  @override
  String roomGoesMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n条消息将消失',
    );
    return '$_temp0';
  }

  @override
  String roomGoesFiles(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '你共享的$n个文件将消失',
    );
    return '$_temp0';
  }

  @override
  String roomGoesBoth(int messages, int files) {
    String _temp0 = intl.Intl.pluralLogic(
      messages,
      locale: localeName,
      other: '$messages条消息',
    );
    String _temp1 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files个文件',
    );
    return '$_temp0和$_temp1将消失';
  }

  @override
  String get roomMuteMicrophone => '麦克风静音';

  @override
  String get roomUnmuteMicrophone => '取消麦克风静音';

  @override
  String get roomStopSharingScreen => '停止共享屏幕';

  @override
  String get roomShareScreen => '共享屏幕';

  @override
  String get roomUndeafen => '重新听到其他人';

  @override
  String get roomDeafen => '不再听到其他人';

  @override
  String get roomCameraOff => '关闭摄像头';

  @override
  String get roomCameraOn => '打开摄像头';

  @override
  String get roomLeave => '离开';

  @override
  String get roomSoundWaitsForMic => '打开麦克风后才会发送声音';

  @override
  String get roomStopSharingSound => '停止共享声音';

  @override
  String get roomShareSoundToo => '同时共享声音';

  @override
  String get roomAddFriendToCall => '添加好友到通话';

  @override
  String get roomHideChat => '隐藏聊天';

  @override
  String get roomShowChat => '显示聊天';

  @override
  String get roomTabChat => '聊天';

  @override
  String get roomTabFiles => '文件';

  @override
  String roomNoteLeftCall(String name) {
    return '$name离开了通话 · ';
  }

  @override
  String roomNoteJoinedCall(String name) {
    return '$name加入了通话 · ';
  }

  @override
  String roomNoteLeftRoom(String name) {
    return '$name离开了房间 · ';
  }

  @override
  String roomNoteEnteredRoom(String name) {
    return '$name进入了房间 · ';
  }

  @override
  String get roomSearchMessages => '搜索消息';

  @override
  String get roomClearSearch => '清除搜索';

  @override
  String get roomChatGoneCall => '通话结束后，这些聊天内容将消失';

  @override
  String get roomChatGoneRoom => '房间关闭后，这些聊天内容将消失';

  @override
  String roomNoMatches(String text) {
    return '没有包含“$text”的消息';
  }

  @override
  String roomMatches(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n个结果',
    );
    return '$_temp0';
  }

  @override
  String get roomReceivingMessages => '正在接收房间消息…';

  @override
  String roomReceivingMessagesFrom(String name) {
    return '正在从$name的电脑接收房间消息…';
  }

  @override
  String get roomSomeone => '有人';

  @override
  String get roomShareFiles => '共享文件';

  @override
  String get roomAddMoreFiles => '添加更多文件';

  @override
  String roomMessageTo(String name) {
    return '发消息给$name';
  }

  @override
  String get roomMessageTheRoom => '发消息到房间';

  @override
  String get roomMessageTheCall => '发消息到通话';

  @override
  String get roomAddMessageOptional => '添加消息（可选）';

  @override
  String get roomMessageLabel => '消息';

  @override
  String roomEnterToSend(String key) {
    return '按$key发送';
  }

  @override
  String roomEscToRemove(String key) {
    return '按$key移除';
  }

  @override
  String roomOrTwo(String first, String second) {
    return '$first或$second';
  }

  @override
  String roomOrMore(String rest, String last) {
    return '$rest或$last';
  }

  @override
  String roomFileNotAvailable(String name) {
    return '无法获取 · $name已离开';
  }

  @override
  String get roomSharingFromHere => '从这台电脑共享';

  @override
  String roomAvailableWhile(String names) {
    return '$names留在房间时可获取';
  }

  @override
  String roomSizeDownloads(String size, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n次下载',
    );
    return '$size · $_temp0';
  }

  @override
  String roomDownloadFile(String file) {
    return '下载$file';
  }

  @override
  String roomShowInFolder(String file) {
    return '在文件夹中显示$file';
  }

  @override
  String roomGettingFile(String file) {
    return '正在获取$file…';
  }

  @override
  String get roomSaveToDownloads => '保存到“下载”';

  @override
  String roomPictureKept(String name) {
    return '$name已离开 · 图片会保留到你离开为止';
  }

  @override
  String get roomSaveFile => '保存';

  @override
  String get roomClosePicture => '关闭图片';

  @override
  String get roomFilesNote => '文件不会上传到任何地方，而是直接从共享者的电脑传来，仅在对方留在房间时可用。';

  @override
  String roomFilesAvailable(int n) {
    return '可获取 · $n';
  }

  @override
  String roomFilesGone(int n) {
    return '已无法获取 · $n';
  }

  @override
  String roomPersonLeft(String name) {
    return '$name已离开';
  }

  @override
  String get roomPickOrDrop => '在这里选择，或拖放到房间中';

  @override
  String get roomFileYours => '你的';

  @override
  String roomFileFrom(String name) {
    return '来自$name';
  }

  @override
  String roomDownloads(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n次下载',
    );
    return '$_temp0';
  }

  @override
  String get roomStopSharingFile => '停止';

  @override
  String get roomWhatToShare => '你想共享什么？';

  @override
  String roomScreenSource(String name) {
    return '屏幕：$name';
  }

  @override
  String roomPlayVideo(String file) {
    return '播放$file';
  }

  @override
  String get roomCloseVideo => '关闭视频';

  @override
  String get roomVideoGone => '这个视频已不在房间里。';

  @override
  String get roomVideoNotFetched => '无法获取这个视频。';

  @override
  String get roomVideoCannotPlay => '这台电脑无法播放这个视频。';

  @override
  String roomGettingVideoPercent(int n) {
    return '正在获取视频 · $n%';
  }

  @override
  String get roomGettingVideo => '正在获取视频…';

  @override
  String get roomPause => '暂停';

  @override
  String get roomPlay => '播放';

  @override
  String get roomSoundOn => '打开声音';

  @override
  String get roomMuteVideo => '静音';

  @override
  String get roomLeaveFullScreen => '退出全屏';

  @override
  String roomCallWaits(String time) {
    return '只剩一人时，通话会等待$time让其他人回来';
  }

  @override
  String roomCallWaitsAlone(String time) {
    return '只剩你一个人了。通话会等待$time让其他人回来';
  }

  @override
  String roomOpenFile(String file) {
    return '打开$file';
  }

  @override
  String activityCreated(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0创建了群组“$name”';
  }

  @override
  String activityRenamed(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0将群组重命名为“$name”';
  }

  @override
  String activityIcon(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0更改了群组图标';
  }

  @override
  String activityPictureRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0移除了群组图片';
  }

  @override
  String activityPicture(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0更改了群组图片';
  }

  @override
  String activityBannerRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0移除了群组横幅';
  }

  @override
  String activityBanner(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0更改了群组横幅';
  }

  @override
  String activityAdded(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '你',
      'other': '$member',
    });
    return '$_temp0添加了$_temp1';
  }

  @override
  String activityJoined(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '你',
      'other': '$member',
    });
    return '$_temp0通过$_temp1的邀请加入';
  }

  @override
  String activityLetIn(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '你',
      'other': '$member',
    });
    return '$_temp0同意$_temp1加入';
  }

  @override
  String activityRemoved(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '你',
      'other': '$member',
    });
    return '$_temp0移除了$_temp1';
  }

  @override
  String activityLeft(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0退出了群组';
  }

  @override
  String activityMovedUp(
    String by,
    String byMe,
    String member,
    String memberMe,
    String place,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '你',
      'other': '$member',
    });
    return '$_temp0将$_temp1上移到$place';
  }

  @override
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '你',
      'other': '$member',
    });
    return '$_temp0将$_temp1移到末尾';
  }

  @override
  String activityManagersAll(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0让所有人都能管理群组';
  }

  @override
  String activityManagersFirst(String by, String byMe, int n) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '前$n人',
      one: '排第一的人',
    );
    return '$_temp0将群组管理交给$_temp1';
  }

  @override
  String activityLimits(String by, String byMe, int people, int perRoom) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0将群组设为最多$people人、每个语音房间最多$perRoom人';
  }

  @override
  String activityRoomOpened(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0开设了语音房间“$room”';
  }

  @override
  String activityRoomRenamed(String by, String byMe, String from, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0将语音房间“$from”重命名为“$room”';
  }

  @override
  String activityRoomClosed(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0删除了语音房间“$room”';
  }

  @override
  String activityInvited(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0创建了邀请链接';
  }

  @override
  String activityRevoked(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0作废了邀请链接';
  }

  @override
  String activityRoomOpenToAll(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0将“$room”向所有人开放';
  }

  @override
  String activityRoomForAbove(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0将“$room”仅留给排在上面的人';
  }

  @override
  String activityRoomOnly(String by, String byMe, String people, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    return '$_temp0只允许$people进入“$room”';
  }

  @override
  String activityKeptOut(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '你',
      'other': '$member',
    });
    return '$_temp0禁止$_temp1进入“$room”，直到被放行';
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
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '你',
      'other': '$member',
    });
    String _temp2 = intl.Intl.selectLogic(untilKind, {
      'today': '直到$until',
      'todayAtOne': '直到$until',
      'yesterday': '直到$until',
      'other': '直到$until',
    });
    return '$_temp0将$_temp1暂时移出“$room”，$_temp2';
  }

  @override
  String activityLetBack(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': '你', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': '你',
      'other': '$member',
    });
    return '$_temp0让$_temp1回到“$room”';
  }

  @override
  String get activityDefaultRoom => '常规';

  @override
  String get activityYouInList => '你';

  @override
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': '你',
      'other': '$author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': '被你',
      'other': '被$by',
      'none': '被',
    });
    return '已撤销：$_temp0在同一时间$_temp1移出了群组';
  }

  @override
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': '你',
      'other': '$author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': '被你',
      'other': '被$by',
      'none': '被',
    });
    return '已撤销：$_temp0在同一时间$_temp1移到了末尾';
  }

  @override
  String activityUndoneLeft(String author, String authorMe) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': '你',
      'other': '$author',
    });
    return '已撤销：$_temp0在同一时间退出了群组';
  }

  @override
  String get activityAboutList =>
      '群组里每个人看到的都是同一份列表，来自群组自身的记录。图片和横幅的更改单独保存：这些是这台设备看到的。';

  @override
  String get activityAboutOrder => '按群组自身的顺序排列；时间以各操作者的时钟为准。';

  @override
  String get activityNothingYet => '暂无记录。';

  @override
  String get activityLoading => '正在加载…';

  @override
  String get activityShowOlder => '显示更早的记录';

  @override
  String activitySeenHere(String when) {
    return '$when · 在这台设备上看到';
  }

  @override
  String get bellNotifications => '通知';

  @override
  String bellNewNotifications(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n条新通知',
    );
    return '$_temp0';
  }

  @override
  String get bellClearAll => '全部清除';

  @override
  String get bellNothingNew => '没有新通知';

  @override
  String get bellFooter => '最近三条会保留在这里。未接来电和请求也会显示在“好友”中。';

  @override
  String bellBothInRoom(String room) {
    return '你们都在$room里。';
  }

  @override
  String get bellDismiss => '忽略';

  @override
  String bellFriendRequest(String name) {
    return '$name想加你为好友';
  }

  @override
  String bellCheckFingerprint(String fingerprint) {
    return '请核对指纹是否一致：$fingerprint';
  }

  @override
  String bellJoinAsk(String name, String group) {
    return '$name请求加入“$group”';
  }

  @override
  String bellJoinAskNote(String fingerprint) {
    return '使用了你群组的一个邀请链接。指纹：$fingerprint';
  }

  @override
  String get bellLetIn => '同意';

  @override
  String get bellSayNo => '拒绝';

  @override
  String bellGroupInvite(String name, String group) {
    return '$name把你添加到了“$group”';
  }

  @override
  String bellMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n位成员',
    );
    return '$_temp0';
  }

  @override
  String get bellJoin => '加入';

  @override
  String bellNewMessages(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$name发来$n条新消息',
      one: '$name发来一条新消息',
    );
    return '$_temp0';
  }

  @override
  String get bellOpen => '打开';

  @override
  String get bellMarkRead => '标为已读';

  @override
  String get bellFirewallTitle => '你的防火墙挡住了一些好友';

  @override
  String get bellCopyCommand => '复制命令';

  @override
  String get bellOtherSessionTitle => '也在另一台电脑上打开了';

  @override
  String get bellSettingsResetTitle => '部分设置已重置';

  @override
  String get bellSettingsResetText =>
      '部分设置无法读取，已恢复为默认值：请在设置中检查。旧文件已保存在新文件旁边，名为app.json.bad。';

  @override
  String get friendCardOnline => '在线';

  @override
  String get friendCardAway => '暂离';

  @override
  String get friendCardOffline => '离线';

  @override
  String get friendCardCallsOnlyOnline => '只有好友在线时才能呼叫。';

  @override
  String friendCardHangUpFirst(String name) {
    return '请先挂断，再呼叫$name。';
  }

  @override
  String friendCardBothInRoom(String room) {
    return '你们都在$room里。';
  }

  @override
  String get friendCardMessage => '发消息';

  @override
  String friendCardMutualFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n位共同好友',
    );
    return '$_temp0';
  }

  @override
  String get userPanelEditProfile => '编辑个人资料';

  @override
  String get userPanelStatus => '状态';

  @override
  String get userPanelOnline => '在线';

  @override
  String get userPanelAway => '暂离';

  @override
  String get userPanelOffline => '离线';

  @override
  String get userPanelOfflineNote => '好友和群组成员会看到你离线，但你仍可进入房间。';

  @override
  String get userPanelOnlineNote => '你的好友和群组成员会看到你在线，10分钟未使用这台电脑后显示为暂离。';

  @override
  String get userPanelBio => '个人简介';

  @override
  String get userPanelAppearingOffline => '显示为离线';

  @override
  String get userPanelCopyLinkLabel => '复制好友链接';

  @override
  String userPanelLinkExpiresIn(String time) {
    return '$time后过期';
  }

  @override
  String get userPanelLinkTerms => '1人 · 10分钟';

  @override
  String get userPanelCopyLink => '复制好友链接';

  @override
  String get switcherChatEnded => '私聊 · 已不是好友';

  @override
  String get switcherChatOnline => '私聊 · 在线';

  @override
  String get switcherChatAway => '私聊 · 暂离';

  @override
  String get switcherChat => '私聊';

  @override
  String switcherFriend(String presence) {
    return '好友 · $presence';
  }

  @override
  String switcherVoiceRoom(String group) {
    return '$group中的语音房间';
  }

  @override
  String switcherVoiceRoomPeople(String group, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$group中的语音房间 · $n人',
    );
    return '$_temp0';
  }

  @override
  String switcherGroup(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '群组 · $n位成员',
    );
    return '$_temp0';
  }

  @override
  String get switcherPlaceholder => '前往聊天、群组或语音房间';

  @override
  String get switcherGoTo => '前往';

  @override
  String get switcherEmpty => '还没有群组、好友或聊天。';

  @override
  String switcherNoMatch(String query) {
    return '没有与“$query”匹配的结果。';
  }

  @override
  String get switcherHintChoose => '选择';

  @override
  String get switcherHintGo => '前往';

  @override
  String get switcherHintClose => '关闭';

  @override
  String switcherTooltip(String shortcut) {
    return '前往聊天、群组或语音房间（$shortcut）';
  }

  @override
  String get friendsIntro => '每段好友关系只保存在双方的设备上。没有目录，没有服务器。';

  @override
  String get friendsTabAll => '全部';

  @override
  String get friendsTabOnline => '在线';

  @override
  String get friendsTabRequests => '请求';

  @override
  String get friendsSearch => '搜索好友';

  @override
  String get friendsNone => '还没有好友。分享你的二维码、链接或好友码，或使用别人的。';

  @override
  String get friendsNoneOnline => '目前没有好友在线。';

  @override
  String get friendsNoMatch => '没有符合搜索的人。';

  @override
  String get friendsInARoom => '在房间中';

  @override
  String friendsInRoomOf(String group) {
    return '在房间中 · $group';
  }

  @override
  String get friendsOnline => '在线';

  @override
  String get friendsAway => '暂离';

  @override
  String get friendsNotSeenYet => '还没上过线';

  @override
  String friendsSeen(String when) {
    return '$when在线';
  }

  @override
  String friendsShowCard(String name) {
    return '显示$name的资料卡';
  }

  @override
  String friendsMissedCall(String when) {
    return '未接来电 · $when';
  }

  @override
  String get friendsMessage => '发消息';

  @override
  String friendsOfflineNoCall(String name) {
    return '$name离线：只有好友在线时才能呼叫';
  }

  @override
  String friendsHangUpFirst(String name) {
    return '请先挂断，再呼叫$name';
  }

  @override
  String friendsBothInRoom(String room) {
    return '你们都在$room里。';
  }

  @override
  String friendsOptions(String name) {
    return '$name的选项';
  }

  @override
  String friendsRemoveTitle(String name) {
    return '将$name从好友中移除？';
  }

  @override
  String get friendsRemoveText =>
      '你们会从彼此的好友列表中消失，不会发送任何通知。你们仍会一起留在共同的群组中。再次成为好友需要重新邀请。';

  @override
  String get friendsRemove => '移除';

  @override
  String friendsAddToGroup(String group) {
    return '添加到“$group”';
  }

  @override
  String get friendsRemoveMenu => '从好友中移除…';

  @override
  String get friendsNoRequests => '没有待处理的请求。';

  @override
  String friendsRequestIncoming(String when) {
    return '想加你为好友 · $when';
  }

  @override
  String friendsRequestOutgoing(String when) {
    return '$when发送 · 等待回复';
  }

  @override
  String get friendsPendingRequest => '待处理的请求';

  @override
  String friendsCheckFingerprint(String name) {
    return '请核对指纹是否与$name屏幕上显示的一致。';
  }

  @override
  String get friendsAddTitle => '添加好友';

  @override
  String get friendsAddNote => '你们的设备会直接交换密钥。';

  @override
  String get friendsTabCode => '好友码';

  @override
  String get friendsTabLink => '链接';

  @override
  String get friendsTabQr => '二维码';

  @override
  String get friendsMakeInvitation => '创建邀请';

  @override
  String get friendsInvitationExpired => '这个邀请已过期。';

  @override
  String get friendsMakeAnother => '再创建一个';

  @override
  String friendsHintQr(String time) {
    return '用Fogata扫描 · $time后过期';
  }

  @override
  String friendsHintLink(String time) {
    return '发送给一个人 · $time后过期';
  }

  @override
  String friendsHintCode(String time) {
    return '念给一个人听 · $time后过期';
  }

  @override
  String get friendsHiddenCode => '已隐藏，以防你正在共享屏幕。';

  @override
  String get friendsHiddenLink => '已隐藏，以防你正在共享屏幕。它包含这台电脑的网络地址，请只私下分享。';

  @override
  String get friendsGotLink => '收到了好友链接、群组链接或好友码？';

  @override
  String get friendsPastePlaceholder => '粘贴链接或好友码';

  @override
  String get friendsLinkOrCode => '链接或好友码';

  @override
  String get friendsAdd => '添加';

  @override
  String get friendsQrLabel => '你的邀请二维码';

  @override
  String friendsAddedToGroup(String name) {
    return '下次你们都在线时，$name会收到这个群组。';
  }

  @override
  String get directToday => '今天';

  @override
  String get directYesterday => '昨天';

  @override
  String directTimerOff(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {'yes': '你', 'other': '$name'});
    return '$_temp0关闭了限时消息';
  }

  @override
  String directTimerFirst(String time) {
    return '限时消息已开启：$time';
  }

  @override
  String directTimerOn(String name, String mine, String time) {
    String _temp0 = intl.Intl.selectLogic(mine, {'yes': '你', 'other': '$name'});
    return '$_temp0开启了限时消息：$time';
  }

  @override
  String directPinnedLine(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {'yes': '你', 'other': '$name'});
    return '$_temp0置顶了一条消息';
  }

  @override
  String get directCallFewSeconds => '几秒';

  @override
  String directCallMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n分钟');
    return '$_temp0';
  }

  @override
  String directCallHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n小时');
    return '$_temp0';
  }

  @override
  String directCallHoursMinutes(int hours, int minutes) {
    String _temp0 = intl.Intl.pluralLogic(
      hours,
      locale: localeName,
      other: '$hours小时',
    );
    String _temp1 = intl.Intl.pluralLogic(
      minutes,
      locale: localeName,
      other: '$minutes分钟',
    );
    return '$_temp0$_temp1';
  }

  @override
  String directCallLine(String name, String mine, String length) {
    String _temp0 = intl.Intl.selectLogic(mine, {'yes': '你', 'other': '$name'});
    return '$_temp0发起了通话，时长$length';
  }

  @override
  String get directBlockedOffline => '你显示为离线。切换到在线后才能发消息。';

  @override
  String directBlockedFriendOffline(String name) {
    return '$name离线。你们都在线时才能发消息。';
  }

  @override
  String get directOnline => '在线';

  @override
  String get directAway => '暂离';

  @override
  String get directOffline => '离线';

  @override
  String directOfflineSeen(String when) {
    return '离线 · $when在线';
  }

  @override
  String get directHeading => '私信';

  @override
  String directPreviewMine(String text) {
    return '你：$text';
  }

  @override
  String directOpenChat(String name) {
    return '打开与$name的聊天';
  }

  @override
  String directShowCard(String name) {
    return '显示$name的资料卡';
  }

  @override
  String get directNoLongerFriend => '已不是好友';

  @override
  String directCallOffline(String name) {
    return '$name离线：只有好友在线时才能呼叫';
  }

  @override
  String directHangUpFirst(String name) {
    return '请先挂断，再呼叫$name';
  }

  @override
  String directPinnedCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n条置顶消息',
    );
    return '$_temp0';
  }

  @override
  String get directPinnedTitle => '置顶消息';

  @override
  String directPinnedNote(int max) {
    return '对你们双方都置顶，且不会消失。最多$max条。';
  }

  @override
  String get directNothingPinned => '还没有置顶消息';

  @override
  String get directNothingPinnedNote => '将鼠标悬停在消息上并点击图钉，即可将其保留在这里。';

  @override
  String directDayTime(String day, String time) {
    return '$day $time';
  }

  @override
  String directFilesCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n个文件',
    );
    return '$_temp0';
  }

  @override
  String directShowMessageFrom(String name) {
    return '显示$name的消息';
  }

  @override
  String get directUnpin => '取消置顶';

  @override
  String get directPin => '置顶';

  @override
  String directTimerLabel(String time) {
    return '限时消息：$time';
  }

  @override
  String get directTimerTitle => '限时消息';

  @override
  String get directCloseMenu => '关闭菜单';

  @override
  String get directTimerNote => '新消息会连同其中的图片和文件，从双方设备上消失。';

  @override
  String get directEmptyNote => '你写的内容只保存在你们两人的电脑上。';

  @override
  String directPinLineLabel(String line) {
    return '$line：显示';
  }

  @override
  String get directShowMessage => '显示消息';

  @override
  String get directPinnedStays => '已置顶：取消置顶前不会消失';

  @override
  String get directPinned => '已置顶';

  @override
  String directPinsFull(int max) {
    return '最多可置顶$max条消息：请先取消一条';
  }

  @override
  String get directDelivered => '已送达';

  @override
  String get directNotDelivered => '未送达';

  @override
  String get directSending => '正在发送…';

  @override
  String directWaiting(String name) {
    return '等待$name上线 · 你们都在线时发送';
  }

  @override
  String get directAvailableWhenYouOnline => '你上线后可获取';

  @override
  String directAvailableWhenOnline(String name) {
    return '$name在线时可获取';
  }

  @override
  String get directDownloadedByItself => '已自动下载';

  @override
  String directOpenFile(String name) {
    return '打开$name';
  }

  @override
  String directPlayFile(String name) {
    return '播放$name';
  }

  @override
  String directDownloadSize(String size) {
    return '下载 · $size';
  }

  @override
  String directVideoPlaysWhenHere(String name, String length) {
    return '$name · $length · 收到后即可播放';
  }

  @override
  String directVideoNotHere(String name, String length, String when) {
    return '$name · $length · $when';
  }

  @override
  String directGettingIt(int percent) {
    return '正在接收 · $percent%';
  }

  @override
  String directFileNotHere(String size, String when) {
    return '$size · $when';
  }

  @override
  String get directSaveToDownloads => '保存到“下载”';

  @override
  String get directRemovedForSpace => '已在此删除以释放空间';

  @override
  String get directAskAgain => '重新获取';

  @override
  String get directClosePicture => '关闭图片';

  @override
  String get directCloseVideo => '关闭视频';

  @override
  String get directCannotPlay => '这台电脑无法播放这个视频。';

  @override
  String get directPause => '暂停';

  @override
  String get directPlay => '播放';

  @override
  String get directSoundOn => '打开声音';

  @override
  String get directMute => '静音';

  @override
  String directRemoveFilesTitle(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '移除这$n个文件？',
      one: '移除这个文件？',
    );
    return '$_temp0';
  }

  @override
  String get directRemoveFilesText => '它们会从这条消息中移除。你电脑上的文件保持不变。';

  @override
  String get directDraftKept => '你正在写的内容会保留在这里，等可以发送时再发。';

  @override
  String directMessagePlaceholder(String name) {
    return '发消息给$name';
  }

  @override
  String get directAddMessageOptional => '添加消息（可选）';

  @override
  String get directAutoDownloadNote => '10 MB以内的图片会自动下载；视频和文件在打开时下载。';

  @override
  String get directVoiceRecord => '录制语音消息';

  @override
  String get directVoiceInCall => '通话中无法录音';

  @override
  String directVoiceWaitingElsewhere(String name) {
    return '请先发送或删除与$name的聊天中的语音消息。';
  }

  @override
  String get directVoiceDelete => '删除';

  @override
  String get directVoiceTooShort => '太短，无法发送：语音消息至少需要半秒。';

  @override
  String directVoiceLeft(String time) {
    return '剩余$time';
  }

  @override
  String directVoiceRecordingFrom(String microphone) {
    return '正在用$microphone录音';
  }

  @override
  String get directVoiceRecordingDefault => '所选麦克风不可用：正在用默认麦克风录音';

  @override
  String directVoiceKeys(String esc, String enter) {
    return '按$esc删除，按$enter发送';
  }

  @override
  String directVoiceKeySend(String enter) {
    return '按$enter发送';
  }

  @override
  String directVoicePreview(String length) {
    return '语音消息 · $length';
  }

  @override
  String get directVoiceMessage => '语音消息';

  @override
  String get directVoiceNoMic => 'Fogata无法使用麦克风。';

  @override
  String get directVoiceMicStopped => '麦克风已停止。已录制的部分仍可发送。';

  @override
  String directVoiceNoMicCheck(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': '请检查是否已连接麦克风、已在“语音和视频”中选中，并已在“系统设置 › 隐私与安全性”中允许。',
      'other': '请检查是否已连接麦克风，并已在“语音和视频”中选中。',
    });
    return '$_temp0';
  }

  @override
  String get directVoiceSpeed => '播放速度';

  @override
  String get directVoiceNotPlayed => '未播放';

  @override
  String directRemoveFile(String name) {
    return '移除$name';
  }

  @override
  String get directAltered => '自Fogata上次保存以来，这台电脑上的聊天记录被改动过。部分消息可能缺失。';

  @override
  String directEnded(String name) {
    return '你和$name已不是好友。这个聊天只能查看。';
  }

  @override
  String get directDeleteChat => '删除聊天';

  @override
  String directDeleteTitle(String name) {
    return '删除与$name的聊天？';
  }

  @override
  String directDeleteText(String name) {
    return '其中的消息、图片、视频和文件会从这台电脑上删除。$name会保留自己的副本。';
  }

  @override
  String get directDelete => '删除';

  @override
  String get directUsageMessages => '消息';

  @override
  String get directUsagePictures => '图片';

  @override
  String get directUsageVideos => '视频';

  @override
  String get directUsageFiles => '文件';

  @override
  String get directUsageHeading => '本设备上的此聊天';

  @override
  String get directFreeUpSpace => '释放空间…';

  @override
  String get directAddFilesTooltip => '添加图片、视频或文件';

  @override
  String get directAddFiles => '添加文件';

  @override
  String get accountMoveLessThanMinuteLeft => '剩余不到1分钟';

  @override
  String get accountMoveAboutMinuteLeft => '大约还剩1分钟';

  @override
  String accountMoveAboutMinutesLeft(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '大约还剩$n分钟',
    );
    return '$_temp0';
  }

  @override
  String accountMoveNoRoom(String need, String free) {
    return '这台电脑需要$need可用空间来存放账号，目前只有$free。';
  }

  @override
  String accountMoveTooLarge(String size) {
    return '另一台电脑提供了$size的数据，远超一个账号应有的大小。';
  }

  @override
  String get accountMoveBusy => '这台电脑正在完成另一次迁移：请先完成或放弃它。';

  @override
  String get accountMoveNothingToFinish => '这台电脑没有来自那台电脑、需要完成的迁移。';

  @override
  String get accountMoveNetworkStopped => '网络已停止。';

  @override
  String get accountMoveConnectionLost => '连接已断开。';

  @override
  String get accountMovePeerNoRoom => '新电脑的可用空间不足。';

  @override
  String get accountMovePeerTooLarge => '账号太大，无法迁移。';

  @override
  String get accountMovePeerBusy => '新电脑正在完成另一次迁移。';

  @override
  String get accountMovePeerCouldNotKeep => '新电脑无法保存这些文件。';

  @override
  String get accountMovePeerCancelled => '迁移已在另一台电脑上取消。';

  @override
  String get accountMovePeerCouldNotFinish => '新电脑暂时无法完成迁移。';

  @override
  String get accountMovePeerStopped => '迁移已在另一台电脑上停止。';

  @override
  String get accountMovePeerNothingToFinish => '那台电脑没有来自这台电脑、需要完成的迁移。';

  @override
  String get accountMovePeerNotProven => '那台电脑无法证明它收到了这次迁移。';

  @override
  String get accountMoveWrongCode => '另一台电脑不接受这个迁移码，因此没有发送任何内容。';

  @override
  String get accountMoveExpired => '迁移码在使用前已过期。';

  @override
  String get accountMoveTooManyTries => '错误迁移码的尝试次数过多，这个迁移码已失效。';

  @override
  String get accountMoveNoContact => '两台电脑无法相互连接。';

  @override
  String get accountMoveUnconfirmed => '迁移的最后一步没有得到确认。';

  @override
  String get accountMoveCancelled => '已取消。';

  @override
  String get accountMoveInterrupted => '迁移在完成前中断了；没有任何更改。';

  @override
  String accountMoveInterruptedWhy(String why) {
    return '迁移在完成前中断了；没有任何更改。$why';
  }

  @override
  String get accountMoveDeclined => '另一台电脑停止了迁移。';

  @override
  String accountMoveDeclinedWhy(String why) {
    return '另一台电脑停止了迁移。$why';
  }

  @override
  String get accountMoveLater => '账号已收到，将在Fogata下次在这里启动时生效。';

  @override
  String accountMoveLaterWhy(String why) {
    return '账号已收到，将在Fogata下次在这里启动时生效。$why';
  }

  @override
  String get accountMoveDidNotFinish => '迁移未完成。';

  @override
  String get accountMoveAnotherComputer => '另一台电脑';

  @override
  String get accountMoveTheOtherComputer => '另一台电脑';

  @override
  String get moveTooLateBeingPlaced => '已无法停止：账号正在生效。';

  @override
  String get moveTooLateDiscard => '太晚了：账号正在生效。';

  @override
  String get moveTooLateOtherHasIt => '已无法停止：另一台电脑已经拥有这个账号。';

  @override
  String moveFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n位好友',
    );
    return '$_temp0';
  }

  @override
  String moveGroups(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n个群组',
    );
    return '$_temp0';
  }

  @override
  String moveGroupsWithKeys(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n个群组及其密钥',
    );
    return '$_temp0';
  }

  @override
  String moveChats(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n个聊天',
    );
    return '$_temp0';
  }

  @override
  String moveInAll(String size) {
    return '共$size';
  }

  @override
  String get moveYourSettings => '你的设置';

  @override
  String get moveNotDevices => '不含所选麦克风或摄像头';

  @override
  String get moveFinishingBoth => '正在两台电脑上完成…';

  @override
  String movePercent(int percent) {
    return '$percent%';
  }

  @override
  String movePercentLeft(int percent, String left) {
    return '$percent% · $left';
  }

  @override
  String get moveStepIdentity => '身份和个人资料';

  @override
  String get moveStepFriends => '好友';

  @override
  String get moveStepGroups => '群组及其密钥';

  @override
  String get moveStepChats => '聊天、图片和文件';

  @override
  String moveBytesOf(String done, String total) {
    return '$done/$total';
  }

  @override
  String get moveStepFinishing => '在两台电脑上完成';

  @override
  String moveBringingNamed(String name) {
    return '正在迁入$name的账号';
  }

  @override
  String get moveBringingYours => '正在迁入你的账号';

  @override
  String get moveKeepBothOn => '请保持两台电脑开机并打开Fogata。';

  @override
  String get moveKeepOpenNote => '请在两台电脑上保持Fogata打开。在同一网络中最快；否则它们会通过互联网直接连接。';

  @override
  String moveFinishBringingNamed(String name) {
    return '完成迁入$name的账号';
  }

  @override
  String get moveFinishBringingYours => '完成迁入你的账号';

  @override
  String get moveEverythingCame => '所有内容已收到。请在另一台电脑上输入这个迁移码以完成。';

  @override
  String moveEverythingCameFrom(String computer) {
    return '所有内容已收到。请在“$computer”上输入这个迁移码以完成。';
  }

  @override
  String get moveBringHereTitle => '将你的账号迁到这里';

  @override
  String get moveBringHereSub => '请在有你账号的电脑上输入这个迁移码。';

  @override
  String get moveHideCode => '隐藏迁移码';

  @override
  String get moveShowCode => '显示迁移码';

  @override
  String moveExpiresIn(String time) {
    return '$time后过期';
  }

  @override
  String moveStepFogataShows(String what) {
    return '那台电脑上的Fogata会显示$what。';
  }

  @override
  String get moveWaitingToConfirm => '等待确认';

  @override
  String get moveStepOpenThere => '在那台电脑上打开Fogata。';

  @override
  String moveStepTypeUnder(String where) {
    return '在$where下输入迁移码。';
  }

  @override
  String get moveFinishTheMove => '完成迁移';

  @override
  String moveStepGoTo(String where) {
    return '前往$where。';
  }

  @override
  String get moveSettingsPath => '设置 → 迁移账号';

  @override
  String get moveToAnotherComputer => '迁移到另一台电脑';

  @override
  String get moveWaitingForOther => '正在等待你的另一台电脑…';

  @override
  String moveWrongCodesTyped(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '其他电脑上已输入$n次错误的迁移码。',
      one: '有人在另一台电脑上输入了错误的迁移码。',
    );
    return '$_temp0';
  }

  @override
  String get moveThrowAwayInstead => '改为丢弃…';

  @override
  String get moveFoundOther => '已找到你的另一台电脑';

  @override
  String get moveFoundSameNetwork => '同一网络 · 两台电脑之间已加密';

  @override
  String get moveFoundInternet => '通过互联网 · 两台电脑之间已加密';

  @override
  String get moveConfirmThere => '请在另一台电脑上确认以开始';

  @override
  String get moveSameAccountNote => '这是这台电脑上已有的账号。将它迁到这里不会丢失任何内容。';

  @override
  String get moveTakeYes => '迁到这里';

  @override
  String get moveWelcomeBack => '欢迎回来';

  @override
  String moveWelcomeBackNamed(String name) {
    return '欢迎回来，$name';
  }

  @override
  String get moveDoneSub => '你的账号现在在这台电脑上了。另一台电脑已停止使用它。';

  @override
  String get moveOpenFogata => '打开Fogata';

  @override
  String get moveCodeExpiredLabel => '已过期';

  @override
  String get moveCodeStoppedLabel => '已失效';

  @override
  String get moveCodeExpiredNote => '这个迁移码在你的另一台电脑使用前已过期。迁移码有效期为10分钟。';

  @override
  String get moveCodeStoppedNote => '其他电脑上多次输入了错误的迁移码，因此这个迁移码已失效。';

  @override
  String get moveNewCode => '新迁移码';

  @override
  String get moveAlmostThere => '即将完成';

  @override
  String get moveLaterSub => '你的账号已完整收到，但暂时无法生效。它将在Fogata下次在这里启动时生效。';

  @override
  String get moveStartAgain => '重新启动Fogata';

  @override
  String get moveUnconfirmedSub =>
      '所有内容已收到，但另一台电脑没有确认结束。请显示一个迁移码并在那台电脑上输入，以完成迁移。';

  @override
  String get moveStoppedThereTitle => '迁移已在另一台电脑上停止';

  @override
  String get moveStoppedThereSub => '没有任何更改：你的账号仍在另一台电脑上，这里没有保留任何内容。';

  @override
  String get moveCouldNotStartTitle => '迁移无法开始';

  @override
  String get moveSomethingWrongHere => '这台电脑上出了问题。';

  @override
  String get moveHalfwayTitle => '迁移中途停止了';

  @override
  String get moveHalfwaySub => '与你另一台电脑的连接已断开。没有任何更改：你的账号仍在另一台电脑上，这里没有保留任何内容。';

  @override
  String get moveShowCodeToFinish => '显示迁移码以完成';

  @override
  String get moveTryNewCode => '用新迁移码重试';

  @override
  String moveBackToNamed(String name) {
    return '返回$name';
  }

  @override
  String get moveBackToThisProfile => '返回这个个人资料';

  @override
  String get moveNewProfileInstead => '改为创建新的个人资料';

  @override
  String get moveDiscardTitle => '丢弃收到的账号？';

  @override
  String get moveDiscardText =>
      '只有在另一台电脑继续使用它时才这样做。那台电脑会一直等待确认，直到其使用者选择在那里继续使用。';

  @override
  String get moveKeepIt => '保留';

  @override
  String get moveThrowAway => '丢弃';

  @override
  String get moveBringFromAnother => '从另一台电脑迁入你的账号';

  @override
  String get moveAlreadyOnFogata => '已经在用Fogata？';

  @override
  String get moveCodeTriedBefore => '这个迁移码已经匹配失败过一次。请在新电脑上获取新的迁移码。';

  @override
  String moveMovingTo(String computer) {
    return '正在将你的账号迁移到“$computer”';
  }

  @override
  String get moveKeepBothOnOffline =>
      '请保持两台电脑开机并打开Fogata。在此期间，这里的Fogata处于离线状态。';

  @override
  String get moveCodeFieldLabel => '新电脑上显示的迁移码';

  @override
  String get moveToAnotherSub =>
      '你的身份、好友、群组、聊天、文件和设置会从这台电脑直接加密传到另一台电脑。之后这台电脑将停止使用你的账号，因此永远不会出现第二个你。';

  @override
  String moveStepChoose(String choice) {
    return '在新电脑上打开Fogata，选择$choice。';
  }

  @override
  String get moveStepTypeItShows => '输入它显示的迁移码：';

  @override
  String get moveWaitingForAnswer => '正在等待你的回答…';

  @override
  String get moveLookingForCode => '正在寻找显示这个迁移码的电脑…';

  @override
  String get moveOfflineWhileMoving => '迁移期间，这台电脑上的Fogata会离线。';

  @override
  String get moveWrongCodeNote =>
      '新电脑不接受这个迁移码，因此没有发送任何内容。请检查后重新输入。如果仍然失败，请在那台电脑上获取新的迁移码。';

  @override
  String get moveCantReachNew => '无法连接新电脑。';

  @override
  String get moveCantReachNewNote =>
      '两台电脑互相寻找了3分钟。请让它们连接到同一网络（同一Wi-Fi或网线）后重试。你的账号仍在这里，Fogata已恢复在线。';

  @override
  String get moveDeclinedNote => '迁移已在新电脑上停止。没有迁移任何内容：你的账号仍在这里，Fogata已恢复在线。';

  @override
  String get moveCouldNotStart => '迁移无法开始。';

  @override
  String get moveHalfwayNote =>
      '迁移中途停止了：与新电脑的连接已断开。没有任何更改：你的账号仍在这里，Fogata已恢复在线。';

  @override
  String get moveThisProfileWords => '这个个人资料';

  @override
  String get moveBringCardTitle => '将账号迁到这里';

  @override
  String get moveBringCardSub => '在这台电脑上显示迁移码，然后在有账号的那台电脑上输入。';

  @override
  String get moveBringCardButton => '将账号迁到这里…';

  @override
  String moveReplacesHere(String name) {
    return '将替换这台电脑上的$name。';
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
      other: '$friends位好友',
    );
    String _temp1 = intl.Intl.pluralLogic(
      groups,
      locale: localeName,
      other: '$groups个群组',
    );
    String _temp2 = intl.Intl.pluralLogic(
      chats,
      locale: localeName,
      other: '$chats个聊天',
    );
    return '$bold是这台电脑当前的个人资料，它将被替换，其$_temp0、$_temp1和$_temp2将在这里删除。若要保留$name，请先将其迁移到另一台电脑。';
  }

  @override
  String moveReplaceAndShow(String name) {
    return '替换$name并显示迁移码';
  }

  @override
  String moveConfirmTitle(String computer) {
    return '将你的账号迁移到“$computer”？';
  }

  @override
  String get moveConfirmSameNetwork => '已在同一网络中找到。所有内容都会加密后直接传过去。';

  @override
  String get moveConfirmInternet => '已通过互联网找到。所有内容都会加密后直接传过去。';

  @override
  String moveConfirmReplaces(String name) {
    return '它会替换那台电脑上的个人资料$name。';
  }

  @override
  String get moveConfirmNote => '完成后，这台电脑将停止使用你的账号，且无法再使用。之后你可以删除这里剩下的内容。';

  @override
  String get moveConfirmYes => '迁移我的账号';

  @override
  String get moveKeepTitle => '在这里继续使用你的账号？';

  @override
  String get moveKeepText =>
      '仅限新电脑上没有这个账号时：请先在那台电脑上打开Fogata确认。如果两台都在使用，你的好友和群组会看到两个你，消息也只会送达其中一个。';

  @override
  String get moveKeepYes => '在这里继续使用';

  @override
  String moveWaitingFor(String computer) {
    return '正在等待“$computer”确认';
  }

  @override
  String get moveWaitingText => '你的账号已发送，但在新电脑表示准备好之前连接断开了。在此期间，这台电脑不会使用这个账号。';

  @override
  String get moveFinishText => '在新电脑上打开Fogata：它会显示一个迁移码。请在这里输入。';

  @override
  String get moveFinish => '完成';

  @override
  String get moveFinishWrongCode => '新电脑不接受这个迁移码。请检查后重新输入。';

  @override
  String get moveFinishNoContact => '无法连接新电脑。请让两台电脑连接到同一网络后重试。';

  @override
  String get moveKeepThisOne => '新电脑上没有这个账号：继续使用这台…';

  @override
  String moveMovedYours(String computer) {
    return '你的账号已迁移到“$computer”';
  }

  @override
  String moveMovedNamed(String name, String computer) {
    return '$name的账号已迁移到“$computer”';
  }

  @override
  String get moveMovedNoteYou => '这台电脑已不再使用它：好友和群组会在另一台电脑上联系你。';

  @override
  String moveMovedNoteNamed(String name) {
    return '这台电脑已不再使用它：好友和群组会在另一台电脑上联系$name。';
  }

  @override
  String moveMovedTodayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': '今天 $time。',
      'other': '今天 $time。',
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
  String get moveLeftHere => '这里剩下的内容';

  @override
  String get moveLeftHereText => '迁移前的聊天、图片和文件，已加密。另一台电脑有自己的副本。';

  @override
  String get moveDeleteData => '删除这台电脑上的Fogata数据';

  @override
  String get moveAfterDeleting => '删除后，这里的Fogata会像全新安装一样启动。';

  @override
  String get moveCodeNotACode => '迁移码由十个字母和数字组成，格式如XXXXX-XXXXX。';

  @override
  String get moveCodeMistyped => '这个迁移码不对：请检查字母和数字。';

  @override
  String get welcomeHeadline => '聊起来就像\n在同一个房间。';

  @override
  String get welcomeIntro => '与好友、家人或团队语音、视频和聊天，直接从你的电脑连到他们的电脑。';

  @override
  String get welcomeNoAccount => '无需账号';

  @override
  String get welcomeEncrypted => '加密';

  @override
  String get welcomeEphemeral => '临时房间';

  @override
  String get welcomeCharacter => 'Fogata的形象：一团戴着耳机、比着和平手势的小火焰';

  @override
  String get welcomeImageUnreadable => '无法读取这张图片。';

  @override
  String get welcomeKeychainTitle => '你的数据暂时无法加密';

  @override
  String welcomeKeychainText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': '在Linux上，钥匙串是正在运行的Secret Service，例如KWallet或GNOME Keyring：请启动或解锁一个，然后重试。',
      'other': '请解锁或修复钥匙串，然后重试。',
    });
    return 'Fogata用保存在系统钥匙串中的密钥加密你的身份、好友和私信，但钥匙串没有保存这个密钥。$_temp0';
  }

  @override
  String get welcomeKeychainRetry => '重试';

  @override
  String get welcomeKeychainPlain => '不加密继续';

  @override
  String get welcomeKeychainPlainNote =>
      '不加密时，任何复制这台电脑文件的人都能读取它们。设置中会一直提示这一点；Fogata首次能使用钥匙串打开时，会把所有内容加密。';

  @override
  String get welcomeWriteName => '请输入你的名字。';

  @override
  String get welcomeCardTitle => '你希望大家怎样看到你？';

  @override
  String get welcomeCardSub => '一个名字和一张图片。随时可以更改。';

  @override
  String get welcomeNoServer =>
      '没有任何内容会发送到Fogata服务器：因为根本没有服务器。公共IPFS网络只用作查找好友的通讯录。';

  @override
  String get welcomeYourName => '你的名字';

  @override
  String welcomePictureLabel(String name) {
    return '图片：$name';
  }

  @override
  String get welcomeUploadPicture => '上传图片';

  @override
  String get welcomeNameLabel => '名字';

  @override
  String get welcomeCreate => '创建';

  @override
  String get welcomeReady => '已创建';

  @override
  String get welcomeCreateLabel => '创建身份';

  @override
  String welcomeKeyLabel(String key) {
    return '在这台电脑上生成的密钥：$key';
  }

  @override
  String get welcomeNewKey => '生成另一个密钥';

  @override
  String get stoppedTitle => 'Fogata停止运行了。';

  @override
  String get stoppedStartingAgain => '正在重新启动…';

  @override
  String get stoppedRestartToConnect => '请重新启动以再次连接。';

  @override
  String get stoppedRestartNow => '立即重启';

  @override
  String get settingsSectionProfile => '我的资料';

  @override
  String get settingsSectionMove => '迁移账号';

  @override
  String get settingsSectionAppearance => '外观';

  @override
  String get settingsSectionLanguageTime => '语言和时间';

  @override
  String get settingsSectionVoice => '语音和视频';

  @override
  String get settingsSectionSounds => '声音';

  @override
  String get settingsSectionBehaviour => '行为';

  @override
  String get settingsSectionNetwork => '网络';

  @override
  String get settingsSectionStorage => '存储和下载';

  @override
  String get settingsHeadingUser => '用户';

  @override
  String get settingsHeadingApp => '应用';

  @override
  String get settingsTitle => '设置';

  @override
  String get settingsAll => '所有设置';

  @override
  String get settingsCloseTooltip => '关闭设置';

  @override
  String get settingsUnsavedSaveTitle => '保存更改？';

  @override
  String get settingsUnsavedDiscardTitle => '放弃更改？';

  @override
  String get settingsUnsavedSaveText => '你的个人资料有尚未保存的更改。';

  @override
  String get settingsUnsavedDiscardText => '你的个人资料有无法按原样保存的更改。';

  @override
  String get settingsProfileSaved => '个人资料已保存。在线的好友和群组现在就能看到，其他人在连接时看到。';

  @override
  String get settingsBioSaved => '个人简介已保存。在线的好友现在就能看到，其他人在连接时看到。';

  @override
  String settingsSwitchOn(String what) {
    return '$what已开启';
  }

  @override
  String settingsSwitchOff(String what) {
    return '$what已关闭';
  }

  @override
  String get settingsOn => '开';

  @override
  String get settingsOff => '关';

  @override
  String get settingsIdentity => '你的身份';

  @override
  String get settingsIdentityNote => '你的名字和头像会签名后发送给你的好友和群组，个人简介只发送给好友。';

  @override
  String get settingsPicture => '图片';

  @override
  String get settingsUploadPicture => '上传图片或GIF';

  @override
  String get settingsName => '名字';

  @override
  String get settingsBio => '个人简介';

  @override
  String get settingsBioHint => '简单介绍一下自己';

  @override
  String get settingsNoBio => '你的资料卡上不显示个人简介。';

  @override
  String settingsBioWhere(int lines) {
    return '你的好友会在你的资料卡上看到它，最多$lines行。';
  }

  @override
  String get settingsBannerTitle => '面板背景';

  @override
  String get settingsBannerNote => '你资料卡的顶部，你和你的好友都能看到。';

  @override
  String get settingsBannerAnother => '换一张图片';

  @override
  String get settingsBannerUpload => '使用这台电脑上的图片';

  @override
  String get settingsFingerprint => '这台电脑的指纹';

  @override
  String get settingsTheme => '主题';

  @override
  String get settingsThemeNote => '窗口的颜色。“系统”会跟随电脑设置在“深色”和“浅色”之间切换。';

  @override
  String get settingsMicSpeaker => '麦克风和扬声器';

  @override
  String get settingsMicSpeakerNote => '通话时收录你声音和播放声音的设备。通话中也可以更改。';

  @override
  String get settingsCamera => '摄像头';

  @override
  String get settingsCameraNote => '摄像头打开时通话中显示的画面。可以在通话前先在这里试一试。';

  @override
  String get settingsSoundEffects => '音效';

  @override
  String get settingsSoundEffectsNote => '进入和离开房间、有人进出、聊天消息、麦克风和共享时播放。';

  @override
  String get settingsSounds => '声音';

  @override
  String settingsChangeFailed(String reason) {
    return 'Fogata无法更改这项设置：$reason';
  }

  @override
  String get settingsAutostartApprove => '请在“系统设置 › 通用 › 登录项”中允许Fogata。';

  @override
  String get settingsAutostartOldMac => '这需要macOS 13或更高版本。';

  @override
  String get settingsAutostartUnsupported => '这个版本的Fogata无法自行启动。';

  @override
  String get settingsAutostartUnread => 'Fogata无法从系统读取这项设置。';

  @override
  String get settingsAutostart => '开机时打开';

  @override
  String get settingsAutostartNote => '电脑启动并登录后打开Fogata。';

  @override
  String get settingsKeepRunning => '保持运行';

  @override
  String get settingsKeepRunningNote => '关闭窗口后，Fogata会留在系统托盘中，好友仍然可以呼叫你。';

  @override
  String get settingsNoTray => '这个桌面不显示托盘图标，因此关闭窗口会退出Fogata。';

  @override
  String get settingsHelping => '帮助成员';

  @override
  String get settingsHelpingNote => '这台电脑如何帮助你群组中的成员。';

  @override
  String get settingsHelpRelay => '帮助连接并中继';

  @override
  String get settingsHelpConnect => '仅帮助连接';

  @override
  String get settingsHelpRelayNote =>
      '当两位成员无法相互连接时，这台电脑会转发他们的加密流量（无法查看内容），每次通话最多2 GB。这会占用你的上传带宽。';

  @override
  String get settingsHelpConnectNote =>
      '这台电脑会帮助两位成员找到彼此，但不转发他们的流量。如果他们无法连接，就听不到对方。';

  @override
  String get settingsRestartTray => '从托盘图标退出Fogata并重新打开后，更改才会生效。';

  @override
  String get settingsRestart => '退出Fogata并重新打开后，更改才会生效。';

  @override
  String get settingsNetworkCheck => '网络检测';

  @override
  String get settingsNetworkCheckNote =>
      '这台电脑如何看待网络，以及这对连接好友意味着什么。不会发送任何数据：只查看连接已知的信息。';

  @override
  String get settingsCheckNetwork => '检测网络';

  @override
  String get settingsCheckAgain => '重新检测';

  @override
  String get settingsDownloads => '下载';

  @override
  String get settingsDownloadsNote => '从房间下载的文件保存的位置。';

  @override
  String settingsVersionData(String version, String path) {
    return 'Fogata $version · 数据位于$path';
  }

  @override
  String get settingsNotEncrypted => '在这台电脑上未加密';

  @override
  String settingsNotEncryptedText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': '在Linux上，钥匙串是正在运行的Secret Service，例如KWallet或GNOME Keyring：启动或安装一个后，Fogata下次打开时会加密其数据。',
      'other': 'Fogata下次打开时会再试一次。',
    });
    return '你的身份、好友和私信以未加密的形式保存在这台电脑上，因为没有找到可以保存密钥的系统钥匙串。$_temp0';
  }

  @override
  String get settingsNotAllEncrypted => '这台电脑上的数据未全部加密';

  @override
  String get settingsNotAllEncryptedText =>
      '这台电脑上的部分数据（你的身份、好友和私信）尚未加密：Fogata仍在加密，或者未能完成，例如磁盘已满。每次打开Fogata时都会继续。';

  @override
  String settingsDamagedSecret(String file) {
    return '$file无法用这台电脑的密钥打开：它在磁盘上已损坏。只要它还在，加密就无法完成，加密前的一份副本也会保持未加密。将它移开以完成加密：它会原样保留。如果是你的身份或好友列表，Fogata会取回那份副本并加密；群组或聊天的文件会在这台电脑上丢失：需要群组重新同意你加入，聊天中较早的消息将不再显示。';
  }

  @override
  String get settingsSetDamagedAside => '移开并重新启动';

  @override
  String get storageTitle => '存储';

  @override
  String get storageNote => '你的私信只保存在你的设备上。房间聊天在房间关闭后就会消失，所以只有私信会增长。';

  @override
  String get storageDirect => '私信';

  @override
  String get storageOfNoLimit => '/ 无限制';

  @override
  String storageOfLimit(String limit) {
    return '/ $limit';
  }

  @override
  String storageLegendPictures(String size) {
    return '图片 $size';
  }

  @override
  String storageLegendVideos(String size) {
    return '视频 $size';
  }

  @override
  String storageLegendFiles(String size) {
    return '文件 $size';
  }

  @override
  String storageLegendMessages(String size) {
    return '消息 $size';
  }

  @override
  String get storageRoom => '私信存储空间';

  @override
  String get storageNoLimitNote => '不会自动删除任何内容。';

  @override
  String storageOverNote(String limit) {
    return '已超出上限：最早的图片、视频和文件将被删除，直到降至$limit。消息始终保留。';
  }

  @override
  String get storageLimitNote =>
      '超出上限后，最早的图片、视频和文件会先被删除。消息始终保留；原位置会显示“已在此删除以释放空间”。';

  @override
  String get storageAutoPictures => '自动下载图片';

  @override
  String get storageAutoPicturesNote => '每张最大10 MB，收到时下载。';

  @override
  String get storageAutoFiles => '自动下载视频和文件';

  @override
  String get storageAutoFilesNote => '关闭时：在你打开它们且发送者在线时下载。';

  @override
  String get storageTimer => '新聊天的限时消息';

  @override
  String get storageTimerNote => '每个聊天都可以更改；双方看到的计时始终相同。';

  @override
  String get storageTimer24h => '24小时';

  @override
  String get storageByChat => '按聊天';

  @override
  String get storageByChatNote => '清除会删除这里的图片、视频和文件。消息会保留，对方也会保留自己的副本。';

  @override
  String get storageMessagesOnly => '仅消息';

  @override
  String get storageClearFiles => '清除文件';

  @override
  String storageClearFilesTooltip(String name) {
    return '清除与$name聊天中的文件';
  }

  @override
  String get storageCleared => '已清除';

  @override
  String storageConfirmTitle(String limit) {
    return '图片、视频和文件只保留$limit？';
  }

  @override
  String storageConfirmText(String limit) {
    return '最早的内容现在会被删除，直到降至$limit，置顶消息中的内容最后删除。消息始终保留，你的好友也会保留自己的副本。';
  }

  @override
  String get storageRemoveOldest => '删除最早的内容';

  @override
  String get themePickerSystem => '系统主题，跟随电脑设置';

  @override
  String themePickerTheme(String name) {
    return '$name主题';
  }

  @override
  String get themeDark => '深色';

  @override
  String get themeMidnight => '午夜';

  @override
  String get themeDim => '昏暗';

  @override
  String get themeEmber => '余烬';

  @override
  String get themeContrast => '高对比';

  @override
  String get themeLight => '浅色';

  @override
  String get themeSystem => '系统';

  @override
  String get themeBannerViolet => '紫罗兰';

  @override
  String get themeBannerBlue => '蓝色';

  @override
  String get themeBannerGreen => '绿色';

  @override
  String get themeBannerClay => '陶土';

  @override
  String get themeBannerRaspberry => '覆盆子';

  @override
  String get themeBannerMustard => '芥末黄';

  @override
  String get themeBannerSlate => '石板灰';

  @override
  String get themeBannerNight => '夜色';

  @override
  String widgetsCopyWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': '复制二维码',
      'link': '复制链接',
      'code': '复制好友码',
      'other': '复制',
    });
    return '$_temp0';
  }

  @override
  String widgetsShowWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': '显示二维码',
      'link': '显示链接',
      'code': '显示好友码',
      'other': '显示',
    });
    return '$_temp0';
  }

  @override
  String get widgetsHide => '隐藏';

  @override
  String revealFailed(String path) {
    return '无法打开所在文件夹。文件位于$path';
  }

  @override
  String get emojiGroupPeople => '人物';

  @override
  String get emojiGroupNature => '自然';

  @override
  String get emojiGroupFood => '食物和饮料';

  @override
  String get emojiGroupActivities => '活动';

  @override
  String get emojiGroupTravel => '旅行和地点';

  @override
  String get emojiGroupObjects => '物品';

  @override
  String get emojiGroupSymbols => '符号';

  @override
  String get emojiGroupFlags => '旗帜';

  @override
  String get emojiPickerLimit => '消息已达到长度上限。';

  @override
  String get emojiPickerButton => '表情';

  @override
  String get emojiPickerResults => '结果';

  @override
  String get emojiPickerRecent => '最近使用';

  @override
  String get emojiPickerSearch => '搜索表情（英文）';

  @override
  String get emojiPickerSkinTone => '肤色';

  @override
  String get emojiPickerNone => '未找到表情';

  @override
  String get emojiPickerNoFont =>
      '这台电脑没有彩色表情字体，因此表情可能显示为单色或空白。安装Noto Color Emoji即可解决。';

  @override
  String get emojiPickerDefaultTone => '默认肤色';

  @override
  String emojiPickerTone(int n) {
    return '肤色$n';
  }

  @override
  String get iconsGroupMountain => '山';

  @override
  String get iconsGroupStar => '星星';

  @override
  String get iconsGroupGameController => '游戏手柄';

  @override
  String get iconsGroupMusic => '音乐';

  @override
  String get iconsGroupBook => '书';

  @override
  String get iconsGroupWaves => '波浪';

  @override
  String get iconsGroupCoffee => '咖啡';

  @override
  String get iconsGroupHeart => '爱心';

  @override
  String get iconsGroupFilm => '电影';

  @override
  String get newGroupPictureUnreadable => '无法读取这张图片。';

  @override
  String newGroupMade(String name) {
    return '群组“$name”已创建。成员上线后就会收到。';
  }

  @override
  String get newGroupTitle => '新建群组';

  @override
  String get newGroupSubtitle => '群组是永久的，房间不是。';

  @override
  String get newGroupName => '群组名称';

  @override
  String get newGroupNameExample => '周日骑行';

  @override
  String get newGroupIconOrPicture => '图标或图片';

  @override
  String get newGroupChooseFromComputer => '从电脑中选择';

  @override
  String get newGroupBannerOptional => '横幅（可选）';

  @override
  String get newGroupMembers => '成员';

  @override
  String newGroupSelected(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '已选$n人',
    );
    return '$_temp0';
  }

  @override
  String get newGroupSearchPlaceholder => '搜索你的好友';

  @override
  String get newGroupSearchLabel => '搜索好友';

  @override
  String get newGroupNoFriends => '还没有好友。你可以先创建群组，之后再添加好友，或用链接邀请别人。';

  @override
  String get newGroupKeptOnDevices => '保存在每位成员的设备上。';

  @override
  String get newGroupCreate => '创建群组';

  @override
  String get groupBannerSettings => '群组设置';

  @override
  String get groupBannerPictureUnreadable => '无法读取这张图片。';

  @override
  String get groupBannerChoose => '选择横幅';

  @override
  String get groupBannerAnother => '更换横幅';

  @override
  String get groupBannerNone => '不要横幅';

  @override
  String get groupBannerNote =>
      '显示在群组栏顶部、群组名称下方。支持PNG、JPEG，或550 KB以内的动态GIF或WebP。';

  @override
  String get homeTitle => '主页';

  @override
  String get homeSubtitle => '有人进入时房间就会亮起，所有人离开后就会熄灭。';

  @override
  String get homeAddFriend => '添加好友';

  @override
  String get homeNewGroup => '新建群组';

  @override
  String get homeLive => '进行中';

  @override
  String get homeNoRoom => '没有房间';

  @override
  String get homeNoRoomNote => '开启房间即可点亮它';

  @override
  String homeInviteBy(String name, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n位成员',
    );
    return '$name添加了你 · $_temp0';
  }

  @override
  String get homeInviteNote => '在你加入之前，这台电脑不会连接他们，也不会向他们发送任何内容。';

  @override
  String get homeInviteJoin => '加入';

  @override
  String get homeNoGroups => '还没有群组';

  @override
  String get homeNoGroupsNote => '添加好友并和他们建一个群组。群组是永久的，房间不是。';

  @override
  String homePersonYou(String name) {
    return '$name（你）';
  }

  @override
  String homeMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n位成员',
    );
    return '$_temp0';
  }

  @override
  String get homeYouAreIn => '你在这里';

  @override
  String get homeMuteMic => '麦克风静音';

  @override
  String get homeUnmuteMic => '取消麦克风静音';

  @override
  String get homeCameraOff => '关闭摄像头';

  @override
  String get homeCameraOn => '打开摄像头';

  @override
  String get homeMembersTooltip => '成员';

  @override
  String get homeReturnToRoom => '回到房间';

  @override
  String get homeJoinRoom => '加入';

  @override
  String homeMembersOnline(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members位成员',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online人在线',
      zero: '其他人都不在线',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get homeNoRoomsYet => '还没有房间';

  @override
  String homeLastRoom(String when) {
    return '上次开启：$when';
  }

  @override
  String get homeOpenRoom => '开启房间';

  @override
  String get membersGroupGone => '这个群组已不在了。';

  @override
  String membersSummary(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members位成员',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online人在线',
      zero: '其他人都不在线',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String membersSummaryLastRoom(int members, int online, String when) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members位成员',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online人在线',
      zero: '其他人都不在线',
    );
    return '$_temp0 · $_temp1 · 上次开启：$when';
  }

  @override
  String get membersCopyInviteLink => '复制邀请链接';

  @override
  String membersInviteLinkTooltip(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': '群组中的任何人',
      'first': '群组中排第一的人',
      'other': '群组中前$n人之一',
    });
    return '仅可使用一次，有效期24小时，需由$_temp0同意加入';
  }

  @override
  String membersInviteLinkCopied(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': '群组中的任何人',
      'first': '群组中排第一的人',
      'other': '群组中前$n人之一',
    });
    return '邀请链接已复制。仅可使用一次，有效期24小时，需由$_temp0同意加入。';
  }

  @override
  String get membersInviteFriends => '邀请好友';

  @override
  String get membersBackToRoom => '回到房间';

  @override
  String get membersOpenRoom => '开启房间';

  @override
  String get membersEnterRoom => '进入房间';

  @override
  String get membersTitle => '成员';

  @override
  String get membersOrderNote => '按顺序排列：每个人可以管理排在自己下面的人';

  @override
  String get membersGroupSettings => '群组设置';

  @override
  String membersManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '只有前$n人可以更改群组。',
      one: '只有排第一的人可以更改群组。',
    );
    return '$_temp0';
  }

  @override
  String get membersLeaveGroup => '退出群组';

  @override
  String membersInviteTitle(String group) {
    return '邀请好友加入“$group”';
  }

  @override
  String get membersInviteNote => '他们会收到邀请，并自行决定是否加入。';

  @override
  String get membersPresenceYou => '你';

  @override
  String membersPresenceOnline(String how) {
    return '在线 · $how';
  }

  @override
  String membersPresenceAway(String how) {
    return '暂离 · $how';
  }

  @override
  String get membersPresenceOffline => '当前不在线';

  @override
  String membersYou(String name) {
    return '$name（你）';
  }

  @override
  String get membersWaitingForKey => '正在等待密钥';

  @override
  String membersFingerprint(String fingerprint) {
    return '指纹：$fingerprint';
  }

  @override
  String membersOptionsFor(String name) {
    return '$name的选项';
  }

  @override
  String get membersMoveUp => '上移到我下面';

  @override
  String get membersMoveToEnd => '移到末尾';

  @override
  String get membersTimeOut => '暂时移出房间…';

  @override
  String get membersRemoveFromGroup => '移出群组';

  @override
  String membersRemoveTitle(String name) {
    return '移除$name？';
  }

  @override
  String membersRemoveText(String name) {
    return '$name将被移出群组，群组会生成新的密钥。对方已经看到的内容会保留。';
  }

  @override
  String get membersFirst => '第一';

  @override
  String membersTimeOutTitle(String name) {
    return '暂时移出$name';
  }

  @override
  String membersTimeOutText(String name) {
    return '$name将离开房间，在时间结束或排在其上面的人让其回来之前，都无法回来。';
  }

  @override
  String get membersUntilLetBack => '直到被放行';

  @override
  String membersLeaveTitle(String group) {
    return '退出“$group”？';
  }

  @override
  String get membersLeaveText =>
      '你将看不到它的房间，其他人也不会再在群组中看到你。群组会在没有你的情况下生成新密钥。若要回来，需要群组中有人再次同意你加入。';

  @override
  String membersLeaveTextFirst(String name) {
    return '你将看不到它的房间，其他人也不会再在群组中看到你。群组会在没有你的情况下生成新密钥。若要回来，需要群组中有人再次同意你加入。$name将排在第一位。';
  }

  @override
  String get groupSettingsTabGeneral => '常规';

  @override
  String get groupSettingsTabVoiceRooms => '语音房间';

  @override
  String get groupSettingsTabMembers => '成员';

  @override
  String get groupSettingsTabInvite => '邀请';

  @override
  String get groupSettingsTabLimits => '上限';

  @override
  String get groupSettingsTabActivity => '动态';

  @override
  String get groupSettingsFirstHere => '在本群组中排第一';

  @override
  String groupSettingsPlaceHere(String place) {
    return '在本群组中排$place';
  }

  @override
  String get groupSettingsLeave => '退出群组';

  @override
  String get groupSettingsInviteTitle => '邀请他人';

  @override
  String get groupSettingsClose => '关闭群组设置';

  @override
  String get groupSettingsPicture => '图片';

  @override
  String get groupSettingsChooseFromComputer => '从电脑中选择';

  @override
  String get groupSettingsUseIcon => '使用图标';

  @override
  String get groupSettingsPictureNote => 'PNG、JPEG或WebP。会像头像一样裁成正方形并缩小。';

  @override
  String get groupSettingsOrIcon => '或使用图标';

  @override
  String get groupSettingsBanner => '横幅';

  @override
  String get groupSettingsName => '名称';

  @override
  String get groupSettingsNameField => '群组名称';

  @override
  String get groupSettingsNameNote => '群组里的每个人都会看到新的名称和图片。';

  @override
  String get groupSettingsRoomsIntro =>
      '每个语音房间都有自己的成员、聊天、文件和通话：一个房间里的人不会与另一个房间里的人连接。每人同一时间只能在一个房间里。';

  @override
  String get groupSettingsRoomName => '房间名称';

  @override
  String get groupSettingsAccessEveryone => '所有人';

  @override
  String get groupSettingsAccessSome => '部分成员';

  @override
  String groupSettingsWhoCanEnter(String room) {
    return '谁可以进入$room';
  }

  @override
  String get groupSettingsFirstRoomStays => '第一个房间不能删除';

  @override
  String get groupSettingsPeopleInside => '房间里有人';

  @override
  String get groupSettingsDeleteRoom => '删除房间';

  @override
  String get groupSettingsAddRoom => '添加语音房间';

  @override
  String groupSettingsNewRoomName(int n) {
    return '房间$n';
  }

  @override
  String groupSettingsRoomsNote(int rooms, int people) {
    return '最多$rooms个房间，每个房间最多$people人（见“上限”）。有人的房间不能删除。';
  }

  @override
  String get groupSettingsKeptOut => '已移出';

  @override
  String get groupSettingsAccessEveryoneLong => '群组里的所有人';

  @override
  String get groupSettingsAccessChosen => '仅限我选择的成员';

  @override
  String get groupSettingsAccessNote => '你和排在你上面的人始终可以进入。未被选中的人会看到房间带锁。';

  @override
  String groupSettingsYou(String name) {
    return '$name（你）';
  }

  @override
  String groupSettingsKeptOutBy(String room, String by) {
    return ' · $room · 由$by操作';
  }

  @override
  String groupSettingsTimeLeft(String time) {
    return '剩余$time';
  }

  @override
  String get groupSettingsUntilLetBack => '直到被放行';

  @override
  String get groupSettingsLetBack => '放行';

  @override
  String groupSettingsManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '只有前$n人可以更改群组：名称、房间、邀请和上限。你可以管理排在你下面的人。',
      one: '只有排第一的人可以更改群组：名称、房间、邀请和上限。你可以管理排在你下面的人。',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsMembersCount(int n, int max) {
    String _temp0 = intl.Intl.pluralLogic(
      max,
      locale: localeName,
      other: '$max位成员',
    );
    return '$n/$_temp0，按顺序';
  }

  @override
  String get groupSettingsMembersNote =>
      '每个人都可以管理排在自己下面的人：将其上移到自己正下方或移到末尾、暂时移出房间，或移出群组。新加入的人从末尾开始。';

  @override
  String get groupSettingsWhoManages => '谁管理这个群组';

  @override
  String get groupSettingsManagersEveryone => '所有人';

  @override
  String groupSettingsManagersFirstN(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '前$n人',
      one: '排第一的人',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsManagersMore(int n, int members) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members',
    );
    return '设为前$n人，超过群组目前的$_temp0人：所以目前就是所有人。';
  }

  @override
  String get groupSettingsManagersEveryoneNote =>
      '管理者可以更改名称、图片、房间和上限，并同意新成员加入。选择所有人时，无论谁丢失了设备，群组都能继续运作。';

  @override
  String groupSettingsManagersFirstNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '只有前$n人可以更改名称、图片、房间和上限，并同意新成员加入。如果他们都丢失了设备，就再也没有人能管理这个群组：Fogata没有可以修复它的服务器。',
      one: '只有排第一的人可以更改名称、图片、房间和上限，并同意新成员加入。如果那台设备丢失，就再也没有人能管理这个群组：Fogata没有可以修复它的服务器。',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsMoveUp => '上移';

  @override
  String groupSettingsMoveUpTooltip(String name) {
    return '将$name上移到你正下方';
  }

  @override
  String get groupSettingsToEnd => '移到末尾';

  @override
  String groupSettingsToEndTooltip(String name) {
    return '将$name移到末尾';
  }

  @override
  String get groupSettingsTimeOut => '暂时移出房间';

  @override
  String get groupSettingsRemove => '移出群组';

  @override
  String get groupSettingsAskingToJoin => '请求加入';

  @override
  String get groupSettingsInviteFriend => '邀请好友';

  @override
  String get groupSettingsInviteFriendNote =>
      '他们会在Fogata中收到邀请，并自行决定是否加入。这里只显示你的好友。';

  @override
  String get groupSettingsInviteLink => '邀请链接';

  @override
  String groupSettingsInviteLinkNote(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': '群组中的任何人',
      'first': '群组中排第一的人',
      'other': '群组中前$n人之一',
    });
    return '给还不是你好友的人使用。仅可使用一次，有效期24小时，需由$_temp0同意加入：如果链接落到别人手里，你会看到是谁在请求，并可以拒绝。';
  }

  @override
  String get groupSettingsCopyLink => '复制邀请链接';

  @override
  String groupSettingsOpenLinks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '有$n个链接尚未使用。',
      zero: '没有待使用的链接。',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsCancelLinks => '全部作废';

  @override
  String get groupSettingsNoFriends => '还没有好友：请在“好友”中添加。';

  @override
  String get groupSettingsAllFriendsIn => '你的所有好友都已在群组中。';

  @override
  String get groupSettingsFriendOnline => '在线';

  @override
  String get groupSettingsFriendOffline => '当前不在线';

  @override
  String get groupSettingsInviteButton => '邀请';

  @override
  String get groupSettingsGroupFull => '群组已满';

  @override
  String get groupSettingsSayNo => '拒绝';

  @override
  String get groupSettingsLetIn => '同意';

  @override
  String get groupSettingsLimitGroup => '群组人数';

  @override
  String groupSettingsLimitGroupNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(n, locale: localeName, other: '$n人');
    return '2到20人。群组满员后，邀请和链接将失效。不能低于现有的$_temp0。';
  }

  @override
  String get groupSettingsLimitRoom => '每个语音房间的人数';

  @override
  String get groupSettingsLimitRoomNote =>
      '2到8人。语音和视频在房间里所有人之间直接传输，所以8人是保证音质的上限；打开摄像头时，人越少效果越好。房间满员后再来的人会看到房间已满。';

  @override
  String get shellChipCallMany => '直连 · 已加密 · 仅通话中的人';

  @override
  String get shellChipCallTwo => '直连 · 已加密 · 仅你们两人';

  @override
  String get shellGroupMembers => '群组成员';

  @override
  String get shellMembers => '成员';

  @override
  String get shellDirectMessages => '私信';

  @override
  String get shellPathDirect => '直连 · 已加密';

  @override
  String shellPathVia(String name) {
    return '经由$name · 已加密';
  }

  @override
  String get shellPathViaSomeone => '经由成员 · 已加密';

  @override
  String shellCallWith(String names) {
    return '与$names通话';
  }

  @override
  String get shellRinging => '呼叫中 · ';

  @override
  String get shellInCall => '通话中 · ';

  @override
  String get shellCallEnded => '通话已结束';

  @override
  String get shellNewGroup => '新建群组';

  @override
  String get shellSectionCall => '通话';

  @override
  String shellCallRinging(String name) {
    return '$name · 呼叫中';
  }

  @override
  String get shellSectionLiveNow => '进行中';

  @override
  String get shellSectionInCall => '通话中';

  @override
  String get shellChat => '聊天';

  @override
  String get shellFiles => '文件';

  @override
  String get shellSectionGroup => '群组';

  @override
  String get shellInvite => '邀请';

  @override
  String get shellSectionVoiceRooms => '语音房间';

  @override
  String shellVoiceRoomYouAreIn(String room) {
    return '$room · 你在这里';
  }

  @override
  String get shellNotForYou => '暂不可进入';

  @override
  String get shellOnlySomeMembers => '仅部分成员';

  @override
  String shellPersonYou(String name) {
    return '$name（你）';
  }

  @override
  String get shellLive => '直播';

  @override
  String get shellYourProfile => '你的个人资料';

  @override
  String shellMuteMic(String keys) {
    return '麦克风静音（$keys）';
  }

  @override
  String shellUnmuteMic(String keys) {
    return '取消麦克风静音（$keys）';
  }

  @override
  String get shellDeafen => '不再听到其他人';

  @override
  String get shellUndeafen => '重新听到其他人';

  @override
  String get shellStateCalling => '呼叫中…';

  @override
  String get shellStateInCall => '通话中';

  @override
  String get shellStateInRoom => '在房间中';

  @override
  String get shellStateOffline => '离线';

  @override
  String get shellStateOnline => '在线';

  @override
  String get shellStateAway => '暂离';

  @override
  String get shellShowInFolder => '在文件夹中显示';

  @override
  String get shellCopyPath => '复制路径';

  @override
  String shellVolumePercent(int n) {
    return '$n%';
  }

  @override
  String get shellMuteKeys => 'Ctrl+Shift+M';

  @override
  String get settingsSectionDonate => '捐助Fogata';

  @override
  String get settingsSectionReport => '报告问题';

  @override
  String get settingsHeadingHelp => '帮助';

  @override
  String get settingsPreRelease => '预发布版';

  @override
  String get donateTitle => '让Fogata保持独立';

  @override
  String get donateText => 'Fogata免费、无广告，也没有监视你的服务器。你的捐助让它一直如此。';

  @override
  String get donateButton => '捐助';

  @override
  String get donateNote => '将在浏览器中打开Ko-fi。Fogata永远看不到你的付款信息。';

  @override
  String get reportWhatTitle => '发生了什么？';

  @override
  String get reportWhatNote => '你当时在做什么、期望发生什么，以及实际看到了什么。';

  @override
  String get reportIncludeLog => '附上最近2天的技术日志';

  @override
  String get reportLogNote => '版本、错误和连接步骤。绝不包括你的消息、密钥、联系人或IP地址。';

  @override
  String get reportSeeSent => '查看发送内容';

  @override
  String get reportHideSent => '隐藏列表';

  @override
  String get reportPreview => '发送前阅读完整报告';

  @override
  String get reportHidePreview => '隐藏报告';

  @override
  String get reportPreviewNote =>
      '这正是文件里的全部内容，发送的也只有这些。名字、文件夹、文件名、地址和密钥已经从日志中移除。';

  @override
  String get reportPreviewWorking => '正在准备报告…';

  @override
  String get reportSentWhat => '你在上面写的内容。';

  @override
  String get reportSentVersion => 'Fogata的版本和构建号、你的系统及其版本，以及Fogata的界面语言。';

  @override
  String get reportSentLog =>
      '最近2天的日志：Fogata何时启动、出现的错误以及它如何连接其他设备。名字、消息、文件夹、文件名、链接、好友码和迁移码、密钥以及IP地址都会被移除，每台设备只显示为一个在报告之外没有意义的编号。';

  @override
  String get reportEmail => '通过邮件发送给Fogata团队';

  @override
  String get reportSave => '保存到文件';

  @override
  String reportHowEmail(String address) {
    return '你的邮件应用会打开，写好发给$address的邮件，旁边会打开报告文件所在的文件夹：附上文件并发送。在你发送之前，什么都不会离开你的电脑。';
  }

  @override
  String get reportReadyTitle => '邮件已准备好';

  @override
  String reportReadyText(String file, String address) {
    return '从打开的文件夹中附上$file，并发送到$address。';
  }

  @override
  String get reportShowFile => '显示文件';

  @override
  String get reportCopyAddress => '复制地址';

  @override
  String get reportAddressCopied => '已复制地址';

  @override
  String reportSaved(String file) {
    return '报告已保存：$file';
  }

  @override
  String reportNoMailApp(String address) {
    return '没有打开邮件应用。请用你的邮箱把文件发送到$address。';
  }

  @override
  String reportFailed(String reason) {
    return '无法写入报告：$reason';
  }

  @override
  String get splashStarting => '正在启动';

  @override
  String get splashConnecting => '正在连接';

  @override
  String get splashChecking => '正在检查更新';

  @override
  String get splashDownloading => '正在下载更新';

  @override
  String get splashInstalling => '正在安装更新';

  @override
  String get splashRestarting => '正在重启';

  @override
  String splashProgress(String done, String total) {
    return '$done / $total MB';
  }

  @override
  String splashProgressFrom(String done, String total, String name) {
    return '$done / $total MB · 来自$name';
  }

  @override
  String updateReady(String version) {
    return '更新已就绪 · $version';
  }

  @override
  String updateRequired(String version) {
    return '需要更新 · $version';
  }

  @override
  String get updateRestart => '重启';

  @override
  String updateReadyTip(String version) {
    return 'Fogata $version已就绪。重启即可使用。';
  }

  @override
  String get updateRequiredTip => '此版本已无法与好友的新版本通信。重启以更新。';

  @override
  String updateFailed(String reason) {
    return '无法安装更新：$reason';
  }

  @override
  String get errJoinInviteGone => '这个邀请已失效，或群组已满。';

  @override
  String get errJoinNoKeyYet => '这台设备还没有群组的密钥。一收到密钥，新成员就会加入。';

  @override
  String get errJoinWroteTooMuch => '这台设备做出的更改其他成员还没有同步。等其他成员在线后，才能同意新成员加入。';

  @override
  String get errJoinAskAgain => '这个加入请求是基于群组之前的状态发出的。对方需要重新申请。';

  @override
  String get errNoSpaceForFile => '没有足够的可用空间保存这个文件。请释放一些空间后重试。';

  @override
  String get recoveryTitle => '恢复副本';

  @override
  String get recoveryNote => '如果这台电脑的钥匙串或磁盘丢失，这个文件可以找回这个账号。只有你的恢复密钥能打开它。';

  @override
  String get recoveryOff => '已关闭';

  @override
  String recoveryOn(String id) {
    return '已开启 · 密钥$id';
  }

  @override
  String get recoveryTurnOn => '开启…';

  @override
  String get recoveryChangeKey => '更换密钥…';

  @override
  String get recoveryMakeCopy => '制作副本…';

  @override
  String get recoveryReplacedKept => '这台电脑上还保留着一个被恢复操作替换掉的账号。';

  @override
  String get recoveryDeleteReplaced => '删除…';

  @override
  String get recoveryChangeTitle => '要创建新的恢复密钥吗？';

  @override
  String get recoveryChangeText =>
      '新的副本只能用新密钥打开。之前做的副本仍然只能用旧密钥打开：只要还留着那些副本，就请保管好旧密钥。';

  @override
  String get recoveryChangeConfirm => '创建新密钥';

  @override
  String get recoveryKeyTitle => '你的恢复密钥';

  @override
  String get recoveryKeyText => '把它抄下来或打印出来，并和副本分开保管。它只会显示这一次。';

  @override
  String recoveryKeyName(String id) {
    return '简称：$id';
  }

  @override
  String get recoveryKeyWarning =>
      'Fogata不会在任何地方保存这个密钥：没有它，任何副本都打不开。同时拥有副本和这个密钥的人，就拥有你的账号。';

  @override
  String get recoveryKeyWritten => '我已抄下';

  @override
  String get recoveryCheckTitle => '核对你抄下的内容';

  @override
  String get recoveryCheckText => '按你抄下的样子，输入密钥的这两组。';

  @override
  String recoveryCheckGroup(int n) {
    return '第$n组';
  }

  @override
  String get recoveryCheckWrong => '不对：再看看你抄下的内容。';

  @override
  String get recoveryTurnOnButton => '开启副本';

  @override
  String recoveryTurnedOn(String id) {
    return '副本已开启，密钥为$id。';
  }

  @override
  String get recoveryCopyTitle => '制作恢复副本';

  @override
  String get recoveryCopyText =>
      '制作副本期间Fogata会离线，包含聊天中的文件时会更久。请把文件存放在这台电脑以外的地方，比如U盘。';

  @override
  String get recoveryCopyAttachments => '包括聊天中发送的文件';

  @override
  String recoveryCopyAttachmentsNote(String size) {
    return '图片、视频和文件：另需$size。';
  }

  @override
  String get recoveryCopySave => '选择位置…';

  @override
  String get recoveryCopyInDataFolder => '副本不能放在Fogata的数据文件夹里。请选择其他位置。';

  @override
  String recoveryCopyMade(String file, String size) {
    return '副本已保存：$file（$size）。';
  }

  @override
  String recoveryCopyFailed(String why) {
    return '无法制作副本。$why';
  }

  @override
  String get recoveryDeleteTitle => '要删除被替换的账号吗？';

  @override
  String get recoveryDeleteText => '恢复副本时，它被放到了一边。删除后将无法找回。';

  @override
  String get recoveryDelete => '删除';

  @override
  String get recoveryDeleted => '已删除被替换的账号。';

  @override
  String get recoveryFileType => 'Fogata副本';

  @override
  String get recoveryLostComputer => '电脑丢了？';

  @override
  String get recoveryRestoreLink => '从副本恢复';

  @override
  String get recoveryRestoreTitle => '从副本恢复';

  @override
  String get recoveryRestoreSub => '选择你保存的恢复副本，然后输入你的恢复密钥。';

  @override
  String get recoveryChooseCopy => '选择副本…';

  @override
  String recoveryMadeOn(String date) {
    return '这是$date制作的副本。';
  }

  @override
  String get recoveryOtherKey => '它是为另一个恢复密钥制作的，不是这台电脑上的那个。请输入制作它时用的密钥。';

  @override
  String get recoveryKeyField => '恢复密钥';

  @override
  String get recoveryKeyNotAKey => '恢复密钥由53个字母和数字组成，每四个一组。';

  @override
  String get recoveryKeyMistyped => '有一个字母或数字不对：请检查。';

  @override
  String get recoveryProblemNotACopy => '这不是Fogata账号的副本。';

  @override
  String get recoveryProblemNewer => '这个副本由更新版本的Fogata制作。请更新Fogata后再打开。';

  @override
  String get recoveryProblemOtherKey => '这个副本是为另一个恢复密钥制作的。';

  @override
  String get recoveryProblemDamaged => '这个副本已损坏，或在制作后被修改过。';

  @override
  String get recoveryProblemUnproven => '这个副本不是由其中的账号制作的，因此不会打开。';

  @override
  String recoveryWhose(String name) {
    return '$name的账号';
  }

  @override
  String recoveryHolds(int files, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files个文件',
    );
    return '$_temp0，共$size';
  }

  @override
  String get recoveryWithAttachments => '包含聊天中发送的文件。';

  @override
  String get recoveryWithoutAttachments => '不包含聊天中发送的文件。';

  @override
  String get recoveryOnlyIf =>
      '只有在其他电脑都不再使用这个账号时才使用副本。如果还有电脑在用，请改为从那台电脑迁移账号：一个账号不能同时在两台电脑上。';

  @override
  String get recoveryMoveInstead => '改为迁移';

  @override
  String recoveryHereNamed(String name) {
    return '这台电脑上有$name的账号。';
  }

  @override
  String recoveryHereDevice(String fingerprint) {
    return '这台电脑上有一个没有个人资料的账号（设备$fingerprint）。';
  }

  @override
  String get recoveryHereUnknown => '这台电脑上有一个打不开的账号的一部分。';

  @override
  String get recoveryHereReplaced =>
      '恢复会替换它。它会被放到一边而不是删除，直到你在“设置 › 存储和下载”中删除它。';

  @override
  String get recoveryRestore => '恢复';

  @override
  String get recoveryReplace => '替换并恢复';

  @override
  String recoveryRestoring(String name) {
    return '正在恢复$name的账号…';
  }

  @override
  String recoveryRestored(String name) {
    return '已恢复$name的账号。';
  }

  @override
  String get recoveryRefusedAccountHere => '这台电脑上现在有账号了。请重新选择副本，看看它会替换什么。';

  @override
  String get recoveryRefusedReplacedKept =>
      '这台电脑上还保留着一个被之前的恢复替换掉的账号。删除它后才能恢复这个副本。';

  @override
  String recoveryRefusedNoRoom(String need, String free) {
    return '这个副本在这个磁盘上需要$need，而可用空间只有$free。没有任何改动。';
  }

  @override
  String get recoveryRefusedInFolder => '副本在Fogata的数据文件夹里。请把它移到别处，然后重新选择。';

  @override
  String get recoveryBusy => 'Fogata正在处理恢复副本或迁移。完成后再试一次。';

  @override
  String get moveBusyWithCopy => 'Fogata正在处理恢复副本。完成后再迁移账号。';

  @override
  String get recoveryDeleteKeptAside => '删除被放到一边的账号…';

  @override
  String get reachOffline => 'Fogata在这里连不上互联网上的任何人。请检查网络连接，也可能是防火墙阻止了Fogata。';

  @override
  String get reachFirewall => '这台电脑的防火墙不允许任何连接进入Fogata。放行Fogata的命令在通知里。';

  @override
  String get reachUdpBlocked =>
      '这个网络或这台电脑阻止了UDP。通话和穿过路由器的直接连接都需要它；在放行之前，其他人只能通过群组成员或从公网地址连接到你。';

  @override
  String get reachSymmetricNat =>
      '你的路由器对每个目标分配不同的公网端口（对称型NAT），因此无法穿过它打开直接通路。位于其他路由器之后的好友只能通过你们共同群组的成员连接到你。';

  @override
  String get reachNoIpv6 => '也没有可用的IPv6来绕过路由器。';

  @override
  String reachFineFriend(String name) {
    return '你的网络对Fogata来说看起来正常，所以你们连不上的原因很可能在$name那边（不在线，或其网络）。';
  }

  @override
  String get reachFineOthers => '你的网络对Fogata来说看起来正常，所以你们连不上的原因很可能在对方那边。';
}
