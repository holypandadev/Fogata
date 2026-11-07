import 'package:flutter/foundation.dart';

/// A part of what the window holds, told apart from the rest: what shows only this part
/// listens to it alone (see `AppScope.watch`), so a change elsewhere does not build it
/// again.
abstract class Piece extends ChangeNotifier {
  bool _disposed = false;

  /// Tells what shows this part that it changed. Work that was waiting on the core may end
  /// after the window let it go: it tells nobody then.
  void changed() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
