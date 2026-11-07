import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/room.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

/// The window sizes checked: the smallest Fogata allows up to a large screen.
const sizes = [
  Size(960, 640),
  Size(1280, 800),
  Size(1440, 900),
  Size(1920, 1080),
  Size(2560, 1440),
];

Future<void> showRoom(WidgetTester tester, AppState state, Size size) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 600));
}

Rect rectOf(WidgetTester tester, Element element) {
  final box = element.renderObject! as RenderBox;
  return box.localToGlobal(Offset.zero) & box.size;
}

Finder byTypeName(String name) =>
    find.byWidgetPredicate((w) => w.runtimeType.toString() == name);

/// The tiles and cards of the room, where they are drawn.
List<Rect> tiles(WidgetTester tester) => [
  for (final e in [
    ...byTypeName('_Tile').evaluate(),
    ...byTypeName('_LiveCard').evaluate(),
  ])
    rectOf(tester, e),
];

/// Each tile's picture keeps clear of its name.
void expectNamesClear(WidgetTester tester, String why) {
  for (final tile in byTypeName('_Tile').evaluate()) {
    final avatars = find.descendant(
      of: find.byWidget(tile.widget),
      matching: find.byType(Avatar),
    );
    final names = find.descendant(
      of: find.byWidget(tile.widget),
      matching: find.byType(Text),
    );
    for (final a in avatars.evaluate()) {
      for (final n in names.evaluate()) {
        final overlap = rectOf(tester, a).intersect(rectOf(tester, n));
        expect(
          overlap.width > .5 && overlap.height > .5,
          isFalse,
          reason: '$why: a picture covers a name',
        );
      }
    }
  }
}

/// Each tile without a camera has its avatar in the middle, and its name at the bottom
/// left, in a low tile (the strip under a screen, a full grid) as in a large one.
void expectAvatarsCentred(WidgetTester tester, String why) {
  for (final tile in byTypeName('_Tile').evaluate()) {
    final of = find.byWidget(tile.widget);
    final avatar = find.descendant(of: of, matching: find.byType(Avatar));
    if (avatar.evaluate().isEmpty) continue;
    final box = rectOf(tester, tile);
    final face = rectOf(tester, avatar.evaluate().single);
    expect(face.center.dx, closeTo(box.center.dx, .5), reason: why);
    expect(face.center.dy, closeTo(box.center.dy, .5), reason: why);
    // The name's pill, around the first words in the tile.
    final words = find.descendant(of: of, matching: find.byType(Text)).first;
    final name = rectOf(
      tester,
      find
          .ancestor(of: words, matching: find.byType(Container))
          .evaluate()
          .first,
    );
    expect(name.left, closeTo(box.left + 10, .5), reason: why);
    expect(name.bottom, closeTo(box.bottom - 10, .5), reason: why);
    expect(name.top, greaterThanOrEqualTo(face.bottom), reason: why);
  }
}

void expectApart(List<Rect> rects, String why) {
  for (var i = 0; i < rects.length; i++) {
    for (var j = i + 1; j < rects.length; j++) {
      final overlap = rects[i].intersect(rects[j]);
      expect(
        overlap.width > .5 && overlap.height > .5,
        isFalse,
        reason: '$why: ${rects[i]} and ${rects[j]} overlap',
      );
    }
  }
}

