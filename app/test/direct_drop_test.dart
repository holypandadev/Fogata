import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/direct_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/theme.dart';

const _me = UiPerson(
  id: 'me',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

const _sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);

UiDirectPage _page({bool online = true, int? ended}) => UiDirectPage(
  away: false,
  friend: _sofia,
  version: 0,
  online: online,
  lastSeenMs: null,
  connected: online,
  items: const [],
  more: false,
  pinned: const [],
  timerMs: 0,
  endedMs: ended,
  altered: false,
  usage: const UiDirectUsage(messages: 0, pictures: 0, videos: 0, files: 0),
);

void main() {
  late Directory dir;
  late String secret, plain;
  setUp(() {
    dir = Directory.systemTemp.createTempSync('fogata_direct_drop');
    secret = '${dir.path}/.env';
    plain = '${dir.path}/lake.jpg';
    File(secret).writeAsStringSync('key');
    File(plain).writeAsStringSync('photo');
  });
  tearDown(() => dir.deleteSync(recursive: true));

  Future<AppState> show(WidgetTester tester, UiDirectPage shown) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = AppState('/nonexistent/fogata-direct-drop-test')
      ..contacts.profile = const UiProfile(
        person: _me,
        fingerprint: '7Q4M K2XR',
      )
      ..nav.page = Screen.direct;
    state.direct
      ..open = _sofia.id
      ..page = shown;
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: DirectPage()),
        ),
      ),
    );
    await tester.pump();
    // The test font draws every letter as a wide square: long lines may overflow here.
    tester.takeException();
    return state;
  }

  Future<void> platform(WidgetTester tester, String method, Object args) async {
    final done = Completer<ByteData?>();
    await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'desktop_drop',
      const StandardMethodCodec().encodeMethodCall(MethodCall(method, args)),
      done.complete,
    );
    await done.future;
  }

  /// Drags [paths] over the chat, as every platform's part of the plugin can hand them
  /// over; [over] runs while they are over it, before they are dropped.
  Future<void> drop(
    WidgetTester tester,
    List<String> paths, {
    void Function()? over,
  }) async {
    await tester.runAsync(() async {
      await platform(tester, 'entered', [300.0, 400.0]);
    });
    await tester.pump();
    over?.call();
    // In the real zone: the files are read for real, outside the test's own clock.
    await tester.runAsync(() async {
      await platform(tester, 'performOperation', paths);
      await Future<void>.delayed(const Duration(milliseconds: 200));
    });
    await tester.pump();
    tester.takeException();
  }

  List<String> waiting(DirectChats direct) => [
    for (final f in direct.waiting) f.path,
  ];

  testWidgets('files dropped on a chat wait in its box, but hidden ones', (
    tester,
  ) async {
    final state = await show(tester, _page());
    var title = false;
    await drop(
      tester,
      [secret, plain],
      over: () => title = find
          .text('Drop to send to ${_sofia.name}')
          .evaluate()
          .isNotEmpty,
    );
    expect(title, isTrue);
    expect(find.text('Drop to send to ${_sofia.name}'), findsNothing);
    expect(waiting(state.direct), [plain]);
    expect(
      state.toasts.list.map((t) => t.text),
      contains(contains('pick it with the + button')),
    );
    state.dispose();
  });

  testWidgets('nothing is taken while the friend is offline', (tester) async {
    final state = await show(tester, _page(online: false));
    await drop(tester, [plain]);
    expect(find.text('Drop to send to ${_sofia.name}'), findsNothing);
    expect(state.direct.outgoing, isEmpty);
    state.dispose();
  });

  testWidgets('or once the chat has ended', (tester) async {
    final state = await show(
      tester,
      _page(ended: DateTime.now().millisecondsSinceEpoch),
    );
    await drop(tester, [plain]);
    expect(state.direct.outgoing, isEmpty);
    state.dispose();
  });

  testWidgets('a drop is dropped if another chat was opened meanwhile', (
    tester,
  ) async {
    final state = await show(tester, _page());
    await tester.runAsync(() async {
      await platform(tester, 'entered', [300.0, 400.0]);
    });
    await tester.pump();
    await tester.runAsync(() async {
      await platform(tester, 'performOperation', [plain]);
      state.direct.open = 'leo';
      await Future<void>.delayed(const Duration(milliseconds: 200));
    });
    await tester.pump();
    tester.takeException();
    expect(state.direct.outgoing, isEmpty);
    state.dispose();
  });
}
