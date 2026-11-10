// A video playing over a window that is hidden does not ask its player where it is: that
// is only for the screen.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/shown.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';
import 'wait.dart';

void main() {
  late Directory dir;
  var states = 0;
  var playing = false;

  setUp(() {
    dir = Directory.systemTemp.createTempSync('fogata-video-hidden');
    states = 0;
    playing = false;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('fogata/video'), (
          call,
        ) async {
          switch (call.method) {
            case 'open':
              return 1;
            case 'play':
              playing = true;
              return null;
            case 'state':
              states++;
              return <String, Object?>{'playing': true};
          }
          return null;
        });
  });
  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('fogata/video'), null);
    windowShown.value = true;
    dir.deleteSync(recursive: true);
  });

  testWidgets('a chat video asks nothing while the window is hidden', (
    tester,
  ) async {
    final path = '${dir.path}/trip.mp4';
    File(path).writeAsBytesSync([0, 0, 0, 24, 0x66, 0x74, 0x79, 0x70]);
    final state = AppState('/nonexistent/fogata-video-hidden');
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
    showDirectVideo(
      context,
      UiDirectFile(
        id: 'v',
        name: 'trip.mp4',
        size: 8,
        kind: UiFileKind.video,
        state: UiFileState.here,
        received: 8,
        path: path,
      ),
      'f',
      other(0),
    );
    await pumpUntil(tester, () => playing, what: 'the video playing');
    await tester.pump(const Duration(seconds: 1));
    expect(states, greaterThan(0));
    windowShown.value = false;
    await tester.pump(const Duration(milliseconds: 300));
    final hidden = states;
    await tester.pump(const Duration(seconds: 2));
    expect(states, hidden);
    windowShown.value = true;
    await tester.pump(const Duration(seconds: 1));
    expect(states, greaterThan(hidden));
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 1));
  });
}
