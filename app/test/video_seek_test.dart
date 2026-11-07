// The player is never asked to go before a video's start, whatever the bar was dragged to.

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata_video/fogata_video.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('fogata/video');

  test('a seek before the start goes to the start', () async {
    final dir = Directory.systemTemp.createTempSync('fogata-video-seek');
    addTearDown(() => dir.deleteSync(recursive: true));
    final path =
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
    final seeks = <Object?>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          if (call.method == 'open') return 1;
          if (call.method == 'seek') {
            seeks.add((call.arguments as Map<Object?, Object?>)['ms']);
          }
          return null;
        });
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null),
    );
    final player = (await VideoPlayer.open(path))!;
    await player.seek(-1500);
    await player.seek(0);
    await player.seek(2500);
    expect(seeks, [0, 0, 2500]);
  });
}
