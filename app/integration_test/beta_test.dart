// Pictures of what the pre-release adds: Settings › Donate to Fogata and Report a problem
// (with the list of what goes, and after the e-mail is ready), the label beside the
// version, the title bar's pill when an update is ready or required, and every step of the
// splash, in some languages. Nothing connects; nothing is sent or opened.
//
//   FOGATA_SHOTS=<folder> flutter test integration_test/beta_test.dart -d linux

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/update.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/splash.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/window.dart' show splashSize;
import 'package:integration_test/integration_test.dart';

import '../test/fake_room.dart';

final _shots = Platform.environment['FOGATA_SHOTS'];

Future<void> _wait(WidgetTester tester, Duration d) async {
  final end = DateTime.now().add(d);
  while (DateTime.now().isBefore(end)) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}

Future<void> _shot(WidgetTester tester, String name) async {
  if (_shots == null) return;
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  final image = await layer.toImage(
    Offset.zero & (view.size * tester.view.devicePixelRatio),
  );
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await File('$_shots/$name.png').writeAsBytes(data!.buffer.asUint8List());
}

const _what =
    'I joined the room “Gaming” and my microphone didn’t work until I left and '
    'came back.';

AppState _state({bool? update}) {
  final state = fakeRoom(people: 2, dataDir: '/nonexistent/fogata-beta-test');
  state.callState.call = null;
  state.nav.page = Screen.friends;
  if (update != null) {
    state.updates.status = UiUpdate(
      thisBuild: 3,
      thisVersion: '0.1.0',
      state: UiUpdateState.ready,
      done: 0,
      total: 0,
      releaseBuild: 4,
      releaseVersion: '0.1.1',
      required_: update,
    );
  }
  return state;
}

Future<AppState> _show(
  WidgetTester tester,
  Fg tokens,
  Size size, {
  bool? update,
  Widget? child,
  String? language,
}) async {
  final state = _state(update: update);
  // After the state, which starts in the system's language.
  if (language != null) useLanguage(language);
  tester.view.physicalSize = size * tester.view.devicePixelRatio;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: tokens.themeData,
        home: Material(child: child ?? const Shell()),
        builder: appFrame,
      ),
    ),
  );
  await _wait(tester, const Duration(milliseconds: 400));
  return state;
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  // The report is written through the core's library, as in the app.
  setUpAll(RustLib.init);

  testWidgets('beta pages', (tester) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('window_manager'),
      (call) async => false,
    );
    final problems = <String>[];
    for (final (theme, tokens, size) in [
      ('dark', Fg.darkTokens, const Size(1440, 900)),
      ('light', Fg.lightTokens, const Size(960, 640)),
    ]) {
      final at = '${size.width.round()}x${size.height.round()}';
      var state = await _show(tester, tokens, size);
      state.openSettings(SettingsSection.donate);
      await _wait(tester, const Duration(milliseconds: 600));
      await _shot(tester, 'd1-donate-$theme-$at');
      state.showSettingsSection(SettingsSection.report);
      await _wait(tester, const Duration(milliseconds: 500));
      tester
              .widget<TextField>(find.byKey(const ValueKey('report-what')))
              .controller!
              .text =
          _what;
      await _wait(tester, const Duration(milliseconds: 300));
      await _shot(tester, 'r1-report-$theme-$at');
      await tester.tap(find.byKey(const ValueKey('report-see-sent')));
      await _wait(tester, const Duration(milliseconds: 600));
      await _shot(tester, 'r1b-report-list-$theme-$at');

      // "Email it", with what would open written down instead of opened.
      final opened = Platform.environment['FOGATA_TEST_OPENED'];
      if (opened != null) {
        final downloads = Directory.systemTemp.createTempSync('fogata-beta-');
        state.prefs.downloadsDir = downloads.path;
        await tester.tap(find.byKey(const ValueKey('report-email')));
        final end = DateTime.now().add(const Duration(seconds: 30));
        while (find.byKey(const ValueKey('report-ready')).evaluate().isEmpty &&
            DateTime.now().isBefore(end)) {
          await _wait(tester, const Duration(milliseconds: 200));
        }
        await _wait(tester, const Duration(milliseconds: 600));
        await _shot(tester, 'r2-sent-$theme-$at');
        final written = downloads.listSync().whereType<File>().toList();
        if (written.length != 1 ||
            !written.single.path.endsWith('.zip') ||
            written.single.lengthSync() < 100) {
          problems.add('$theme: no report written in ${downloads.path}');
        }
        final asked = File(opened).readAsStringSync();
        // What the person wrote goes in the report's file, never in the e-mail's address.
        if (!asked.contains('mailto:contact@holypanda.dev?subject=') ||
            asked.contains(Uri.encodeComponent('microphone didn’t work')) ||
            !asked.contains(written.single.path)) {
          problems.add('$theme: what opened: $asked');
        }
        File(opened).writeAsStringSync('');
        downloads.deleteSync(recursive: true);
      }
      await tester.pumpWidget(const SizedBox());

      for (final required in [false, true]) {
        await _show(tester, tokens, size, update: required);
        await _shot(
          tester,
          'u1-update-${required ? 'required' : 'ready'}-$theme-$at',
        );
        await tester.pumpWidget(const SizedBox());
      }
      final e = tester.takeException();
      if (e != null) problems.add('$theme: $e');
    }

    // The splash: each step, in English and in the longest languages.
    for (final code in ['en', 'de', 'fr', 'ru', 'ja']) {
      for (final step in SplashStep.values) {
        await _show(
          tester,
          Fg.darkTokens,
          splashSize,
          language: code,
          child: SplashCard(
            step: step,
            done: 34000000,
            total: 58000000,
            from: 'Leo',
          ),
        );
        await _wait(tester, const Duration(milliseconds: 300));
        await _shot(
          tester,
          's1-splash-$code-${SplashStep.values.indexOf(step) + 1}-${step.name}',
        );
        await tester.pumpWidget(const SizedBox());
      }
    }
    useLanguage('en');
    expect(problems, isEmpty);
  });
}
