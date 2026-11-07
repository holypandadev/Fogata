import 'dart:async';

import 'package:fogata/src/host.dart';

/// Every test sees the same system, whichever computer runs it: the interface takes its
/// font, title bar, shortcuts and some words from the system ([host]), and the tests
/// measure Inter's text (fake_room.dart's loadFonts) and Linux's title bar. A test of
/// another system's way pins that one for a while.
Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  pinnedHost = HostSystem.linux;
  await testMain();
}
