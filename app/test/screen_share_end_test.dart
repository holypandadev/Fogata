// A screen share the system refuses, or that ends by itself (the shared window closed,
// the system's own "stop sharing"), or that can no longer be captured again at another
// quality, leaves the call not sharing, with nothing captured.

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/l10n.dart';

const me = 'f000000000000000';

/// A window picked from Fogata's own list.
class Window extends DesktopCapturerSource {
  @override
  String get id => 'w1';
  @override
  String get name => 'A window';
  @override
  Uint8List? get thumbnail => null;
  @override
  ThumbnailSize get thumbnailSize => ThumbnailSize(0, 0);
  @override
  SourceType get type => SourceType.Window;
}

void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  final calls = <String>[];
  // The streams renderers were given to draw.
  final drawn = <String>[];
  final textures = <int, MockStreamHandlerEventSink>{};
  MockStreamHandlerEventSink? events;
  // What getDisplayMedia answers.
  late Future<Object?> Function() display;

  setUpAll(() {
    messenger.setMockStreamHandler(
      const EventChannel('FlutterWebRTC.Event'),
      MockStreamHandler.inline(onListen: (_, sink) => events = sink),
    );
  });

  setUp(() {
    calls.clear();
    drawn.clear();
    textures.clear();
    onWebRtcEventError = (what, error, stack) {};
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        calls.add(call.method);
        switch (call.method) {
          case 'createVideoRenderer':
            final id = textures.length + 1;
            messenger.setMockStreamHandler(
              EventChannel('FlutterWebRTC/Texture$id'),
              MockStreamHandler.inline(
                onListen: (_, sink) => textures[id] = sink,
              ),
            );
            return <String, dynamic>{'textureId': id};
          case 'getDisplayMedia':
            return display();
          case 'videoRendererSetSrcObject':
            // The capture's first picture: its size.
            final id = (call.arguments as Map)['textureId'] as int;
            final stream = (call.arguments as Map)['streamId'] as String;
            if (stream != '') {
              drawn.add(stream);
              textures[id]?.success({
                'event': 'didTextureChangeVideoSize',
                'id': id,
                'width': 1280,
                'height': 720,
              });
            }
            return null;
        }
        return <String, dynamic>{};
      },
    );
  });
  tearDown(() {
    onWebRtcEventError = null;
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      null,
    );
  });

  Future<(Call, List<String>)> call() async {
    final call = Call(group: 'g', me: me);
    final problems = <String>[];
    call.onProblem = problems.add;
    await call.localScreen.initialize();
    await call.localCamera.initialize();
    return (call, problems);
  }

  test('a share the system refuses is told, and nothing is shared', () async {
    display = () async => throw PlatformException(
      code: 'getDisplayMedia',
      message:
          'The user declined TCCs for application, window, display capture',
    );
    final (c, problems) = await call();
    await c.shareScreen(null);
    expect(c.sharing, isFalse);
    expect(problems, hasLength(1));
    expect(drawn, isEmpty);
  });

  test('the system\'s dialog closed is no failure', () async {
    display = () async =>
        throw PlatformException(code: 'getDisplayMedia', message: 'cancelled');
    final (c, problems) = await call();
    await c.shareScreen(null);
    expect(c.sharing, isFalse);
    expect(problems, isEmpty);
  });

  test(
    'a share that ends by itself stops sharing and lets the capture go',
    () async {
      display = () async => {
        'streamId': 'screen',
        'audioTracks': [],
        'videoTracks': [
          {'id': 'window', 'label': 'window', 'kind': 'video', 'enabled': true},
        ],
      };
      final (c, problems) = await call();
      await c.shareScreen(null);
      expect(c.sharing, isTrue);
      calls.clear();
      // The shared window is closed: the platform tells the track ended.
      events!.success({'event': 'mediaStreamTrackEnded', 'trackId': 'window'});
      await pumpEventQueue();
      expect(c.sharing, isFalse);
      expect(calls, containsAllInOrder(['trackDispose', 'streamDispose']));
      expect(problems, isEmpty);
    },
  );

  test('a window that cannot be captured again at another quality stops the share, told', () async {
    display = () async => {
      'streamId': 'screen',
      'audioTracks': [],
      'videoTracks': [
        {'id': 'window', 'label': 'window', 'kind': 'video', 'enabled': true},
      ],
    };
    final (c, problems) = await call();
    await c.shareScreen(Window());
    expect(c.sharing, isTrue);
    calls.clear();
    // The window has closed: the plugin cannot start a capturer of it any more.
    display = () async => throw PlatformException(
      code: 'getDisplayMedia',
      message: 'the screen or window could not be captured',
    );
    await c.setScreenQuality(
      ScreenQuality(fps: c.screenQuality.fps == 30 ? 15 : 30),
    );
    expect(c.sharing, isFalse);
    expect(problems, [l10n.callShareLost]);
    expect(calls, containsAllInOrder(['trackDispose', 'streamDispose']));
  });

  test("Fogata's own window is not offered to share under X11", () {
    expect(ownWindow(SourceType.Window, 'Fogata', linux: true), isTrue);
    // Another window, a screen, or a title that only has the name in it.
    expect(ownWindow(SourceType.Window, 'Terminal', linux: true), isFalse);
    expect(ownWindow(SourceType.Screen, 'Fogata', linux: true), isFalse);
    expect(ownWindow(SourceType.Window, 'Fogata - Firefox', linux: true), isFalse);
    // macOS and Windows leave it out of the list themselves.
    expect(ownWindow(SourceType.Window, 'Fogata', linux: false), isFalse);
  });
}
