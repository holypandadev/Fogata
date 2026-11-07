import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../format.dart';
import '../l10n.dart';
import '../rust/api/account_move.dart' as api;
import '../rust/api/types.dart';

/// Where a move of the account between two computers is, as the screens follow it: on the
/// new computer ([sending] false) from the code shown to the account in place, on the old
/// one from the code typed to "moved".
enum MoveStep { starting, looking, matched, moving, finishing, done, failed }

class AccountMove extends ChangeNotifier {
  AccountMove({required this.sending});

  /// This computer has the account and sends it.
  final bool sending;

  MoveStep step = MoveStep.starting;

  /// New computer: the code shown, and until when it works.
  String? code;
  int expiresMs = 0;

  /// The other computer's name, whether the two share a local network, and the profile
  /// the new computer has now (replaced by the move).
  String? computer;
  bool lan = false;
  String? replaces;
  api.UiMoveSummary? summary;

  /// New computer: the fingerprint of the profile this computer ran when the move began,
  /// to tell an offer of that same account.
  String? ownFingerprint;

  /// Bytes gone or come, of all, and when the first went.
  int done = 0;
  int total = 0;
  int? _firstAtMs;

  /// New computer: wrong codes tried against the one shown.
  int wrongTries = 0;

  /// This computer's person said yes to the move shown.
  bool answered = false;

  /// Why it stopped: `wrong_code`, `expired`, `too_many_tries`, `no_contact`,
  /// `interrupted`, `unconfirmed`, `cancelled`, `declined`, `later` or `other`; then the
  /// reason the screens word, or else the words that came with it (the system's, or the
  /// other computer's), and the bytes a reason tells of.
  String? failure;
  api.UiMoveReason? reason;
  String? detail;
  int need = 0;
  int free = 0;
  int offered = 0;

  StreamSubscription<api.UiMoveEvent>? _events;
  bool _disposed = false;

  bool get over => step == MoveStep.done || step == MoveStep.failed;

  /// The account offered is the one this computer already runs.
  bool get sameAccount {
    String plain(String f) => f.replaceAll(RegExp(r'\s'), '').toUpperCase();
    final own = ownFingerprint;
    final offered = summary?.profile.fingerprint;
    return own != null &&
        offered != null &&
        own.isNotEmpty &&
        plain(own) == plain(offered);
  }

  /// What the person sees of what moves.
  UiProfile? get profile => summary?.profile;

  /// Follows the move's [events]. Should they end, or fail, before the move did, nothing
  /// more would come: it is over and failed, rather than waited for for good. A move
  /// cancelled or let go of here stopped following them first.
  void follow(Stream<api.UiMoveEvent> events) {
    late final StreamSubscription<api.UiMoveEvent> followed;
    bool following() => !_disposed && identical(_events, followed) && !over;
    followed = events.listen(
      _on,
      onError: (Object e) {
        if (following()) fail('other', explain(e));
      },
      onDone: () {
        if (following()) fail('other');
      },
    );
    _events = followed;
  }

  void fail(String kind, [String? text]) {
    failure = kind;
    detail = text;
    step = MoveStep.failed;
    changed();
  }

  void _on(api.UiMoveEvent e) {
    switch (e.kind) {
      case api.UiMoveEventKind.code:
        code = e.code;
        expiresMs = e.expiresMs;
        step = MoveStep.looking;
      case api.UiMoveEventKind.looking:
        if (step == MoveStep.starting) step = MoveStep.looking;
      case api.UiMoveEventKind.wrongCodeTried:
        wrongTries++;
      case api.UiMoveEventKind.matched:
        computer = e.computer;
        lan = e.lan;
        replaces = e.replaces;
        summary = e.summary;
        step = MoveStep.matched;
      case api.UiMoveEventKind.progress:
        done = e.done;
        total = e.total;
        _firstAtMs ??= DateTime.now().millisecondsSinceEpoch;
        step = MoveStep.moving;
      case api.UiMoveEventKind.finishing:
        done = total;
        step = MoveStep.finishing;
      case api.UiMoveEventKind.done:
        computer = (e.computer?.isNotEmpty ?? false) ? e.computer : computer;
        summary = e.summary ?? summary;
        done = total;
        step = MoveStep.done;
      case api.UiMoveEventKind.failed:
        failure = e.failure;
        reason = e.reason;
        detail = e.detail;
        need = e.need;
        free = e.free;
        offered = e.offered;
        step = MoveStep.failed;
    }
    changed();
  }

  /// The share gone so far, from 0 to 1.
  double get share => total <= 0 ? 0 : (done / total).clamp(0, 1).toDouble();

