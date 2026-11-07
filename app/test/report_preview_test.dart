// Report a problem: the page shows the whole report before it goes, scrubbed as it goes,
// shows it again as the person writes, and what goes is what shows: nothing goes while
// the report shown is being made again.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/report.dart';
import 'package:fogata/src/screens/report.dart';
import 'package:fogata/src/theme.dart';

import 'wait.dart';

import 'fake_room.dart';
import 'private_writer.dart';
import 'unzip.dart';

/// All the text the report's box shows, line after line.
String _shown(WidgetTester tester) => tester
    .widgetList<Text>(
      find.descendant(
        of: find.byKey(const ValueKey('report-preview-text')),
        matching: find.byType(Text),
      ),
    )
    .map((t) => t.data)
    .join('\n');

void main() {
  setUpAll(writePrivatelyInTests);
  testWidgets('the report shows scrubbed, and again after typing', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(900, 1400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final dir = Directory.systemTemp.createTempSync('fogata-preview-');
    addTearDown(() => dir.deleteSync(recursive: true));
    final now = DateTime.now().toUtc();
    final day =
        '${now.year}-${now.month.toString().padLeft(2, '0')}-${now.day.toString().padLeft(2, '0')}';
    Directory('${dir.path}/$logFolder').createSync();
    File('${dir.path}/$logFolder/fogata-$day.log').writeAsStringSync(
      '${now.toIso8601String()}  WARN fogata_core: Marta in Friday games could '
      'not open /home/marta/Pictures/beach.jpg from 203.0.113.7\n',
    );
    final state = fakeRoom(people: 2, dataDir: dir.path);
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('window_manager'),
      (call) async => false,
    );
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: ReportPage()),
        ),
      ),
    );
    expect(find.byKey(const ValueKey('report-preview-text')), findsNothing);

    await tester.tap(find.byKey(const ValueKey('report-preview')));
    // The report is made off the window's thread: time passes for real until it shows.
    await _until(tester, (shown) => shown.contains('WARN'));
    final first = _shown(tester);
    expect(first, contains('report.txt'));
    expect(
      first,
      contains('WARN fogata_core: ‹name› in ‹name› could not open'),
    );
    expect(first, contains('‹path› from ‹ip›'));
    for (final gone in ['Marta', 'Friday', 'marta', 'beach', '203.0.113.7']) {
      expect(first, isNot(contains(gone)), reason: first);
    }

    // What the person writes shows in the report too, word for word.
    await tester.enterText(
      find.byKey(const ValueKey('report-what')),
      'It froze when I joined.',
    );
    await _until(tester, (shown) => shown.contains('It froze'));
    expect(_shown(tester), contains('It froze when I joined.'));
    await tester.pumpWidget(const SizedBox());
  });

  _sendingTests();
}

/// The page with a log of today in a data folder of its own, and what "Email it" sent.
Future<List<PreparedReport>> _page(WidgetTester tester) async {
  tester.view.physicalSize = const Size(900, 1400);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final dir = Directory.systemTemp.createTempSync('fogata-send-');
  addTearDown(() => dir.deleteSync(recursive: true));
  final now = DateTime.now().toUtc();
  final day =
      '${now.year}-${now.month.toString().padLeft(2, '0')}-${now.day.toString().padLeft(2, '0')}';
  Directory('${dir.path}/$logFolder').createSync();
  File('${dir.path}/$logFolder/fogata-$day.log').writeAsStringSync(
    '${now.toIso8601String()}  WARN fogata_core: Marta could not reach 203.0.113.7\n',
  );
  final state = fakeRoom(people: 2, dataDir: dir.path);
  final sent = <PreparedReport>[];
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(
          child: ReportPage(deliver: (report) async => sent.add(report)),
        ),
      ),
    ),
  );
  return sent;
}

/// Writes [report] as "Email it" does, and reads the archive back from the disk.
Future<Map<String, String>> _written(
  WidgetTester tester,
  PreparedReport report,
) async {
  final dir = Directory.systemTemp.createTempSync('fogata-written-');
  addTearDown(() => dir.deleteSync(recursive: true));
  final path = (await tester.runAsync(
    () => writeReport(folder: dir.path, report: report),
  ))!;
  return {
    for (final MapEntry(:key, :value) in unzip(
      File(path).readAsBytesSync(),
    ).entries)
      key: utf8.decode(value),
  };
}

