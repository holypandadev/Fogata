import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/welcome.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

/// A keychain that keeps no key until [works], and what the welcome page asked of it.
class FakeCore implements RustLibApi {
  bool works = false;
  int asked = 0;
  bool keptPlain = false;
  int keys = 0;
  int newKeys = 0;
  int created = 0;
  int started = 0;

  /// Making the profile waits on it, and then works, giving [made].
  Completer<void>? creating;
  UiProfile? made;

  /// Asking the keychain never ends, or fails.
  Completer<void>? hangs;
  bool throws = false;

  @override
  Future<String?> crateApiAppKeychainProblem({required String dataDir}) async {
    asked++;
    await hangs?.future;
    if (throws) throw StateError('the keychain could not be asked');
    return works || keptPlain ? null : 'the keychain is locked';
  }

  @override
  Future<void> crateApiAppKeepUnencrypted({required String dataDir}) async =>
      keptPlain = true;

  @override
  Future<String> crateApiAppDeviceFingerprint({required String dataDir}) async {
    keys++;
    return '7Q4M K2XR 9PLD 3VHC';
  }

  @override
  Future<String> crateApiAppNewDeviceKey({required String dataDir}) async {
    newKeys++;
    return '1111 2222 3333 4444';
  }

  @override
  Future<UiProfile> crateApiAppCreateProfile({
    required String dataDir,
    required String name,
    required int preset,
    Uint8List? photo,
  }) async {
    created++;
    if (creating case final creating?) {
      await creating.future;
      return made!;
    }
    throw StateError('stop here');
  }

  @override
  Future<UiProfile> crateApiAppStart({required String dataDir}) async {
    started++;
    throw StateError('stop here');
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

final core = FakeCore();

Future<void> _show(WidgetTester tester) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1440, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    AppScope(
      state: fakeRoom(people: 1),
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Welcome()),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 500));
}

void main() {
  setUpAll(() => RustLib.initMock(api: core));

  testWidgets(
    'no key of the profile is made before the person chooses how data is kept',
    (tester) async {
      core
        ..works = false
        ..keptPlain = false
        ..asked = 0
        ..keys = 0;
      await _show(tester);
      expect(find.byKey(const ValueKey('keychain-problem')), findsOneWidget);
      expect(core.keys, 0);
      // Tried again while the keychain still keeps nothing: asked again.
      await tester.tap(find.text('Try again'));
      await tester.pump(const Duration(milliseconds: 500));
      expect(core.asked, 2);
      expect(find.byKey(const ValueKey('keychain-problem')), findsOneWidget);
      expect(core.keys, 0);
      // The keychain works now: the key is made, encrypted.
      core.works = true;
      await tester.tap(find.text('Try again'));
      await tester.pump(const Duration(milliseconds: 500));
      expect(find.byKey(const ValueKey('keychain-problem')), findsNothing);
      expect(core.keys, 1);
      expect(core.keptPlain, false);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 2));
    },
  );

  testWidgets('continuing unencrypted is a choice the core is told of', (
    tester,
  ) async {
    core
      ..works = false
      ..keptPlain = false
      ..asked = 0
      ..keys = 0;
    await _show(tester);
    await tester.tap(find.text('Continue unencrypted'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(core.keptPlain, true);
    expect(core.keys, 1);
    expect(find.byKey(const ValueKey('keychain-problem')), findsNothing);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('closing the keychain question does not ask it again at once', (
    tester,
  ) async {
    core
      ..works = false
      ..keptPlain = false
      ..asked = 0
      ..keys = 0;
    await _show(tester);
    expect(find.byKey(const ValueKey('keychain-problem')), findsOneWidget);
    // Escape closes it: the keychain is not asked again and the question stays closed.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.byKey(const ValueKey('keychain-problem')), findsNothing);
    expect(core.asked, 1);
    expect(core.keys, 0);
    // The page says why there is no key, with its own way to try again.
    expect(find.text('Your data can’t be encrypted yet'), findsOneWidget);
    core.works = true;
    await tester.tap(find.text('Try again'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(core.asked, 2);
    expect(core.keys, 1);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  Future<void> reset() async {
    core
      ..works = false
      ..keptPlain = false
      ..throws = false
      ..hangs = null
      ..asked = 0
      ..keys = 0
      ..newKeys = 0
      ..created = 0
      ..started = 0
      ..creating = null;
  }

  Future<void> tryCreate(WidgetTester tester) async {
    await tester.enterText(find.byType(TextField), 'Marta');
    await tester.pump();
    await tester.tap(find.text('Create'), warnIfMissed: false);
    await tester.pump();
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
  }

  testWidgets('nothing is made while the keychain has not been asked', (
    tester,
  ) async {
    await reset();
    core.hangs = Completer<void>();
    await _show(tester);
    await tryCreate(tester);
    await tester.tap(find.byTooltip('Make another key'), warnIfMissed: false);
    await tester.pump();
    expect(core.created, 0);
    expect(core.newKeys, 0);
    core.hangs!.complete();
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('a keychain that cannot be asked says so, and is asked again', (
    tester,
  ) async {
    await reset();
    core.throws = true;
    await _show(tester);
    expect(find.text('Try again'), findsOneWidget);
    expect(find.textContaining('could not be asked'), findsOneWidget);
    await tryCreate(tester);
    expect(core.created, 0);
    // It answers now: the key is made and the profile can be.
    core
      ..throws = false
      ..works = true;
    await tester.tap(find.text('Try again'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(core.keys, 1);
    expect(find.text('Try again'), findsNothing);
    expect(find.textContaining('could not be asked'), findsNothing);
    await tryCreate(tester);
    expect(core.created, greaterThan(0));
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets(
    'a profile made after the card went still starts the application',
    (tester) async {
      await reset();
      core
        ..works = true
        ..creating = Completer<void>()
        ..made = fakeRoom(people: 1).contacts.profile;
      await _show(tester);
      expect(core.keys, 1);
      await tryCreate(tester);
      expect(core.created, 1);
      // The card goes (the window shows something else) while the profile is made.
      await tester.pumpWidget(const SizedBox());
      core.creating!.complete();
      await tester.pump();
      // After its moment on "Ready", the application starts all the same.
      await tester.pump(const Duration(seconds: 1));
      expect(core.started, 1);
      await tester.pump(const Duration(seconds: 2));
    },
  );
}
