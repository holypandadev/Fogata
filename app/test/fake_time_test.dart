// Fake time leaves no timer behind unless told why.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';

import 'fake_time.dart';

/// [check]'s failure, or null when it passed.
Future<String?> failureOf(Future<void> Function() check) async {
  try {
    await check();
    return null;
  } on TestFailure catch (e) {
    return e.message;
  }
}

void main() {
  test('fake time fails on a timer left behind, saying how many', () async {
    final failure = await failureOf(
      () => inFakeTime((_) async {
        Timer(const Duration(minutes: 5), () {});
        Timer.periodic(const Duration(seconds: 1), (_) {});
      }),
    );
    expect(failure, contains('2 timers left pending when it ended'));
  });

  test('fake time takes timers left by what it is told of', () async {
    void leftOnPurpose() => Timer(const Duration(minutes: 5), () {});
    expect(
      await failureOf(
        () => inFakeTime(
          (_) async => leftOnPurpose(),
          timersLeftBy: {'leftOnPurpose': 'a test of the check'},
        ),
      ),
      isNull,
    );
  });
}
