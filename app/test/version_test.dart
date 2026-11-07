import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/version.dart';

void main() {
  test('the version the window shows is the one in pubspec.yaml', () {
    final line = File('pubspec.yaml')
        .readAsLinesSync()
        .firstWhere((l) => l.startsWith('version:'));
    expect(line.substring(8).trim().split('+').first, appVersion);
  });
}
