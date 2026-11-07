import 'dart:async';

import 'piece.dart';

/// Something said in passing at the bottom of the window.
class Toast {
  Toast(this.text, {this.error = false, this.path});
  final String text;
  final bool error;

  /// A file the toast is about: it can be shown in its folder, and its path copied.
  final String? path;
  final int id = _next++;
  static int _next = 0;

  /// Fading out, about to be removed.
  bool leaving = false;
}

/// The toasts shown at the bottom of the window. Only they build again when one comes or
/// goes.
class ToastBoard extends Piece {
  final List<Toast> list = [];

  /// The toasts' timers, to stop with the window.
  final Set<Timer> _timers = {};

  /// Runs [then] after [wait], unless the window goes first.
  void _after(Duration wait, void Function() then) {
    late final Timer timer;
    timer = Timer(wait, () {
      _timers.remove(timer);
      then();
    });
    _timers.add(timer);
  }

  void say(String text, {bool error = false, String? path}) {
    final toast = Toast(text, error: error, path: path);
    list.add(toast);
    changed();
    // One with a file stays long enough to use its buttons.
    _after(Duration(seconds: error ? 7 : (path != null ? 10 : 4)), () {
      toast.leaving = true;
      changed();
      _after(const Duration(milliseconds: 260), () {
        list.remove(toast);
        changed();
      });
    });
  }

  @override
  void dispose() {
    for (final t in _timers) {
      t.cancel();
    }
    super.dispose();
  }
}