void main() {
  setUpAll(loadFonts);

  setUp(() {
    // The window's buttons ask the window manager, which tests do not have.
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  test('tiles fill the room and keep a usable size, or scroll', () {
    for (final area in const [
      Size(276, 500),
      Size(600, 420),
      Size(916, 500),
      Size(1876, 1300),
    ]) {
      for (var n = 1; n <= 16; n++) {
        final fit = gridFit(n, area);
        final rows = (n / fit.columns).ceil();
        final why = '$n tiles in $area';
        expect(
          fit.tile.width,
          greaterThanOrEqualTo(minTile.width),
          reason: why,
        );
        expect(
          fit.tile.height,
          greaterThanOrEqualTo(minTile.height),
          reason: why,
        );
        expect(
          fit.columns * fit.tile.width + (fit.columns - 1) * tileGap,
          lessThanOrEqualTo(area.width + .5),
          reason: why,
        );
        if (!fit.scrolls) {
          expect(
            rows * fit.tile.height + (rows - 1) * tileGap,
            lessThanOrEqualTo(area.height + .5),
            reason: why,
          );
        }
      }
    }
  });

  for (final people in [2, 3, 4, 6, 8, 9, 12]) {
    testWidgets('$people people fit at every window size', (tester) async {
      for (final size in sizes) {
        await showRoom(tester, fakeRoom(people: people), size);
        expect(tester.takeException(), isNull, reason: '$size');
        final room = rectOf(tester, byTypeName('_Tiles').evaluate().single);
        final drawn = tiles(tester);
        expect(drawn, hasLength(people), reason: '$size');
        expectApart(drawn, '$people people at $size');
        expectNamesClear(tester, '$people people at $size');
        expectAvatarsCentred(tester, '$people people at $size');
        final scrolls = find
            .descendant(
              of: byTypeName('_Tiles'),
              matching: find.byType(Scrollable),
            )
            .evaluate()
            .isNotEmpty;
        for (final tile in drawn) {
          expect(tile.width, greaterThanOrEqualTo(minTile.width - .5));
          expect(tile.height, greaterThanOrEqualTo(minTile.height - .5));
          expect(tile.left, greaterThanOrEqualTo(room.left - .5));
          expect(tile.right, lessThanOrEqualTo(room.right + .5));
          if (!scrolls) {
            expect(tile.top, greaterThanOrEqualTo(room.top - .5));
            expect(tile.bottom, lessThanOrEqualTo(room.bottom + .5));
          }
        }
      }
      await tester.pumpWidget(const SizedBox());
    });
  }

  testWidgets('the own screen’s tile opens with the keyboard too', (
    tester,
  ) async {
    final state = fakeRoom(people: 3, own: true);
    await showRoom(tester, state, const Size(1440, 900));
    final tile = find.byWidgetPredicate(
      (w) => w is Pressable && w.label == 'Your screen',
    );
    expect(tile, findsOneWidget);
    Focus.of(
      tester.element(
        find.descendant(of: tile, matching: find.byType(Semantics)).first,
      ),
    ).requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(state.callState.call!.ownScreen, isTrue);
    state.callState.call = null;
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('every screen shared at once fits as cards', (tester) async {
    for (final size in sizes) {
      await showRoom(tester, fakeRoom(people: 8, sharing: 7, own: true), size);
      expect(tester.takeException(), isNull, reason: '$size');
      final drawn = tiles(tester);
      // Eight people, seven cards to watch and this device's own screen.
      expect(drawn, hasLength(16), reason: '$size');
      expectApart(drawn, 'shared screens at $size');
      for (final tile in drawn) {
        expect(tile.width, greaterThanOrEqualTo(minTile.width - .5));
        expect(tile.height, greaterThanOrEqualTo(minTile.height - .5));
      }
    }
    await tester.pumpWidget(const SizedBox());
  });

  for (final (name, state) in [
    (
      'a watched screen',
      () => fakeRoom(people: 8, sharing: 7, own: true, watching: true),
    ),
    (
      'a watched screen among few',
      () => fakeRoom(people: 3, sharing: 1, watching: true),
    ),
    (
      'this device’s own screen',
      () => fakeRoom(people: 8, own: true, ownShown: true),
    ),
  ]) {
    testWidgets('$name sits right above the others', (tester) async {
      for (final size in sizes) {
        await showRoom(tester, state(), size);
        expect(tester.takeException(), isNull, reason: '$size');
        final room = rectOf(tester, byTypeName('_Tiles').evaluate().single);
        final stream = rectOf(tester, byTypeName('_Stream').evaluate().single);
        final strip = rectOf(tester, byTypeName('_Strip').evaluate().single);
        // No empty band between the picture and the people.
        expect(
          strip.top - stream.bottom,
          closeTo(tileGap, .5),
          reason: '$size',
        );
        for (final r in [stream, strip]) {
          expect(r.left, greaterThanOrEqualTo(room.left - .5));
          expect(r.right, lessThanOrEqualTo(room.right + .5));
          expect(r.top, greaterThanOrEqualTo(room.top - .5));
          expect(r.bottom, lessThanOrEqualTo(room.bottom + .5));
        }
        expect(stream.width, greaterThan(200), reason: '$size');
        final drawn = [
          for (final tile in tiles(tester))
            if (tile.overlaps(strip)) tile,
        ];
        expectApart(drawn, '$name at $size');
        expectNamesClear(tester, '$name at $size');
        expectAvatarsCentred(tester, '$name at $size');
        for (final tile in drawn) {
          expect(tile.width, greaterThanOrEqualTo(135.5), reason: '$size');
        }
        // Tiles past the strip's end are reached with its arrow.
        final cut = drawn.any((t) => t.right > strip.right + .5);
        expect(
          find.byTooltip('More people').evaluate().isNotEmpty,
          cut,
          reason: '$size',
        );
      }
      await tester.pumpWidget(const SizedBox());
    });
  }

  testWidgets('cards of shared screens stay whole, chat shown or not', (
    tester,
  ) async {
    for (final shown in [true, false]) {
      for (final size in [...sizes, const Size(1100, 700)]) {
        for (final sharing in [1, 2, 3]) {
          final why = '$size, $sharing shared, chat $shown';
          final state = fakeRoom(people: 4, sharing: sharing, own: true)
            ..prefs.roomChat = shown;
          await showRoom(tester, state, size);
          expect(tester.takeException(), isNull, reason: why);
          expect(byTypeName('_LiveCard'), findsNWidgets(sharing), reason: why);
        }
      }
    }
    await tester.pumpWidget(const SizedBox());
  });
}
