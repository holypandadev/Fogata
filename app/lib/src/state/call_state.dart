import 'package:flutter/foundation.dart';

import '../call.dart';
import '../l10n.dart';
import '../rust/api/app.dart' as api;
import '../rust/api/types.dart';
import 'piece.dart';

/// How a direct call ended, kept while its page still shows it.
class CallEnding {
  CallEnding(this.group, this.person, this.reason);
  final String group;
  final UiPerson person;
  final UiCallEnd reason;

  /// Why the call did not go through, in a few words.
  String get title => switch (reason) {
    UiCallEnd.noAnswer => l10n.callStateNoAnswer(person.name),
    UiCallEnd.declined => l10n.callStateDeclined(person.name),
    UiCallEnd.busy => l10n.callStateBusy(person.name),
    UiCallEnd.outdated => l10n.callStateOutdated(person.name),
    _ => l10n.callStateUnreached(person.name),
  };
}

/// What the room's call tells as it goes: who connects, mutes, shares, watches. Only what
/// shows the call's people listens here; who is talking is told apart again, by
/// [Call.talking].
class LiveCall extends Piece {}

/// The call of the room this device is in and the direct calls ringing or going on. What
/// the call tells as it goes is told to [live] instead.
class CallState extends Piece {
  /// The call of the room this device is in. Its own changes are told to [live].
  Call? get call => _call;
  set call(Call? value) {
    if (identical(value, _call)) return;
    _call?.removeListener(live.changed);
    _call = value;
    value?.addListener(live.changed);
    changed();
    live.changed();
  }

  Call? _call;

  /// Told what the call tells as it goes.
  final live = LiveCall();

  /// How to enter the next room with the camera: it starts off every time.
  bool joinWithCamera = false;

  /// Not hearing anyone (deafened): the voices and the shared screens' sound
  /// stop playing here, and the microphone goes off; hearing again brings the microphone
  /// back as it was.
  bool deafened = false;

  /// Direct calls ringing or going on.
  List<UiCall> calls = [];

  /// Friends still in a call this device left: calling one goes back into that call.
  Set<String> callReturns = {};

  /// How the call shown last ended, while its page says so (no answer, declined…).
  CallEnding? callEnding;

  /// The ringing call put aside into the corner, by its group.
  String? ringAside;

  /// The call this device rings or is in.
  UiCall? get activeCall {
    for (final c in calls) {
      if (c.phase != UiCallPhase.incoming) return c;
    }
    return null;
  }

  /// A call ringing here, waiting for an answer.
  UiCall? get incomingCall {
    for (final c in calls) {
      if (c.phase == UiCallPhase.incoming) return c;
    }
    return null;
  }

  /// The call whose room is `group`.
  UiCall? callIn(String? group) {
    for (final c in calls) {
      if (c.groupId == group) return c;
    }
    return null;
  }

  /// What a button that calls `friend` says: going back into a call this device left
  /// while the friend is still in it, calling back after a missed call, or calling.
  String callText(String friend, {bool missed = false}) =>
      callReturns.contains(friend)
      ? l10n.callStateReturn
      : missed
      ? l10n.callStateCallBack
      : l10n.callStateCall;

  /// Called when a friend's call starts ringing here, to bring back a window hidden in the
  /// tray.
  VoidCallback? onRing;

  /// Starts the ring or the ringback, or stops it with null: the sound player plays it
  /// again every bar until told otherwise. Tests give their own.
  void Function(UiSound? sound) ringer = _ringOut;

  static void _ringOut(UiSound? sound) {
    try {
      api.playRing(sound: sound);
    } catch (_) {}
  }

  /// What rings now: the ring while a call rings here, or the ringback while this
  /// device rings.
  UiSound? get ringing => _ringing;
  UiSound? _ringing;

  /// Calls answered, declined or hung up here that may still show as ringing until the
  /// core says otherwise: they ring no more.
  final Set<String> _hushed = {};

  /// Keeps the right ring playing: a friend's call ringing here first.
  void ring() {
    _hushed.removeWhere(
      (group) => !calls.any(
        (c) =>
            c.groupId == group &&
            (c.phase == UiCallPhase.incoming || c.phase == UiCallPhase.ringing),
      ),
    );
    bool rings(UiCall c, UiCallPhase phase) =>
        c.phase == phase && !_hushed.contains(c.groupId);
    final want = calls.any((c) => rings(c, UiCallPhase.incoming))
        ? UiSound.ring
        : calls.any((c) => rings(c, UiCallPhase.ringing))
        ? UiSound.ringback
        : null;
    if (want == _ringing) return;
    _ringing = want;
    // Calls ring whether or not the room's sounds are on.
    ringer(want);
    if (want == UiSound.ring) onRing?.call();
  }

  /// Stops the ring of the call in `group` at once, as it is answered, declined or hung
  /// up here, before the core is done with it.
  void hush(String group) {
    _hushed.add(group);
    ring();
  }

  /// Lets the call in `group` ring again: what was done with it did not go through.
  void unhush(String group) {
    if (_hushed.remove(group)) ring();
  }

  @override
  void dispose() {
    if (_ringing != null) ringer(null);
    _call?.removeListener(live.changed);
    _call = null;
    super.dispose();
  }
}
