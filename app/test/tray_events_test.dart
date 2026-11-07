// The Linux tray icon's clicks: should their stream end or fail, the window no longer
// counts on the icon to come back.

import 'dart:async';
import 'dart:ui';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/rust/api/tray.dart';
import 'package:fogata/src/window.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  Future<void> quit() async {}

  test('the window stops counting on the icon when its events end', () async {
    final events = StreamController<UiTrayEvent>();
    followLinuxTray(events.stream, quit);
    expect(hasTray, isTrue);
    await events.close();
    await pumpEventQueue();
    expect(hasTray, isFalse);
  });

  test(
    'a failure of the icon\'s events is noted, and the icon let go',
    () async {
      final noted = <Object>[];
      final before = PlatformDispatcher.instance.onError;
      PlatformDispatcher.instance.onError = (error, stack) {
        noted.add(error);
        return true;
      };
      addTearDown(() => PlatformDispatcher.instance.onError = before);
      final events = StreamController<UiTrayEvent>();
      followLinuxTray(events.stream, quit);
      expect(hasTray, isTrue);
      events.addError(StateError('the bus went away'));
      await pumpEventQueue();
      expect(hasTray, isFalse);
      expect(noted.single.toString(), contains('the bus went away'));
      await events.close();
    },
  );
}
