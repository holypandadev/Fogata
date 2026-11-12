import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/test_switches.dart';

/// The variables of the environment a release build acts on. Any other the application
/// reads by name is one its tests set, and goes through [testSwitch].
const _readInRelease = {'FOGATA_DATA_DIR', 'FOGATA_CALL_LOG'};

void main() {
  test('a release build reads no test switch', () {
    const environment = {'FOGATA_TEST_OPENED': '/tmp/opened.txt'};
    expect(
      testSwitch('FOGATA_TEST_OPENED', environment: environment),
      '/tmp/opened.txt',
      reason: 'debug and profile builds, as the tests and the call lab run',
    );
    expect(
      testSwitch('FOGATA_TEST_OPENED', release: true, environment: environment),
      isNull,
    );
  });

  test(
    'every test-only variable the application reads goes through testSwitch',
    () {
      final named = RegExp(r"""['"](FOGATA_[A-Z0-9_]+|FLUTTER_TEST)['"]""");
      final found = <String>[];
      for (final file in Directory('lib').listSync(recursive: true)) {
        if (file is! File || !file.path.endsWith('.dart')) continue;
        final path = file.path.replaceAll(r'\', '/');
        if (path.startsWith('lib/src/rust/') ||
            path.endsWith('/test_switches.dart')) {
          continue;
        }
        final lines = file.readAsLinesSync();
        for (var i = 0; i < lines.length; i++) {
          final line = lines[i];
          if (line.trimLeft().startsWith('//')) continue;
          for (final match in named.allMatches(line)) {
            final name = match.group(1)!;
            if (_readInRelease.contains(name)) continue;
            if (line.contains("testSwitch('$name')")) continue;
            found.add('$path:${i + 1} $name');
          }
        }
      }
      expect(found, isEmpty);
    },
  );
}
