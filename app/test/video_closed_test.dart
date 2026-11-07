// A video closed while its player is still starting leaves nothing running behind it.

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/screens/room.dart';
import 'package:fogata/src/theme.dart';

import 'wait.dart';

import 'fake_room.dart';

/// A state that plays the file at [path].
class _Local extends AppState {
  _Local(this.path) : super('/nonexistent/fogata-video-closed');
  final String path;

  @override
  Future<String?> videoToPlay(UiFile f) async => path;
}

/// The native player: `play` answers only when [release] completes; `state` calls count.
class _Native {
  final release = Completer<void>();
  final playing = Completer<void>();
  var states = 0;

  void install() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('fogata/video'), (
          call,
        ) async {
          switch (call.method) {
            case 'open':
              return 1;
            case 'play':
              if (!playing.isCompleted) playing.complete();
              await release.future;
              return null;
            case 'state':
              states++;
              return <String, Object?>{'playing': true};
          }
          return null;
        });
  }
}

String _video(Directory dir) {
  final f = File('${dir.path}/trip.mp4')
    ..writeAsBytesSync([0, 0, 0, 24, 0x66, 0x74, 0x79, 0x70, 0, 0, 0, 0]);
  return f.path;
}

Future<BuildContext> _page(WidgetTester tester, AppState state) async {
  late BuildContext context;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Builder(
          builder: (c) {
            context = c;
            return const SizedBox();
          },
        ),
      ),
    ),
  );
  return context;
}

void main() {
  late Directory dir;
  late _Native native;

  setUp(() {
    dir = Directory.systemTemp.createTempSync('fogata-video-closed');
    native = _Native()..install();
  });
  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('fogata/video'), null);
    dir.deleteSync(recursive: true);
  });

  Future<void> closedWhilePlayStarts(WidgetTester tester) async {
    // The file is read for real, so real time passes while the view opens.
    await pumpUntil(
      tester,
      () => native.playing.isCompleted,
      what: 'the video asked to play',
    );
    // The view goes while `play` has not answered yet.
    Navigator.of(tester.element(find.byType(Material).last)).pop();
    await tester.pump(const Duration(seconds: 1));
    await tester.pump(const Duration(seconds: 1));
    native.release.complete();
    await tester.runAsync(() => Future<void>.delayed(Duration.zero));
    await tester.pump();
    await tester.pump(const Duration(seconds: 2));
    expect(native.states, 0, reason: 'nothing asks the player after the close');
    // A periodic timer left behind fails the test when it ends.
  }

  testWidgets('a room video closed while it starts leaves no timer', (
    tester,
  ) async {
    final state = _Local(_video(dir))
      ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
      ..roomState.roomGroup = 'g'
      ..roomState.room = UiRoom(
        startedMs: 0,
        joined: true,
        participants: const [],
        chat: const [],
        files: [
          UiFile(
            id: 'v',
            name: 'trip.mp4',
            size: 1000,
            sharedBy: other(0),
            holders: [other(0)],
            mine: false,
            sent: 0,
          ),
        ],
      );
    final context = await _page(tester, state);
    showRoomVideo(
      context,
      state,
      SeenFile('v', 'trip.mp4', 1000, other(0).id, other(0).name),
    );
    await tester.pump(const Duration(seconds: 1));
    await closedWhilePlayStarts(tester);
  });

  testWidgets('a chat video closed while it starts leaves no timer', (
    tester,
  ) async {
    final state = AppState('/nonexistent/fogata-video-closed');
    final context = await _page(tester, state);
    showDirectVideo(
      context,
      UiDirectFile(
        id: 'v',
        name: 'trip.mp4',
        size: 1000,
        kind: UiFileKind.video,
        state: UiFileState.here,
        received: 1000,
        path: _video(dir),
      ),
      'f',
      other(0),
    );
    await tester.pump(const Duration(seconds: 1));
    await closedWhilePlayStarts(tester);
  });
}
