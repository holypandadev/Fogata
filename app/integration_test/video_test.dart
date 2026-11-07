// Videos with this system's own player: reads the test clip's first picture and length,
// plays it and checks it moves. With FOGATA_SHOTS set to a folder, saves the first picture
// there. The clip (integration_test/clip.mp4, four seconds of a test pattern and a tone)
// is found next to the working folder or given in FOGATA_CLIP; the other test videos are
// in the folder `videos` next to it.
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata_video/fogata_video.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  final clip =
      Platform.environment['FOGATA_CLIP'] ??
      '${Directory.current.path}/integration_test/clip.mp4';

  testWidgets('a video is read and played', (tester) async {
    final probe = await probeVideo(clip, maxWidth: 320);
    expect(probe, isNotNull, reason: 'cannot read $clip');
    expect((probe!.width, probe.height), (480, 270));
    expect(probe.durationMs, inInclusiveRange(3900, 4100));
    expect(probe.thumb.sublist(0, 2), [0xFF, 0xD8], reason: 'not a JPEG');
    final shots = Platform.environment['FOGATA_SHOTS'];
    if (shots != null) {
      await File('$shots/video-first-picture.jpg').writeAsBytes(probe.thumb);
    }

    final player = await VideoPlayer.open(clip);
    expect(player, isNotNull);
    await tester.pumpWidget(
      MaterialApp(
        home: Center(
          child: SizedBox(
            width: 480,
            height: 270,
            child: VideoView(player: player!),
          ),
        ),
      ),
    );
    await player.setVolume(0);
    await player.play();
    var state = await player.state();
    for (var i = 0; i < 40 && state.positionMs < 1000; i++) {
      await tester.pump(const Duration(milliseconds: 100));
      await Future<void>.delayed(const Duration(milliseconds: 100));
      state = await player.state();
    }
    expect(state.error, isNull);
    expect(state.positionMs, greaterThanOrEqualTo(1000));
    expect((state.width, state.height), (480, 270));
    await player.seek(3500);
    for (var i = 0; i < 40 && !state.ended; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 100));
      state = await player.state();
    }
    expect(state.ended, isTrue);
    await player.close();
  });

  testWidgets('a video filmed upright on a phone is shown upright', (
    tester,
  ) async {
    // The clip as phones store an upright video: the same landscape pictures, and a
    // quarter turn in its video track's matrix.
    final dir = await Directory.systemTemp.createTemp('fogata-upright');
    final upright = File('${dir.path}/upright.mp4');
    await upright.writeAsBytes(quarterTurned(await File(clip).readAsBytes()));
    final probe = await probeVideo(upright.path, maxWidth: 320);
    expect(probe, isNotNull);
    expect((probe!.width, probe.height), (270, 480));
    final shots = Platform.environment['FOGATA_SHOTS'];
    if (shots != null) {
      await File('$shots/video-upright-picture.jpg').writeAsBytes(probe.thumb);
    }
    // Its first picture is the clip's turned a quarter turn clockwise (the clip's left
    // edge, read upwards, is the top edge): far closer to that than to the other turn.
    final flat = await _pixels((await probeVideo(clip, maxWidth: 320))!.thumb);
    final turned = await _pixels(probe.thumb);
    expect(turned.height, greaterThan(turned.width));
    double apart(List<double> Function(double, double) from) {
      var sum = 0.0;
      for (var i = 1; i < 10; i++) {
        for (var j = 1; j < 10; j++) {
          final a = turned.at(i / 10, j / 10);
          final b = flat.at(from(i / 10, j / 10)[0], from(i / 10, j / 10)[1]);
          for (var c = 0; c < 3; c++) {
            sum += (a[c] - b[c]).abs();
          }
        }
      }
      return sum / (81 * 3);
    }

    final clockwise = apart((u, v) => [v, 1 - u]);
    final anticlockwise = apart((u, v) => [1 - v, u]);
    expect(
      clockwise,
      lessThan(anticlockwise / 3),
      reason: 'apart $clockwise turned clockwise, $anticlockwise the other way',
    );
    final player = await VideoPlayer.open(upright.path);
    expect(player, isNotNull);
    await tester.pumpWidget(
      MaterialApp(
        home: Center(
          child: SizedBox(
            width: 270,
            height: 480,
            child: VideoView(player: player!),
          ),
        ),
      ),
    );
    await player.setVolume(0);
    await player.play();
    var state = await player.state();
    for (var i = 0; i < 40 && state.positionMs < 1000; i++) {
      await tester.pump(const Duration(milliseconds: 100));
      await Future<void>.delayed(const Duration(milliseconds: 100));
      state = await player.state();
    }
    expect(state.error, isNull);
    expect((state.width, state.height), (270, 480));
    await player.close();
    await dir.delete(recursive: true);
  });

  final videos = '${File(clip).parent.path}/videos';

  // Reads `player`'s state until `done` holds, for at most `seconds`.
  Future<VideoState> stateWhen(
    VideoPlayer player,
    bool Function(VideoState) done, {
    int seconds = 5,
  }) async {
    var state = await player.state();
    for (var i = 0; i < seconds * 10 && !done(state); i++) {
      await Future<void>.delayed(const Duration(milliseconds: 100));
      state = await player.state();
    }
    return state;
  }

  testWidgets('a video whose pictures are too large is not played', (
    tester,
  ) async {
    // Two black pictures of 8200 by 16 pixels, MJPEG in Matroska: wider than 8192.
    final large = '$videos/large.mkv';
    expect(await probeVideo(large, maxWidth: 320), isNull);
    final player = await VideoPlayer.open(large);
    if (player != null) {
      await player.setVolume(0);
      await player.play();
      final state = await stateWhen(player, (s) => s.error != null);
      expect(state.error, isNotNull);
      await player.close();
    }
  });

  testWidgets('a video in MJPEG plays', (tester) async {
    // Two seconds of a test pattern, MJPEG in AVI, as some cameras record.
    final mjpeg = '$videos/mjpeg.avi';
    final probe = await probeVideo(mjpeg, maxWidth: 160);
    // Media Foundation and AVFoundation may not read AVI; GStreamer must.
    if (!Platform.isLinux && probe == null) return;
    expect(probe, isNotNull);
    expect((probe!.width, probe.height), (160, 90));
    final player = await VideoPlayer.open(mjpeg);
    expect(player, isNotNull);
    await player!.setVolume(0);
    await player.play();
    final state = await stateWhen(player, (s) => s.ended || s.error != null);
    expect(state.error, isNull);
    expect(state.ended, isTrue);
    await player.close();
  });

  testWidgets('a video in a codec the chat does not offer is not played', (
    tester,
  ) async {
    // Theora in Matroska: GStreamer has a decoder for it, which is not used.
    final theora = '$videos/theora.mkv';
    expect(await probeVideo(theora, maxWidth: 160), isNull);
    final player = await VideoPlayer.open(theora);
    if (player != null) {
      await player.play();
      final state = await stateWhen(player, (s) => s.error != null);
      expect(state.error, isNotNull);
      expect(state.positionMs, 0);
      await player.close();
    }
  });

  testWidgets('a video that is not what its name says is not read', (
    tester,
  ) async {
    final dir = await Directory.systemTemp.createTemp('fogata-named');
    final fake = File('${dir.path}/clip.mp4');
    await File('$videos/mjpeg.avi').copy(fake.path);
    expect(await probeVideo(fake.path, maxWidth: 160), isNull);
    expect(await VideoPlayer.open(fake.path), isNull);
    await dir.delete(recursive: true);
  });

  testWidgets('a playlist posing as a video is not followed', (tester) async {
    // Whatever the system's player would fetch from it arrives here.
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    var fetched = false;
    server.listen((socket) {
      fetched = true;
      socket.destroy();
    });
    final dir = await Directory.systemTemp.createTemp('fogata-playlist');
    final fake = File('${dir.path}/clip.mp4');
    final address = 'http://127.0.0.1:${server.port}';
    await fake.writeAsString(
      '#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:4\n'
      '#EXTINF:4.0,\n$address/part.ts\n#EXT-X-ENDLIST\n',
    );
    final probe = await probeVideo(fake.path, maxWidth: 320);
    final player = await VideoPlayer.open(fake.path);
    if (player != null) {
      await player.play();
      await Future<void>.delayed(const Duration(seconds: 2));
      await player.close();
    } else {
      await Future<void>.delayed(const Duration(seconds: 2));
    }
    await server.close();
    await dir.delete(recursive: true);
    expect(probe, isNull);
    expect(fetched, isFalse, reason: 'the player fetched $address');
  });
}

