import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/moving.dart';
import 'package:fogata/src/shown.dart';

import 'wait.dart';

/// Three frames of 8 by 8 pixels, red, green and blue, 50 ms each.
final _gif = base64Decode(
  'R0lGODlhCAAIAIEAAP8AAAAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQABQAAACwAAAAACAAIAAAIDwAB'
  'CBxIsKDBgwgTKkwYEAAh+QQBBQABACwAAAAACAAIAIEA/wAAAAAAAAAAAAAIDwABCBxIsKDBgwgTKkwYEAAh+QQB'
  'BQABACwAAAAACAAIAIEAAP8AAAAAAAAAAAAIDwABCBxIsKDBgwgTKkwYEAA7',
);

/// The same in WebP, 16 by 16 pixels, 80 ms each.
final _webp = base64Decode(
  'UklGRhgBAABXRUJQVlA4WAoAAAACAAAADwAADwAAQU5JTQYAAAAAAAAAAABBTk1GSgAAAAAAAAAAAA8AAA8AAFAA'
  'AAJWUDggMgAAADABAJ0BKhAAEAABQCYloAADcAD+8ut///mwP/bz/wR6Af//0uD//pcH//S4P/SkAAAAQU5NRkwA'
  'AAAAAAAAAAAPAAAPAABQAAAAVlA4IDQAAAA0AQCdASoQABAAAAAmJaAAA3AA/ukiH//3nz//ufP/+58/6M///yn7'
  '//I4//8jj/5QIAAAQU5NRkYAAAAAAAAAAAAPAAAPAABQAAAAVlA4IC4AAAA0AQCdASoQABAAAAAmJaAAA3AA/vtV'
  '4///S4P/+lwf/9Lg/9Lg//rV5Vesq6AA',
);

/// A GIF [width] by [height] pixels with [frames] frames of one pixel each, shown one after
/// the other with no wait: tens of bytes that ask for a full-size frame for each.
Uint8List _animation(int width, int height, int frames) {
  final size = ByteData(4)
    ..setUint16(0, width, Endian.little)
    ..setUint16(2, height, Endian.little);
  return Uint8List.fromList([
    ...'GIF89a'.codeUnits,
    ...size.buffer.asUint8List(),
    0x80, 0, 0, // a global table of two colours
    0, 0, 0, 0xff, 0xff, 0xff,
    for (var i = 0; i < frames; i++) ...[
      0x21, 0xf9, 4, 0, 0, 0, 0, 0, // no wait before the next frame
      0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, // one pixel at the top left
      2, 2, 0x44, 0x01, 0, // clear, colour 0, end
    ],
    0x3b,
  ]);
}

/// Lets the pictures' frames be read and drawn, outside the test's own time.
Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 60)),
    );
    await tester.pump();
  }
}

