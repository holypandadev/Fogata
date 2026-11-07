import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata_video/fogata_video.dart';

List<int> bytes(String text) => text.codeUnits;

void main() {
  const ebml = [0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81];

  test('a video is read only when it begins as its name says', () {
    expect(
      videoHeadMatches('clip.mp4', bytes('\x00\x00\x00\x20ftypisom')),
      isTrue,
    );
    expect(
      videoHeadMatches('clip.M4V', bytes('\x00\x00\x00\x20ftypM4V ')),
      isTrue,
    );
    expect(
      videoHeadMatches('old.mov', bytes('\x00\x00\x01\x00moov....')),
      isTrue,
    );
    expect(
      videoHeadMatches('old.mov', bytes('\x00\x00\x00\x08wide....')),
      isTrue,
    );
    expect(videoHeadMatches('clip.webm', ebml), isTrue);
    expect(videoHeadMatches('clip.mkv', ebml), isTrue);
    expect(
      videoHeadMatches('clip.avi', bytes('RIFF\x10\x00\x00\x00AVI LIST')),
      isTrue,
    );
  });

  test('anything else is not', () {
    // A playlist, another container under a video's name, a wave file.
    expect(videoHeadMatches('clip.mp4', bytes('#EXTM3U\n#EXT-X')), isFalse);
    expect(videoHeadMatches('clip.mp4', ebml), isFalse);
    expect(
      videoHeadMatches('clip.mkv', bytes('\x00\x00\x00\x20ftypisom')),
      isFalse,
    );
    expect(
      videoHeadMatches('clip.avi', bytes('RIFF\x10\x00\x00\x00WAVEfmt ')),
      isFalse,
    );
    expect(videoHeadMatches('clip.webm', bytes('\x47\x40\x00\x10')), isFalse);
    // Too short, or not a video's name.
    expect(videoHeadMatches('clip.mp4', bytes('\x00\x00\x00')), isFalse);
    expect(videoHeadMatches('clip.mkv', const []), isFalse);
    expect(videoHeadMatches('clip.ts', bytes('\x47\x40\x00\x10')), isFalse);
    expect(videoHeadMatches('clip', ebml), isFalse);
  });

  test('the heads say which bytes count, as the players read them', () {
    final avi = videoHeads('clip.avi').single;
    expect(avi.bytes, [...'RIFF'.codeUnits, 0, 0, 0, 0, ...'AVI '.codeUnits]);
    expect(avi.mask, [for (var i = 0; i < 12; i++) i < 4 || i >= 8 ? 255 : 0]);
    for (final h in videoHeads('clip.mov')) {
      expect(h.bytes.length, 8);
      expect(h.mask, [0, 0, 0, 0, 255, 255, 255, 255]);
    }
    // At most as many bytes as the players read (video_head.h, OpenedVideo.swift).
    for (final name in [
      'a.mp4',
      'a.m4v',
      'a.mov',
      'a.mkv',
      'a.webm',
      'a.avi',
    ]) {
      expect(videoHeads(name), isNotEmpty, reason: name);
      for (final h in videoHeads(name)) {
        expect(h.bytes.length, lessThanOrEqualTo(16));
        expect(h.mask.length, h.bytes.length);
      }
    }
  });

  group('the player', () {
    TestWidgetsFlutterBinding.ensureInitialized();
    const channel = MethodChannel('fogata/video');
    final calls = <MethodCall>[];
    setUp(() {
      calls.clear();
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            calls.add(call);
            return call.method == 'open' ? 7 : null;
          });
    });
    tearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null),
    );

    test(
      'is given the path and the heads to check, and opens it itself',
      () async {
        final player = await VideoPlayer.open('/videos/clip.mkv');
        expect(player?.textureId, 7);
        await probeVideo('/videos/clip.mp4');
        expect(calls.map((c) => c.method), ['open', 'probe']);
        for (final call in calls) {
          final args = call.arguments as Map;
          final heads = args['heads'] as List;
          expect(heads, isNotEmpty);
          expect((heads.first as Map).keys, containsAll(['bytes', 'mask']));
        }
        expect((calls.first.arguments as Map)['path'], '/videos/clip.mkv');
        expect(
          ((calls.first.arguments as Map)['heads'] as List).single['bytes'],
          [0x1A, 0x45, 0xDF, 0xA3],
        );
      },
    );

    test('is not asked about what is no video by its name', () async {
      expect(await VideoPlayer.open('/videos/notes.txt'), isNull);
      expect(await probeVideo('/videos/clip.ts'), isNull);
      expect(calls, isEmpty);
    });

    test('a file it would not open gives no player', () async {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async => null);
      expect(await VideoPlayer.open('/videos/fake.mp4'), isNull);
    });
  });
}
