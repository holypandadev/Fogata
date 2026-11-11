// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class L10nEn extends L10n {
  L10nEn([String locale = 'en']) : super(locale);

  @override
  String get commonCancel => 'Cancel';

  @override
  String get commonSave => 'Save';

  @override
  String get commonClose => 'Close';

  @override
  String commonNewMessageFrom(String name, String text) {
    return 'New message from $name: $text';
  }

  @override
  String commonNewMessagesFrom(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n new messages from $name',
      one: '1 new message from $name',
    );
    return '$_temp0';
  }

  @override
  String commonNewMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n new messages',
      one: '1 new message',
    );
    return '$_temp0';
  }

  @override
  String get commonAccept => 'Accept';

  @override
  String get commonDecline => 'Decline';

  @override
  String get commonSend => 'Send';

  @override
  String get commonTryAgain => 'Try again';

  @override
  String get commonCopy => 'Copy';

  @override
  String get commonCopied => 'Copied';

  @override
  String get commonBack => 'Back';

  @override
  String get commonDiscard => 'Discard';

  @override
  String get commonKeepEditing => 'Keep editing';

  @override
  String get commonNotNow => 'Not now';

  @override
  String get commonDownload => 'Download';

  @override
  String get commonSettings => 'Settings';

  @override
  String get commonHome => 'Home';

  @override
  String get commonFriends => 'Friends';

  @override
  String get commonContinue => 'Continue';

  @override
  String get commonDone => 'Done';

  @override
  String get commonRemove => 'Remove';

  @override
  String get commonYou => 'You';

  @override
  String timeYesterdayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'yesterday $time',
      'other': 'yesterday $time',
    });
    return '$_temp0';
  }

  @override
  String timeDayAt(String date, String time) {
    return '$date, $time';
  }

  @override
  String get timeNow => 'now';

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
      other: '$n days',
      one: '1 day',
    );
    return '$_temp0';
  }

  @override
  String spanMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minutes',
      one: '1 minute',
    );
    return '$_temp0';
  }

  @override
  String spanHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n hours',
      one: '1 hour',
    );
    return '$_temp0';
  }

  @override
  String spanDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n days',
      one: '1 day',
    );
    return '$_temp0';
  }

  @override
  String spanWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n weeks',
      one: '1 week',
    );
    return '$_temp0';
  }

  @override
  String get agoMoment => 'a moment ago';

  @override
  String agoMinutes(int n) {
    return '$n min ago';
  }

  @override
  String agoHours(int n) {
    return '$n h ago';
  }

  @override
  String get agoYesterday => 'yesterday';

  @override
  String agoDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n days ago',
    );
    return '$_temp0';
  }

  @override
  String agoWeeks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n weeks ago',
      one: '1 week ago',
    );
    return '$_temp0';
  }

  @override
  String agoMonths(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n months ago',
      one: '1 month ago',
    );
    return '$_temp0';
  }

  @override
  String get agoOverYear => 'over a year ago';

  @override
  String sizeBytes(int n) {
    return '$n B';
  }

  @override
  String sizeKB(String n) {
    return '$n KB';
  }

  @override
  String sizeMB(String n) {
    return '$n MB';
  }

  @override
  String sizeGB(String n) {
    return '$n GB';
  }

  @override
  String sizeTB(String n) {
    return '$n TB';
  }

  @override
  String listTwo(String first, String second) {
    return '$first and $second';
  }

  @override
  String listMore(String rest, String last) {
    return '$rest and $last';
  }

  @override
  String get listSeparator => ', ';

  @override
  String ordinal(int n) {
    return 'No. $n';
  }

  @override
  String get errLinkExpired => 'The link has expired. Ask for a new one.';

  @override
  String get errGroupInvite => 'That is a group invitation.';

  @override
  String get errDeviceNotFound =>
      'The other device was not found. Check that you are both online.';

  @override
  String get errNotFriendCode =>
      'That does not look like a Fogata link or code.';

  @override
  String get errAlreadyInGroup => 'You are already in that group.';

  @override
  String get errJoinRoomFirst => 'Enter the room first.';

  @override
  String get errNewerVersion =>
      'Your profile was saved by a newer version of Fogata. Update Fogata to open it.';

  @override
  String get errKeychainUnreachable =>
      'Fogata could not get the key to your profile from this computer\'s keychain. Unlock the keychain or allow Fogata to use it, then try again.';

  @override
  String get errKeychainNoKey =>
      'This computer\'s keychain no longer holds the key to your profile, so Fogata cannot open it.';

  @override
  String get errKeychainWrongKey =>
      'The key in this computer\'s keychain does not open your profile.';

  @override
  String get errNoGroupKey =>
      'This group is not ready on this computer yet. Try again in a moment.';

  @override
  String get errAlreadyRunning =>
      'Fogata is already open with this profile. Close the other one first.';

  @override
  String get errFileGone => 'That file is no longer in the room.';

  @override
  String get errNobodyHasFile => 'Nobody online has that file right now.';

  @override
  String get errStillStarting => 'Fogata is still starting.';

  @override
  String get errIdentityExists =>
      'This computer already has an identity; its key cannot change.';

  @override
  String get errNameRule => 'Write a name of 1 to 40 visible characters.';

  @override
  String get errBioRule =>
      'A bio has at most 190 characters on 4 lines, and no hidden characters.';

  @override
  String get errPictureRule =>
      'The picture must be a PNG or JPEG of 64 KB at most.';

  @override
  String get errNotFriendLink => 'That is not a Fogata friend link.';

  @override
  String get errLinkDamaged => 'The link is incomplete or damaged.';

  @override
  String get errLinkNeedsNewer => 'That link needs a newer version of Fogata.';

  @override
  String get errCodeShape => 'A code looks like FGT-XXXXX-XXXXX.';

  @override
  String get errCodeWrong => 'The code is wrong or no longer works.';

  @override
  String get errNotProven => 'The other device could not prove who it is.';

  @override
  String get errCallGone => 'That call is no longer ringing.';

  @override
  String get errAlreadyInCall => 'You are already in a call. Hang up first.';

  @override
  String get errAlreadyFriends => 'You are already friends.';

  @override
  String get errMemberNotConnected =>
      'That member is not connected right now. Try again when they are online.';

  @override
  String get errMemberTooOld =>
      'That member’s copy of Fogata is too old to take friend requests from a group. Send them a friend link instead.';

  @override
  String get errMemberAskLost =>
      'The friend request did not reach that member. Try again.';

  @override
  String get errOwnLink =>
      'That link is your own: send it to the friend you want to add.';

  @override
  String get errLinkUsed =>
      'That link was already used or has expired. Ask for a new one.';

  @override
  String get errLinkExpiredUnreached =>
      'The link expired before its device could be reached. Ask for a new one.';

  @override
  String get errCodeUnanswered =>
      'No device answered the code. Check it, and that the other device still shows it.';

  @override
  String get errRefused =>
      'The other device said no. The code may have been used already.';

  @override
  String get errNobodyLetIn =>
      'Nobody let you in. Try again when someone in the group is online.';

  @override
  String get errNotBelow =>
      'You can only do that to someone below you in the group.';

  @override
  String get errNotManager => 'Only those who manage the group can do that.';

  @override
  String get errLastManager =>
      'You can’t take yourself off the group’s managers.';

  @override
  String get errRoomNotEmpty =>
      'People are in that voice room: remove it once it is empty.';

  @override
  String get errTooManyFiles =>
      'You already share the most files one person can in a room. Stop sharing some first.';

  @override
  String get errDiskFull => 'The disk is full. Free some space and try again.';

  @override
  String get errTooManyChanges =>
      'This device has made too many changes to the group that no other member has taken in yet. Try again after someone else in the group makes a change.';

  @override
  String get errSomething => 'Something went wrong.';

  @override
  String errRoomFull(int n) {
    return 'That voice room is full: it takes $n people at most.';
  }

  @override
  String get findingOffline =>
      'No peer of the public network answered: this computer does not seem to reach the internet, or a firewall blocks Fogata.';

  @override
  String get findingUdpWorks =>
      'UDP works. Calls and getting through routers need it.';

  @override
  String get findingUdpBlocked =>
      'This network or this computer seems to block UDP (peers that answered over TCP did not over QUIC). Chat can still go through other members, but direct connections through routers and calls will mostly fail. Check the firewall or the network rules.';

  @override
  String get findingCarrierNat =>
      'This computer has a carrier-grade NAT address (100.64.0.0/10): the provider shares one public address among many customers.';

  @override
  String get findingPublicIpv4 =>
      'This computer has a public IPv4 address: others can reach it directly.';

  @override
  String get findingHardNat =>
      'Behind a router that gives a different public port to each destination (a hard NAT).';

  @override
  String get findingEasyNat =>
      'Behind a router that keeps the same public port for every destination (an easy NAT): it can be opened from both sides at once.';

  @override
  String get findingUnknownRouter =>
      'Too few peers have seen this computer’s public address to tell what kind of router there is.';

  @override
  String get findingNoIpv6 =>
      'No public IPv6 address. Fogata does not need it, but it makes direct connections easier.';

  @override
  String get findingIpv6Works =>
      'Public IPv6 works: friends with IPv6 connect with no router in the way.';

  @override
  String get findingIpv6Unreached =>
      'This computer has a public IPv6 address, but no peer has reached it over IPv6 yet.';

  @override
  String get findingSummaryNoUdp =>
      'In short: expect trouble with direct connections until UDP is allowed.';

  @override
  String get findingSummaryPublic =>
      'In short: your friends should be able to reach this computer directly.';

  @override
  String get findingSummaryEasyNat =>
      'In short: direct connections should work with most of your friends, opening both routers at once; with a friend behind a hard NAT, a group member in between is needed.';

  @override
  String get findingSummaryHardNatWithIpv6 =>
      'In short: friends with IPv6 or a public address connect directly; with the others, a group member in between is needed.';

  @override
  String get findingSummaryHardNat =>
      'In short: only friends with a public address connect directly; with the others, a group member in between is needed.';

  @override
  String get findingSummaryUnknown =>
      'In short: not enough information; check again in a minute.';

  @override
  String findingInternet(int n) {
    return 'Internet: this computer reaches the public network ($n peers known).';
  }

  @override
  String findingDns(String detail) {
    return 'DNS: the system settings could not be read as a whole ($detail)';
  }

  @override
  String findingLanDevices(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n Fogata devices',
      one: '1 Fogata device',
    );
    return '$_temp0 on this local network: friends here connect without going through the internet.';
  }

  @override
  String findingFirewall(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'in PowerShell as administrator',
      'macos': 'in Terminal',
      'other': 'in a terminal',
    });
    return 'This computer’s firewall lets nothing in for Fogata, so a friend in the same place behind another router (two routers at home, for example) cannot reach it. Run this command once $_temp0 to let Fogata in:';
  }

  @override
  String get settingsLanguage => 'Language';

  @override
  String get settingsLanguageNote =>
      'The language of Fogata’s own words. Messages stay as they were written.';

  @override
  String get settingsLanguageChoose => 'Choose the language';

  @override
  String get settingsTime => 'Time';

  @override
  String get settingsTimeNote =>
      'How times of day show, in the chat, calls and lists.';

  @override
  String settingsTime24(String example) {
    return '24 hours · $example';
  }

  @override
  String settingsTime12(String example) {
    return '12 hours · $example';
  }

  @override
  String firewallNoticeText(String system, String command) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'in PowerShell as administrator',
      'macos': 'in Terminal',
      'other': 'in a terminal',
    });
    return 'A friend in the same place behind another router cannot reach this computer. Run once $_temp0: $command';
  }

  @override
  String firewallCopied(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'windows': 'in PowerShell as administrator',
      'macos': 'in Terminal',
      'other': 'in a terminal',
    });
    return 'Copied. Paste it $_temp0 and run it.';
  }

  @override
  String get appRestarted => 'Fogata stopped working and started again.';

  @override
  String appVoiceRoomInGroup(String room, String group) {
    return '$room in $group';
  }

  @override
  String appBothIn(String room) {
    return 'You’re both in $room.';
  }

  @override
  String appHungUp(String name) {
    return '$name hung up';
  }

  @override
  String get appYourFriend => 'Your friend';

  @override
  String get appCallEndedAlone => 'Nobody came back, so the call ended';

  @override
  String get appAnsweredElsewhere => 'Answered on another computer';

  @override
  String appVoiceRoomDeleted(String room) {
    return '“$room” was deleted.';
  }

  @override
  String get appSomeone => 'Someone';

  @override
  String get appAGroup => 'a group';

  @override
  String appFriendRequest(String name) {
    return '$name wants to be your friend.';
  }

  @override
  String appRequestDelivered(String name) {
    return 'Request delivered to $name. It waits for their answer.';
  }

  @override
  String appFriendsNow(String name) {
    return 'You and $name are friends now.';
  }

  @override
  String appJoinedGroup(String group) {
    return 'You are in “$group” now.';
  }

  @override
  String appRemovedFromGroup(String group) {
    return 'You are no longer in “$group”.';
  }

  @override
  String appAddedToGroup(String name, String group) {
    return '$name added you to “$group”. Join it or not: your call.';
  }

  @override
  String appAsksToJoin(String name, String group) {
    return '$name asks to join “$group”.';
  }

  @override
  String appJoinWaiting(String group) {
    return 'Waiting for someone in “$group” to let you in.';
  }

  @override
  String appJoinDeclined(String group) {
    return 'Someone in “$group” said no. That invitation no longer works.';
  }

  @override
  String appDownloadedTo(String path) {
    return 'Downloaded to $path';
  }

  @override
  String appDownloadFailed(String why) {
    return 'Could not download: $why';
  }

  @override
  String get appOtherSession =>
      'This identity is open on another computer too. Close Fogata there: one identity works on one computer at a time.';

  @override
  String get appFriendsRestored =>
      'Your list of friends was damaged on this computer and was restored from the copy made before its last change. A friend added or removed just before may need adding or removing again.';

  @override
  String appFilesRefused(String folder) {
    return 'Some of Fogata’s files on this computer were found unencrypted where they are kept encrypted, so they were not used. Nothing was deleted: they are kept in $folder.';
  }

  @override
  String appInvitationRanOut(String group) {
    return 'The invitation to “$group” ran out before someone in the group let you in. Ask for a new one.';
  }

  @override
  String appTakenOut(String who, String room) {
    return '$who took you out of $room.';
  }

  @override
  String appRoomForSome(String room) {
    return '$room is for some members only now.';
  }

  @override
  String appTakenOutUntilLetBack(String who, String room) {
    return '$who took you out of $room until someone above you lets you back.';
  }

  @override
  String appTakenOutFor(String who, String room, String span) {
    return '$who took you out of $room for $span.';
  }

  @override
  String appTakenOutWhile(String who, String room) {
    return '$who took you out of $room for a while.';
  }

  @override
  String appComeBackIn(String room, String wait) {
    return 'You can come back to $room in $wait.';
  }

  @override
  String appRoomNotForYou(String room) {
    return '$room is not for you now.';
  }

  @override
  String appFoldersNotShared(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Folders can’t be shared, only files: $n folders were left out.',
      one: 'Folders can’t be shared, only files: the folder was left out.',
    );
    return '$_temp0';
  }

  @override
  String appSharingOne(String file) {
    return 'Sharing $file from this computer. It stops being available when you leave the room.';
  }

  @override
  String appSharingMany(int n) {
    return 'Sharing $n files from this computer. They stop being available when you leave the room.';
  }

  @override
  String appSavedTo(String path) {
    return 'Saved to $path';
  }

  @override
  String appAskingToJoin(String group) {
    return 'Asking to join “$group”. Someone in the group lets you in when they see it.';
  }

  @override
  String get appLookingForDevice =>
      'Looking for the other device to send your request…';

  @override
  String callStateNoAnswer(String name) {
    return '$name didn’t answer';
  }

  @override
  String callStateDeclined(String name) {
    return '$name can’t talk right now';
  }

  @override
  String callStateBusy(String name) {
    return '$name is in another call';
  }

  @override
  String callStateOutdated(String name) {
    return '$name needs a newer Fogata to join';
  }

  @override
  String callStateUnreached(String name) {
    return '$name couldn’t be reached';
  }

  @override
  String get callStateReturn => 'Return to call';

  @override
  String get callStateCallBack => 'Call back';

  @override
  String get callStateCall => 'Call';

  @override
  String noticesDeclined(String name, String group) {
    return '$name declined the invitation to “$group”';
  }

  @override
  String noticesDeclinedToast(String name, String group) {
    return '$name declined the invitation to “$group”.';
  }

  @override
  String noticesMissedInCall(String name) {
    return '$name called while you were in a call';
  }

  @override
  String noticesMissedOffline(String name) {
    return '$name called while you appeared offline';
  }

  @override
  String noticesMissedCall(String name) {
    return 'Missed call from $name';
  }

  @override
  String get contactsGeneral => 'General';

  @override
  String get directStateTimerOff => 'Off';

  @override
  String get directStateTimer24h => '24 hours';

  @override
  String directStateTimerDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n days',
      one: '1 day',
    );
    return '$_temp0';
  }

  @override
  String directStateDisappearsMinutes(int n) {
    return 'Disappears in $n min';
  }

  @override
  String directStateDisappearsHours(int n) {
    return 'Disappears in $n h';
  }

  @override
  String directStateDisappearsDays(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Disappears in $n days',
    );
    return '$_temp0';
  }

  @override
  String get directStateNoLimit => 'No limit';

  @override
  String get directStateMessageGone => 'That message is no longer here.';

  @override
  String directStateFoldersNotSent(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Folders can’t be sent, only files: $n folders were left out.',
      one: 'Folders can’t be sent, only files: the folder was left out.',
    );
    return '$_temp0';
  }

  @override
  String directStateFileFailed(String why) {
    return 'Could not get the file: $why';
  }

  @override
  String movingTooLarge(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Animated banners can be up to $most pixels; this one is $size.',
      'other':
          'Animated pictures can be up to $most pixels; this one is $size.',
    });
    return '$_temp0';
  }

  @override
  String movingTooLong(String what, String most, String count) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners':
          'Animated banners can have up to $most frames; this one has $count.',
      'other':
          'Animated pictures can have up to $most frames; this one has $count.',
    });
    return '$_temp0';
  }

  @override
  String movingTooHeavy(String what, String most, String size) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'banners': 'Animated banners can be up to $most; this one is $size.',
      'other': 'Animated pictures can be up to $most; this one is $size.',
    });
    return '$_temp0';
  }

  @override
  String get namesHidden =>
      'This name has hidden characters. Type it again without them.';

  @override
  String get namesBlank =>
      'Nothing in this name shows. Use letters, numbers or emoji.';

  @override
  String get namesDefaultBio => 'Hey there! I’m on Fogata.';

  @override
  String get namesBioHidden =>
      'This bio has hidden characters. Type it again without them.';

  @override
  String namesBioLines(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'A bio has $n lines at most.',
      one: 'A bio has 1 line at most.',
    );
    return '$_temp0';
  }

  @override
  String namesBioChars(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'A bio has $n characters at most.',
      one: 'A bio has 1 character at most.',
    );
    return '$_temp0';
  }

  @override
  String get restartFailed =>
      'Fogata could not start again. Quit it and open it yourself.';

  @override
  String typingOne(String name) {
    return '$name is typing…';
  }

  @override
  String typingTwo(String first, String second) {
    return '$first and $second are typing…';
  }

  @override
  String get typingSeveral => 'Several people are typing…';

  @override
  String get outgoingTheRoom => 'the room';

  @override
  String outgoingDropTitle(String group) {
    return 'Drop to share in $group';
  }

  @override
  String get outgoingDropNote =>
      'You can add a message and check them before they go. The others download them from this computer while you are in the room.';

  @override
  String outgoingPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n hidden or private files were left out: files like these are not added by dragging, since a web page can start a drag. To share them, pick them with the paperclip.',
      one: 'A hidden or private file was left out: files like it are not added by dragging, since a web page can start a drag. To share it, pick it with the paperclip.',
    );
    return '$_temp0';
  }

  @override
  String outgoingToShare(int n, String count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$count files to share · $size',
      one: '$count file to share · $size',
    );
    return '$_temp0';
  }

  @override
  String get outgoingRemoveAll => 'Remove all';

  @override
  String outgoingRemoveFile(String file) {
    return 'Remove $file';
  }

  @override
  String get windowTrayOpen => 'Open Fogata';

  @override
  String get windowTrayQuit => 'Quit Fogata';

  @override
  String get windowMinimize => 'Minimize';

  @override
  String get windowRestore => 'Restore';

  @override
  String get windowMaximize => 'Maximize';

  @override
  String get autostartReason => 'Open Fogata when you sign in.';

  @override
  String get devicesMicrophone => 'Microphone';

  @override
  String get devicesSpeaker => 'Speaker';

  @override
  String get devicesCamera => 'Camera';

  @override
  String get devicesChooseMicrophone => 'Choose microphone';

  @override
  String get devicesChooseSpeaker => 'Choose speaker';

  @override
  String get devicesChooseCamera => 'Choose camera';

  @override
  String get devicesNoCamera => 'No camera is connected.';

  @override
  String get devicesCameraInCall => 'The camera is on in your call.';

  @override
  String devicesCameraFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Could not use the camera. Allow it in System Settings › Privacy & Security.',
      'other': 'Could not use the camera.',
    });
    return '$_temp0';
  }

  @override
  String get devicesCameraDenied =>
      'The system does not let Fogata use the camera. Allow it in the system settings, then try again.';

  @override
  String get devicesCameraUnavailable =>
      'Could not reach the camera: this system has no camera portal.';

  @override
  String get devicesPreviewOnlyYou => 'Only you see the preview.';

  @override
  String get devicesStopVideo => 'Stop video';

  @override
  String get devicesTestVideo => 'Test video';

  @override
  String get devicesNoiseOn => 'Noise suppression on';

  @override
  String get devicesNoiseOff => 'Noise suppression off';

  @override
  String get devicesNoise => 'Noise suppression';

  @override
  String get devicesNoiseNote =>
      'Lowers steady background noise, like a fan or a hum, in what the others hear.';

  @override
  String get devicesNoiseShort => 'Lowers a steady fan or hum.';

  @override
  String get devicesMirror => 'Mirror my camera';

  @override
  String get devicesMirrorNote =>
      'Only how you see yourself here. Others always see your camera as it is.';

  @override
  String get devicesMirrorOn => 'Mirror my camera on';

  @override
  String get devicesMirrorOff => 'Mirror my camera off';

  @override
  String get devicesTestMic => 'Test microphone';

  @override
  String get devicesStopMicTest => 'Stop test';

  @override
  String get devicesMicLevel => 'Microphone level';

  @override
  String get devicesHearMyself => 'Hear myself';

  @override
  String get devicesHearMyselfNote =>
      'Plays your microphone through the speaker. Use headphones, or the microphone may pick up the speaker.';

  @override
  String get devicesHearMyselfOn => 'Hear myself on';

  @override
  String get devicesHearMyselfOff => 'Hear myself off';

  @override
  String get devicesMicTestOnlyYou =>
      'Nothing from the test is sent to anyone.';

  @override
  String get devicesMicInCall => 'The microphone is in use in your call.';

  @override
  String get devicesNoMic => 'No microphone is connected.';

  @override
  String get devicesSystemDefault => 'System default';

  @override
  String get devicesNotConnected => 'Not connected: the default is used';

  @override
  String get devicesNoCameraShort => 'No camera is connected';

  @override
  String get devicesMicOptions => 'Microphone options';

  @override
  String get devicesSpeakerOptions => 'Speaker options';

  @override
  String get devicesCameraOptions => 'Camera options';

  @override
  String get devicesInputDevice => 'Input device';

  @override
  String get devicesOutputDevice => 'Output device';

  @override
  String get devicesChosenMissing =>
      'The one chosen is not connected: the default is used.';

  @override
  String get devicesVoiceSettings => 'Voice & video settings';

  @override
  String get photoImages => 'Images';

  @override
  String photoTooLarge(int n) {
    return 'That picture is too large: at most $n megapixels.';
  }

  @override
  String get photoUnreadable => 'That picture could not be read.';

  @override
  String get callSource => 'Source';

  @override
  String callQuality(String resolution, int fps) {
    return '$resolution · $fps fps';
  }

  @override
  String get callSetupFailed => 'Could not set the call up.';

  @override
  String get callNoSoundSystem =>
      'Could not reach this computer\'s sound system, so the call cannot start.';

  @override
  String callMicGone(String name) {
    return 'Your microphone was disconnected: using $name now.';
  }

  @override
  String get callMicBack => 'Your microphone is back.';

  @override
  String callMicSilent(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Could not use the microphone. Check that it is connected and allowed in System Settings › Privacy & Security.',
      'windows': 'Could not use the microphone. Check that it is connected and allowed in Settings › Privacy & security › Microphone.',
      'other': 'Could not use the microphone. Check that it is connected.',
    });
    return '$_temp0';
  }

  @override
  String get callNoMicrophone => 'No microphone is connected.';

  @override
  String callMicFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Could not use the microphone. Allow it in System Settings › Privacy & Security.',
      'windows': 'Could not use the microphone. Allow it in Settings › Privacy & security › Microphone.',
      'other': 'Could not use the microphone.',
    });
    return '$_temp0';
  }

  @override
  String callScreenFailed(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Could not share the screen. Allow it in System Settings › Privacy & Security.',
      'other': 'Could not share the screen.',
    });
    return '$_temp0';
  }

  @override
  String get callShareLost =>
      'Screen sharing stopped: what you were sharing can no longer be captured.';

  @override
  String callKeptOut(String name) {
    return 'Lost the connection with $name.';
  }

  @override
  String get qualityGood => 'Good';

  @override
  String get qualityOkay => 'Okay';

  @override
  String get qualityPoor => 'Poor';

  @override
  String get qualityLost => 'Lost';

  @override
  String qualityDelay(int ms) {
    return '$ms ms';
  }

  @override
  String qualityPercent(String value) {
    return '$value %';
  }

  @override
  String get qualityAnotherMember => 'another member';

  @override
  String qualityLostHint(String name, int seconds) {
    return 'Nothing has come from $name for $seconds s. Fogata keeps looking for them; they come back on their own as soon as their network does.';
  }

  @override
  String qualityBothPoor(String name) {
    return 'Both networks are poor right now: you and $name may sound choppy to each other.';
  }

  @override
  String qualityTheirSide(String name) {
    return 'The trouble is on $name’s side: they sound choppy to everyone. Their Wi-Fi or mobile data is poor right now.';
  }

  @override
  String qualityYourSideWith(String name) {
    return 'The trouble is on your side: you may sound choppy to $name and the others. Your Wi-Fi or mobile data is poor right now.';
  }

  @override
  String qualityTheirsSlow(String name) {
    return '$name’s network is a bit slow right now: they may sound choppy to everyone now and then.';
  }

  @override
  String qualityAllGoodWith(String name) {
    return '$name hears you and you hear them without trouble.';
  }

  @override
  String qualityMineSlowWith(String name) {
    return 'Your network is a bit slow right now: you may sound choppy to $name now and then.';
  }

  @override
  String qualityWayThrough(String name, String carrier) {
    return '$name’s own network is fine; the way between you two is slower because it goes through $carrier. They may sound choppy to you now and then; the others hear them well.';
  }

  @override
  String qualityWaySlowFine(String name) {
    return '$name’s own network is fine; the way between you two is slow right now. They may sound choppy to you now and then; the others hear them well.';
  }

  @override
  String qualityWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'The way between you and $name is a bit slow right now; it may be either network. You may sound choppy to each other now and then.',
      'other':
          'The way between you and $name is poor right now; it may be either network. You may sound choppy to each other.',
    });
    return '$_temp0';
  }

  @override
  String qualityWayVia(String level, String name, String carrier) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay':
          'The way between you and $name is a bit slow right now; it may be either network. You may sound choppy to each other now and then. It goes through $carrier.',
      'other':
          'The way between you and $name is poor right now; it may be either network. You may sound choppy to each other. It goes through $carrier.',
    });
    return '$_temp0';
  }

  @override
  String get qualityYourSide =>
      'The trouble is on your side: you may sound choppy to the others. Your Wi-Fi or mobile data is poor right now.';

  @override
  String get qualityMineSlow =>
      'Your network is a bit slow right now: you may sound choppy to the others now and then.';

  @override
  String qualityLostShort(String name, int seconds) {
    return 'Nothing has come from $name for $seconds s. Fogata keeps looking for them.';
  }

  @override
  String qualityAllWellWith(String name) {
    return 'All is well: you and $name hear each other without trouble.';
  }

  @override
  String qualityNoteDropped(String name) {
    return '$name dropped and Fogata is looking for them again';
  }

  @override
  String qualityNoteTheirsPoor(String name) {
    return '$name’s network is poor, so they sound choppy to everyone';
  }

  @override
  String qualityNoteTheirsSlow(String name) {
    return '$name’s network is a bit slow';
  }

  @override
  String qualityNoteThrough(String name, String carrier) {
    return '$name comes to you through $carrier';
  }

  @override
  String qualityNoteWay(String level, String name) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'okay': 'the way to $name is a bit slow right now',
      'other': 'the way to $name is poor right now',
    });
    return '$_temp0';
  }

  @override
  String get qualityNoteSeparator => '; ';

  @override
  String get qualityAllWell =>
      'All is well: you hear everyone and everyone hears you.';

  @override
  String qualityAllWellMine(String notes) {
    return 'All is well on your side. $notes.';
  }

  @override
  String qualityYourDetails(String level) {
    return 'Your connection: $level. Show details';
  }

  @override
  String qualityTheirDetails(String name, String level) {
    return '$name’s connection: $level. Show details';
  }

  @override
  String get qualityReconnecting => 'Reconnecting…';

  @override
  String get qualityReconnectingLabel => 'Reconnecting';

  @override
  String qualityLevelConnection(String level) {
    String _temp0 = intl.Intl.selectLogic(level, {
      'good': 'Good connection',
      'okay': 'Okay connection',
      'weak': 'Poor connection',
      'other': 'Lost connection',
    });
    return '$_temp0';
  }

  @override
  String get qualityPoorConnection => 'Poor connection';

  @override
  String get qualityMinePoorBanner =>
      'Your network is poor: you may sound choppy to others';

  @override
  String get qualityYourNetwork => 'Your network';

  @override
  String qualityTo(String name) {
    return 'To $name';
  }

  @override
  String qualityBetween(String name) {
    return 'Between you and $name';
  }

  @override
  String qualityTheirNetwork(String name) {
    return '$name’s own network';
  }

  @override
  String get qualityLastHeard => 'Last heard';

  @override
  String qualitySecondsAgo(int n) {
    return '$n s ago';
  }

  @override
  String get qualityPath => 'Path';

  @override
  String get qualityLookingAgain => 'Looking for them again';

  @override
  String get qualityDelayLabel => 'Delay';

  @override
  String get qualityLostPackets => 'Lost packets';

  @override
  String get qualityYourConnection => 'Your connection';

  @override
  String qualityTheirConnection(String name) {
    return '$name’s connection';
  }

  @override
  String roomCountFullLabel(int count, int max) {
    return 'Full, $count of $max people';
  }

  @override
  String roomCountLabel(int count, int max) {
    return '$count of $max people';
  }

  @override
  String get roomCountFullPrefix => 'Full · ';

  @override
  String get roomCountFullWord => 'Full ';

  @override
  String get volumeVoice => 'Voice volume';

  @override
  String get volumeOnlyYouHear => 'Only you hear these changes';

  @override
  String get volumeScreenSound => 'Screen sound';

  @override
  String volumePercent(int n) {
    return '$n%';
  }

  @override
  String get volumeMuteVoiceForMe => 'Mute voice for me';

  @override
  String get volumeMuteForMe => 'Mute for me';

  @override
  String get volumeMuteScreenForMe => 'Mute screen sound for me';

  @override
  String get volumeBackTo100 => 'Back to 100%';

  @override
  String get volumeBelowYou => 'BELOW YOU';

  @override
  String volumeRemoveFrom(String room) {
    return 'Remove from $room';
  }

  @override
  String volumeRemoveTitle(String name, String room) {
    return 'Remove $name from $room?';
  }

  @override
  String volumeRemoveText(String name, String room) {
    return '$name leaves $room now and can come back in at once. A time out keeps someone out for a while.';
  }

  @override
  String volumeTimeOutOf(String room) {
    return 'Time out of $room';
  }

  @override
  String volumeTimeOutText(String name, String room) {
    return '$name leaves $room and cannot come back for:';
  }

  @override
  String get volumeUntilLetBack => 'Until someone above them lets them back';

  @override
  String get pipYourScreen => 'Your screen';

  @override
  String pipScreenOf(String name) {
    return '$name’s screen';
  }

  @override
  String get pipYourCamera => 'Your camera';

  @override
  String get pipBackToCall => 'Back to the call';

  @override
  String get pipBackToRoom => 'Back to the room';

  @override
  String get pipStopWatching => 'Stop watching';

  @override
  String get addToCallTitle => 'Add to the call';

  @override
  String addToCallNote(int n) {
    return 'Friends who are online. Up to $n people in a call.';
  }

  @override
  String get addToCallSearch => 'Search friends';

  @override
  String get addToCallNoFriends => 'No friends yet.';

  @override
  String get addToCallNoMatch => 'No friend by that name.';

  @override
  String get addToCallInCall => 'In the call';

  @override
  String get addToCallCalling => 'Calling…';

  @override
  String get addToCallOffline => 'Offline';

  @override
  String get addToCallCall => 'Call';

  @override
  String get streamQualityTitle => 'Stream quality';

  @override
  String get streamQualityNoteSharing =>
      'How your screen reaches the others. A change applies at once.';

  @override
  String get streamQualityNote =>
      'How your screen reaches the others when you share it.';

  @override
  String get streamQualityResolution => 'Resolution';

  @override
  String streamQualitySourceLines(int lines) {
    return 'Source (${lines}p)';
  }

  @override
  String get streamQualitySource => 'Source';

  @override
  String get streamQualityFrameRate => 'Frame rate';

  @override
  String streamQualityFps(int n) {
    return '$n fps';
  }

  @override
  String get streamQualityHigherNote =>
      'Higher settings take more of your upload and your computer.';

  @override
  String get streamQualitySomeone => 'Someone';

  @override
  String streamQualityLoweredOne(String name, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' The others get what you chose.',
      'other': '',
    });
    return '$name’s connection can’t take more right now, so they get a smaller picture.$_temp0';
  }

  @override
  String streamQualityLoweredMany(String names, String others) {
    String _temp0 = intl.Intl.selectLogic(others, {
      'yes': ' The others get what you chose.',
      'other': '',
    });
    return 'Some connections can’t take more right now: $names get a smaller picture.$_temp0';
  }

  @override
  String get streamQualityGoingOut => 'GOING OUT NOW';

  @override
  String get streamQualityNobody => 'Nobody is watching yet.';

  @override
  String streamQualityNow(int height, int fps) {
    return '${height}p · $fps fps';
  }

  @override
  String callLayersIsCalling(String name) {
    return '$name is calling';
  }

  @override
  String get callLayersGoToCall => 'F6 goes to the call';

  @override
  String callLayersWith(String names) {
    return 'With $names';
  }

  @override
  String get callLayersDirectCall => 'Direct call · ';

  @override
  String get callLayersAnswerMicOff => 'Answer with the microphone off';

  @override
  String callLayersAnsweringLeaves(String room) {
    return 'Answering leaves $room';
  }

  @override
  String get callLayersLeaveAndAnswer => 'Leave and answer';

  @override
  String callLayersBothIn(String room) {
    return 'You’re both in $room.';
  }

  @override
  String get callLayersDismiss => 'Dismiss';

  @override
  String callLayersCallWith(String names) {
    return 'Call with $names';
  }

  @override
  String get callLayersRoom => 'Room';

  @override
  String callLayersBackTo(String title) {
    return 'Back to $title';
  }

  @override
  String callLayersRinging(String time) {
    return 'Ringing · $time';
  }

  @override
  String callLayersInCall(String time) {
    return 'In call · $time';
  }

  @override
  String callLayersOnlyYou(String time) {
    return 'Only you · $time';
  }

  @override
  String callLayersInRoom(int n, String time) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n in the room',
    );
    return '$_temp0 · $time';
  }

  @override
  String get callLayersMute => 'Mute';

  @override
  String get callLayersUnmute => 'Unmute';

  @override
  String get callLayersHangUp => 'Hang up';

  @override
  String get callLayersLeaveRoom => 'Leave the room';

  @override
  String roomDeclineCallTitle(String name) {
    return 'Decline $name’s call?';
  }

  @override
  String roomDeclineCallText(String caller, String callee) {
    return '$caller is calling you. You can be in one call at a time: calling $callee declines it.';
  }

  @override
  String roomDeclineCallTextSomeone(String caller) {
    return '$caller is calling you. You can be in one call at a time: calling someone else declines it.';
  }

  @override
  String get roomDeclineAndCall => 'Decline and call';

  @override
  String roomHangUpTitle(String name) {
    return 'Hang up the call with $name?';
  }

  @override
  String get roomHangUpText =>
      'You can be in one call or room at a time: going on ends the call.';

  @override
  String get roomHangUp => 'Hang up';

  @override
  String get roomCloseTitle => 'Leave and close the room?';

  @override
  String get roomCloseText =>
      'You are the last one here. When you leave, the room closes and what was said is gone from this computer; what others downloaded stays on theirs.';

  @override
  String get roomCloseChatMessages => 'Chat messages';

  @override
  String get roomCloseFilesShared => 'Files you shared';

  @override
  String get roomStay => 'Stay';

  @override
  String get roomLeaveAndClose => 'Leave and close';

  @override
  String get roomLivePrefix => 'Live · ';

  @override
  String get roomPillConnecting => 'Connecting';

  @override
  String get roomPillOnlyYou => 'Only you';

  @override
  String get roomPillError => 'Error';

  @override
  String get roomLastOne => 'You are the last one in the room.';

  @override
  String get roomOnlyYouIn => 'Only you are in the room.';

  @override
  String get roomAloneWarning =>
      ' When you leave, it closes and everything in it is gone.';

  @override
  String get roomEarlierPeople => 'Earlier people';

  @override
  String get roomMorePeople => 'More people';

  @override
  String roomTileYou(String name) {
    return '$name (you)';
  }

  @override
  String roomTileNoSound(String name) {
    return '$name · no sound';
  }

  @override
  String roomTileConnecting(String name) {
    return '$name · connecting';
  }

  @override
  String roomTileAway(String name) {
    return '$name · no connection';
  }

  @override
  String roomTileMicOff(String name) {
    return '$name, microphone off';
  }

  @override
  String roomTileMutedByYou(String said) {
    return '$said, muted for you';
  }

  @override
  String roomTileVolume(String said, String percent) {
    return '$said, volume $percent';
  }

  @override
  String roomTileLeft(String name) {
    return '$name left · may come back';
  }

  @override
  String roomIsSharing(String name) {
    return '$name is sharing the screen';
  }

  @override
  String get roomNothingSentUntilWatch =>
      'Nothing is sent to you until you watch';

  @override
  String get roomWatch => 'Watch';

  @override
  String get roomLive => 'LIVE';

  @override
  String get roomSharingYourScreen => 'You\'re sharing your screen';

  @override
  String get roomShowPreview => 'Show preview';

  @override
  String get roomPreviewMirror =>
      'Showing it while you share Fogata itself makes an endless mirror';

  @override
  String get roomNobodyWatching => 'Nobody watching';

  @override
  String roomPeopleWatching(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n people watching',
      one: '1 person watching',
    );
    return '$_temp0';
  }

  @override
  String get roomExitFullScreen => 'Exit full screen';

  @override
  String get roomFullScreen => 'Full screen';

  @override
  String get roomPlayScreenSound => 'Play the screen’s sound';

  @override
  String get roomMuteScreenSound => 'Mute the screen’s sound';

  @override
  String get roomQualityPoor => 'Poor';

  @override
  String get roomQualityOkay => 'Okay';

  @override
  String get roomQualityGood => 'Good';

  @override
  String get roomQualityPoorTitle => 'Poor connection';

  @override
  String get roomQualityOkayTitle => 'Okay connection';

  @override
  String get roomQualityGoodTitle => 'Good connection';

  @override
  String get roomQualityMeasuring => 'Measuring';

  @override
  String get roomQualityDelay => 'Delay';

  @override
  String roomQualityMs(String n) {
    return '$n ms';
  }

  @override
  String get roomQualityLost => 'Lost packets';

  @override
  String roomQualityPercent(String n) {
    return '$n %';
  }

  @override
  String get roomQualityPicture => 'Picture';

  @override
  String get roomQualityPath => 'Path';

  @override
  String get roomQualityMayFreeze =>
      'The picture may freeze. Fewer people sharing at once helps.';

  @override
  String get roomQualityMayShrink =>
      'The picture may get smaller to keep moving.';

  @override
  String roomCallingName(String name) {
    return 'Calling $name…';
  }

  @override
  String get roomRingsFor30 =>
      'It rings on their computer for up to 30 seconds.';

  @override
  String get roomReturnToCall => 'Return to call';

  @override
  String get roomCallAgain => 'Call again';

  @override
  String get roomAloneHint =>
      'When someone from the group comes in, they appear here.';

  @override
  String roomAlreadyLeft(String names) {
    return '$names already left.';
  }

  @override
  String get roomGroupFound => 'Group found on the network';

  @override
  String get roomLookingForRoute => 'looking for the best route…';

  @override
  String get roomConnectingTitle => 'Connecting to the room';

  @override
  String roomSeconds(String n) {
    return '${n}s';
  }

  @override
  String roomReachedOf(int reached, int total, String time) {
    return '$reached of $total · $time';
  }

  @override
  String get roomPathLan => 'local network, encrypted';

  @override
  String get roomPathDirect => 'direct, encrypted';

  @override
  String get roomPathThroughMember => 'through a member, encrypted';

  @override
  String roomPathThrough(String name) {
    return 'through $name, encrypted';
  }

  @override
  String get roomCouldNotConnect => 'Could not connect to the room';

  @override
  String get roomCouldNotConnectText =>
      'Nobody answered in time. Your network may be blocking direct connections between devices.';

  @override
  String get roomNoAnswer => 'no answer';

  @override
  String get roomSeeNetworkCheck => 'See the network check →';

  @override
  String get roomGoesNothing => 'The room closes for everyone';

  @override
  String roomGoesMessages(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n messages go',
      one: '1 message goes',
    );
    return '$_temp0';
  }

  @override
  String roomGoesFiles(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n files you shared go',
      one: '1 file you shared goes',
    );
    return '$_temp0';
  }

  @override
  String roomGoesBoth(int messages, int files) {
    String _temp0 = intl.Intl.pluralLogic(
      messages,
      locale: localeName,
      other: '$messages messages',
      one: '1 message',
    );
    String _temp1 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files files',
      one: '1 file',
    );
    return '$_temp0 and $_temp1 go';
  }

  @override
  String get roomMuteMicrophone => 'Mute microphone';

  @override
  String get roomUnmuteMicrophone => 'Unmute microphone';

  @override
  String get roomStopSharingScreen => 'Stop sharing the screen';

  @override
  String get roomShareScreen => 'Share the screen';

  @override
  String get roomUndeafen => 'Hear the others again';

  @override
  String get roomDeafen => 'Stop hearing the others';

  @override
  String get roomCameraOff => 'Turn the camera off';

  @override
  String get roomCameraOn => 'Turn the camera on';

  @override
  String get roomLeave => 'Leave';

  @override
  String get roomSoundWaitsForMic =>
      'The sound goes out with your microphone on';

  @override
  String get roomStopSharingSound => 'Stop sharing the sound';

  @override
  String get roomShareSoundToo => 'Share the sound too';

  @override
  String get roomSoundNeedsPipeWire =>
      'Sound can’t be shared from this computer: it needs PipeWire, which isn’t running here. Only the picture is shared.';

  @override
  String get roomAddFriendToCall => 'Add a friend to the call';

  @override
  String get roomHideChat => 'Hide the chat';

  @override
  String get roomShowChat => 'Show the chat';

  @override
  String get roomTabChat => 'Chat';

  @override
  String get roomTabFiles => 'Files';

  @override
  String roomNoteLeftCall(String name) {
    return '$name left the call · ';
  }

  @override
  String roomNoteJoinedCall(String name) {
    return '$name joined the call · ';
  }

  @override
  String roomNoteLeftRoom(String name) {
    return '$name left the room · ';
  }

  @override
  String roomNoteEnteredRoom(String name) {
    return '$name entered the room · ';
  }

  @override
  String get roomSearchMessages => 'Search messages';

  @override
  String get roomClearSearch => 'Clear the search';

  @override
  String get roomChatGoneCall => 'This chat disappears when the call ends';

  @override
  String get roomChatGoneRoom => 'This chat disappears when the room closes';

  @override
  String roomNoMatches(String text) {
    return 'No messages with “$text”';
  }

  @override
  String roomMatches(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n matches',
      one: '1 match',
    );
    return '$_temp0';
  }

  @override
  String get roomReceivingMessages => 'Receiving the room’s messages…';

  @override
  String roomReceivingMessagesFrom(String name) {
    return 'Receiving the room’s messages from $name’s computer…';
  }

  @override
  String get roomSomeone => 'Someone';

  @override
  String get roomShareFiles => 'Share files';

  @override
  String get roomAddMoreFiles => 'Add more files';

  @override
  String roomMessageTo(String name) {
    return 'Message $name';
  }

  @override
  String get roomMessageTheRoom => 'Message the room';

  @override
  String get roomMessageTheCall => 'Message the call';

  @override
  String get roomAddMessageOptional => 'Add a message (optional)';

  @override
  String get roomMessageLabel => 'Message';

  @override
  String roomEnterToSend(String key) {
    return '$key to send';
  }

  @override
  String roomEscToRemove(String key) {
    return '$key to remove them';
  }

  @override
  String roomOrTwo(String first, String second) {
    return '$first or $second';
  }

  @override
  String roomOrMore(String rest, String last) {
    return '$rest or $last';
  }

  @override
  String roomFileNotAvailable(String name) {
    return 'Not available · $name left';
  }

  @override
  String get roomSharingFromHere => 'Sharing from this computer';

  @override
  String roomAvailableWhile(String names) {
    return 'Available while $names stays in the room';
  }

  @override
  String roomSizeDownloads(String size, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n downloads',
      one: '1 download',
    );
    return '$size · $_temp0';
  }

  @override
  String roomDownloadFile(String file) {
    return 'Download $file';
  }

  @override
  String roomShowInFolder(String file) {
    return 'Show $file in its folder';
  }

  @override
  String roomGettingFile(String file) {
    return 'Getting $file…';
  }

  @override
  String get roomSaveToDownloads => 'Save to Downloads';

  @override
  String roomPictureKept(String name) {
    return '$name left · the picture is kept here until you leave';
  }

  @override
  String get roomSaveFile => 'Save';

  @override
  String get roomClosePicture => 'Close the picture';

  @override
  String get roomFilesNote =>
      'Files are not uploaded anywhere: they travel straight from the sharer’s computer, and only while they stay in the room.';

  @override
  String roomFilesAvailable(int n) {
    return 'Available · $n';
  }

  @override
  String roomFilesGone(int n) {
    return 'No longer available · $n';
  }

  @override
  String roomPersonLeft(String name) {
    return '$name left';
  }

  @override
  String get roomPickOrDrop => 'Pick them here or drop them on the room';

  @override
  String get roomFileYours => 'Yours';

  @override
  String roomFileFrom(String name) {
    return 'From $name';
  }

  @override
  String roomDownloads(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n downloads',
      one: '1 download',
    );
    return '$_temp0';
  }

  @override
  String get roomStopSharingFile => 'Stop';

  @override
  String get roomWhatToShare => 'What do you want to share?';

  @override
  String roomScreenSource(String name) {
    return 'Screen: $name';
  }

  @override
  String roomPlayVideo(String file) {
    return 'Play $file';
  }

  @override
  String get roomCloseVideo => 'Close the video';

  @override
  String get roomVideoGone => 'This video is no longer in the room.';

  @override
  String get roomVideoNotFetched => 'Could not get this video.';

  @override
  String get roomVideoCannotPlay => 'This computer cannot play this video.';

  @override
  String roomGettingVideoPercent(int n) {
    return 'Getting the video · $n%';
  }

  @override
  String get roomGettingVideo => 'Getting the video…';

  @override
  String get roomPause => 'Pause';

  @override
  String get roomPlay => 'Play';

  @override
  String get roomSoundOn => 'Sound on';

  @override
  String get roomMuteVideo => 'Mute';

  @override
  String get roomLeaveFullScreen => 'Leave full screen';

  @override
  String roomCallWaits(String time) {
    return 'When only one is left, the call waits $time for the others to come back';
  }

  @override
  String roomCallWaitsAlone(String time) {
    return 'You’re the only one left. The call waits $time for the others to come back';
  }

  @override
  String roomOpenFile(String file) {
    return 'Open $file';
  }

  @override
  String activityCreated(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 started the group as “$name”';
  }

  @override
  String activityRenamed(String by, String byMe, String name) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 renamed the group “$name”';
  }

  @override
  String activityIcon(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 changed the group’s icon';
  }

  @override
  String activityPictureRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 removed the group’s picture';
  }

  @override
  String activityPicture(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 changed the group’s picture';
  }

  @override
  String activityBannerRemoved(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 removed the group’s banner';
  }

  @override
  String activityBanner(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 changed the group’s banner';
  }

  @override
  String activityAdded(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'you',
      'other': '$member',
    });
    return '$_temp0 added $_temp1';
  }

  @override
  String activityJoined(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'you',
      'other': '$member',
    });
    return '$_temp0 joined with an invite from $_temp1';
  }

  @override
  String activityLetIn(String by, String byMe, String member, String memberMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'you',
      'other': '$member',
    });
    return '$_temp0 let $_temp1 in';
  }

  @override
  String activityRemoved(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'you',
      'other': '$member',
    });
    return '$_temp0 removed $_temp1';
  }

  @override
  String activityLeft(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 left the group';
  }

  @override
  String activityMovedUp(
    String by,
    String byMe,
    String member,
    String memberMe,
    String place,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'you',
      'other': '$member',
    });
    return '$_temp0 moved $_temp1 up to $place';
  }

  @override
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'you',
      'other': '$member',
    });
    return '$_temp0 moved $_temp1 to the end';
  }

  @override
  String activityManagersAll(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 let everyone manage the group';
  }

  @override
  String activityManagersFirst(String by, String byMe, int n) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'the first $n',
      one: 'the first one only',
    );
    return '$_temp0 left managing the group to $_temp1';
  }

  @override
  String activityLimits(String by, String byMe, int people, int perRoom) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 set the group to $people people and $perRoom in each voice room';
  }

  @override
  String activityRoomOpened(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 opened the voice room “$room”';
  }

  @override
  String activityRoomRenamed(String by, String byMe, String from, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 renamed the voice room “$from” to “$room”';
  }

  @override
  String activityRoomClosed(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 deleted the voice room “$room”';
  }

  @override
  String activityInvited(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 made an invite link';
  }

  @override
  String activityRevoked(String by, String byMe) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 cancelled an invite link';
  }

  @override
  String activityRoomOpenToAll(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 opened “$room” to everyone';
  }

  @override
  String activityRoomForAbove(String by, String byMe, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 kept “$room” for those above';
  }

  @override
  String activityRoomOnly(String by, String byMe, String people, String room) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    return '$_temp0 let only $people into “$room”';
  }

  @override
  String activityKeptOut(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'you',
      'other': '$member',
    });
    return '$_temp0 kept $_temp1 out of “$room” until let back';
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
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'you',
      'other': '$member',
    });
    String _temp2 = intl.Intl.selectLogic(untilKind, {
      'today': 'until $until',
      'todayAtOne': 'until $until',
      'yesterday': 'until $until',
      'other': 'until $until',
    });
    return '$_temp0 timed $_temp1 out of “$room” $_temp2';
  }

  @override
  String activityLetBack(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  ) {
    String _temp0 = intl.Intl.selectLogic(byMe, {'yes': 'You', 'other': '$by'});
    String _temp1 = intl.Intl.selectLogic(memberMe, {
      'yes': 'you',
      'other': '$member',
    });
    return '$_temp0 let $_temp1 back into “$room”';
  }

  @override
  String get activityDefaultRoom => 'General';

  @override
  String get activityYouInList => 'you';

  @override
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'you',
      'other': '$author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' by you',
      'other': ' by $by',
      'none': '',
    });
    return 'Undone: $_temp0 had been removed$_temp1 at the same time';
  }

  @override
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  ) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'you',
      'other': '$author',
    });
    String _temp1 = intl.Intl.selectLogic(byWho, {
      'me': ' by you',
      'other': ' by $by',
      'none': '',
    });
    return 'Undone: $_temp0 had been moved to the end$_temp1 at the same time';
  }

  @override
  String activityUndoneLeft(String author, String authorMe) {
    String _temp0 = intl.Intl.selectLogic(authorMe, {
      'yes': 'you',
      'other': '$author',
    });
    return 'Undone: $_temp0 had left at the same time';
  }

  @override
  String get activityAboutList =>
      'Everyone in the group sees the same list, from the group’s own record. Changes to the picture and banner are kept apart: those are the ones this device saw.';

  @override
  String get activityAboutOrder =>
      'In the group’s own order; times are each author’s clock.';

  @override
  String get activityNothingYet => 'Nothing yet.';

  @override
  String get activityLoading => 'Loading…';

  @override
  String get activityShowOlder => 'Show older';

  @override
  String activitySeenHere(String when) {
    return '$when · seen on this device';
  }

  @override
  String get bellNotifications => 'Notifications';

  @override
  String bellNewNotifications(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n new notifications',
      one: '1 new notification',
    );
    return '$_temp0';
  }

  @override
  String get bellClearAll => 'Clear all';

  @override
  String get bellNothingNew => 'Nothing new';

  @override
  String get bellFooter =>
      'The last three stay here. Missed calls and requests also show in Friends.';

  @override
  String bellBothInRoom(String room) {
    return 'You’re both in $room.';
  }

  @override
  String get bellDismiss => 'Dismiss';

  @override
  String bellFriendRequest(String name) {
    return '$name wants to be your friend';
  }

  @override
  String bellCheckFingerprint(String fingerprint) {
    return 'Check that the fingerprint matches: $fingerprint';
  }

  @override
  String bellJoinAsk(String name, String group) {
    return '$name asks to join “$group”';
  }

  @override
  String bellJoinAskNote(String fingerprint) {
    return 'With one of your group’s invite links. Fingerprint: $fingerprint';
  }

  @override
  String get bellLetIn => 'Let in';

  @override
  String get bellSayNo => 'Say no';

  @override
  String bellGroupInvite(String name, String group) {
    return '$name added you to “$group”';
  }

  @override
  String bellMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n members',
      one: '1 member',
    );
    return '$_temp0';
  }

  @override
  String get bellJoin => 'Join';

  @override
  String bellNewMessages(int n, String name) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n new messages from $name',
      one: 'New message from $name',
    );
    return '$_temp0';
  }

  @override
  String get bellOpen => 'Open';

  @override
  String get bellMarkRead => 'Mark as read';

  @override
  String get bellFirewallTitle => 'Your firewall keeps some friends out';

  @override
  String get bellCopyCommand => 'Copy command';

  @override
  String get bellOtherSessionTitle => 'Open on another computer too';

  @override
  String get bellSettingsResetTitle => 'Some settings were reset';

  @override
  String get bellSettingsResetText =>
      'Some of your settings could not be read and are back to their defaults: check them in Settings. The old file was kept beside the new one, as app.json.bad.';

  @override
  String get friendCardOnline => 'Online';

  @override
  String get friendCardAway => 'Away';

  @override
  String get friendCardOffline => 'Offline';

  @override
  String get friendCardCallsOnlyOnline =>
      'Calls ring only while a friend is online.';

  @override
  String friendCardHangUpFirst(String name) {
    return 'Hang up first to call $name.';
  }

  @override
  String friendCardBothInRoom(String room) {
    return 'You’re both in $room.';
  }

  @override
  String get friendCardMessage => 'Message';

  @override
  String friendCardMutualFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n mutual friends',
      one: '1 mutual friend',
    );
    return '$_temp0';
  }

  @override
  String get memberCardSendRequest => 'Send friend request';

  @override
  String get memberCardRequestSent => 'Friend request sent';

  @override
  String get memberCardAccept => 'Accept friend request';

  @override
  String memberCardAsksYou(String name) {
    return '$name wants to be your friend.';
  }

  @override
  String memberCardInGroup(String group) {
    return 'Member of $group';
  }

  @override
  String memberCardOnlyOnline(String name) {
    return 'Friend requests go only while $name is online.';
  }

  @override
  String get memberCardShowOwn => 'Show your card';

  @override
  String get userPanelEditProfile => 'Edit profile';

  @override
  String get userPanelStatus => 'Status';

  @override
  String get userPanelOnline => 'Online';

  @override
  String get userPanelAway => 'Away';

  @override
  String get userPanelOffline => 'Offline';

  @override
  String get userPanelOfflineNote =>
      'Friends and members of your groups see you as offline. You can still be in rooms.';

  @override
  String get userPanelOnlineNote =>
      'Friends and members of your groups see you online, or away after 10 min without using this computer.';

  @override
  String get userPanelBio => 'BIO';

  @override
  String get userPanelAppearingOffline => 'Appearing offline';

  @override
  String get userPanelCopyLinkLabel => 'Copy a friend link';

  @override
  String userPanelLinkExpiresIn(String time) {
    return 'Expires in $time';
  }

  @override
  String get userPanelLinkTerms => '1 person · 10 min';

  @override
  String get userPanelCopyLink => 'Copy friend link';

  @override
  String get switcherChatEnded => 'Direct chat · no longer friends';

  @override
  String get switcherChatOnline => 'Direct chat · online';

  @override
  String get switcherChatAway => 'Direct chat · away';

  @override
  String get switcherChat => 'Direct chat';

  @override
  String switcherFriend(String presence) {
    return 'Friend · $presence';
  }

  @override
  String switcherVoiceRoom(String group) {
    return 'Voice room in $group';
  }

  @override
  String switcherVoiceRoomPeople(String group, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Voice room in $group · $n in it',
    );
    return '$_temp0';
  }

  @override
  String switcherGroup(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Group · $n members',
      one: 'Group · 1 member',
    );
    return '$_temp0';
  }

  @override
  String get switcherPlaceholder => 'Go to a chat, group or voice room';

  @override
  String get switcherGoTo => 'Go to';

  @override
  String get switcherEmpty => 'No groups, friends or chats yet.';

  @override
  String switcherNoMatch(String query) {
    return 'Nothing matches “$query”.';
  }

  @override
  String get switcherHintChoose => 'to choose';

  @override
  String get switcherHintGo => 'to go';

  @override
  String get switcherHintClose => 'to close';

  @override
  String switcherTooltip(String shortcut) {
    return 'Go to a chat, group or voice room ($shortcut)';
  }

  @override
  String get friendsIntro =>
      'Each friendship lives only on both devices. No directory, no server.';

  @override
  String get friendsTabAll => 'All';

  @override
  String get friendsTabOnline => 'Online';

  @override
  String get friendsTabRequests => 'Requests';

  @override
  String get friendsSearch => 'Search friends';

  @override
  String get friendsNone =>
      'No friends yet. Share your QR, link or code, or use someone else’s.';

  @override
  String get friendsNoneOnline => 'No friend is online right now.';

  @override
  String get friendsNoMatch => 'Nobody matches the search.';

  @override
  String get friendsInARoom => 'In a room';

  @override
  String friendsInRoomOf(String group) {
    return 'In room · $group';
  }

  @override
  String get friendsOnline => 'Online';

  @override
  String get friendsAway => 'Away';

  @override
  String get friendsNotSeenYet => 'Not seen online yet';

  @override
  String friendsSeen(String when) {
    return 'Seen $when';
  }

  @override
  String friendsShowCard(String name) {
    return 'Show $name’s card';
  }

  @override
  String friendsMissedCall(String when) {
    return 'Missed call · $when';
  }

  @override
  String get friendsMessage => 'Message';

  @override
  String friendsOfflineNoCall(String name) {
    return '$name is offline: calls ring only while a friend is online';
  }

  @override
  String friendsHangUpFirst(String name) {
    return 'Hang up first to call $name';
  }

  @override
  String friendsBothInRoom(String room) {
    return 'You’re both in $room.';
  }

  @override
  String friendsOptions(String name) {
    return 'Options for $name';
  }

  @override
  String friendsRemoveTitle(String name) {
    return 'Remove $name from your friends?';
  }

  @override
  String get friendsRemoveText =>
      'You disappear from each other’s friends, without a message. You stay together in the groups you share. Becoming friends again takes another invitation.';

  @override
  String get friendsRemove => 'Remove';

  @override
  String friendsAddToGroup(String group) {
    return 'Add to “$group”';
  }

  @override
  String get friendsRemoveMenu => 'Remove from friends…';

  @override
  String get friendsNoRequests => 'No pending requests.';

  @override
  String friendsRequestIncoming(String when) {
    return 'Wants to be your friend · $when';
  }

  @override
  String friendsRequestOutgoing(String when) {
    return 'Sent $when · waiting for an answer';
  }

  @override
  String get friendsPendingRequest => 'Pending request';

  @override
  String friendsCheckFingerprint(String name) {
    return 'Check that the fingerprint matches the one $name sees on their screen.';
  }

  @override
  String get friendsAddTitle => 'Add a friend';

  @override
  String get friendsAddNote => 'Your devices exchange keys directly.';

  @override
  String get friendsTabCode => 'Code';

  @override
  String get friendsTabLink => 'Link';

  @override
  String get friendsTabQr => 'QR';

  @override
  String get friendsMakeInvitation => 'Make an invitation';

  @override
  String get friendsInvitationExpired => 'This invitation has expired.';

  @override
  String get friendsMakeAnother => 'Make another';

  @override
  String friendsHintQr(String time) {
    return 'Scan it from Fogata · expires in $time';
  }

  @override
  String friendsHintLink(String time) {
    return 'Send it to one person · expires in $time';
  }

  @override
  String friendsHintCode(String time) {
    return 'Read it out to one person · expires in $time';
  }

  @override
  String get friendsHiddenCode => 'Hidden in case you share your screen.';

  @override
  String get friendsHiddenLink =>
      'Hidden in case you share your screen. It carries this computer’s network addresses: share it in private only.';

  @override
  String get friendsGotLink => 'Got a friend or group link, or a code?';

  @override
  String get friendsPastePlaceholder => 'Paste a link or code';

  @override
  String get friendsLinkOrCode => 'Link or code';

  @override
  String get friendsAdd => 'Add';

  @override
  String get friendsQrLabel => 'QR code of your invitation';

  @override
  String friendsAddedToGroup(String name) {
    return '$name gets the group the next time you are both online.';
  }

  @override
  String get directToday => 'Today';

  @override
  String get directYesterday => 'Yesterday';

  @override
  String directTimerOff(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'You',
      'other': '$name',
    });
    return '$_temp0 turned off disappearing messages';
  }

  @override
  String directTimerFirst(String time) {
    return 'Disappearing messages on: $time';
  }

  @override
  String directTimerOn(String name, String mine, String time) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'You',
      'other': '$name',
    });
    return '$_temp0 turned on disappearing messages: $time';
  }

  @override
  String directPinnedLine(String name, String mine) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'You',
      'other': '$name',
    });
    return '$_temp0 pinned a message';
  }

  @override
  String get directCallFewSeconds => 'a few seconds';

  @override
  String directCallMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minutes',
      one: '1 minute',
    );
    return '$_temp0';
  }

  @override
  String directCallHours(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n hours',
      one: '1 hour',
    );
    return '$_temp0';
  }

  @override
  String directCallHoursMinutes(int hours, int minutes) {
    String _temp0 = intl.Intl.pluralLogic(
      hours,
      locale: localeName,
      other: '$hours hours',
      one: '1 hour',
    );
    String _temp1 = intl.Intl.pluralLogic(
      minutes,
      locale: localeName,
      other: '$minutes minutes',
      one: '1 minute',
    );
    return '$_temp0 $_temp1';
  }

  @override
  String directCallLine(String name, String mine, String length) {
    String _temp0 = intl.Intl.selectLogic(mine, {
      'yes': 'You',
      'other': '$name',
    });
    return '$_temp0 started a call that lasted $length';
  }

  @override
  String get directBlockedOffline => 'You appear offline. Go online to write.';

  @override
  String directBlockedFriendOffline(String name) {
    return '$name is offline. You can write when you’re both online.';
  }

  @override
  String get directOnline => 'Online';

  @override
  String get directAway => 'Away';

  @override
  String get directOffline => 'Offline';

  @override
  String directOfflineSeen(String when) {
    return 'Offline · seen $when';
  }

  @override
  String get directHeading => 'DIRECT MESSAGES';

  @override
  String directPreviewMine(String text) {
    return 'You: $text';
  }

  @override
  String directOpenChat(String name) {
    return 'Open the chat with $name';
  }

  @override
  String directShowCard(String name) {
    return 'Show $name’s card';
  }

  @override
  String get directNoLongerFriend => 'No longer a friend';

  @override
  String directCallOffline(String name) {
    return '$name is offline: calls ring only while a friend is online';
  }

  @override
  String directHangUpFirst(String name) {
    return 'Hang up first to call $name';
  }

  @override
  String directPinnedCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n pinned messages',
      one: '1 pinned message',
    );
    return '$_temp0';
  }

  @override
  String get directPinnedTitle => 'Pinned messages';

  @override
  String directPinnedNote(int max) {
    return 'Pinned for both of you, and they don’t disappear. Up to $max.';
  }

  @override
  String get directNothingPinned => 'Nothing pinned yet';

  @override
  String get directNothingPinnedNote =>
      'Hover over a message and pick the pin to keep it here.';

  @override
  String directDayTime(String day, String time) {
    return '$day, $time';
  }

  @override
  String directFilesCount(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n files',
    );
    return '$_temp0';
  }

  @override
  String directShowMessageFrom(String name) {
    return 'Show the message from $name';
  }

  @override
  String get directUnpin => 'Unpin';

  @override
  String get directPin => 'Pin';

  @override
  String get directDeleteMessage => 'Delete for me';

  @override
  String get directDeleteMessageTitle => 'Delete this message?';

  @override
  String directDeleteMessageNote(String name) {
    return 'It goes from this computer, with its pictures and files. $name keeps their copy.';
  }

  @override
  String get directDeleteMessageAction => 'Delete';

  @override
  String get directDeleteForBoth => 'Delete for both';

  @override
  String directDeleteOwnNote(String name) {
    return 'Delete for both also takes it out of $name’s chat, as soon as their Fogata gets it. Delete for me leaves it there.';
  }

  @override
  String directTimerLabel(String time) {
    return 'Disappearing messages: $time';
  }

  @override
  String get directTimerTitle => 'Disappearing messages';

  @override
  String get directCloseMenu => 'Close the menu';

  @override
  String get directTimerNote =>
      'New messages vanish on both devices, with their pictures and files.';

  @override
  String get directEmptyNote =>
      'What you write is kept on your two computers only.';

  @override
  String directPinLineLabel(String line) {
    return '$line: show it';
  }

  @override
  String get directShowMessage => 'Show the message';

  @override
  String get directPinnedStays => 'Pinned: it stays until unpinned';

  @override
  String get directPinned => 'Pinned';

  @override
  String directPinsFull(int max) {
    return 'Up to $max messages can be pinned: unpin one first';
  }

  @override
  String get directDelivered => 'Delivered';

  @override
  String get directNotDelivered => 'Not delivered';

  @override
  String get directSending => 'Sending…';

  @override
  String directWaiting(String name) {
    return 'Waiting for $name to come online · sent when you both are';
  }

  @override
  String get directAvailableWhenYouOnline => 'available when you go online';

  @override
  String directAvailableWhenOnline(String name) {
    return 'available when $name is online';
  }

  @override
  String get directDownloadedByItself => 'downloaded automatically';

  @override
  String directOpenFile(String name) {
    return 'Open $name';
  }

  @override
  String directPlayFile(String name) {
    return 'Play $name';
  }

  @override
  String directDownloadSize(String size) {
    return 'Download · $size';
  }

  @override
  String directVideoPlaysWhenHere(String name, String length) {
    return '$name · $length · plays once it is here';
  }

  @override
  String directVideoNotHere(String name, String length, String when) {
    return '$name · $length · $when';
  }

  @override
  String directGettingIt(int percent) {
    return 'Getting it · $percent%';
  }

  @override
  String directFileNotHere(String size, String when) {
    return '$size · $when';
  }

  @override
  String get directSaveToDownloads => 'Save to Downloads';

  @override
  String get directRemovedForSpace => 'Removed here to free space';

  @override
  String get directAskAgain => 'Ask again';

  @override
  String get directClosePicture => 'Close the picture';

  @override
  String get directCloseVideo => 'Close the video';

  @override
  String get directCannotPlay => 'This computer cannot play this video.';

  @override
  String get directPause => 'Pause';

  @override
  String get directPlay => 'Play';

  @override
  String directRemoveFilesTitle(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Remove the $n files?',
      one: 'Remove the file?',
    );
    return '$_temp0';
  }

  @override
  String get directRemoveFilesText =>
      'They are taken out of this message. The files on your computer stay as they are.';

  @override
  String get directDraftKept =>
      'What you were writing stays here for when you can send it.';

  @override
  String directMessagePlaceholder(String name) {
    return 'Message $name';
  }

  @override
  String get directAddMessageOptional => 'Add a message (optional)';

  @override
  String get directAutoDownloadNote =>
      'Pictures up to 10 MB download automatically; videos and files when you open them.';

  @override
  String get directVoiceRecord => 'Record a voice message';

  @override
  String get directVoiceInCall => 'You can’t record during a call';

  @override
  String directVoiceWaitingElsewhere(String name) {
    return 'Send or delete the voice message in your chat with $name first.';
  }

  @override
  String get directVoiceDelete => 'Delete';

  @override
  String get directVoiceTooShort =>
      'Too short to send: a voice message takes at least half a second.';

  @override
  String directVoiceLeft(String time) {
    return '$time left';
  }

  @override
  String directVoiceRecordingFrom(String microphone) {
    return 'Recording from $microphone';
  }

  @override
  String get directVoiceRecordingDefault =>
      'The chosen microphone isn’t available: recording from the default one';

  @override
  String directVoiceKeys(String esc, String enter) {
    return '$esc deletes it, $enter sends it';
  }

  @override
  String directVoiceKeySend(String enter) {
    return '$enter sends it';
  }

  @override
  String directVoicePreview(String length) {
    return 'Voice message · $length';
  }

  @override
  String get directVoiceMessage => 'Voice message';

  @override
  String get directVoiceNoMic => 'Fogata can’t use the microphone.';

  @override
  String get directVoiceMicStopped =>
      'The microphone stopped. You can still send what was recorded.';

  @override
  String directVoiceNoMicCheck(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'macos': 'Check that one is plugged in, chosen in Voice & video and allowed in System Settings › Privacy & Security.',
      'other': 'Check that one is plugged in and chosen in Voice & video.',
    });
    return '$_temp0';
  }

  @override
  String get directVoiceSpeed => 'Playback speed';

  @override
  String get directVoiceNotPlayed => 'Not played yet';

  @override
  String directRemoveFile(String name) {
    return 'Remove $name';
  }

  @override
  String get directAltered =>
      'This chat’s history on this computer changed since Fogata last saved it. Some messages may be missing.';

  @override
  String directEnded(String name) {
    return 'You and $name are no longer friends. This chat can only be read.';
  }

  @override
  String get directDeleteChat => 'Delete chat';

  @override
  String directDeleteTitle(String name) {
    return 'Delete the chat with $name?';
  }

  @override
  String directDeleteText(String name) {
    return 'Its messages, pictures, videos and files go from this computer. $name keeps their copy.';
  }

  @override
  String get directDelete => 'Delete';

  @override
  String get directUsageMessages => 'Messages';

  @override
  String get directUsagePictures => 'Pictures';

  @override
  String get directUsageVideos => 'Videos';

  @override
  String get directUsageFiles => 'Files';

  @override
  String get directUsageHeading => 'THIS CHAT ON THIS DEVICE';

  @override
  String get directFreeUpSpace => 'Free up space…';

  @override
  String get directAddFilesTooltip => 'Add pictures, videos or files';

  @override
  String get directAddFiles => 'Add files';

  @override
  String get accountMoveLessThanMinuteLeft => 'less than a minute left';

  @override
  String get accountMoveAboutMinuteLeft => 'about a minute left';

  @override
  String accountMoveAboutMinutesLeft(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'about $n minutes left',
      one: 'about a minute left',
    );
    return '$_temp0';
  }

  @override
  String accountMoveNoRoom(String need, String free) {
    return 'This computer needs $need free for the account, and has $free.';
  }

  @override
  String accountMoveTooLarge(String size) {
    return 'The other computer offered $size, far more than an account holds.';
  }

  @override
  String get accountMoveBusy =>
      'This computer is finishing another move: finish or discard it first.';

  @override
  String get accountMoveNothingToFinish =>
      'This computer has no move to finish from that computer.';

  @override
  String get accountMoveNetworkStopped => 'The network stopped.';

  @override
  String get accountMoveConnectionLost => 'The connection was lost.';

  @override
  String get accountMovePeerNoRoom =>
      'The new computer doesn’t have enough free space.';

  @override
  String get accountMovePeerTooLarge => 'The account is too large to move.';

  @override
  String get accountMovePeerBusy =>
      'The new computer is finishing another move.';

  @override
  String get accountMovePeerCouldNotKeep =>
      'The new computer could not keep the files.';

  @override
  String get accountMovePeerCancelled =>
      'The move was cancelled on the other computer.';

  @override
  String get accountMovePeerCouldNotFinish =>
      'The new computer could not finish the move yet.';

  @override
  String get accountMovePeerStopped =>
      'The move was stopped on the other computer.';

  @override
  String get accountMovePeerNothingToFinish =>
      'That computer has no move to finish from this one.';

  @override
  String get accountMovePeerNotProven =>
      'That computer did not prove it has this move.';

  @override
  String get accountMoveWrongCode =>
      'The other computer did not accept the code, so nothing was sent.';

  @override
  String get accountMoveExpired => 'The code expired before it was used.';

  @override
  String get accountMoveTooManyTries =>
      'Several wrong codes were tried; the code stopped working.';

  @override
  String get accountMoveNoContact =>
      'The two computers could not reach each other.';

  @override
  String get accountMoveUnconfirmed =>
      'The very end of the move was not confirmed.';

  @override
  String get accountMoveCancelled => 'Cancelled.';

  @override
  String get accountMoveInterrupted =>
      'The move stopped before the end; nothing changed.';

  @override
  String accountMoveInterruptedWhy(String why) {
    return 'The move stopped before the end; nothing changed. $why';
  }

  @override
  String get accountMoveDeclined => 'The other computer stopped the move.';

  @override
  String accountMoveDeclinedWhy(String why) {
    return 'The other computer stopped the move. $why';
  }

  @override
  String get accountMoveLater =>
      'The account arrived, and is put in place the next time Fogata starts here.';

  @override
  String accountMoveLaterWhy(String why) {
    return 'The account arrived, and is put in place the next time Fogata starts here. $why';
  }

  @override
  String get accountMoveDidNotFinish => 'It did not finish.';

  @override
  String get accountMoveAnotherComputer => 'another computer';

  @override
  String get accountMoveTheOtherComputer => 'the other computer';

  @override
  String get moveTooLateBeingPlaced =>
      'Too late to stop: the account is being put in place.';

  @override
  String get moveTooLateDiscard =>
      'Too late: the account is being put in place.';

  @override
  String get moveTooLateOtherHasIt =>
      'Too late to stop: the other computer has the account now.';

  @override
  String moveFriends(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n friends',
      one: '1 friend',
    );
    return '$_temp0';
  }

  @override
  String moveGroups(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n groups',
      one: '1 group',
    );
    return '$_temp0';
  }

  @override
  String moveGroupsWithKeys(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n groups, with their keys',
      one: '1 group, with their keys',
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
    return '$size in all';
  }

  @override
  String get moveYourSettings => 'Your settings';

  @override
  String get moveNotDevices => 'not the microphone or camera chosen';

  @override
  String get moveFinishingBoth => 'Finishing on both computers…';

  @override
  String movePercent(int percent) {
    return '$percent%';
  }

  @override
  String movePercentLeft(int percent, String left) {
    return '$percent% · $left';
  }

  @override
  String get moveStepIdentity => 'Identity and profile';

  @override
  String get moveStepFriends => 'Friends';

  @override
  String get moveStepGroups => 'Groups and their keys';

  @override
  String get moveStepChats => 'Chats, pictures and files';

  @override
  String moveBytesOf(String done, String total) {
    return '$done of $total';
  }

  @override
  String get moveStepFinishing => 'Finishing on both computers';

  @override
  String moveBringingNamed(String name) {
    return 'Bringing $name’s account';
  }

  @override
  String get moveBringingYours => 'Bringing your account';

  @override
  String get moveKeepBothOn => 'Keep both computers on and Fogata open.';

  @override
  String get moveKeepOpenNote =>
      'Keep Fogata open on both computers. It is quickest on the same network; otherwise they connect over the internet, directly.';

  @override
  String moveFinishBringingNamed(String name) {
    return 'Finish bringing $name’s account';
  }

  @override
  String get moveFinishBringingYours => 'Finish bringing your account';

  @override
  String get moveEverythingCame =>
      'Everything arrived. Type this code on the other computer to finish.';

  @override
  String moveEverythingCameFrom(String computer) {
    return 'Everything arrived. Type this code on “$computer” to finish.';
  }

  @override
  String get moveBringHereTitle => 'Bring your account here';

  @override
  String get moveBringHereSub =>
      'Type this code on the computer that has your account.';

  @override
  String get moveHideCode => 'Hide the code';

  @override
  String get moveShowCode => 'Show the code';

  @override
  String moveExpiresIn(String time) {
    return 'expires in $time';
  }

  @override
  String moveStepFogataShows(String what) {
    return 'On that computer, Fogata shows $what.';
  }

  @override
  String get moveWaitingToConfirm => 'Waiting to confirm';

  @override
  String get moveStepOpenThere => 'On that computer, open Fogata.';

  @override
  String moveStepTypeUnder(String where) {
    return 'Type the code under $where.';
  }

  @override
  String get moveFinishTheMove => 'Finish the move';

  @override
  String moveStepGoTo(String where) {
    return 'Go to $where.';
  }

  @override
  String get moveSettingsPath => 'Settings → Move account';

  @override
  String get moveToAnotherComputer => 'Move to another computer';

  @override
  String get moveWaitingForOther => 'Waiting for your other computer…';

  @override
  String moveWrongCodesTyped(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Wrong codes were typed $n times on other computers.',
      one: 'A wrong code was typed on another computer.',
    );
    return '$_temp0';
  }

  @override
  String get moveThrowAwayInstead => 'Throw it away instead…';

  @override
  String get moveFoundOther => 'Found your other computer';

  @override
  String get moveFoundSameNetwork => 'Same network · encrypted between the two';

  @override
  String get moveFoundInternet =>
      'Over the internet · encrypted between the two';

  @override
  String get moveConfirmThere => 'Confirm on the other computer to start';

  @override
  String get moveSameAccountNote =>
      'This is the account already on this computer. Nothing is lost by bringing it here.';

  @override
  String get moveTakeYes => 'Bring it here';

  @override
  String get moveWelcomeBack => 'Welcome back';

  @override
  String moveWelcomeBackNamed(String name) {
    return 'Welcome back, $name';
  }

  @override
  String get moveDoneSub =>
      'Your account is on this computer now. The other computer stopped using it.';

  @override
  String get moveOpenFogata => 'Open Fogata';

  @override
  String get moveCodeExpiredLabel => 'expired';

  @override
  String get moveCodeStoppedLabel => 'stopped working';

  @override
  String get moveCodeExpiredNote =>
      'This code expired before your other computer used it. Codes last 10 minutes.';

  @override
  String get moveCodeStoppedNote =>
      'Wrong codes were typed several times on other computers, so this code stopped working.';

  @override
  String get moveNewCode => 'New code';

  @override
  String get moveAlmostThere => 'Almost there';

  @override
  String get moveLaterSub =>
      'Your account arrived whole, but it could not be put in place yet. It will be the next time Fogata starts here.';

  @override
  String get moveStartAgain => 'Start Fogata again';

  @override
  String get moveUnconfirmedSub =>
      'Everything arrived, but the other computer did not confirm the end. Show a code and type it there to finish the move.';

  @override
  String get moveStoppedThereTitle =>
      'The move was stopped on the other computer';

  @override
  String get moveStoppedThereSub =>
      'Nothing changed: your account is still on the other computer, and none of it was kept here.';

  @override
  String get moveCouldNotStartTitle => 'The move could not start';

  @override
  String get moveSomethingWrongHere => 'Something went wrong on this computer.';

  @override
  String get moveHalfwayTitle => 'The move stopped halfway';

  @override
  String get moveHalfwaySub =>
      'The connection to your other computer was lost. Nothing changed: your account is still on the other computer, and none of it was kept here.';

  @override
  String get moveShowCodeToFinish => 'Show a code to finish';

  @override
  String get moveTryNewCode => 'Try again with a new code';

  @override
  String moveBackToNamed(String name) {
    return 'Back to $name';
  }

  @override
  String get moveBackToThisProfile => 'Back to this profile';

  @override
  String get moveNewProfileInstead => 'Make a new profile instead';

  @override
  String get moveDiscardTitle => 'Throw away the account that arrived?';

  @override
  String get moveDiscardText =>
      'Do this only if the other computer keeps using it. That computer waits to confirm until its person chooses to keep using it there.';

  @override
  String get moveKeepIt => 'Keep it';

  @override
  String get moveThrowAway => 'Throw it away';

  @override
  String get moveBringFromAnother => 'Bring your account from another computer';

  @override
  String get moveAlreadyOnFogata => 'Already on Fogata?';

  @override
  String get moveCodeTriedBefore =>
      'That code already failed to match once. Get a new code on the new computer.';

  @override
  String moveMovingTo(String computer) {
    return 'Moving your account to “$computer”';
  }

  @override
  String get moveKeepBothOnOffline =>
      'Keep both computers on and Fogata open. Fogata is offline here meanwhile.';

  @override
  String get moveCodeFieldLabel => 'The code shown on the new computer';

  @override
  String get moveToAnotherSub =>
      'Your identity, friends, groups, chats, files and settings go to the other computer, straight from this one and encrypted. Then this computer stops using your account, so there is never a second you.';

  @override
  String moveStepChoose(String choice) {
    return 'On the new computer, open Fogata and choose $choice.';
  }

  @override
  String get moveStepTypeItShows => 'Type the code it shows:';

  @override
  String get moveWaitingForAnswer => 'Waiting for your answer…';

  @override
  String get moveLookingForCode =>
      'Looking for the computer showing this code…';

  @override
  String get moveOfflineWhileMoving =>
      'Fogata goes offline on this computer while it moves.';

  @override
  String get moveWrongCodeNote =>
      'The new computer didn’t accept that code, so nothing was sent. Check it and type it again. If it still fails, get a new code there.';

  @override
  String get moveUpdateBoth =>
      'The other computer has an older version of Fogata. Update both computers to the same version and try again. Nothing was moved.';

  @override
  String get moveCantReachNew => 'Can’t reach the new computer.';

  @override
  String get moveCantReachNewNote =>
      'Both looked for each other for 3 minutes. Put them on the same network (the same Wi-Fi, or a cable) and try again. Your account is still here and Fogata is back online.';

  @override
  String get moveDeclinedNote =>
      'The move was stopped on the new computer. Nothing was moved: your account is still here and Fogata is back online.';

  @override
  String get moveCouldNotStart => 'The move could not start.';

  @override
  String get moveHalfwayNote =>
      'The move stopped halfway: the connection to the new computer was lost. Nothing changed: your account is still here and Fogata is back online.';

  @override
  String get moveThisProfileWords => 'this profile';

  @override
  String get moveBringCardTitle => 'Bring an account here';

  @override
  String get moveBringCardSub =>
      'Show a code on this computer and type it on the one that has the account.';

  @override
  String get moveBringCardButton => 'Bring an account here…';

  @override
  String moveReplacesHere(String name) {
    return 'Replaces $name on this computer.';
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
      other: '$friends friends',
      one: '1 friend',
    );
    String _temp1 = intl.Intl.pluralLogic(
      groups,
      locale: localeName,
      other: '$groups groups',
      one: '1 group',
    );
    String _temp2 = intl.Intl.pluralLogic(
      chats,
      locale: localeName,
      other: '$chats chats',
      one: '1 chat',
    );
    return '$bold, the profile on this computer now, will be replaced and its $_temp0, $_temp1 and $_temp2 deleted here. To keep $name, move it to another computer first.';
  }

  @override
  String moveReplaceAndShow(String name) {
    return 'Replace $name and show a code';
  }

  @override
  String moveConfirmTitle(String computer) {
    return 'Move your account to “$computer”?';
  }

  @override
  String get moveConfirmSameNetwork =>
      'Found on the same network. Everything goes encrypted, straight to it.';

  @override
  String get moveConfirmInternet =>
      'Found over the internet. Everything goes encrypted, straight to it.';

  @override
  String moveConfirmReplaces(String name) {
    return 'It replaces the profile $name there.';
  }

  @override
  String get moveConfirmNote =>
      'When it is done, this computer stops using your account and can’t use it again. You can then delete what is left here.';

  @override
  String get moveConfirmYes => 'Move my account';

  @override
  String get moveKeepTitle => 'Keep using your account here?';

  @override
  String get moveKeepText =>
      'Only if the new computer does not have it: open Fogata there first. If both used it, your friends and groups would see two of you, and messages would reach only one.';

  @override
  String get moveKeepYes => 'Keep using it here';

  @override
  String moveWaitingFor(String computer) {
    return 'Waiting for “$computer” to confirm';
  }

  @override
  String get moveWaitingText =>
      'Your account was sent, but the connection broke before the new computer said it is ready. This computer won’t use the account meanwhile.';

  @override
  String get moveFinishText =>
      'Open Fogata on the new computer: it shows a code. Type it here.';

  @override
  String get moveFinish => 'Finish';

  @override
  String get moveFinishWrongCode =>
      'The new computer didn’t accept that code. Check it and type it again.';

  @override
  String get moveFinishNoContact =>
      'Can’t reach the new computer. Put both on the same network and try again.';

  @override
  String get moveKeepThisOne =>
      'The new computer doesn’t have it: keep using this one…';

  @override
  String moveMovedYours(String computer) {
    return 'Your account moved to “$computer”';
  }

  @override
  String moveMovedNamed(String name, String computer) {
    return '$name’s account moved to “$computer”';
  }

  @override
  String get moveMovedNoteYou =>
      'This computer doesn’t use it any more: friends and groups reach you on the other one.';

  @override
  String moveMovedNoteNamed(String name) {
    return 'This computer doesn’t use it any more: friends and groups reach $name on the other one.';
  }

  @override
  String moveMovedTodayAt(String time, String hour) {
    String _temp0 = intl.Intl.selectLogic(hour, {
      'one': 'Today at $time.',
      'other': 'Today at $time.',
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
  String get moveLeftHere => 'What is left here';

  @override
  String get moveLeftHereText =>
      'Chats, pictures and files from before the move, encrypted. The other computer has its own copy.';

  @override
  String get moveDeleteData => 'Delete Fogata’s data on this computer';

  @override
  String get moveAfterDeleting =>
      'After deleting, Fogata here starts like new.';

  @override
  String get moveCodeNotACode =>
      'A code has ten letters and digits, like XXXXX-XXXXX.';

  @override
  String get moveCodeMistyped =>
      'That code isn’t right: check the letters and digits.';

  @override
  String get welcomeHeadline => 'Talk like you’re\nin the same room.';

  @override
  String get welcomeIntro =>
      'Voice, video and chat with friends, family or your team, straight from your computer to theirs.';

  @override
  String get welcomeNoAccount => 'No account';

  @override
  String get welcomeEncrypted => 'Encrypted';

  @override
  String get welcomeEphemeral => 'Ephemeral rooms';

  @override
  String get welcomeCharacter =>
      'Fogata’s character, a little flame with headphones, making the peace sign';

  @override
  String get welcomeImageUnreadable => 'Could not read that image.';

  @override
  String get welcomeKeychainTitle => 'Your data can’t be encrypted yet';

  @override
  String welcomeKeychainText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' On Linux the keychain is a running Secret Service, such as KWallet or GNOME Keyring: start or unlock one, then try again.',
      'other': ' Unlock or repair the keychain, then try again.',
    });
    return 'Fogata encrypts your identity, friends and direct messages with a key it keeps in the system keychain, and the keychain did not keep it.$_temp0';
  }

  @override
  String get welcomeKeychainRetry => 'Try again';

  @override
  String get welcomeKeychainPlain => 'Continue unencrypted';

  @override
  String get welcomeKeychainPlainNote =>
      'Unencrypted, anyone who copies this computer’s files can read them. Settings will keep saying so, and Fogata encrypts everything the first time it opens with a keychain.';

  @override
  String get welcomeProfileKeptTitle => 'Your profile could not be read';

  @override
  String welcomeProfileKeptText(String folder) {
    return 'The profile and friends made on this computer could not be read, so Fogata opens as if it were new. Nothing was deleted: the files are kept in $folder.';
  }

  @override
  String get welcomeProfileKeptConfirm =>
      'A new profile starts with no friends. The old profile’s files stay where they are, and Fogata does not use them.';

  @override
  String get welcomeProfileKeptMake => 'Make a new profile';

  @override
  String get welcomeWriteName => 'Write your name.';

  @override
  String get welcomeCardTitle => 'How should your people see you?';

  @override
  String get welcomeCardSub =>
      'A name and a picture. Change them whenever you like.';

  @override
  String get welcomeNoServer =>
      'Nothing goes to a Fogata server: there is none. The public IPFS network is used only as a phone book to find your friends.';

  @override
  String get welcomeYourName => 'Your name';

  @override
  String welcomePictureLabel(String name) {
    return 'Picture: $name';
  }

  @override
  String get welcomeUploadPicture => 'Upload a picture';

  @override
  String get welcomeNameLabel => 'Name';

  @override
  String get welcomeCreate => 'Create';

  @override
  String get welcomeReady => 'Ready';

  @override
  String get welcomeCreateLabel => 'Create identity';

  @override
  String welcomeKeyLabel(String key) {
    return 'Key made on this computer: $key';
  }

  @override
  String get welcomeNewKey => 'Make another key';

  @override
  String get stoppedTitle => 'Fogata stopped working.';

  @override
  String get stoppedStartingAgain => 'Starting it again…';

  @override
  String get stoppedRestartToConnect => 'Restart it to connect again.';

  @override
  String get stoppedRestartNow => 'Restart now';

  @override
  String get settingsSectionProfile => 'My profile';

  @override
  String get settingsSectionMove => 'Move account';

  @override
  String get settingsSectionAppearance => 'Appearance';

  @override
  String get settingsSectionLanguageTime => 'Language & time';

  @override
  String get settingsSectionVoice => 'Voice & video';

  @override
  String get settingsSectionSounds => 'Sounds';

  @override
  String get settingsSectionBehaviour => 'Behaviour';

  @override
  String get settingsSectionNetwork => 'Network';

  @override
  String get settingsSectionStorage => 'Storage & downloads';

  @override
  String get settingsHeadingUser => 'User';

  @override
  String get settingsHeadingApp => 'App';

  @override
  String get settingsTitle => 'Settings';

  @override
  String get settingsAll => 'All settings';

  @override
  String get settingsCloseTooltip => 'Close settings';

  @override
  String get settingsUnsavedSaveTitle => 'Save your changes?';

  @override
  String get settingsUnsavedDiscardTitle => 'Discard your changes?';

  @override
  String get settingsUnsavedSaveText =>
      'Your profile has changes that are not saved yet.';

  @override
  String get settingsUnsavedDiscardText =>
      'Your profile has changes that cannot be saved as they are.';

  @override
  String get settingsProfileSaved =>
      'Profile saved. Friends and groups online see it now, the rest when they connect.';

  @override
  String get settingsBioSaved =>
      'Bio saved. Friends online see it now, the rest when they connect.';

  @override
  String settingsSwitchOn(String what) {
    return '$what on';
  }

  @override
  String settingsSwitchOff(String what) {
    return '$what off';
  }

  @override
  String get settingsOn => 'On';

  @override
  String get settingsOff => 'Off';

  @override
  String get settingsIdentity => 'Your identity';

  @override
  String get settingsIdentityNote =>
      'Your name and picture travel signed to your friends and groups, your bio to your friends.';

  @override
  String get settingsPicture => 'Picture';

  @override
  String get settingsUploadPicture => 'Upload a picture or a GIF';

  @override
  String get settingsName => 'Name';

  @override
  String get settingsBio => 'Bio';

  @override
  String get settingsBioHint => 'A few words about you';

  @override
  String get settingsNoBio => 'Your card shows no bio.';

  @override
  String settingsBioWhere(int lines) {
    return 'Your friends see it on your card, in up to $lines lines.';
  }

  @override
  String get settingsBannerTitle => 'Panel background';

  @override
  String get settingsBannerNote =>
      'The top of your card, for you and your friends.';

  @override
  String get settingsBannerAnother => 'Another picture';

  @override
  String get settingsBannerUpload => 'Use a picture from this computer';

  @override
  String get settingsFingerprint => 'This computer’s fingerprint';

  @override
  String get settingsTheme => 'Theme';

  @override
  String get settingsThemeNote =>
      'The window’s colours. System follows the computer between Dark and Light.';

  @override
  String get settingsMicSpeaker => 'Microphone and speaker';

  @override
  String get settingsMicSpeakerNote =>
      'What calls hear you with and play through. They can also be changed in the middle of a call.';

  @override
  String get settingsCamera => 'Camera';

  @override
  String get settingsCameraNote =>
      'What calls show of you when your camera is on. Try it here before a call.';

  @override
  String get settingsSoundEffects => 'Sound effects';

  @override
  String get settingsSoundEffectsNote =>
      'Entering and leaving a room, people coming and going, chat messages, the microphone and sharing.';

  @override
  String get settingsSounds => 'Sounds';

  @override
  String settingsChangeFailed(String reason) {
    return 'Fogata could not change that: $reason';
  }

  @override
  String get settingsAutostartApprove =>
      'Allow Fogata in System Settings › General › Login Items.';

  @override
  String get settingsAutostartOldMac => 'This needs macOS 13 or later.';

  @override
  String get settingsAutostartUnsupported =>
      'This copy of Fogata cannot start automatically.';

  @override
  String get settingsAutostartUnread =>
      'Fogata could not read this setting from your system.';

  @override
  String get settingsAutostart => 'Open at startup';

  @override
  String get settingsAutostartNote =>
      'Open Fogata when your computer starts, once you sign in.';

  @override
  String get settingsKeepRunning => 'Keep running';

  @override
  String get settingsKeepRunningNote =>
      'When the window is closed, Fogata stays in the tray, so friends can still call you.';

  @override
  String get settingsNoTray =>
      'This desktop shows no tray icons, so closing the window quits Fogata.';

  @override
  String get settingsHelping => 'Helping members';

  @override
  String get settingsHelpingNote =>
      'How this computer helps the members of your groups.';

  @override
  String get settingsHelpRelay => 'Help connect and relay';

  @override
  String get settingsHelpConnect => 'Help connect only';

  @override
  String get settingsHelpRelayNote =>
      'When two members cannot connect to each other, this computer passes their encrypted traffic (without seeing it), up to 2 GB per call. It uses your upload.';

  @override
  String get settingsHelpConnectNote =>
      'This computer helps two members find each other but does not pass their traffic. If they cannot connect, they will not hear each other.';

  @override
  String get settingsRestartTray =>
      'The change applies once you quit Fogata from its tray icon and open it again.';

  @override
  String get settingsRestart =>
      'The change applies once you quit Fogata and open it again.';

  @override
  String get settingsNetworkCheck => 'Network check';

  @override
  String get settingsNetworkCheckNote =>
      'How this computer sees the network and what it means for reaching your friends. Nothing leaves here: it only looks at what the connection already knows.';

  @override
  String get settingsCheckNetwork => 'Check the network';

  @override
  String get settingsCheckAgain => 'Check again';

  @override
  String get settingsDownloads => 'Downloads';

  @override
  String get settingsDownloadsNote =>
      'Where files downloaded from a room are saved.';

  @override
  String settingsVersionData(String version, String path) {
    return 'Fogata $version · Data in $path';
  }

  @override
  String get settingsNotEncrypted => 'Not encrypted on this computer';

  @override
  String settingsNotEncryptedText(String system) {
    String _temp0 = intl.Intl.selectLogic(system, {
      'linux': ' On Linux, the keychain is a running Secret Service, such as KWallet or GNOME Keyring: start or install one, and Fogata encrypts its data the next time it opens.',
      'other': ' Fogata tries again the next time it opens.',
    });
    return 'Your identity, friends and direct messages are stored on this computer unencrypted, because no system keychain was found to keep the key.$_temp0';
  }

  @override
  String get settingsNotAllEncrypted => 'Not all encrypted on this computer';

  @override
  String get settingsNotAllEncryptedText =>
      'Part of the data on this computer (your identity, friends and direct messages) is not encrypted yet: Fogata is still encrypting it, or could not finish, as when the disk is full. It goes on each time Fogata opens.';

  @override
  String settingsDamagedSecret(String file) {
    return '$file does not open with this computer\'s key: it was damaged on the disk. Encrypting cannot finish while it is there, and a copy of it from before encrypting stays unencrypted. Set it aside to finish: it is kept as it is. If it is your identity or your friends, Fogata takes back that copy and encrypts it; a group\'s or a chat\'s file is lost on this computer: the group has to let you in again, and the chat\'s earlier messages no longer show.';
  }

  @override
  String get settingsSetDamagedAside => 'Set it aside and restart';

  @override
  String get storageTitle => 'Storage';

  @override
  String get storageNote =>
      'Your direct messages live only on your devices. Room chats are gone when the room closes, so only these grow.';

  @override
  String get storageDirect => 'Direct messages';

  @override
  String get storageOfNoLimit => 'with no limit';

  @override
  String storageOfLimit(String limit) {
    return 'of $limit';
  }

  @override
  String storageLegendPictures(String size) {
    return 'Pictures $size';
  }

  @override
  String storageLegendVideos(String size) {
    return 'Videos $size';
  }

  @override
  String storageLegendFiles(String size) {
    return 'Files $size';
  }

  @override
  String storageLegendMessages(String size) {
    return 'Messages $size';
  }

  @override
  String get storageRoom => 'Space for direct messages';

  @override
  String get storageNoLimitNote => 'Nothing is removed automatically.';

  @override
  String storageOverNote(String limit) {
    return 'Over the limit: the oldest pictures, videos and files go, down to $limit. Messages always stay.';
  }

  @override
  String get storageLimitNote =>
      'Past the limit, the oldest pictures, videos and files go first. Messages always stay; in their place it says “Removed here to free space”.';

  @override
  String get storageAutoPictures => 'Download pictures automatically';

  @override
  String get storageAutoPicturesNote => 'Up to 10 MB each, as they arrive.';

  @override
  String get storageAutoFiles => 'Download videos and files automatically';

  @override
  String get storageAutoFilesNote =>
      'Off: they come when you open them, while the sender is online.';

  @override
  String get storageTimer => 'Disappearing messages in new chats';

  @override
  String get storageTimerNote =>
      'Each chat can change it; both people always see the same timer.';

  @override
  String get storageTimer24h => '24 h';

  @override
  String get storageByChat => 'By chat';

  @override
  String get storageByChatNote =>
      'Clearing removes pictures, videos and files here. The messages stay, and the other person keeps their copy.';

  @override
  String get storageMessagesOnly => 'Messages only';

  @override
  String get storageClearFiles => 'Clear files';

  @override
  String storageClearFilesTooltip(String name) {
    return 'Clear the files of the chat with $name';
  }

  @override
  String get storageCleared => 'Cleared';

  @override
  String storageConfirmTitle(String limit) {
    return 'Keep $limit of pictures, videos and files?';
  }

  @override
  String storageConfirmText(String limit) {
    return 'The oldest go now, down to $limit, those of pinned messages last. Messages always stay, and your friends keep their copies.';
  }

  @override
  String get storageRemoveOldest => 'Remove the oldest';

  @override
  String get themePickerSystem => 'System theme, as the computer is set';

  @override
  String themePickerTheme(String name) {
    return '$name theme';
  }

  @override
  String get themeDark => 'Dark';

  @override
  String get themeMidnight => 'Midnight';

  @override
  String get themeDim => 'Dim';

  @override
  String get themeEmber => 'Ember';

  @override
  String get themeContrast => 'Contrast';

  @override
  String get themeLight => 'Light';

  @override
  String get themeSystem => 'System';

  @override
  String get themeBannerViolet => 'Violet';

  @override
  String get themeBannerBlue => 'Blue';

  @override
  String get themeBannerGreen => 'Green';

  @override
  String get themeBannerClay => 'Clay';

  @override
  String get themeBannerRaspberry => 'Raspberry';

  @override
  String get themeBannerMustard => 'Mustard';

  @override
  String get themeBannerSlate => 'Slate';

  @override
  String get themeBannerNight => 'Night';

  @override
  String widgetsCopyWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Copy QR code',
      'link': 'Copy link',
      'code': 'Copy code',
      'other': 'Copy',
    });
    return '$_temp0';
  }

  @override
  String widgetsShowWhat(String what) {
    String _temp0 = intl.Intl.selectLogic(what, {
      'qr': 'Show QR code',
      'link': 'Show link',
      'code': 'Show code',
      'other': 'Show',
    });
    return '$_temp0';
  }

  @override
  String get widgetsHide => 'Hide';

  @override
  String revealFailed(String path) {
    return 'Could not open its folder. It is at $path';
  }

  @override
  String get emojiGroupPeople => 'People';

  @override
  String get emojiGroupNature => 'Nature';

  @override
  String get emojiGroupFood => 'Food & drink';

  @override
  String get emojiGroupActivities => 'Activities';

  @override
  String get emojiGroupTravel => 'Travel & places';

  @override
  String get emojiGroupObjects => 'Objects';

  @override
  String get emojiGroupSymbols => 'Symbols';

  @override
  String get emojiGroupFlags => 'Flags';

  @override
  String get emojiPickerLimit => 'The message is at its limit.';

  @override
  String get emojiPickerButton => 'Emoji';

  @override
  String get emojiPickerResults => 'Results';

  @override
  String get emojiPickerRecent => 'Recently used';

  @override
  String get emojiPickerSearch => 'Search emoji';

  @override
  String get emojiPickerSkinTone => 'Skin tone';

  @override
  String get emojiPickerNone => 'No emoji found';

  @override
  String get emojiPickerNoFont =>
      'This computer has no colour emoji font, so emoji may look plain or empty. Installing Noto Color Emoji fixes it.';

  @override
  String get emojiPickerDefaultTone => 'Default skin tone';

  @override
  String emojiPickerTone(int n) {
    return 'Skin tone $n';
  }

  @override
  String get iconsGroupMountain => 'Mountain';

  @override
  String get iconsGroupStar => 'Star';

  @override
  String get iconsGroupGameController => 'Game controller';

  @override
  String get iconsGroupMusic => 'Music';

  @override
  String get iconsGroupBook => 'Book';

  @override
  String get iconsGroupWaves => 'Waves';

  @override
  String get iconsGroupCoffee => 'Coffee';

  @override
  String get iconsGroupHeart => 'Heart';

  @override
  String get iconsGroupFilm => 'Film';

  @override
  String get newGroupPictureUnreadable => 'That picture could not be read.';

  @override
  String newGroupMade(String name) {
    return 'Group “$name” created. Its members get it when they come online.';
  }

  @override
  String get newGroupTitle => 'New group';

  @override
  String get newGroupSubtitle => 'The group is permanent. Its rooms are not.';

  @override
  String get newGroupName => 'Group name';

  @override
  String get newGroupNameExample => 'Sunday ride';

  @override
  String get newGroupIconOrPicture => 'Icon or picture';

  @override
  String get newGroupChooseFromComputer => 'Choose from computer';

  @override
  String get newGroupBannerOptional => 'Banner (optional)';

  @override
  String get newGroupMembers => 'Members';

  @override
  String newGroupSelected(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n selected',
      one: '1 selected',
    );
    return '$_temp0';
  }

  @override
  String get newGroupSearchPlaceholder => 'Search your friends';

  @override
  String get newGroupSearchLabel => 'Search friends';

  @override
  String get newGroupNoFriends =>
      'No friends yet. You can make the group and add them later, or invite people with a link.';

  @override
  String get newGroupKeptOnDevices => 'Kept on every member’s device.';

  @override
  String get newGroupCreate => 'Create group';

  @override
  String get groupBannerSettings => 'Group settings';

  @override
  String get groupBannerPictureUnreadable => 'That picture could not be read.';

  @override
  String get groupBannerChoose => 'Choose a banner';

  @override
  String get groupBannerAnother => 'Another banner';

  @override
  String get groupBannerNone => 'No banner';

  @override
  String get groupBannerNote =>
      'Shown at the top of the group’s column, under its name. PNG or JPEG, or an animated GIF or WebP up to 550 KB.';

  @override
  String get homeTitle => 'Home';

  @override
  String get homeSubtitle =>
      'Rooms light up when someone enters and go out when everyone leaves.';

  @override
  String get homeAddFriend => 'Add a friend';

  @override
  String get homeNewGroup => 'New group';

  @override
  String get homeLive => 'Live';

  @override
  String get homeNoRoom => 'No room';

  @override
  String get homeNoRoomNote => 'Opening a group’s room lights it up';

  @override
  String homeInviteBy(String name, int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n members',
      one: '1 member',
    );
    return '$name added you · $_temp0';
  }

  @override
  String get homeInviteNote =>
      'Until you join, this computer neither connects to them nor sends them anything.';

  @override
  String get homeInviteJoin => 'Join';

  @override
  String get homeNoGroups => 'No groups yet';

  @override
  String get homeNoGroupsNote =>
      'Add your friends and make a group with them. The group is permanent; its rooms are not.';

  @override
  String homePersonYou(String name) {
    return '$name (you)';
  }

  @override
  String homeMembers(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n members',
      one: '1 member',
    );
    return '$_temp0';
  }

  @override
  String get homeYouAreIn => 'You are in';

  @override
  String get homeMuteMic => 'Mute microphone';

  @override
  String get homeUnmuteMic => 'Unmute microphone';

  @override
  String get homeCameraOff => 'Turn the camera off';

  @override
  String get homeCameraOn => 'Turn the camera on';

  @override
  String get homeMembersTooltip => 'Members';

  @override
  String get homeReturnToRoom => 'Return to room';

  @override
  String get homeJoinRoom => 'Join';

  @override
  String homeMembersOnline(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members members',
      one: '1 member',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'nobody else online',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get homeNoRoomsYet => 'No rooms yet';

  @override
  String homeLastRoom(String when) {
    return 'Last room $when';
  }

  @override
  String get homeOpenRoom => 'Open room';

  @override
  String get membersGroupGone => 'This group is no longer here.';

  @override
  String membersSummary(int members, int online) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members members',
      one: '1 member',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'nobody else online',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String membersSummaryLastRoom(int members, int online, String when) {
    String _temp0 = intl.Intl.pluralLogic(
      members,
      locale: localeName,
      other: '$members members',
      one: '1 member',
    );
    String _temp1 = intl.Intl.pluralLogic(
      online,
      locale: localeName,
      other: '$online online',
      zero: 'nobody else online',
    );
    return '$_temp0 · $_temp1 · last room $when';
  }

  @override
  String get membersCopyInviteLink => 'Copy invite link';

  @override
  String membersInviteLinkTooltip(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'someone in the group',
      'first': 'the first in the group',
      'other': 'one of the first $n in the group',
    });
    return 'Works once, for 24 hours, and $_temp0 lets them in';
  }

  @override
  String membersInviteLinkCopied(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'someone in the group',
      'first': 'the first in the group',
      'other': 'one of the first $n in the group',
    });
    return 'Invite link copied. It works once, for 24 hours, and $_temp0 lets them in.';
  }

  @override
  String get membersInviteFriends => 'Invite friends';

  @override
  String get membersBackToRoom => 'Back to the room';

  @override
  String get membersOpenRoom => 'Open room';

  @override
  String get membersEnterRoom => 'Enter the room';

  @override
  String get membersTitle => 'Members';

  @override
  String get membersOrderNote => 'In order: each one acts on those below';

  @override
  String get membersGroupSettings => 'Group settings';

  @override
  String membersManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'Only the first $n can change the group.',
      one: 'Only the first one can change the group.',
    );
    return '$_temp0';
  }

  @override
  String get membersLeaveGroup => 'Leave group';

  @override
  String membersInviteTitle(String group) {
    return 'Invite friends to “$group”';
  }

  @override
  String get membersInviteNote =>
      'They get an invitation and choose whether to join.';

  @override
  String get membersPresenceYou => 'you';

  @override
  String membersPresenceOnline(String how) {
    return 'online · $how';
  }

  @override
  String membersPresenceAway(String how) {
    return 'away · $how';
  }

  @override
  String get membersPresenceOffline => 'not online now';

  @override
  String membersYou(String name) {
    return '$name (you)';
  }

  @override
  String get membersWaitingForKey => 'waiting for the key';

  @override
  String membersFingerprint(String fingerprint) {
    return 'Fingerprint $fingerprint';
  }

  @override
  String membersOptionsFor(String name) {
    return 'Options for $name';
  }

  @override
  String get membersMoveUp => 'Move up below me';

  @override
  String get membersMoveToEnd => 'Move to the end';

  @override
  String get membersTimeOut => 'Time out of a room…';

  @override
  String get membersRemoveFromGroup => 'Remove from the group';

  @override
  String membersRemoveTitle(String name) {
    return 'Remove $name?';
  }

  @override
  String membersRemoveText(String name) {
    return '$name leaves the group and the group makes a new key. What they already saw, they keep.';
  }

  @override
  String get membersFirst => 'First';

  @override
  String membersTimeOutTitle(String name) {
    return 'Time $name out';
  }

  @override
  String membersTimeOutText(String name) {
    return '$name leaves the room and cannot come back until the time is up or someone above them lets them back.';
  }

  @override
  String get membersUntilLetBack => 'Until let back';

  @override
  String membersLeaveTitle(String group) {
    return 'Leave “$group”?';
  }

  @override
  String get membersLeaveText =>
      'You stop seeing its rooms and the others stop seeing you in the group. The group makes a new key without you. Coming back takes someone in the group letting you in again.';

  @override
  String membersLeaveTextFirst(String name) {
    return 'You stop seeing its rooms and the others stop seeing you in the group. The group makes a new key without you. Coming back takes someone in the group letting you in again. $name becomes first.';
  }

  @override
  String get groupSettingsTabGeneral => 'General';

  @override
  String get groupSettingsTabVoiceRooms => 'Voice rooms';

  @override
  String get groupSettingsTabMembers => 'Members';

  @override
  String get groupSettingsTabInvite => 'Invite';

  @override
  String get groupSettingsTabLimits => 'Limits';

  @override
  String get groupSettingsTabActivity => 'Activity';

  @override
  String get groupSettingsFirstHere => 'First in this group';

  @override
  String groupSettingsPlaceHere(String place) {
    return '$place in this group';
  }

  @override
  String get groupSettingsLeave => 'Leave group';

  @override
  String get groupSettingsInviteTitle => 'Invite people';

  @override
  String get groupSettingsClose => 'Close group settings';

  @override
  String get groupSettingsPicture => 'Picture';

  @override
  String get groupSettingsChooseFromComputer => 'Choose from computer';

  @override
  String get groupSettingsUseIcon => 'Use an icon';

  @override
  String get groupSettingsPictureNote =>
      'PNG, JPEG or WebP, cut to a square and made smaller like profile pictures, or an animated GIF or WebP up to 550 KB.';

  @override
  String get groupSettingsOrIcon => 'Or an icon';

  @override
  String get groupSettingsBanner => 'Banner';

  @override
  String get groupSettingsName => 'Name';

  @override
  String get groupSettingsNameField => 'Group name';

  @override
  String get groupSettingsNameNote =>
      'Everyone in the group sees the new name and picture.';

  @override
  String get groupSettingsRoomsIntro =>
      'Each voice room has its own people, chat, files and calls: those in one do not connect with those in another. Someone is in one room at a time.';

  @override
  String get groupSettingsRoomName => 'Room name';

  @override
  String get groupSettingsAccessEveryone => 'Everyone';

  @override
  String get groupSettingsAccessSome => 'Only some';

  @override
  String groupSettingsWhoCanEnter(String room) {
    return 'Who can enter $room';
  }

  @override
  String get groupSettingsFirstRoomStays => 'The first room stays';

  @override
  String get groupSettingsPeopleInside => 'People are inside';

  @override
  String get groupSettingsDeleteRoom => 'Delete room';

  @override
  String get groupSettingsAddRoom => 'Add voice room';

  @override
  String groupSettingsNewRoomName(int n) {
    return 'Room $n';
  }

  @override
  String groupSettingsRoomsNote(int rooms, int people) {
    return 'Up to $rooms rooms, and $people people in each (Limits). A room with people inside cannot be deleted.';
  }

  @override
  String get groupSettingsKeptOut => 'Kept out';

  @override
  String get groupSettingsAccessEveryoneLong => 'Everyone in the group';

  @override
  String get groupSettingsAccessChosen => 'Only the members I choose';

  @override
  String get groupSettingsAccessNote =>
      'You and those above you can always enter. Someone left out sees the room with a lock.';

  @override
  String groupSettingsYou(String name) {
    return '$name (you)';
  }

  @override
  String groupSettingsKeptOutBy(String room, String by) {
    return ' · $room · by $by';
  }

  @override
  String groupSettingsTimeLeft(String time) {
    return '$time left';
  }

  @override
  String get groupSettingsUntilLetBack => 'Until let back';

  @override
  String get groupSettingsLetBack => 'Let back';

  @override
  String groupSettingsManagersNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Only the first $n can change the group: its name, rooms, invitations and limits. You act on those below you.',
      one: 'Only the first one can change the group: its name, rooms, invitations and limits. You act on those below you.',
    );
    return '$_temp0';
  }

  @override
  String groupSettingsMembersCount(int n, int max) {
    String _temp0 = intl.Intl.pluralLogic(
      max,
      locale: localeName,
      other: '$max members',
      one: '1 member',
    );
    return '$n of $_temp0, in order';
  }

  @override
  String get groupSettingsMembersNote =>
      'Everyone acts on the people below them: moves them up to just below themselves or down to the end, times them out of a room, or removes them. Whoever comes in starts at the end.';

  @override
  String get groupSettingsWhoManages => 'Who manages this group';

  @override
  String get groupSettingsManagersEveryone => 'Everyone';

  @override
  String groupSettingsManagersFirstN(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: 'The first $n',
      one: 'The first one',
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
    return 'Set to the first $n, more than the $_temp0 in the group now: for now that is everyone.';
  }

  @override
  String get groupSettingsManagersEveryoneNote =>
      'Those who manage change the name, pictures, rooms and limits, and let people in. With everyone, the group keeps going whoever loses their device.';

  @override
  String groupSettingsManagersFirstNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          'Only the first $n change the name, pictures, rooms and limits, and let people in. If they all lose their devices, nobody can manage the group any more: Fogata has no server that could fix it.',
      one: 'Only the first one changes the name, pictures, rooms and limits, and lets people in. If that device is lost, nobody can manage the group any more: Fogata has no server that could fix it.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsMoveUp => 'Move up';

  @override
  String groupSettingsMoveUpTooltip(String name) {
    return 'Move $name up to just below you';
  }

  @override
  String get groupSettingsToEnd => 'To the end';

  @override
  String groupSettingsToEndTooltip(String name) {
    return 'Move $name to the end';
  }

  @override
  String get groupSettingsTimeOut => 'Time out of a room';

  @override
  String get groupSettingsRemove => 'Remove from the group';

  @override
  String get groupSettingsAskingToJoin => 'Asking to join';

  @override
  String get groupSettingsInviteFriend => 'Invite a friend';

  @override
  String get groupSettingsInviteFriendNote =>
      'They get an invitation in Fogata and choose whether to join. Only your friends show here.';

  @override
  String get groupSettingsInviteLink => 'Invite link';

  @override
  String groupSettingsInviteLinkNote(String who, int n) {
    String _temp0 = intl.Intl.selectLogic(who, {
      'anyone': 'someone in the group',
      'first': 'the first in the group',
      'other': 'one of the first $n in the group',
    });
    return 'For someone who is not your friend yet. It works once and for 24 hours, and $_temp0 lets them in: if it reaches someone else, you see who asks and say no.';
  }

  @override
  String get groupSettingsCopyLink => 'Copy invite link';

  @override
  String groupSettingsOpenLinks(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n links not used yet.',
      one: '1 link not used yet.',
      zero: 'No links waiting to be used.',
    );
    return '$_temp0';
  }

  @override
  String get groupSettingsCancelLinks => 'Cancel them';

  @override
  String get groupSettingsNoFriends => 'No friends yet: add them in Friends.';

  @override
  String get groupSettingsAllFriendsIn => 'All your friends are in the group.';

  @override
  String get groupSettingsFriendOnline => 'Online';

  @override
  String get groupSettingsFriendOffline => 'Not online now';

  @override
  String get groupSettingsInviteButton => 'Invite';

  @override
  String get groupSettingsGroupFull => 'The group is full';

  @override
  String get groupSettingsSayNo => 'Say no';

  @override
  String get groupSettingsLetIn => 'Let in';

  @override
  String get groupSettingsLimitGroup => 'People in the group';

  @override
  String groupSettingsLimitGroupNote(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n people',
      one: '1 person',
    );
    return 'From 2 to 20. When it is full, invitations and links stop working. It cannot go below the $_temp0 already in.';
  }

  @override
  String get groupSettingsLimitRoom => 'People in each voice room';

  @override
  String get groupSettingsLimitRoomNote =>
      'From 2 to 8. Voice and video go straight between everyone in a room, so 8 is the most that sounds good; with cameras on, fewer work better. Whoever comes when it is full sees that the room is full.';

  @override
  String get shellChipCallMany => 'Direct · encrypted · only those in the call';

  @override
  String get shellChipCallTwo => 'Direct · encrypted · only you two';

  @override
  String get shellGroupMembers => 'Group members';

  @override
  String get shellMembers => 'Members';

  @override
  String get shellDirectMessages => 'Direct messages';

  @override
  String get shellPathDirect => 'Direct · encrypted';

  @override
  String shellPathVia(String name) {
    return 'Via $name · encrypted';
  }

  @override
  String get shellPathViaSomeone => 'Via a member · encrypted';

  @override
  String shellCallWith(String names) {
    return 'Call with $names';
  }

  @override
  String get shellRinging => 'Ringing · ';

  @override
  String get shellInCall => 'In call · ';

  @override
  String get shellCallEnded => 'Call ended';

  @override
  String get shellNewGroup => 'New group';

  @override
  String get shellSectionCall => 'Call';

  @override
  String shellCallRinging(String name) {
    return '$name · ringing';
  }

  @override
  String get shellSectionLiveNow => 'Live now';

  @override
  String get shellSectionInCall => 'In the call';

  @override
  String get shellChat => 'Chat';

  @override
  String get shellFiles => 'Files';

  @override
  String get shellSectionGroup => 'Group';

  @override
  String get shellInvite => 'Invite';

  @override
  String get shellSectionVoiceRooms => 'Voice rooms';

  @override
  String shellVoiceRoomYouAreIn(String room) {
    return '$room · you are in';
  }

  @override
  String get shellNotForYou => 'Not for you now';

  @override
  String get shellOnlySomeMembers => 'Only some members';

  @override
  String shellPersonYou(String name) {
    return '$name (you)';
  }

  @override
  String get shellLive => 'LIVE';

  @override
  String get shellYourProfile => 'Your profile';

  @override
  String shellMuteMic(String keys) {
    return 'Mute microphone ($keys)';
  }

  @override
  String shellUnmuteMic(String keys) {
    return 'Unmute microphone ($keys)';
  }

  @override
  String get shellDeafen => 'Stop hearing the others';

  @override
  String get shellUndeafen => 'Hear the others again';

  @override
  String get shellStateCalling => 'Calling…';

  @override
  String get shellStateInCall => 'In a call';

  @override
  String get shellStateInRoom => 'In the room';

  @override
  String get shellStateOffline => 'Offline';

  @override
  String get shellStateOnline => 'Online';

  @override
  String get shellStateAway => 'Away';

  @override
  String get shellShowInFolder => 'Show in folder';

  @override
  String get shellCopyPath => 'Copy path';

  @override
  String shellVolumePercent(int n) {
    return '$n%';
  }

  @override
  String get shellMuteKeys => 'Ctrl+Shift+M';

  @override
  String get settingsSectionDonate => 'Donate to Fogata';

  @override
  String get settingsSectionReport => 'Report a problem';

  @override
  String get settingsHeadingHelp => 'Help';

  @override
  String get settingsPreRelease => 'Pre-release';

  @override
  String get donateTitle => 'Keep Fogata independent';

  @override
  String get donateText =>
      'Fogata is free, has no ads and no servers watching you. Your donation keeps it that way.';

  @override
  String get donateButton => 'Donate';

  @override
  String get donateNote =>
      'Opens Ko-fi in your browser. Fogata never sees your payment.';

  @override
  String get reportWhatTitle => 'What happened?';

  @override
  String get reportWhatNote =>
      'What you were doing, what you expected and what you saw instead.';

  @override
  String get reportIncludeLog => 'Include the technical log of the last 2 days';

  @override
  String get reportLogNote =>
      'Versions, errors and connection steps. Never your messages, keys, contacts or IP addresses.';

  @override
  String get reportSeeSent => 'See what’s sent';

  @override
  String get reportHideSent => 'Hide the list';

  @override
  String get reportPreview => 'Read the whole report before sending';

  @override
  String get reportHidePreview => 'Hide the report';

  @override
  String get reportPreviewNote =>
      'This is exactly what the file holds, and all that goes. Names, folders, file names, addresses and keys are already taken out of the log.';

  @override
  String get reportPreviewWorking => 'Preparing the report…';

  @override
  String get reportSentWhat => 'What you wrote above.';

  @override
  String get reportSentVersion =>
      'Fogata’s version and build, your system and its version, and the language Fogata is in.';

  @override
  String get reportSentLog =>
      'The log of the last 2 days: when Fogata started, its errors and how it reached other devices. Names, messages, folders, file names, links, codes, keys and IP addresses are taken out, and each device shows only as a number that means nothing outside the report.';

  @override
  String get reportEmail => 'Email it to the Fogata team';

  @override
  String get reportSave => 'Save to a file';

  @override
  String reportHowEmail(String address) {
    return 'Your email app opens with the message ready for $address, and the folder with the report file opens beside it: attach the file and send. Nothing leaves your computer until you do.';
  }

  @override
  String get reportReadyTitle => 'Your email is ready';

  @override
  String reportReadyText(String file, String address) {
    return 'Attach $file from the folder that opened and send it to $address.';
  }

  @override
  String get reportShowFile => 'Show the file';

  @override
  String get reportCopyAddress => 'Copy the address';

  @override
  String get reportAddressCopied => 'Address copied';

  @override
  String reportSaved(String file) {
    return 'Report saved: $file';
  }

  @override
  String reportNoMailApp(String address) {
    return 'No email app opened. Send the file to $address from the email you use.';
  }

  @override
  String reportFailed(String reason) {
    return 'The report could not be written: $reason';
  }

  @override
  String get splashStarting => 'Starting';

  @override
  String get splashConnecting => 'Connecting';

  @override
  String get splashChecking => 'Checking for updates';

  @override
  String get splashDownloading => 'Downloading update';

  @override
  String get splashInstalling => 'Installing update';

  @override
  String get splashRestarting => 'Restarting';

  @override
  String splashProgress(String done, String total) {
    return '$done of $total MB';
  }

  @override
  String splashProgressFrom(String done, String total, String name) {
    return '$done of $total MB · from $name';
  }

  @override
  String updateReady(String version) {
    return 'Update ready · $version';
  }

  @override
  String updateRequired(String version) {
    return 'Update required · $version';
  }

  @override
  String get updateRestart => 'Restart';

  @override
  String updateReadyTip(String version) {
    return 'Fogata $version is here. Restart to use it.';
  }

  @override
  String get updateRequiredTip =>
      'This version no longer works with your friends’ newer one. Restart to update.';

  @override
  String updateFailed(String reason) {
    return 'The update could not be installed: $reason';
  }

  @override
  String get errJoinInviteGone =>
      'That invite no longer works, or the group is full.';

  @override
  String get errJoinNoKeyYet =>
      'This device doesn’t have the group’s key yet. The new member gets in as soon as it does.';

  @override
  String get errJoinWroteTooMuch =>
      'This device has made changes the other members haven’t taken in yet. The new member can be let in once another member is online.';

  @override
  String get errJoinAskAgain =>
      'That request to join was made on an earlier state of the group. The person has to ask again.';

  @override
  String get errNoSpaceForFile =>
      'There isn’t enough free space for this file. Free some space and try again.';

  @override
  String get recoveryTitle => 'Recovery copy';

  @override
  String get recoveryNote =>
      'A file that brings this account back if this computer’s keychain or disk is lost. Only your recovery key opens it.';

  @override
  String get recoveryOff => 'Off';

  @override
  String recoveryOn(String id) {
    return 'On · key $id';
  }

  @override
  String get recoveryTurnOn => 'Turn on…';

  @override
  String get recoveryChangeKey => 'Change key…';

  @override
  String get recoveryMakeCopy => 'Make a copy…';

  @override
  String get recoveryReplacedKept =>
      'An account that a restore replaced is still kept on this computer.';

  @override
  String get recoveryDeleteReplaced => 'Delete it…';

  @override
  String get recoveryChangeTitle => 'Make a new recovery key?';

  @override
  String get recoveryChangeText =>
      'New copies will open only with the new key. Copies made before still open only with the old one, so keep it as long as you keep them.';

  @override
  String get recoveryChangeConfirm => 'Make a new key';

  @override
  String get recoveryKeyTitle => 'Your recovery key';

  @override
  String get recoveryKeyText =>
      'Write it down or print it, and keep it apart from your copies. It is shown only this once.';

  @override
  String recoveryKeyName(String id) {
    return 'Its short name: $id';
  }

  @override
  String get recoveryKeyWarning =>
      'Fogata doesn’t keep this key anywhere: without it, no copy opens. Anyone with a copy and this key has your account.';

  @override
  String get recoveryKeyWritten => 'I wrote it down';

  @override
  String get recoveryCheckTitle => 'Check what you wrote down';

  @override
  String get recoveryCheckText =>
      'Type these two groups of the key, as you wrote them.';

  @override
  String recoveryCheckGroup(int n) {
    return 'Group $n';
  }

  @override
  String get recoveryCheckWrong =>
      'That isn’t it: look again at what you wrote down.';

  @override
  String get recoveryTurnOnButton => 'Turn on copies';

  @override
  String recoveryTurnedOn(String id) {
    return 'Copies are on, for the key $id.';
  }

  @override
  String get recoveryCopyTitle => 'Make a recovery copy';

  @override
  String get recoveryCopyText =>
      'Fogata is offline while it makes the copy, longer with the files sent in chats. Keep the file away from this computer, on a USB drive for example.';

  @override
  String get recoveryCopyAttachments => 'Include files sent in chats';

  @override
  String recoveryCopyAttachmentsNote(String size) {
    return 'Pictures, videos and files: $size more.';
  }

  @override
  String get recoveryCopySave => 'Choose where…';

  @override
  String get recoveryCopyInDataFolder =>
      'A copy can’t be kept in Fogata’s data folder. Choose another place.';

  @override
  String recoveryCopyMade(String file, String size) {
    return 'Copy saved: $file ($size).';
  }

  @override
  String recoveryCopyFailed(String why) {
    return 'The copy couldn’t be made. $why';
  }

  @override
  String get recoveryDeleteTitle => 'Delete the replaced account?';

  @override
  String get recoveryDeleteText =>
      'It was set aside when a copy was restored over it. Once deleted, it can’t be brought back.';

  @override
  String get recoveryDelete => 'Delete';

  @override
  String get recoveryDeleted => 'The replaced account was deleted.';

  @override
  String get recoveryFileType => 'Fogata copy';

  @override
  String get recoveryLostComputer => 'Lost your computer?';

  @override
  String get recoveryRestoreLink => 'Restore from a copy';

  @override
  String get recoveryRestoreTitle => 'Restore from a copy';

  @override
  String get recoveryRestoreSub =>
      'Choose the recovery copy you saved, then type your recovery key.';

  @override
  String get recoveryChooseCopy => 'Choose the copy…';

  @override
  String recoveryMadeOn(String date) {
    return 'A copy made on $date.';
  }

  @override
  String get recoveryOtherKey =>
      'It was made for a different recovery key from the one this computer has. Type the key it was made for.';

  @override
  String get recoveryKeyField => 'Recovery key';

  @override
  String get recoveryKeyNotAKey =>
      'A recovery key has 53 letters and digits, in groups of four.';

  @override
  String get recoveryKeyMistyped => 'A letter or digit is wrong: check them.';

  @override
  String get recoveryProblemNotACopy =>
      'This isn’t a copy of a Fogata account.';

  @override
  String get recoveryProblemNewer =>
      'This copy was made by a newer version of Fogata. Update Fogata to open it.';

  @override
  String get recoveryProblemOtherKey =>
      'This copy was made for another recovery key.';

  @override
  String get recoveryProblemDamaged =>
      'This copy is damaged, or was changed after it was made.';

  @override
  String get recoveryProblemUnproven =>
      'This copy wasn’t made by the account it holds, so it won’t be opened.';

  @override
  String recoveryWhose(String name) {
    return '$name’s account';
  }

  @override
  String recoveryHolds(int files, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      files,
      locale: localeName,
      other: '$files files',
      one: '1 file',
    );
    return '$_temp0, $size in all';
  }

  @override
  String get recoveryWithAttachments => 'With the files sent in chats.';

  @override
  String get recoveryWithoutAttachments => 'Without the files sent in chats.';

  @override
  String get recoveryOnlyIf =>
      'Use a copy only if no other computer uses this account any more. If one still does, move the account from it instead: one account can’t be on two computers.';

  @override
  String get recoveryMoveInstead => 'Move it instead';

  @override
  String recoveryHereNamed(String name) {
    return 'This computer holds $name’s account.';
  }

  @override
  String recoveryHereDevice(String fingerprint) {
    return 'This computer holds an account with no profile (device $fingerprint).';
  }

  @override
  String get recoveryHereUnknown =>
      'This computer holds part of an account that doesn’t open.';

  @override
  String get recoveryHereReplaced =>
      'Restoring replaces it. It is set aside, not deleted, until you delete it in Settings › Storage & downloads.';

  @override
  String get recoveryRestore => 'Restore';

  @override
  String get recoveryReplace => 'Replace and restore';

  @override
  String recoveryRestoring(String name) {
    return 'Restoring $name’s account…';
  }

  @override
  String recoveryRestored(String name) {
    return '$name’s account is restored.';
  }

  @override
  String get recoveryRefusedAccountHere =>
      'This computer holds an account now. Choose the copy again to see what it replaces.';

  @override
  String get recoveryRefusedReplacedKept =>
      'This computer still keeps an account that an earlier restore replaced. Delete it to restore this copy.';

  @override
  String recoveryRefusedNoRoom(String need, String free) {
    return 'The copy needs $need on this disk, and $free are free. Nothing changed.';
  }

  @override
  String get recoveryRefusedInFolder =>
      'The copy is inside Fogata’s data folder. Move it somewhere else, then choose it again.';

  @override
  String get recoveryBusy =>
      'Fogata is busy with a recovery copy or a move. Try again when it’s done.';

  @override
  String get moveBusyWithCopy =>
      'Fogata is busy with a recovery copy. Move the account once it’s done.';

  @override
  String get recoveryDeleteKeptAside => 'Delete the account kept aside…';

  @override
  String get reachOffline =>
      'Fogata can’t reach anyone on the internet from here. Check the connection, or a firewall may be blocking Fogata.';

  @override
  String get reachFirewall =>
      'This computer’s firewall lets nothing in for Fogata. The command that lets it in is in the notifications.';

  @override
  String get reachUdpBlocked =>
      'This network or this computer blocks UDP. Calls and direct connections through routers need it; until it is allowed, others reach you only through a group member or from a public address.';

  @override
  String get reachSymmetricNat =>
      'Your router gives a different public port to each destination (a symmetric NAT), so no direct way can be opened through it. Friends behind another router reach you only through a member of a group you share.';

  @override
  String get reachNoIpv6 =>
      'There is no working IPv6 either, which would get around the router.';

  @override
  String reachFineFriend(String name) {
    return 'Your network looks fine for Fogata, so what keeps you apart is probably on $name’s side (offline, or their network).';
  }

  @override
  String get reachFineOthers =>
      'Your network looks fine for Fogata, so what keeps you apart is probably on the other side.';

  @override
  String directDropTitle(String name) {
    return 'Drop to send to $name';
  }

  @override
  String get directDropNote =>
      'They wait in the message box, where you can add a message and check them before they go.';

  @override
  String directPrivateLeftOut(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other:
          '$n hidden or private files were left out: files like these are not added by dragging, since a web page can start a drag. To send them, pick them with the + button.',
      one: 'A hidden or private file was left out: files like it are not added by dragging, since a web page can start a drag. To send it, pick it with the + button.',
    );
    return '$_temp0';
  }

  @override
  String appMustUpdateToJoin(String name, String group) {
    return '$name has to update Fogata before they can join “$group”. Once they do, they get in as before.';
  }

  @override
  String appUpdateToJoin(String group) {
    return '“$group” needs a newer Fogata. Update Fogata to join it.';
  }

  @override
  String get errMustUpdateToJoin =>
      'Your friend has to update Fogata before they can join this group.';

  @override
  String groupSettingsLevelWaiting(String names) {
    return 'This group is waiting to turn on stronger protection. It turns on by itself once everyone has updated Fogata. Still to update: $names';
  }
}
