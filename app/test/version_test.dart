import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/version.dart';

void main() {
  test('the version the window shows is THIS_VERSION of the Rust side', () {
    final source = File(
      '../crates/fogata-update/src/lib.rs',
    ).readAsStringSync();
    final match = RegExp(
      r'pub const THIS_VERSION: &str = "([^"]+)";',
    ).firstMatch(source);
    expect(match, isNotNull);
    expect(appVersion, match!.group(1));
  });

  test('pubspec.yaml names the same version without its pre-release part', () {
    final line = File('pubspec.yaml')
        .readAsLinesSync()
        .firstWhere((l) => l.startsWith('version:'));
    expect(
      line.substring(8).trim().split('+').first,
      appVersion.split('-').first,
    );
  });
}
