import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';

import 'rust/api/app.dart' as api;

bool _watching = false;
final _since = Stopwatch();

/// Tells the core when someone uses the mouse or the keyboard in the window, at most once a
/// second. Where the system says how long the whole computer has gone unused, this adds
/// nothing; where it does not (a compositor without idle notifications), it is what keeps
/// this device from showing as away while the person uses Fogata.
void watchInput() {
  if (_watching) return;
  _watching = true;
  GestureBinding.instance.pointerRouter.addGlobalRoute((_) => _seen());
  HardwareKeyboard.instance.addHandler((_) {
    _seen();
    return false;
  });
}

void _seen() {
  if (_since.isRunning && _since.elapsed < const Duration(seconds: 1)) return;
  _since
    ..reset()
    ..start();
  try {
    api.noteInput();
  } catch (_) {}
}
