// Files dropped on a room by a real drag from another application, as
// tools/smoke-wayland/drop.sh makes them in a Flatpak on KDE Plasma Wayland: the room's drop
// zone fills the window, and for each drop n the conductor announces
// ($FOGATA_TEST_OUT/drop-<n>-go) it writes to drop-<n>.txt what the drop plugin handed over
// (`raw <path>`, or `raw none` when no drop came in 20 s), the files the room took to send
// (`waiting <path>\t<folder shown>`) and what it said (`toast <text>`), then `end`. It stops
// at drop-end and writes "passed" to drop-result.txt.
import 'dart:io';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/outgoing.dart';
import 'package:fogata/src/theme.dart';
import 'package:integration_test/integration_test.dart';

final _env = Platform.environment;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('files dropped by a real drag', (tester) async {
    final out = _env['FOGATA_TEST_OUT'] ?? Directory.systemTemp.path;
    final result = File('$out/drop-result.txt');
    try {
      await _drops(tester, out);
      result.writeAsStringSync('passed\n');
    } catch (e, trace) {
      result.writeAsStringSync('failed: $e\n$trace\n');
      rethrow;
    }
  });
}

Future<void> _wait(WidgetTester tester, Duration time) async {
  await Future<void>.delayed(time);
  await tester.pump();
}

/// Where [path] leads inside the sandbox, links followed, and what the drop plugin's
/// documentHostPaths says of it (a copy whose plugin has none says so).
Future<String> _seen(String path) async {
  String linked;
  try {
    linked = await File(path).resolveSymbolicLinks();
  } on Exception catch (e) {
    linked = 'unresolved ($e)';
  }
  final id = path.split('/').where((p) => p.isNotEmpty).toList();
  String host = '-';
  if (id.length == 6 && id[3] == 'doc') {
    try {
      final places = await const MethodChannel('desktop_drop')
          .invokeMapMethod<String, Uint8List>('documentHostPaths', [id[4]]);
      host = places?[id[4]] == null
          ? 'none'
          : String.fromCharCodes(places![id[4]]!);
    } on Exception catch (e) {
      host = 'unanswered ($e)';
    }
  }
  return 'linked $linked\thost $host';
}

Future<void> _drops(WidgetTester tester, String out) async {
  final state = AppState('$out/data');
  state.roomState.roomGroup = 'group';
  final handed = <List<String>>[];
  final refusals = <String>[];
  DesktopDrop.instance.addRawDropEventListener((event) {
    if (event is DropDoneEvent) {
      handed.add([for (final f in event.files) f.path]);
      // Why the plugin took nothing from the portal, where it says so (a copy whose plugin
      // never does has no such field).
      try {
        final refusal = (event as dynamic).portalRefusal;
        if (refusal != null) {
          refusals.add('refused $refusal ${(event as dynamic).portalCount}');
        }
      } on NoSuchMethodError {
        // The plugin before the portal.
      }
    }
  });
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Scaffold(
          body: DropZone(
            state: state,
            enabled: true,
            child: const ColoredBox(color: Color(0xFF203040)),
          ),
        ),
      ),
    ),
  );
  File('$out/drop-ready').writeAsStringSync('');
  stderr.writeln('drop test: ready');
  for (var n = 1; ; n++) {
    while (!File('$out/drop-$n-go').existsSync()) {
      if (File('$out/drop-end').existsSync()) return;
      await _wait(tester, const Duration(milliseconds: 100));
    }
    state.roomState.outgoing.clear();
    state.toasts.list.clear();
    refusals.clear();
    final handedBefore = handed.length;
    final until = DateTime.now().add(const Duration(seconds: 20));
    while (handed.length == handedBefore && DateTime.now().isBefore(until)) {
      await _wait(tester, const Duration(milliseconds: 100));
    }
    // What was dropped is looked at, then added, without holding up the window.
    await _wait(tester, const Duration(seconds: 2));
    final lines = <String>[
      if (handed.length == handedBefore) 'raw none',
      for (final drop in handed.skip(handedBefore))
        if (drop.isEmpty) 'raw (no file)' else for (final p in drop) 'raw $p',
      ...refusals,
      for (final f in state.roomState.outgoingHere)
        'waiting ${f.path}\t${f.folder}',
      for (final t in state.toasts.list) 'toast ${t.text}',
      for (final drop in handed.skip(handedBefore))
        for (final p in drop) 'seen $p\t${await _seen(p)}',
      'end',
    ];
    File('$out/drop-$n.txt').writeAsStringSync('${lines.join('\n')}\n');
    stderr.writeln('drop test: drop $n: ${lines.join(' | ')}');
  }
}
