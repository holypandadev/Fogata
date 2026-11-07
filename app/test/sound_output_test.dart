// The output chosen in the settings takes the application's own sounds and the videos of
// its chats, as it takes calls; nothing goes to the system's default instead.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/screens/room.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

class _Core implements RustLibApi {
  final outputs = <String?>[];

  @override
  void crateApiAppSetSoundOutput({String? id}) => outputs.add(id);

  @override
  Future<void> crateApiAppWritePrefs({
    required String dataDir,
    required String json,
  }) async {}

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

/// A state that plays the file at [path].
class _Local extends AppState {
  _Local(this.path) : super('/nonexistent/fogata-sound-output');
  final String path;

  @override
  Future<String?> videoToPlay(UiFile f) async => path;
}

void main() {
  final core = _Core();
  setUpAll(() => RustLib.initMock(api: core));

  late Directory dir;
  late String video;
  // What the native player was opened with, once it is.
  Map<Object?, Object?>? opened;
  setUp(() {
    dir = Directory.systemTemp.createTempSync('fogata-sound-output');
    video =
        (File('${dir.path}/trip.mp4')..writeAsBytesSync([
              0,
              0,
              0,
              24,
              0x66,
              0x74,
              0x79,
              0x70,
              0,
              0,
              0,
              0,
            ]))
            .path;
    opened = null;
    // The native player: what `open` was given; no player comes of it.
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('fogata/video'), (
          call,
        ) async {
          if (call.method == 'open') {
            opened ??= call.arguments as Map<Object?, Object?>;
          }
          return null;
        });
  });
  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('fogata/video'), null);
    dir.deleteSync(recursive: true);
  });

  test('the sounds go to the output chosen', () async {
    core.outputs.clear();
    final state = AppState('/nonexistent/fogata-sound-output');
    await state.setAudioOutput('speakers-2');
    expect(core.outputs, ['speakers-2']);
    // Back to the system's default.
    await state.setAudioOutput(null);
    expect(core.outputs, ['speakers-2', null]);
    state.dispose();
  });

  /// What the player was opened with. The file's start is read for real before the
  /// player is asked for: frames go on, with real time between them, until it is.
  Future<Map<Object?, Object?>> openArgs(WidgetTester tester) async {
    final waited = Stopwatch()..start();
    while (opened == null) {
      if (waited.elapsed > const Duration(seconds: 20)) {
        fail('the player was never opened');
      }
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 10)),
      );
      await tester.pump();
    }
    return opened!;
  }

  Future<BuildContext> page(WidgetTester tester, AppState state) async {
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

  testWidgets('a room video plays on the output chosen', (tester) async {
    final state = _Local(video)
      ..prefs.audioOutput = 'speakers-2'
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
    final context = await page(tester, state);
    showRoomVideo(
      context,
      state,
      SeenFile('v', 'trip.mp4', 1000, other(0).id, other(0).name),
    );
    await tester.pump(const Duration(seconds: 1));
    final args = await openArgs(tester);
    expect(args['output'], 'speakers-2');
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 1));
  });

  testWidgets('a chat video plays on the output chosen', (tester) async {
    final state = AppState('/nonexistent/fogata-sound-output')
      ..prefs.audioOutput = 'speakers-2';
    final context = await page(tester, state);
    showDirectVideo(
      context,
      UiDirectFile(
        id: 'v',
        name: 'trip.mp4',
        size: 1000,
        kind: UiFileKind.video,
        state: UiFileState.here,
        received: 1000,
        path: video,
      ),
      'f',
      other(0),
    );
    await tester.pump(const Duration(seconds: 1));
    final args = await openArgs(tester);
    expect(args['output'], 'speakers-2');
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 1));
  });
}
