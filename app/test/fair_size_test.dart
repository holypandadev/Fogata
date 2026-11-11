// Sizes and lengths another device describes (a video's shape, how long it lasts) are
// drawn within what can be drawn, whatever they say.

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/fair_size.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/pip.dart' show pipSize;
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/video_view.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

/// A picture of one pixel, as a video's first picture.
final _pixel = base64Decode(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
);

/// The shapes a video can say it has, as the core hands them over (any `u32`).
const _hostile = [
  (1, 4294967295),
  (4294967295, 1),
  (0, 720),
  (1280, 0),
  (0, 0),
  (-5, 720),
];

void main() {
  test('a shape is kept within what can be drawn, 16:9 when it tells '
      'nothing', () {
    expect(fairAspect(1280, 720), closeTo(16 / 9, 1e-9));
    expect(fairAspect(1, 4294967295), minAspect);
    expect(fairAspect(4294967295, 1), maxAspect);
    expect(fairAspect(0, 720), 16 / 9);
    expect(fairAspect(1280, 0), 16 / 9);
    expect(fairAspect(-1280, -720), 16 / 9);
    expect(fairAspect(double.nan, 1), 16 / 9);
    expect(fairAspect(double.infinity, 1), 16 / 9);
    expect(fairAspect(1, double.infinity), 16 / 9);
    expect(pipSize(double.nan).isFinite, isTrue);
    expect(pipSize(double.infinity).height, greaterThan(0));
  });

  test('a length is kept between none and a hundred hours', () {
    expect(fairLength(-1), 0);
    expect(fairLength(42000), 42000);
    expect(fairLength(9223372036854775807), maxShownMs);
  });

  test('a video says its shape and length within those bounds', () {
    final video = UiVideo(
      thumb: _pixel,
      width: 1,
      height: 4294967295,
      durationMs: 9223372036854775807,
    );
    expect(video.aspect, minAspect);
    expect(video.lengthMs, maxShownMs);
    expect(videoLength(-90000), '0:00');
    expect(videoLength(9223372036854775807), '100:00:00');
  });

  setUpAll(loadFonts);

  for (final (width, height) in _hostile) {
    testWidgets(
      'a video in the room chat that says it is $width by $height has a '
      'card of a shape that can be drawn',
      (tester) async {
        tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
        tester.view.physicalSize = const Size(1440, 900);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        final state = fakeRoom(people: 2);
        final room = state.roomState.room!;
        state.roomState.room = UiRoom(
          startedMs: room.startedMs,
          joined: true,
          participants: room.participants,
          chat: const [],
          files: [
            UiFile(
              sharedMs: 0,
              id: 'v',
              name: 'clip.mp4',
              size: 1 << 20,
              sharedBy: other(0),
              holders: [other(0)],
              mine: false,
              sent: 0,
              video: UiVideo(
                thumb: _pixel,
                width: width,
                height: height,
                durationMs: -1,
              ),
            ),
          ],
        );
        state.roomState.remember('g', state.roomState.room);
        await tester.pumpWidget(
          AppScope(
            state: state,
            child: MaterialApp(
              theme: Fg.darkTokens.themeData,
              home: const Material(child: Shell()),
            ),
          ),
        );
        for (var i = 0; i < 4; i++) {
          await tester.pump(const Duration(milliseconds: 200));
        }
        final size = tester.getSize(find.byKey(const ValueKey('video:v')));
        expect(
          size.width / size.height,
          inInclusiveRange(minAspect - .01, maxAspect + .01),
        );
        expect(size.height, lessThanOrEqualTo(320 / minAspect));
        // How long it lasts, said below none.
        expect(find.text('0:00'), findsOneWidget);
        await tester.pumpWidget(const SizedBox());
        await tester.pump(const Duration(seconds: 10));
      },
    );
  }
}