void main() {
  test('GIFs and WebPs are animations, other pictures are not', () {
    expect(isMovingKind(_gif), isTrue);
    expect(
      isMovingKind(
        Uint8List.fromList(utf8.encode('RIFF\x00\x00\x00\x00WEBPVP8X')),
      ),
      isTrue,
    );
    expect(
      isMovingKind(
        Uint8List.fromList([0x89, 0x50, 0x4E, 0x47, 13, 10, 26, 10]),
      ),
      isFalse,
    );
    expect(isMovingKind(Uint8List.fromList([0xFF, 0xD8, 0xFF])), isFalse);
    expect(isMovingKind(Uint8List(0)), isFalse);
  });

  testWidgets('a GIF and a WebP count their frames', (tester) async {
    final frames = await tester.runAsync(() => frameCount(_gif));
    expect(frames, 3);
    expect(isMovingKind(_webp), isTrue);
    expect(await tester.runAsync(() => frameCount(_webp)), 3);
    final none = await tester.runAsync(
      () => frameCount(Uint8List.fromList([1, 2, 3])),
    );
    expect(none, 0);
  });

  testWidgets('animations of up to 550 KB are taken, bigger ones refused', (
    tester,
  ) async {
    // The three frames padded with a comment to `size` bytes.
    Uint8List padded(int size) {
      final out = BytesBuilder()
        ..add(_gif.sublist(0, _gif.length - 1))
        ..add([0x21, 0xFE]);
      while (out.length + 256 + 2 <= size) {
        out.add([255, ...List.filled(255, 0x41)]);
      }
      final last = size - out.length - 3;
      out
        ..add([last, ...List.filled(last, 0x41)])
        ..add([0, 0x3B]);
      return out.toBytes();
    }

    expect(maxMovingPhotoBytes, 550 * 1024);
    expect(maxMovingBannerBytes, 550 * 1024);
    final fits = padded(550 * 1024);
    expect(fits.length, 550 * 1024);
    expect(await tester.runAsync(() => groupBannerOf(fits)), fits);
    final big = padded(560 * 1024);
    Object? refused;
    await tester.runAsync(() async {
      try {
        await groupBannerOf(big);
      } catch (e) {
        refused = e;
      }
    });
    expect(
      '$refused',
      'Animated banners can be up to 550 KB; this one is 560 KB.',
    );
  });

  test('an animation shows only with the still picture it was made for', () {
    final book = MovingBook();
    final asked = <String>[];
    book.ask = (id, {required bool banner}) => asked.add('$id:$banner');
    final still = Uint8List.fromList([1, 2, 3]);
    expect(book.photo('ana', still), isNull);
    expect(book.photo('ana', still), isNull);
    expect(asked, ['ana:false'], reason: 'asked once');
    book.put('ana', (still: still, picture: _gif), banner: false);
    expect(book.photo('ana', Uint8List.fromList([1, 2, 3])), _gif);
    expect(book.banner('ana', still), isNull);
    expect(asked, ['ana:false', 'ana:true']);
    book.put('ana', null, banner: true);
    expect(book.banner('ana', still), isNull);
    expect(asked, [
      'ana:false',
      'ana:true',
    ], reason: 'the same still: not again');
    // Another still picture (the animation came before the picture): asked again.
    expect(book.photo('ana', Uint8List.fromList([4, 5])), isNull);
    expect(asked, ['ana:false', 'ana:true', 'ana:false']);
    book.put('ana', (
      still: Uint8List.fromList([4, 5]),
      picture: _gif,
    ), banner: false);
    expect(book.photo('ana', Uint8List.fromList([4, 5])), _gif);
    // Changed: asked again when shown next.
    book.forget('ana');
    expect(book.photo('ana', still), isNull);
    expect(asked.last, 'ana:false');
  });

  test(
    'the book keeps the people shown last, and asks again for one that went',
    () {
      final book = MovingBook(kept: 3);
      final asked = <String>[];
      book.ask = (id, {required bool banner}) => asked.add(id);
      final still = Uint8List.fromList([1, 2, 3]);
      for (final id in ['ana', 'bea', 'cid']) {
        expect(book.photo(id, still), isNull);
        book.put(id, (still: still, picture: _gif), banner: false);
      }
      // Ana shown again goes last; Dan pushes out Bea, shown least lately.
      expect(book.photo('ana', still), _gif);
      expect(book.photo('dan', still), isNull);
      book.put('dan', (still: still, picture: _gif), banner: false);
      expect(asked, ['ana', 'bea', 'cid', 'dan']);
      expect(book.photo('ana', still), _gif);
      expect(book.photo('cid', still), _gif);
      expect(book.photo('dan', still), _gif);
      expect(asked, ['ana', 'bea', 'cid', 'dan'], reason: 'kept, not asked');
      // Bea went: asked again, and her animation shows once it is back.
      expect(book.photo('bea', still), isNull);
      expect(asked.last, 'bea');
      book.put('bea', (still: still, picture: _gif), banner: false);
      expect(book.photo('bea', still), _gif);
      // An answer for someone no longer kept is dropped.
      book.put('ana', (still: still, picture: _gif), banner: false);
      expect(book.photo('ana', still), isNull);
      expect(asked.last, 'ana');
    },
  );

  testWidgets('an animation steps only while the window is seen and focused', (
    tester,
  ) async {
    Future<void> settle() async {
      for (var i = 0; i < 6; i++) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 60)),
        );
        await tester.pump();
      }
    }

    await tester.pumpWidget(
      Center(
        child: SizedBox.square(
          dimension: 64,
          child: MovingPicture(
            bytes: _gif,
            maxPixels: maxMovingPhotoPixels,
            round: true,
            still: const ColoredBox(
              key: ValueKey('still'),
              color: Color(0xFF000000),
            ),
          ),
        ),
      ),
    );
    await pumpUntil(
      tester,
      () => find.byKey(const ValueKey('still')).evaluate().isEmpty,
      what: 'the first frame shown',
    );
    final painter = find.descendant(
      of: find.byType(MovingPicture),
      matching: find.byType(CustomPaint),
    );
    expect(painter, findsOneWidget);

    windowFocused.value = false;
    expect(motionAllowed.value, isFalse);
    windowFocused.value = true;
    windowShown.value = false;
    expect(motionAllowed.value, isFalse);
    windowShown.value = true;
    expect(motionAllowed.value, isTrue);
    await settle();
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('a damaged animation is said to be unreadable, not too large', (
    tester,
  ) async {
    final damaged = Uint8List.fromList([...'GIF89a'.codeUnits, 1, 2, 3, 4]);
    expect(isMovingKind(damaged), isTrue);
    final problem = await tester.runAsync(() async {
      try {
        await groupBannerOf(damaged);
        return null;
      } on PictureProblem catch (e) {
        return e.message;
      }
    });
    expect(problem, 'That picture could not be read.');
  });

  testWidgets('frames of a load replaced or left unfinished are freed', (
    tester,
  ) async {
    final before = liveMovingFrames;
    Widget sized(double side) => Center(
      child: SizedBox.square(
        dimension: side,
        child: MovingPicture(
          bytes: _webp,
          maxPixels: maxMovingPhotoPixels,
          still: const SizedBox(),
        ),
      ),
    );
    // Three sizes, each more than twice the one before (a new load) before the one
    // before it is ready, then gone before any is.
    await tester.pumpWidget(sized(64));
    await tester.pumpWidget(sized(160));
    await tester.pumpWidget(sized(400));
    expect(liveMovingFrames, before + 3);
    await tester.pumpWidget(const SizedBox());
    await pumpUntil(
      tester,
      () => liveMovingFrames == before,
      what: 'the unfinished loads freed',
    );

    // Shown, then a new size: the old frames go once the new ones are ready.
    await tester.pumpWidget(sized(64));
    await pumpUntil(
      tester,
      () => find
          .descendant(
            of: find.byType(MovingPicture),
            matching: find.byType(CustomPaint),
          )
          .evaluate()
          .isNotEmpty,
      what: 'the first frames shown',
    );
    await tester.pumpWidget(sized(256));
    expect(liveMovingFrames, before + 2);
    await pumpUntil(
      tester,
      () => liveMovingFrames == before + 1,
      what: 'the old frames freed once the new were ready',
    );
    await tester.pumpWidget(const SizedBox());
    expect(liveMovingFrames, before);
  });

  testWidgets(
    'a new size keeps the frames unless the box grows more than twice',
    (tester) async {
      final before = liveMovingFrames;
      Widget sized(double side) => Center(
        child: SizedBox.square(
          dimension: side,
          child: MovingPicture(
            bytes: _gif,
            maxPixels: maxMovingPhotoPixels,
            still: const SizedBox(key: ValueKey('still')),
          ),
        ),
      );
      await tester.pumpWidget(sized(64));
      await pumpUntil(
        tester,
        () => find.byKey(const ValueKey('still')).evaluate().isEmpty,
        what: 'the first frame shown',
      );
      expect(liveMovingFrames, before + 1);
      // Smaller, or larger by up to twice: the same frames, still showing.
      for (final side in [40.0, 100.0, 128.0, 64.0]) {
        await tester.pumpWidget(sized(side));
        expect(liveMovingFrames, before + 1);
        expect(find.byKey(const ValueKey('still')), findsNothing);
      }
      // More than twice as large: decoded again, the old frames go once the new are ready.
      await tester.pumpWidget(sized(200));
      expect(liveMovingFrames, before + 2);
      await pumpUntil(
        tester,
        () => liveMovingFrames == before + 1,
        what: 'the old frames freed once the new were ready',
      );
      await tester.pumpWidget(const SizedBox());
      expect(liveMovingFrames, before);
    },
  );

  testWidgets(
    'an animation plays only within the pixels of what it moves for',
    (tester) async {
      // 600 by 600: too many pixels for a profile picture, not for a banner.
      final moving = _animation(600, 600, 2);
      Widget shown(int maxPixels) => Center(
        child: SizedBox.square(
          dimension: 64,
          child: MovingPicture(
            bytes: moving,
            maxPixels: maxPixels,
            still: const SizedBox(key: ValueKey('still')),
          ),
        ),
      );
      await tester.pumpWidget(shown(maxMovingPhotoPixels));
      await _settle(tester);
      expect(find.byKey(const ValueKey('still')), findsOneWidget);
      expect(playingAnimations, 0);
      await tester.pumpWidget(const SizedBox());
      await tester.pumpWidget(shown(maxMovingBannerPixels));
      await pumpUntil(
        tester,
        () => playingAnimations == 1,
        what: 'the banner-sized animation playing',
      );
      expect(find.byKey(const ValueKey('still')), findsNothing);
      await tester.pumpWidget(const SizedBox());
      expect(playingAnimations, 0);
    },
  );

  testWidgets('an animation of too many frames is not played', (tester) async {
    await tester.pumpWidget(
      Center(
        child: SizedBox.square(
          dimension: 64,
          child: MovingPicture(
            bytes: _animation(8, 8, maxMovingFrames + 1),
            maxPixels: maxMovingPhotoPixels,
            still: const SizedBox(key: ValueKey('still')),
          ),
        ),
      ),
    );
    await _settle(tester);
    expect(find.byKey(const ValueKey('still')), findsOneWidget);
    expect(playingAnimations, 0);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('animations too large or too long for their use are refused', (
    tester,
  ) async {
    Future<String?> problem(Future<Object?> Function() pick) =>
        tester.runAsync<String?>(() async {
          try {
            await pick();
            return null;
          } on PictureProblem catch (e) {
            return e.message;
          }
        });
    expect(
      await problem(() => groupBannerOf(_animation(1100, 1100, 2))),
      'Animated banners can be up to 1024\u00a0×\u00a0576 pixels; this one is '
      '1100\u00a0×\u00a01100.',
    );
    expect(
      await problem(() => groupBannerOf(_animation(8, 8, maxMovingFrames + 1))),
      'Animated banners can have up to 500 frames; this one has 501.',
    );
    // A group's banner of 16 by 9 fits.
    final fits = _animation(960, 540, maxMovingFrames);
    expect(await tester.runAsync(() => groupBannerOf(fits)), fits);
  });

  testWidgets('a few animations play at once, the others wait on a frame', (
    tester,
  ) async {
    Widget many(int count) => Wrap(
      children: [
        for (var i = 0; i < count; i++)
          SizedBox.square(
            dimension: 64,
            child: MovingPicture(
              key: ValueKey(i),
              bytes: _gif,
              maxPixels: maxMovingPhotoPixels,
              still: SizedBox(key: ValueKey('still $i')),
            ),
          ),
      ],
    );
    await tester.pumpWidget(
      Directionality(textDirection: TextDirection.ltr, child: many(8)),
    );
    // The ones waiting show their first frame, not the still picture.
    await pumpUntil(
      tester,
      () =>
          playingAnimations == maxPlayingAnimations &&
          find
              .byWidgetPredicate((w) => '${w.key}'.contains('still'))
              .evaluate()
              .isEmpty,
      what: 'every animation shown, a few playing',
    );
    expect(playingAnimations, maxPlayingAnimations);
    expect(
      find.byWidgetPredicate((w) => '${w.key}'.contains('still')),
      findsNothing,
    );
    // Three go: the two waiting play.
    await tester.pumpWidget(
      Directionality(textDirection: TextDirection.ltr, child: many(5)),
    );
    await pumpUntil(
      tester,
      () => playingAnimations == 5,
      what: 'the waiting animations playing',
    );
    // The window loses its focus: none plays.
    windowFocused.value = false;
    await tester.pump();
    expect(playingAnimations, 0);
    windowFocused.value = true;
    await tester.pump();
    expect(playingAnimations, 5);
    await tester.pumpWidget(const SizedBox());
    expect(playingAnimations, 0);
  });
}
