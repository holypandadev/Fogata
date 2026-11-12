import 'package:flutter/foundation.dart' show visibleForTesting;
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:intl/intl.dart';

import 'host.dart';
import 'l10n.dart';
import 'rust/api/types.dart';

/// Longest chat message, in characters; the others' apps ignore longer ones.
const maxMessage = 2000;

/// Whether clock times use 12 hours and AM or PM, as chosen in Settings, instead of 24.
bool clock12 = false;

/// `20:13`, or `8:13 PM` with [clock12] (in the language's own way: `午後8:13`).
String clock(int ms) {
  final t = DateTime.fromMillisecondsSinceEpoch(ms);
  final minutes = t.minute.toString().padLeft(2, '0');
  if (!clock12) return '${t.hour.toString().padLeft(2, '0')}:$minutes';
  if (currentLanguage == 'en') {
    final hour = t.hour % 12 == 0 ? 12 : t.hour % 12;
    return '$hour:$minutes ${t.hour < 12 ? 'AM' : 'PM'}';
  }
  // Japanese and Chinese put the half of the day first.
  final pattern = currentLanguage == 'ja' || currentLanguage == 'zh-Hans'
      ? 'ah:mm'
      : 'h:mm a';
  return DateFormat(pattern, intlLocale).format(t);
}

/// [clock], with the day before it when it is not today: `yesterday 23:23`,
/// `Oct 21, 23:23`, `Oct 21 2025, 23:23`.
String dayClock(int ms, {DateTime? now}) {
  final t = DateTime.fromMillisecondsSinceEpoch(ms);
  final today = now ?? DateTime.now();
  final days = _daysBefore(t, today);
  if (days == 0) return clock(ms);
  if (days == 1) return l10n.timeYesterdayAt(clock(ms), clockHour(ms));
  return l10n.timeDayAt(day(t, withYear: t.year != today.year), clock(ms));
}

/// How [dayClock] writes [ms], for the words a language puts before it: `today`
/// (only the clock), `todayAtOne` (only the clock, its hour 1), `yesterday`, or
/// `other` (a day of the year). A language may need different words before the time
/// in each case (“until 23:23”, “until yesterday 23:23”, “until 21 Oct, 23:23”).
String dayClockKind(int ms, {DateTime? now}) {
  final t = DateTime.fromMillisecondsSinceEpoch(ms);
  return switch (_daysBefore(t, now ?? DateTime.now())) {
    0 => clockHour(ms) == 'one' ? 'todayAtOne' : 'today',
    1 => 'yesterday',
    _ => 'other',
  };
}

/// `one` when [clock] writes [ms] with the hour 1 (`1:05`, `01:05`, `1:05 PM`), else
/// `other`: the languages with an article before a time choose it by this
/// (Spanish “a la 1:05” but “a las 23:05”).
String clockHour(int ms) {
  final hour = DateTime.fromMillisecondsSinceEpoch(ms).hour;
  return (clock12 ? hour % 12 : hour) == 1 ? 'one' : 'other';
}

/// How many calendar days [t] is before [today]: 0 the same day, 1 the day before.
int _daysBefore(DateTime t, DateTime today) =>
    // Rounded: a day with a clock change has 23 or 25 hours.
    (DateTime(
              today.year,
              today.month,
              today.day,
            ).difference(DateTime(t.year, t.month, t.day)).inHours /
            24)
        .round();

/// A day of the year in the language in use: `Oct 21`, `21 oct.`, `10月21日`, with the
/// year when asked: `Oct 21 2025`.
String day(DateTime t, {bool withYear = false}) {
  if (currentLanguage == 'en') {
    return DateFormat(withYear ? 'MMM d y' : 'MMM d', intlLocale).format(t);
  }
  return (withYear ? DateFormat.yMMMd(intlLocale) : DateFormat.MMMd(intlLocale))
      .format(t);
}

