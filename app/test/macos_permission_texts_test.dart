import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// macOS's permission prompts (microphone, camera, local network) take their text from
/// Info.plist, in English, unless the bundle has `<language>.lproj/InfoPlist.strings`.
/// Every language of the application has its own, with every usage key Info.plist has,
/// bundled by the Xcode project.

/// The code Info.plist and the `.lproj` folders use for an ARB file's language.
String _appleCode(String arb) => switch (arb) {
  'pt' => 'pt-BR',
  'zh' => 'zh-Hans',
  _ => arb,
};

final _plist = File('macos/Runner/Info.plist').readAsStringSync();

/// The `<string>` after `<key>name</key>` in Info.plist.
String _plistString(String name) =>
    RegExp('<key>${RegExp.escape(name)}</key>\\s*<string>([^<]*)</string>')
        .firstMatch(_plist)!
        .group(1)!;

/// `"key" = "value";` lines of a .strings file.
Map<String, String> _strings(File file) => {
  for (final m in RegExp(
    r'^"([^"]+)"\s*=\s*"((?:[^"\\]|\\.)*)";\s*$',
    multiLine: true,
  ).allMatches(file.readAsStringSync()))
    m.group(1)!: m.group(2)!,
};

void main() {
  final usageKeys = [
    for (final m in RegExp(
      r'<key>(NS\w+UsageDescription)</key>',
    ).allMatches(_plist))
      m.group(1)!,
  ];
  final localizations = [
    for (final m in RegExp(r'<string>([^<]+)</string>').allMatches(
      RegExp(
        r'<key>CFBundleLocalizations</key>\s*<array>(.*?)</array>',
        dotAll: true,
      ).firstMatch(_plist)!.group(1)!,
    ))
      m.group(1)!,
  ];
  final languages = [
    for (final f in Directory('lib/l10n').listSync())
      if (RegExp(r'app_(\w+)\.arb$').firstMatch(f.path) case final m?)
        _appleCode(m.group(1)!),
  ]..sort();

  test('Info.plist names every language of the application', () {
    expect(usageKeys, isNotEmpty);
    expect(languages, isNotEmpty);
    expect([...localizations]..sort(), languages);
  });

  test('every language has every permission prompt\'s text', () {
    for (final language in localizations) {
      final file = File('macos/Runner/$language.lproj/InfoPlist.strings');
      expect(file.existsSync(), isTrue, reason: file.path);
      final texts = _strings(file);
      expect(texts.keys.toSet(), usageKeys.toSet(), reason: file.path);
      for (final key in usageKeys) {
        expect(texts[key]!.trim(), isNotEmpty, reason: '$language $key');
        if (language == 'en') {
          expect(
            texts[key],
            _plistString(key),
            reason: 'English is Info.plist',
          );
        } else {
          expect(
            texts[key],
            isNot(_plistString(key)),
            reason: '$language $key is still English',
          );
        }
      }
    }
  });

  test('the Xcode project bundles every language\'s texts', () {
    final project = File('macos/Runner.xcodeproj/project.pbxproj')
        .readAsStringSync();
    // Built into the Runner target's resources.
    final resources = RegExp(
      r'/\* Resources \*/ = \{\s*isa = PBXResourcesBuildPhase;[^}]*?files = \(([^)]*)\)',
    ).allMatches(project).map((m) => m.group(1)!);
    expect(
      resources.where((files) => files.contains('InfoPlist.strings')),
      hasLength(1),
    );
    final group = RegExp(
      r'/\* InfoPlist\.strings \*/ = \{\s*isa = PBXVariantGroup;\s*children = \(([^)]*)\)',
    ).firstMatch(project);
    expect(group, isNotNull);
    final regions = RegExp(r'knownRegions = \(([^)]*)\)')
        .firstMatch(project)!
        .group(1)!;
    for (final language in localizations) {
      expect(
        project,
        contains('path = $language.lproj/InfoPlist.strings;'),
        reason: language,
      );
      expect(group!.group(1), contains('/* $language */'), reason: language);
      expect(
        RegExp('(^|[\\s,])"?${RegExp.escape(language)}"?,').hasMatch(regions),
        isTrue,
        reason: 'knownRegions has $language',
      );
    }
  });
}
