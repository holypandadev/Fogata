import 'dart:async';
import 'dart:ui';

import '../direct_state.dart';
import '../l10n.dart';
import '../rust/api/direct.dart' as dm;
import '../rust/api/types.dart';
import 'piece.dart';
import 'prefs.dart';

/// Something for the bell: a missed call, a friend request, someone asking to join a
/// group this device manages, a group a friend added it to, messages from a friend
/// not read yet, this identity open on another computer too, or someone who declined an
/// invitation of this device to a group.
enum NoticeKind {
  missedCall,
  friendRequest,
  joinAsk,
  groupInvite,
  firewall,
  directMessage,
  otherSession,
  invitationDeclined,
  warning,
}

class Notice {
  Notice(
    this.kind,
    this.key,
    this.atMs, {
    this.person,
    this.request,
    this.ask,
    this.invite,
    this.chat,
    this.declined,
    this.warning,
  });
  final NoticeKind kind;

  /// Tells it from the others, to dismiss it.
  final String key;
  final int atMs;

  /// Who called or asks to be a friend.
  final UiPerson? person;
  final UiRequest? request;
  final UiJoinAsk? ask;
  final UiGroupInvite? invite;
  final dm.UiDirectChat? chat;
  final DeclinedInvitation? declined;
  final Warning? warning;
}

/// Something about this computer the person should know, said at start: settings that
/// could not be read, secrets kept without a keychain. In the bell until dismissed.
class Warning {
  Warning(this.key, this.title, this.text, this.atMs);
  final String key;
  final String title;
  final String text;
  final int atMs;
}

/// Someone ([person]) declined an invitation of this device to a group.
class DeclinedInvitation {
  DeclinedInvitation(this.group, this.groupName, this.person, this.atMs);
  final String group;
  final String groupName;
  final UiPerson person;
  final int atMs;

  String get key => 'declined:$group:${person.id}:$atMs';
  String get said => l10n.noticesDeclined(person.name, groupName);

  /// [said] as a toast says it.
  String get toast => l10n.noticesDeclinedToast(person.name, groupName);
}

/// What the bell reads its notices from and never changes, besides the direct chats:
/// friends, requests, people asking to join and invitations.
abstract interface class NoticeSources {
  List<UiFriend> get friends;
  UiRequests get requests;
  List<UiJoinAsk> get joinAsks;
  List<UiGroupInvite> get invitations;
}

/// The bell and what it tells: missed calls (also in Friends and in a corner note),
/// invitations declined, this identity open elsewhere and the firewall's command, beside
/// what it reads from [NoticeSources] and the direct chats.
class Notices extends Piece {
  Notices(this._from, this._direct, this._prefs, this._chime);

  final NoticeSources _from;
  final DirectChats _direct;
  final Prefs _prefs;

  /// Plays the notice's sound.
  final VoidCallback _chime;

  /// Calls missed, by friend id, with when; and whether some were not seen in Friends yet.
  final Map<String, int> missedCalls = {};
  bool missedUnseen = false;

  /// The friends whose last missed call rang while this device was in another call.
  final Set<String> missedInCall = {};

  /// The friends whose last missed call rang while this device appeared offline.
  final Set<String> missedOffline = {};

  /// The friend whose missed call shows in the corner. Like a toast it goes by itself
  /// after [missedNoteFor] (not while the pointer is on it), or with a click; the call
  /// stays in the bell and in Friends.
  String? missedNote;

  /// The missed call's card is fading out.
  bool missedNoteLeaving = false;
  Timer? _missedNoteTimer;
  static const missedNoteFor = Duration(seconds: 8);

  /// The command that lets Fogata through this computer's own firewall, when that lets
  /// nothing in for it (Linux with ufw), and when it was found.
  String? firewallCommand;
  int firewallSinceMs = 0;

  /// When this run was told that its identity is open on another computer too, and how
  /// it was said; in the bell until dismissed, besides the toast.
  int? otherSessionMs;
  String? otherSessionText;

  /// Invitations of this device that someone declined, as this run heard of them; in the
  /// bell until dismissed, besides the toast.
  final List<DeclinedInvitation> declines = [];

  /// What this run found about this computer at start ([Warning]).
  final List<Warning> warnings = [];

  /// When each group invitation was first seen, for the bell's order.
  final Map<String, int> invitationSeen = {};

  /// The bounds of the bell while its list is open, to open it under it.
  Rect? bellFrom;

  /// When the bell was opened before this time: newer notices show as new in its list.
  int newSince = 0;

  /// Whether every kind of notice was read from the core once. Until then a dismissed
  /// notice is kept dismissed though nothing shows it yet.
  bool known = false;

  /// The bell's notices, newest first: missed calls, friend requests, people asking to
  /// join a group, groups a friend added this device to and invitations declined.
  List<Notice> get list {
    final all = _all()
      ..removeWhere((n) => _prefs.dismissedNotices.contains(n.key));
    all.sort((a, b) => b.atMs.compareTo(a.atMs));
    return all;
  }