/// Lets real time pass, and the page's own, until [done] holds of what the report's box
/// shows and nothing newer is being made; fails when that does not come within
/// [realWorkLimit].
Future<void> _until(
  WidgetTester tester,
  bool Function(String shown) done,
) async {
  bool working() => find
      .byKey(const ValueKey('report-preview-working'))
      .evaluate()
      .isNotEmpty;
  final waited = Stopwatch()..start();
  while (!done(_shown(tester)) || working()) {
    if (waited.elapsed > realWorkLimit) {
      fail(
        'waited ${realWorkLimit.inSeconds} s for the report; it shows: '
        '${_shown(tester)}',
      );
    }
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 20)),
    );
    await tester.pump(const Duration(milliseconds: 500));
  }
}

/// The text the box shows for [files], as it lays them out.
String _asShown(Map<String, String> files) => [
  for (final MapEntry(key: name, value: text) in files.entries) ...[
    name,
    ...const LineSplitter().convert(text),
    '',
  ],
].join('\n');

/// Taps the button that sends the report. When it [goes], waits until it is handed on;
/// when not, gives it a while to go all the same.
Future<void> _tapEmail(
  WidgetTester tester,
  List<PreparedReport> sent, {
  required bool goes,
}) async {
  final before = sent.length;
  await tester.tap(find.byKey(const ValueKey('report-email')));
  if (goes) {
    await pumpUntil(
      tester,
      () => sent.length > before,
      what: 'the report handed on',
    );
  } else {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 300)),
    );
    await tester.pump();
  }
}

void _sendingTests() {
  testWidgets('what goes is the report shown, never one being made', (
    tester,
  ) async {
    final sent = await _page(tester);
    await tester.tap(find.byKey(const ValueKey('report-preview')));
    await tester.enterText(
      find.byKey(const ValueKey('report-what')),
      'It froze.',
    );
    await _until(tester, (shown) => shown.contains('It froze.'));
    // Typing more: the report shown is old until it is made again, and nothing goes.
    await tester.enterText(
      find.byKey(const ValueKey('report-what')),
      'It froze when I joined.',
    );
    await tester.pump();
    expect(
      find.byKey(const ValueKey('report-preview-working')),
      findsOneWidget,
    );
    await _tapEmail(tester, sent, goes: false);
    expect(sent, isEmpty);
    await _until(tester, (shown) => shown.contains('when I joined'));
    await _tapEmail(tester, sent, goes: true);
    expect(sent, hasLength(1));
    expect(
      sent.single.files['report.txt'],
      contains('It froze when I joined.'),
    );
    expect(sent.single.files.keys, contains(startsWith('log/')));
    expect(_shown(tester), _asShown(sent.single.files));
    expect(await _written(tester, sent.single), sent.single.files);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'turning the log off and sending at once sends the report shown',
    (tester) async {
      final sent = await _page(tester);
      await tester.enterText(
        find.byKey(const ValueKey('report-what')),
        'No sound.',
      );
      await tester.tap(find.byKey(const ValueKey('report-preview')));
      await _until(tester, (shown) => shown.contains('No sound.'));
      await tester.tap(find.text(l10n.reportIncludeLog));
      await tester.pump();
      await _tapEmail(tester, sent, goes: false);
      expect(sent, isEmpty);
      await _until(tester, (shown) => !shown.contains('log/'));
      await _tapEmail(tester, sent, goes: true);
      expect(sent.single.files.keys, ['report.txt']);
      expect(sent.single.files['report.txt'], contains('not included'));
      expect(_shown(tester), _asShown(sent.single.files));
      // And on again: the log is read again, and shows before it goes.
      await tester.tap(find.text(l10n.reportIncludeLog));
      await tester.pump();
      await _tapEmail(tester, sent, goes: false);
      expect(sent, hasLength(1));
      await _until(tester, (shown) => shown.contains('log/'));
      await _tapEmail(tester, sent, goes: true);
      expect(sent, hasLength(2));
      expect(sent.last.files.keys, contains(startsWith('log/')));
      expect(sent.last.files.values.join(), isNot(contains('Marta')));
      expect(_shown(tester), _asShown(sent.last.files));
      await tester.pumpWidget(const SizedBox());
    },
  );
}
