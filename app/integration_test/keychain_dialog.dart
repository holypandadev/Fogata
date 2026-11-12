// What the integration tests share for the welcome page's keychain question: the test
// copies run without a keychain (`DBUS_SESSION_BUS_ADDRESS=disabled:`), so the welcome page
// opens a dialog as soon as it shows, and nothing under it can be tapped until the test
// chooses "Continue unencrypted".

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// Call it when the welcome page shows or is about to (it waits for the page): answers
/// the keychain dialog with "Continue unencrypted" every time it appears, and returns
/// when the page has made its key (the placeholder fingerprint is gone). With a keychain that keeps the key the dialog never
/// shows and this returns as soon as the key does. When neither happens in [seconds] it
/// just returns: the caller's own waits report the failure.
Future<void> continueUnencryptedIfAsked(
  WidgetTester tester, {
  int seconds = 60,
}) async {
  final end = DateTime.now().add(Duration(seconds: seconds));
  final plain = find.byKey(const ValueKey('keychain-plain'));
  // The key shows as dots (one widget per character) until it is made.
  final placeholder = find.text('·');
  var seen = false;
  while (DateTime.now().isBefore(end)) {
    if (placeholder.evaluate().isNotEmpty) seen = true;
    if (plain.evaluate().isNotEmpty) {
      // The dialog fades in; a tap before it is solid misses the button.
      for (var i = 0; i < 10; i++) {
        await Future<void>.delayed(const Duration(milliseconds: 50));
        await tester.pump();
      }
      seen = true;
      // It can be gone again by now (the page made it anew): the next turn finds it.
      if (plain.evaluate().isEmpty) continue;
      await tester.tap(plain.first);
      await tester.pump(const Duration(milliseconds: 300));
    } else if (seen && placeholder.evaluate().isEmpty) {
      return;
    }
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}