/// `42:18`, or `1:02:05` past an hour.
String elapsed(int sinceMs) {
  final d = Duration(
    milliseconds: (DateTime.now().millisecondsSinceEpoch - sinceMs).clamp(
      0,
      1 << 52,
    ),
  );
  final h = d.inHours;
  final m = d.inMinutes % 60;
  final s = d.inSeconds % 60;
  final mm = m.toString().padLeft(2, '0');
  final ss = s.toString().padLeft(2, '0');
  return h > 0 ? '$h:$mm:$ss' : '${d.inMinutes}:$ss';
}

/// `42 min`, `2 h 5 min`.
String minutes(int sinceMs) {
  final d = Duration(
    milliseconds: (DateTime.now().millisecondsSinceEpoch - sinceMs).clamp(
      0,
      1 << 52,
    ),
  );
  if (d.inMinutes < 1) return l10n.timeNow;
  if (d.inHours < 1) return l10n.timeMinutes(d.inMinutes);
  final m = d.inMinutes % 60;
  return m == 0
      ? l10n.timeHours(d.inHours)
      : l10n.timeHoursMinutes(d.inHours, m);
}

/// `9:41` left until `untilMs`.
String countdown(int untilMs) {
  final left = untilMs - DateTime.now().millisecondsSinceEpoch;
  if (left <= 0) return '0:00';
  final d = Duration(milliseconds: left);
  return '${d.inMinutes}:${(d.inSeconds % 60).toString().padLeft(2, '0')}';
}

/// A wait of [ms], rounded up: `58 min`, `3 h`, `2 days`.
String timeLeft(int ms) {
  final minutes = ((ms < 0 ? 0 : ms) + 59999) ~/ 60000;
  if (minutes < 60) return l10n.timeMinutes(minutes < 1 ? 1 : minutes);
  if (minutes < 24 * 60) return l10n.timeHours((minutes + 59) ~/ 60);
  return l10n.timeDays((minutes + 24 * 60 - 1) ~/ (24 * 60));
}

/// A span of [ms] as it was chosen: `5 minutes`, `1 hour`, `1 day`, `1 week`.
String spanWords(int ms) {
  const day = 24 * 60;
  final minutes = (ms / 60000).round();
  if (minutes > 0 && minutes % (7 * day) == 0) {
    return l10n.spanWeeks(minutes ~/ (7 * day));
  }
  if (minutes > 0 && minutes % day == 0) return l10n.spanDays(minutes ~/ day);
  if (minutes > 0 && minutes % 60 == 0) return l10n.spanHours(minutes ~/ 60);
  return l10n.spanMinutes(minutes);
}

/// `3 days ago`, `yesterday`, `5 min ago`.
String ago(int ms, {DateTime? now}) {
  now ??= DateTime.now();
  final then = DateTime.fromMillisecondsSinceEpoch(ms);
  final d = now.difference(then);
  if (d.inMinutes < 1) return l10n.agoMoment;
  if (d.inHours < 1) return l10n.agoMinutes(d.inMinutes);
  final today = DateTime(now.year, now.month, now.day);
  final day = DateTime(then.year, then.month, then.day);
  final days = today.difference(day).inDays;
  if (days == 0) return l10n.agoHours(d.inHours);
  if (days == 1) return l10n.agoYesterday;
  if (days < 7) return l10n.agoDays(days);
  if (days < 31) return l10n.agoWeeks(days ~/ 7);
  // Up to 11: the last days before a year are not "12 months".
  if (days < 365) return l10n.agoMonths((days ~/ 30).clamp(1, 11));
  return l10n.agoOverYear;
}

