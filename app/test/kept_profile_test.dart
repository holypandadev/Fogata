import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/welcome.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

/// A computer whose keychain works, and how many profiles the welcome page made.
class FakeCore implements RustLibApi {
  int created = 0;

  @override
  Future<String?> crateApiAppKeychainProblem({required String dataDir}) async =>
      null;

  @override
  Future<String> crateApiAppDeviceFingerprint({
    required String dataDir,
  }) async => '7Q4M K2XR 9PLD 3VHC';

  @override
  Future<UiProfile> crateApiAppCreateProfile({
    required String dataDir,
    required String name,
    required int preset,
    Uint8List? photo,
  }) async {
    created++;
    throw StateError('stop here');
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

final core = FakeCore();

const _folder = '/home/ana/.local/share/fogata/refused';

Future<AppState> _show(WidgetTester tester, {String? kept}) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1440, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = fakeRoom(people: 1)..keptProfile = kept;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Welcome()),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 500));
  return state;
}

Future<void> _tryCreate(WidgetTester tester) async {
  await tester.enterText(find.byType(TextField), 'Marta');
  await tester.pump();
  await tester.testTextInput.receiveAction(TextInputAction.done);
  await tester.pump(const Duration(milliseconds: 500));
}

void main() {
  setUpAll(() => RustLib.initMock(api: core));

  testWidgets(
    'a profile that could not be read is said, with where it is kept',
    (tester) async {
      await _show(tester, kept: _folder);
      expect(find.byKey(const ValueKey('profile-kept')), findsOneWidget);
      expect(find.text('Your profile could not be read'), findsOneWidget);
      expect(
        find.textContaining(
          'Nothing was deleted: the files are kept in $_folder.',
        ),
        findsOneWidget,
      );
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 2));
    },
  );

  testWidgets(
    'a new profile is made over a kept one only once the person says so',
    (tester) async {
      core.created = 0;
      await _show(tester, kept: _folder);
      await _tryCreate(tester);
      expect(
        find.byKey(const ValueKey('profile-kept-confirm')),
        findsOneWidget,
      );
      expect(core.created, 0);
      await tester.tap(find.text('Cancel'));
      await tester.pump(const Duration(milliseconds: 500));
      expect(find.byKey(const ValueKey('profile-kept-confirm')), findsNothing);
      expect(core.created, 0);
      await _tryCreate(tester);
      await tester.tap(find.byKey(const ValueKey('profile-kept-make')));
      await tester.pump(const Duration(milliseconds: 500));
      expect(core.created, 1);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 2));
    },
  );

  testWidgets('without a kept profile nothing is said nor asked', (
    tester,
  ) async {
    core.created = 0;
    await _show(tester);
    expect(find.byKey(const ValueKey('profile-kept')), findsNothing);
    await _tryCreate(tester);
    expect(find.byKey(const ValueKey('profile-kept-confirm')), findsNothing);
    expect(core.created, 1);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  test('the start notice of refused files is said in the person\'s words', () {
    expect(
      AppState.coreNotice(
        'Some of Fogata\'s files on this computer were found unencrypted where '
        'they are kept encrypted, so they were not used. Nothing was deleted: '
        'they are kept in $_folder',
      ),
      'Some of Fogata’s files on this computer were found unencrypted where '
      'they are kept encrypted, so they were not used. Nothing was deleted: '
      'they are kept in $_folder.',
    );
  });
}
