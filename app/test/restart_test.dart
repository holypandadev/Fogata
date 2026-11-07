import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/restart.dart';
import 'package:fogata/src/screens/stopped.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart' show loadFonts;

void main() {
  setUpAll(loadFonts);

  late Directory dir;
  setUp(() async {
    dir = await Directory.systemTemp.createTemp('fogata-restart-test');
    await Directory('${dir.path}/window').create();
  });
  tearDown(() => dir.delete(recursive: true));

  Future<void> leave(Map<String, int> data) =>
      File('${dir.path}/window/restart').writeAsString(jsonEncode(data));

  final now = DateTime.now().millisecondsSinceEpoch;

  // The marker is read once per run of the application: the cases go in this order.
  test('a restart marker from long ago is not a restart', () async {
    await leave({'at': now - 5 * 60 * 1000, 'automatic': now - 5 * 60 * 1000});
    await readRestart(dir.path);
    expect(startedByRestart, isFalse);
    expect(Stopping(dir.path).restartsByItself, isTrue);
    expect(File('${dir.path}/window/restart').existsSync(), isFalse);
  });

  testWidgets(
    'stopped again soon after starting again by itself, it waits for the person',
    (tester) async {
      await tester.runAsync(() async {
        await leave({'at': now - 2000, 'automatic': now - 2 * 60 * 1000});
        await readRestart(dir.path);
      });
      expect(startedByRestart, isTrue);
      expect(startedByItself, isFalse, reason: 'that restart was minutes ago');
      final stopping = Stopping(dir.path);
      expect(stopping.restartsByItself, isFalse);
      await tester.pumpWidget(
        MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Material(child: StoppedBanner(stopping: stopping)),
        ),
      );
      expect(find.textContaining('stopped working'), findsNothing);
      stopping.onStopped();
      await tester.pump();
      expect(stopping.restarting, isFalse);
      expect(
        find.textContaining('Restart it to connect again.', findRichText: true),
        findsOneWidget,
      );
      expect(find.text('Restart now'), findsOneWidget);
      // Nothing starts by itself later either.
      await tester.pump(const Duration(seconds: 10));
      expect(stopping.starting, isFalse);
    },
  );

  test('inside an AppImage the AppImage itself starts again', () {
    expect(relaunched(const {}), Platform.resolvedExecutable);
    const inside = {
      'APPIMAGE': '/opt/apps/Fogata.AppImage',
      'APPDIR': '/tmp/.mount_FogataX1',
    };
    expect(
      relaunched(inside, executable: '/tmp/.mount_FogataX1/fogata'),
      '/opt/apps/Fogata.AppImage',
    );
    // Started from another AppImage, a launcher say: both are inherited, and this copy
    // starts itself, not the launcher.
    const inherited = {
      'APPIMAGE': '/opt/apps/Launcher.AppImage',
      'APPDIR': '/tmp/.mount_LauncherQ2',
    };
    expect(
      relaunched(inherited, executable: '/opt/fogata/fogata'),
      '/opt/fogata/fogata',
    );
    expect(
      relaunched(const {
        'APPIMAGE': '/opt/apps/Launcher.AppImage',
      }, executable: '/opt/fogata/fogata'),
      '/opt/fogata/fogata',
    );
  });
}