/// [mp4] with its first track's matrix turned a quarter turn clockwise, as phones write
/// it for a video filmed upright.
List<int> quarterTurned(List<int> mp4) {
  final bytes = Uint8List.fromList(mp4);
  final data = ByteData.sublistView(bytes);
  for (var i = 4; i + 80 < bytes.length; i++) {
    if (String.fromCharCodes(bytes.sublist(i, i + 4)) != 'tkhd') continue;
    // Version 0: the matrix follows four times, two reserved words, layer, group, volume.
    expect(bytes[i + 4], 0, reason: 'a tkhd of version 0');
    // Moved right by the picture's height (16.16, after the matrix and the width), so
    // the turned picture stays in place.
    final height = data.getUint32(i + 84);
    final values = [0, 0x10000, 0, -0x10000, 0, 0, height, 0, 0x40000000];
    for (var k = 0; k < values.length; k++) {
      data.setInt32(i + 44 + 4 * k, values[k]);
    }
    return bytes;
  }
  throw StateError('no tkhd');
}

/// A picture's RGBA pixels.
class _Pixels {
  _Pixels(this.width, this.height, this.rgba);
  final int width;
  final int height;
  final ByteData rgba;

  /// Red, green and blue at (x, y), each from 0 to 1 of the picture's width and height.
  List<int> at(double x, double y) {
    final px = (x * (width - 1)).round();
    final py = (y * (height - 1)).round();
    final i = (py * width + px) * 4;
    return [rgba.getUint8(i), rgba.getUint8(i + 1), rgba.getUint8(i + 2)];
  }
}

Future<_Pixels> _pixels(List<int> encoded) async {
  final image = await decodeImageFromList(Uint8List.fromList(encoded));
  final data = await image.toByteData();
  final pixels = _Pixels(image.width, image.height, data!);
  image.dispose();
  return pixels;
}
