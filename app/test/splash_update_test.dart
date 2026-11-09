import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/rust/api/update.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/splash.dart';
import 'package:fogata/src/update.dart';
import 'package:fogata/src/window.dart';

/// A core whose updater goes through [script]: the state it says at each moment since the
/// splash asked it to check.
class _Core implements RustLibApi {
  UiUpdateState Function(Duration since) script = (_) => UiUpdateState.upToDate;
  DateTime? asked;
  int installs = 0;

  @override
  Future<void> crateApiUpdateCheckForUpdate() async {
    asked = DateTime.now();
  }

  @override
  Future<UiUpdate?> crateApiUpdateUpdateStatus() async {
    final since = DateTime.now().difference(asked ?? DateTime.now());
    final state = script(since);
    final coming =
        state != UiUpdateState.upToDate && state != UiUpdateState.checking;
    return UiUpdate(
      thisBuild: 5,
      thisVersion: '0.1.0-beta.5',
      state: state,
      // A download moving: a little more each moment.
      done: state == UiUpdateState.downloading ? since.inMilliseconds : 0,
      total: state == UiUpdateState.downloading ? 1000000 : 0,
      releaseBuild: coming ? 6 : null,
      releaseVersion: coming ? '0.1.0-beta.6' : null,
      required_: false,
    );
  }

  @override
  Future<void> crateApiUpdateInstallUpdate() async {
    installs++;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final core = _Core();
  setUpAll(() => RustLib.initMock(api: core));
  var quits = 0;
  setUp(() {
    core
      ..asked = null
      ..installs = 0;
    quits = 0;
    quitApplication = () async => quits++;
    for (final channel in ['fogata/desktop', 'window_manager']) {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(
            MethodChannel(channel),
            (call) async => null,
          );
    }
  });

  /// Runs the splash's end as Fogata does after starting; true when it opened this version.
  Future<bool> splash({bool askFriends = true}) => finishSplash(
    splash: Splash(),
    updates: Updates(),
    started: true,
    askFriends: askFriends,
    shownAt: DateTime.now(),
    say: (_, {error = false}) {},
    nameOf: (_) => null,
  );

  UiUpdateState at(Duration since, List<(int ms, UiUpdateState state)> steps) {
    var state = steps.first.$2;
    for (final (ms, s) in steps) {
      if (since.inMilliseconds >= ms) state = s;
    }
    return state;
  }

  // The releases page answers after a few seconds, a newer version comes down and is
  // checked: the splash installs it and Fogata restarts into it.
  test(
    'a newer version found while checking is installed by the splash',
    () async {
      core.script = (since) => at(since, [
        (0, UiUpdateState.checking),
        (2500, UiUpdateState.downloading),
        (3500, UiUpdateState.ready),
      ]);
      expect(await splash(), isFalse);
      expect(core.installs, 1);
      await Future<void>.delayed(const Duration(milliseconds: 700));
      expect(quits, 1);
    },
  );

  // A newer version downloaded before (while Fogata ran) is found once what is kept on
  // disk is read: installed too.
  test('a newer version already here is installed by the splash', () async {
    core.script = (since) =>
        at(since, [(0, UiUpdateState.checking), (400, UiUpdateState.ready)]);
    expect(await splash(), isFalse);
    expect(core.installs, 1);
  });

  // Nothing newer anywhere: the splash opens this version as soon as the check ends.
  test('with nothing newer the splash opens this version', () async {
    core.script = (since) => at(since, [
      (0, UiUpdateState.checking),
      (1200, UiUpdateState.upToDate),
    ]);
    final start = DateTime.now();
    expect(await splash(), isTrue);
    expect(core.installs, 0);
    expect(
      DateTime.now().difference(start),
      lessThan(const Duration(seconds: 3)),
    );
  });

  // A download that stops moving is left to go on behind the main window.
  test('a download that stalls is not waited for', () async {
    core.script = (since) => at(since, [
      (0, UiUpdateState.checking),
      (500, UiUpdateState.verifying),
    ]);
    final start = DateTime.now();
    expect(await splash(), isTrue);
    expect(core.installs, 0);
    expect(
      DateTime.now().difference(start),
      lessThan(const Duration(seconds: 20)),
    );
  }, timeout: const Timeout(Duration(seconds: 40)));
}