/// `48.2 MB` (`48,2 Mo` in French).
String bytes(int n) {
  if (n < 1000) return l10n.sizeBytes(n);
  final units = [l10n.sizeKB, l10n.sizeMB, l10n.sizeGB, l10n.sizeTB];
  var v = n / 1000;
  var i = 0;
  // Up a unit also when the number would round to 1000: `1.0 MB`, not `1000 KB`.
  while (v.round() >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return units[i](NumberFormat(v >= 100 ? '0' : '0.0', intlLocale).format(v));
}

/// `Marta`, `Marta and Leo`, `Marta, Leo and Sofía`.
String names(List<String> list) {
  if (list.isEmpty) return '';
  if (list.length == 1) return list.first;
  if (list.length == 2) return l10n.listTwo(list.first, list.last);
  return l10n.listMore(
    list.sublist(0, list.length - 1).join(l10n.listSeparator),
    list.last,
  );
}

/// A place in a list, counted from 1: 1st, 2nd, 3rd, 4th, 11th, 21st (`1.º`, `1.`, `1番目`
/// in other languages).
String ordinal(int n) {
  if (currentLanguage != 'en') return l10n.ordinal(n);
  final teen = n % 100 >= 11 && n % 100 <= 13;
  final end = teen
      ? 'th'
      : switch (n % 10) {
          1 => 'st',
          2 => 'nd',
          3 => 'rd',
          _ => 'th',
        };
  return '$n$end';
}

/// What went wrong, in words for people. The core says it in English: its known
/// messages are found by a part of their words, which stay as the core writes them. A
/// panic of the core's, or a task of it that ended in one, says only that something went
/// wrong: its words (scrubbed, see crates/fogata-ffi/src/bridge.rs) are for the log, and
/// tell people nothing.
String explain(Object error) {
  if (error is PanicException) return l10n.errSomething;
  var text = error is AnyhowException ? error.message : error.toString();
  text = text.split('\n').first.trim();
  if (text.contains(' panicked') ||
      text.contains('an internal error stopped the work')) {
    return l10n.errSomething;
  }
  final full = RegExp('$_roomFull: (\\d+)').firstMatch(text);
  if (full != null) return l10n.errRoomFull(int.parse(full.group(1)!));
  for (final entry in _known.entries) {
    if (text.contains(entry.key)) return entry.value();
  }
  if (text.isEmpty) return l10n.errSomething;
  // Anything else as a sentence, in the core's words.
  final sentence = capitalized(text);
  return RegExp(r'[.!?…]$').hasMatch(sentence) ? sentence : '$sentence.';
}

/// The core's words for a voice room that is full, before the number of people it takes.
const _roomFull = 'voice room is full';

/// The core's known messages, found by these parts of their words (see [explain]).
final _known = <String, String Function()>{
  'this link has expired': () => l10n.errLinkExpired,
  'this is a group invite': () => l10n.errGroupInvite,
  'this device is already in that group': () => l10n.errAlreadyInGroup,
  'join the room first': () => l10n.errJoinRoomFirst,
  'saved by a newer version of Fogata': () => l10n.errNewerVersion,
  'the system keychain cannot be reached': () => l10n.errKeychainUnreachable,
  'the system keychain does not hold the key': () => l10n.errKeychainNoKey,
  'the key in the system keychain does not open': () =>
      l10n.errKeychainWrongKey,
  'does not have the group key yet': () => l10n.errNoGroupKey,
  'Fogata is already running with the data': () => l10n.errAlreadyRunning,
  'that file is no longer in the room': () => l10n.errFileGone,
  'nobody connected has it': () => l10n.errNobodyHasFile,
  'Fogata is not running yet': () => l10n.errStillStarting,
  'already known by its key': () => l10n.errIdentityExists,
  'a name needs': () => l10n.errNameRule,
  'a bio has at most': () => l10n.errBioRule,
  'the picture must be': () => l10n.errPictureRule,
  'not a Fogata friend link': () => l10n.errNotFriendLink,
  'friend link is damaged': () => l10n.errLinkDamaged,
  'needs a newer version': () => l10n.errLinkNeedsNewer,
  'must update Fogata before they can join': () => l10n.errMustUpdateToJoin,
  'a friend code looks like': () => l10n.errCodeShape,
  'the code is wrong': () => l10n.errCodeWrong,
  'did not prove who it is': () => l10n.errNotProven,
  'that call no longer rings': () => l10n.errCallGone,
  'you are already in a call': () => l10n.errAlreadyInCall,
  'you are already friends': () => l10n.errAlreadyFriends,
  'this member is not connected now': () => l10n.errMemberNotConnected,
  'too old to take friend requests from a group': () => l10n.errMemberTooOld,
  'the friend request did not reach the member': () => l10n.errMemberAskLost,
  'this link was made on this device': () => l10n.errOwnLink,
  'the link was already used or has expired': () => l10n.errLinkUsed,
  'the link expired before its device could be reached': () =>
      l10n.errLinkExpiredUnreached,
  'no device answered the code': () => l10n.errCodeUnanswered,
  'the other device refused the request': () => l10n.errRefused,
  'not someone below the author': () => l10n.errNotBelow,
  'author does not manage the group': () => l10n.errNotManager,
  'would leave its author out': () => l10n.errLastManager,
  'people are in that room': () => l10n.errRoomNotEmpty,
  'files in this room already': () => l10n.errTooManyFiles,
  'No space left on device': () => l10n.errDiskFull,
  'not enough space on the disk': () => l10n.errDiskFull,
  'wrote too much that no other member signed on yet': () =>
      l10n.errTooManyChanges,
  'not enough free space for this file': () => l10n.errNoSpaceForFile,
  'not enough free space to share this file': () => l10n.errNoSpaceForFile,
  ..._notLetIn,
};

/// Every part of the core's messages that [explain] finds, as app/test/core_errors.txt
/// lists them for the core's own check that it still writes each one.
@visibleForTesting
Set<String> get explainedParts => {_roomFull, ..._known.keys};

/// Why a manager's yes did not let someone into a group, as the core notes it.
final _notLetIn = <String, String Function()>{
  'that invite no longer works, or the group is full': () =>
      l10n.errJoinInviteGone,
  'this device has no group key yet': () => l10n.errJoinNoKeyYet,
  'this device wrote more than the others have seen yet': () =>
      l10n.errJoinWroteTooMuch,
  'was not made on this group as it is': () => l10n.errJoinAskAgain,
};

/// A note of the core ([UiEventKind.info]) that says why someone this device let into a
/// group did not get in, in words for people; null for any other note.
String? notLetIn(String text) {
  for (final entry in _notLetIn.entries) {
    if (text.contains(entry.key)) return entry.value();
  }
  return null;
}

/// Which system's way the firewall's command is run: it needs an administrator's rights
/// on Windows, and asks for the password itself elsewhere (`sudo`). For messages that
/// choose their words by it: `windows`, `macos` or `other`.
String get firewallSystem => onWindows
    ? 'windows'
    : onMacOS
    ? 'macos'
    : 'other';

/// A network finding in words for people.
String findingText(UiFinding f) => switch (f.topic) {
  UiTopic.offline => l10n.findingOffline,
  UiTopic.internet => l10n.findingInternet(f.count),
  UiTopic.dns => l10n.findingDns(f.text.replaceFirst('DNS: ', '')),
  UiTopic.udpWorks => l10n.findingUdpWorks,
  UiTopic.udpBlocked => l10n.findingUdpBlocked,
  UiTopic.carrierNat => l10n.findingCarrierNat,
  UiTopic.publicIpv4 => l10n.findingPublicIpv4,
  UiTopic.hardNat => l10n.findingHardNat,
  UiTopic.easyNat => l10n.findingEasyNat,
  UiTopic.unknownRouter => l10n.findingUnknownRouter,
  UiTopic.noIpv6 => l10n.findingNoIpv6,
  UiTopic.ipv6Works => l10n.findingIpv6Works,
  UiTopic.ipv6Unreached => l10n.findingIpv6Unreached,
  UiTopic.lanDevices => l10n.findingLanDevices(f.count),
  UiTopic.firewall => l10n.findingFirewall(firewallSystem),
  UiTopic.summaryNoUdp => l10n.findingSummaryNoUdp,
  UiTopic.summaryPublic => l10n.findingSummaryPublic,
  UiTopic.summaryEasyNat => l10n.findingSummaryEasyNat,
  UiTopic.summaryHardNatWithIpv6 => l10n.findingSummaryHardNatWithIpv6,
  UiTopic.summaryHardNat => l10n.findingSummaryHardNat,
  UiTopic.summaryUnknown => l10n.findingSummaryUnknown,
};

/// `text` with its first letter in capitals.
String capitalized(String text) =>
    text.isEmpty ? text : text[0].toUpperCase() + text.substring(1);
