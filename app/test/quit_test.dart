import 'dart:io';
import 'dart:ui' show AppExitResponse;

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/report.dart';
import 'package:fogata/src/window.dart';

import 'private_writer.dart';

void main() {
  setUpAll(writePrivatelyInTests);
  testWidgets(
    'the window\'s errors still waiting are on the disk as it quits',
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => null,
      );
      final dir = Directory.systemTemp.createTempSync('fogata-quit-');
      addTearDown(() => dir.deleteSync(recursive: true));
      await tester.runAsync(() async {
        await beforeQuitting(() async {}, keepRunning: () => false);
        noteAppError(dir.path, 'the first error', null);
        await flushAppErrors();
        // Within a second of the first: it waits for that second to end.
        noteAppError(dir.path, 'the last error', null);
        expect(
          await tester.binding.handleRequestAppExit(),
          AppExitResponse.exit,
        );
        final log = Directory('${dir.path}/$logFolder')
            .listSync()
            .whereType<File>()
            .map((f) => f.readAsStringSync())
            .join();
        expect(log, contains('the first error'));
        expect(log, contains('the last error'));
      });
    },
  );
}
