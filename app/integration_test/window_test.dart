// The title bar's window buttons on Windows and Linux.
//
//   FOGATA_DATA_DIR=<empty folder> flutter test integration_test/window_test.dart -d linux

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/main.dart' as app;
import 'package:integration_test/integration_test.dart';
import 'package:window_manager/window_manager.dart';

Future<void> _wait(WidgetTester tester, Duration d) async {
  final end = DateTime.now().add(d);
  while (DateTime.now().isBefore(end)) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}

Future<void> _until(WidgetTester tester, Future<bool> Function() check) async {
  final end = DateTime.now().add(const Duration(seconds: 10));
  while (!await check()) {
    if (DateTime.now().isAfter(end)) fail('the window did not change');
    await _wait(tester, const Duration(milliseconds: 100));
  }
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  // macOS uses the system's traffic lights instead.
  testWidgets('maximize, restore, minimize', skip: Platform.isMacOS, (
    tester,
  ) async {
    await app.main();
    await _wait(tester, const Duration(seconds: 2));
    expect(find.byTooltip('Close'), findsOneWidget);

    await tester.tap(find.byTooltip('Maximize'));
    await _until(tester, windowManager.isMaximized);
    await _wait(tester, const Duration(milliseconds: 500));
    expect(find.byTooltip('Restore'), findsOneWidget);

    await tester.tap(find.byTooltip('Restore'));
    await _until(tester, () async => !await windowManager.isMaximized());
    await _wait(tester, const Duration(milliseconds: 500));
    expect(find.byTooltip('Maximize'), findsOneWidget);

    // A double click on an empty part of the header maximizes too.
    final header = tester.getRect(
      find.text('How should your people see you?').first,
    );
    final empty = Offset(header.center.dx, 28);
    await tester.tapAt(empty);
    await tester.pump(const Duration(milliseconds: 60));
    await tester.tapAt(empty);
    await _until(tester, windowManager.isMaximized);
    await windowManager.unmaximize();
    await _until(tester, () async => !await windowManager.isMaximized());

    // Wayland does not tell a window it was minimized; FOGATA_MARK lets a script outside
    // look while the window stays minimized for a few seconds.
    await tester.tap(find.byTooltip('Minimize'));
    final mark = Platform.environment['FOGATA_MARK'];
    if (mark != null) {
      await File(mark).writeAsString('minimized');
      await _wait(tester, const Duration(seconds: 6));
    } else if (Platform.environment['WAYLAND_DISPLAY'] == null) {
      await _until(tester, windowManager.isMinimized);
    } else {
      await _wait(tester, const Duration(seconds: 1));
    }
    await windowManager.restore();
  });
}