  /// The keys of every notice there is, the dismissed ones too, once [known].
  Iterable<String>? get keysNow =>
      known ? [for (final n in _all()) n.key] : null;

  /// [list], the dismissed ones too, in no order.
  List<Notice> _all() {
    UiPerson? friend(String id) {
      for (final f in _from.friends) {
        if (f.person.id == id) return f.person;
      }
      return null;
    }

    final direct = _direct;
    final all = <Notice>[
      for (final MapEntry(key: id, value: at) in missedCalls.entries)
        if (friend(id) case final person?)
          Notice(NoticeKind.missedCall, 'call:$id:$at', at, person: person),
      for (final r in _from.requests.incoming)
        Notice(
          NoticeKind.friendRequest,
          'friend:${r.person.id}',
          r.sinceMs,
          person: r.person,
          request: r,
        ),
      for (final a in _from.joinAsks)
        Notice(
          NoticeKind.joinAsk,
          'join:${a.groupId}:${a.deviceId}',
          a.sinceMs,
          ask: a,
        ),
      if (firewallCommand case final command?)
        Notice(NoticeKind.firewall, 'firewall:$command', firewallSinceMs),
      if (otherSessionMs case final at?)
        Notice(NoticeKind.otherSession, 'session:$at', at),
      for (final w in warnings)
        Notice(NoticeKind.warning, w.key, w.atMs, warning: w),
      for (final d in declines)
        Notice(
          NoticeKind.invitationDeclined,
          d.key,
          d.atMs,
          person: d.person,
          declined: d,
        ),
      for (final i in _from.invitations)
        Notice(
          NoticeKind.groupInvite,
          'group:${i.id}',
          invitationSeen[i.id] ?? 0,
          invite: i,
        ),
      for (final c in direct.chats)
        if (c.unread > 0 && c.endedMs == null && !direct.reading(c.friend.id))
          Notice(
            NoticeKind.directMessage,
            'direct:${c.friend.id}:${c.lastMs}',
            direct.cameMs(c),
            person: c.friend,
            chat: c,
          ),
    ];
    return all;
  }

  /// Notices the bell has not shown yet.
  int get unseen => list.where((n) => n.atMs > _prefs.noticesSeenMs).length;

  /// What a missed call from `person` says: who called, and when this device was in
  /// another call or appeared offline.
  String missedCallTitle(UiPerson person) => missedInCall.contains(person.id)
      ? l10n.noticesMissedInCall(person.name)
      : missedOffline.contains(person.id)
      ? l10n.noticesMissedOffline(person.name)
      : l10n.noticesMissedCall(person.name);

  /// Shows `device`'s missed call in the corner.
  void noteMissed(String device) {
    missedNote = device;
    missedNoteLeaving = false;
    _armMissedNote();
  }

  void _armMissedNote() {
    _missedNoteTimer?.cancel();
    final device = missedNote;
    if (device == null) return;
    _missedNoteTimer = Timer(missedNoteFor, () {
      if (missedNote == device) dismissMissedNote();
    });
  }

  /// The pointer is on the missed call's card: it stays until the pointer leaves.
  void holdMissedNote(bool held) {
    if (held) {
      _missedNoteTimer?.cancel();
    } else if (!missedNoteLeaving) {
      _armMissedNote();
    }
  }

  /// Takes the missed call's card away, fading as a toast does; the call stays in the
  /// bell.
  void dismissMissedNote() {
    final device = missedNote;
    _missedNoteTimer?.cancel();
    if (device == null || missedNoteLeaving) return;
    missedNoteLeaving = true;
    changed();
    Timer(const Duration(milliseconds: 260), () {
      // Another missed call came meanwhile: that one stays.
      if (missedNote != device || !missedNoteLeaving) return;
      missedNote = null;
      missedNoteLeaving = false;
      changed();
    });
  }

  /// The bell's notices already sounded for, by key; null until the first refreshes after
  /// starting, so what was waiting from before makes no sound.
  Set<String>? _sounded;
  int _lastSound = 0;

  /// What the bell holds, by key; the firewall's notice is the computer's, not news.
  Set<String> _keys() => {
    for (final n in list)
      if (n.kind != NoticeKind.firewall) n.key,
  };

  /// From now on, notices the bell gets make a sound; those it holds already do not.
  void listen() => _sounded = _keys();

  /// A chime when the bell gets a notice it did not have: a missed call, a friend request,
  /// someone asking to join, a group a friend added this device to. At most one every two
  /// seconds.
  void soundForNew() {
    final sounded = _sounded;
    if (sounded == null) return;
    final keys = _keys();
    final now = DateTime.now().millisecondsSinceEpoch;
    if (!sounded.containsAll(keys) && now - _lastSound >= 2000) {
      _lastSound = now;
      _chime();
    }
    _sounded = keys;
  }

  @override
  void dispose() {
    _missedNoteTimer?.cancel();
    super.dispose();
  }
}
