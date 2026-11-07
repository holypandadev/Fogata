import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';

void main() {
  group('from the environment, where GTK\'s display is not known', () {
    test('a desktop session that says it is Wayland is one', () {
      expect(
        waylandSession({
          'XDG_SESSION_TYPE': 'wayland',
          'WAYLAND_DISPLAY': 'wayland-0',
          'DISPLAY': ':0',
        }),
        isTrue,
      );
    });

    test('only a Wayland display named, as from a service, is Wayland too', () {
      // The environment a program started from a user service had when sharing the
      // screen ended it: no session type and no X display, so nothing could be listed.
      expect(waylandSession({'WAYLAND_DISPLAY': 'wayland-0'}), isTrue);
      expect(
        waylandSession({'WAYLAND_DISPLAY': 'wayland-0', 'DISPLAY': ''}),
        isTrue,
      );
    });

    test('an X display with no Wayland session is X11', () {
      expect(waylandSession({'DISPLAY': ':0'}), isFalse);
      expect(
        waylandSession({'XDG_SESSION_TYPE': 'x11', 'DISPLAY': ':0'}),
        isFalse,
      );
      // XWayland named alongside, with no session type: X11, unless GTK says otherwise.
      expect(
        waylandSession({'WAYLAND_DISPLAY': 'wayland-0', 'DISPLAY': ':0'}),
        isFalse,
      );
    });

    test('a Wayland display named empty counts as named, as for WebRTC', () {
      expect(
        waylandSession({'XDG_SESSION_TYPE': 'wayland', 'WAYLAND_DISPLAY': ''}),
        isTrue,
      );
      expect(waylandSession({'WAYLAND_DISPLAY': ''}), isTrue);
      expect(waylandSession({'WAYLAND_DISPLAY': '', 'DISPLAY': ':0'}), isFalse);
      expect(
        waylandSession({'XDG_SESSION_TYPE': 'wayland', 'DISPLAY': ':0'}),
        isFalse,
      );
    });
  });

  group('from the display GTK draws on, as the video library tells it', () {
    final messenger =
        TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
    final calls = <String>[];
    late bool wayland;

    setUp(() {
      forgetWaylandAnswer();
      calls.clear();
      messenger.setMockMethodCallHandler(
        const MethodChannel('FlutterWebRTC.Method'),
        (call) async {
          // WebRTC's own setup, the first time anything is asked, is not counted.
          if (call.method != 'initialize') calls.add(call.method);
          switch (call.method) {
            case 'fogataWaylandSession':
              return wayland;
            case 'getDesktopSources':
              return {'sources': <Object>[]};
          }
          return null;
        },
      );
    });
    tearDown(() {
      forgetWaylandAnswer();
      messenger.setMockMethodCallHandler(
        const MethodChannel('FlutterWebRTC.Method'),
        null,
      );
    });

    test(
      'GTK on Wayland: no list is asked for, whatever the environment',
      () async {
        wayland = true;
        final call = Call(group: 'g', me: 'f000000000000000');
        expect(await call.screens(), isEmpty);
        expect(await call.screens(), isEmpty);
        // Asked once, and libwebrtc never for its X11 lists.
        expect(calls, ['fogataWaylandSession']);
      },
    );

    test('GTK on X11: the screens and windows are listed', () async {
      wayland = false;
      final call = Call(group: 'g', me: 'f000000000000000');
      await call.screens();
      expect(calls, ['fogataWaylandSession', 'getDesktopSources']);
    });

    test(
      'the portal is the way where GTK draws on Wayland, and only there',
      () async {
        // As the smoke test asks it: a program given only WAYLAND_DISPLAY still has GTK on
        // Wayland, and must not look for a list there.
        wayland = true;
        expect(await screenThroughPortal(), isTrue);
        forgetWaylandAnswer();
        wayland = false;
        expect(await screenThroughPortal(), isFalse);
        expect(calls, ['fogataWaylandSession', 'fogataWaylandSession']);
      },
    );
  });
}