  /// How much longer, at the pace so far: `about a minute left`; null until it can tell.
  String? get timeLeftWords {
    final first = _firstAtMs;
    if (first == null || done <= 0 || total <= done) return null;
    final spent = DateTime.now().millisecondsSinceEpoch - first;
    if (spent < 1500) return null;
    final left = (total - done) * spent ~/ done;
    if (left < 45 * 1000) return l10n.accountMoveLessThanMinuteLeft;
    if (left < 90 * 1000) return l10n.accountMoveAboutMinuteLeft;
    return l10n.accountMoveAboutMinutesLeft((left / 60000).round());
  }

  /// Why it stopped, beyond its kind, as a sentence: the reason in words, or the words
  /// that came with it; null with neither.
  String? get reasonWords {
    final words = switch (reason) {
      null => null,
      api.UiMoveReason.noRoom => l10n.accountMoveNoRoom(
        bytes(need),
        bytes(free),
      ),
      api.UiMoveReason.tooLarge => l10n.accountMoveTooLarge(bytes(offered)),
      api.UiMoveReason.busy => l10n.accountMoveBusy,
      api.UiMoveReason.nothingToFinish => l10n.accountMoveNothingToFinish,
      api.UiMoveReason.networkStopped => l10n.accountMoveNetworkStopped,
      api.UiMoveReason.connectionLost => l10n.accountMoveConnectionLost,
      api.UiMoveReason.peerNoRoom => l10n.accountMovePeerNoRoom,
      api.UiMoveReason.peerTooLarge => l10n.accountMovePeerTooLarge,
      api.UiMoveReason.peerBusy => l10n.accountMovePeerBusy,
      api.UiMoveReason.peerCouldNotKeep => l10n.accountMovePeerCouldNotKeep,
      api.UiMoveReason.peerCancelled => l10n.accountMovePeerCancelled,
      api.UiMoveReason.peerCouldNotFinish => l10n.accountMovePeerCouldNotFinish,
      api.UiMoveReason.peerStopped => l10n.accountMovePeerStopped,
      api.UiMoveReason.peerNothingToFinish =>
        l10n.accountMovePeerNothingToFinish,
      api.UiMoveReason.peerNotProven => l10n.accountMovePeerNotProven,
    };
    if (words != null) return words;
    final text = detail?.trim() ?? '';
    if (text.isEmpty) return null;
    // The system's or the other computer's own words, as a sentence.
    final sentence = capitalized(text);
    return RegExp(r'[.!?…]$').hasMatch(sentence) ? sentence : '$sentence.';
  }

  /// Why it stopped, in a sentence or two: what happened, then why when known.
  String get failureWords {
    final why = reasonWords;
    return switch (failure) {
      'wrong_code' => l10n.accountMoveWrongCode,
      'expired' => l10n.accountMoveExpired,
      'too_many_tries' => l10n.accountMoveTooManyTries,
      'no_contact' => l10n.accountMoveNoContact,
      'unconfirmed' => l10n.accountMoveUnconfirmed,
      'cancelled' => l10n.accountMoveCancelled,
      'interrupted' when why != null => l10n.accountMoveInterruptedWhy(why),
      'interrupted' => l10n.accountMoveInterrupted,
      'declined' when why != null => l10n.accountMoveDeclinedWhy(why),
      'declined' => l10n.accountMoveDeclined,
      'later' when why != null => l10n.accountMoveLaterWhy(why),
      'later' => l10n.accountMoveLater,
      _ => why ?? l10n.accountMoveDidNotFinish,
    };
  }

  /// This computer's person says yes, or no, to the move shown: on each computer, before
  /// anything goes.
  void confirm(bool yes) {
    api.moveConfirm(yes: yes);
    if (!yes) {
      fail('cancelled');
      return;
    }
    answered = true;
    changed();
  }

  /// Stops the move, and whether it stops: nothing changes on either computer then. Once
  /// its end began (the account staged on the new computer, or the old one no longer using
  /// it) it goes on to its end.
  bool cancel() {
    if (over) return true;
    if (!api.moveCancel()) return false;
    _events?.cancel();
    fail('cancelled');
    return true;
  }

  /// Tells those following the move that it changed.
  void changed() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _events?.cancel();
    super.dispose();
  }
}

/// What this computer is called, as the other computer shows it: the computer's network
/// name without the local network's ending.
String computerName() {
  try {
    final name = Platform.localHostname.trim();
    final short = name.endsWith('.local')
        ? name.substring(0, name.length - 6)
        : name;
    return short.isEmpty ? l10n.accountMoveAnotherComputer : short;
  } catch (_) {
    return l10n.accountMoveAnotherComputer;
  }
}

/// The other computer's name as it came, or words for it when it gave none that shows
/// (older versions sent their own English words for that).
String computerShown(String? name) =>
    name == null || name.trim().isEmpty || name == 'the other computer'
    ? l10n.accountMoveTheOtherComputer
    : name;
