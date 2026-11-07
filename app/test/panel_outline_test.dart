// The floating panels' outline is drawn over what is inside them: a column or a banner
// that reaches the edge (the settings' list of parts on the left, a panel's banner on top)
// would otherwise be painted over the outline at the rounded corners, and cut it there.

import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/settings.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

/// The boxes that clip what is inside them to rounded corners and still draw their outline
/// under it.
List<String> _outlinesUnder(WidgetTester tester) => [
  for (final box in tester.widgetList<Container>(find.byType(Container)))
    if (box.clipBehavior != Clip.none &&
        box.decoration is BoxDecoration &&
        (box.decoration! as BoxDecoration).border != null &&
        (box.decoration! as BoxDecoration).borderRadius != null)
      '${box.key ?? box.child.runtimeType}',
];

void main() {
  setUpAll(loadFonts);

  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  for (final dark in [true, false]) {
    testWidgets('settings, group settings and the user panel '
        '(${dark ? 'dark' : 'light'})', (tester) async {
      tester.view.physicalSize = const Size(1440, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final state = fakeRoom(people: 2)..callState.call = null;
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: (dark ? Fg.darkTokens : Fg.lightTokens).themeData,
            home: const Material(child: Shell()),
          ),
        ),
      );
      Future<void> settle() async {
        for (var i = 0; i < 4; i++) {
          await tester.pump(const Duration(milliseconds: 200));
        }
        // Tests have no devices to list, nor a core to ask.
        tester.takeException();
      }

      await settle();
      expect(_outlinesUnder(tester), isEmpty, reason: 'the window');
      state.openSettings();
      await settle();
      expect(find.byType(Settings), findsOneWidget);
      expect(_outlinesUnder(tester), isEmpty, reason: 'settings');
      await state.closeSettings();
      await settle();
      state.openGroupSettings('g');
      await settle();
      expect(_outlinesUnder(tester), isEmpty, reason: 'group settings');
      state.closeGroupSettings();
      await settle();
      state.openUserPanel(const Rect.fromLTWH(20, 800, 40, 40));
      await settle();
      expect(_outlinesUnder(tester), isEmpty, reason: 'user panel');
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    });
  }

  testWidgets(
    'the settings\' outline shows at the rounded corners, and lists scroll '
    'inside it',
    (tester) async {
      tester.view.physicalSize = const Size(1440, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final state = fakeRoom(people: 2)..callState.call = null;
      const shot = GlobalObjectKey('shot');
      await tester.pumpWidget(
        RepaintBoundary(
          key: shot,
          child: AppScope(
            state: state,
            child: MaterialApp(
              theme: Fg.darkTokens.themeData,
              home: const Material(child: Shell()),
            ),
          ),
        ),
      );
      state.openSettings();
      for (var i = 0; i < 5; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
      tester.takeException();
      // The box that holds the settings, its outline drawn over them.
      final frame = tester.getRect(
        find
            .ancestor(
              of: find.byType(Settings),
              matching: find.byWidgetPredicate(
                (w) => w is Container && w.foregroundDecoration != null,
              ),
            )
            .first,
      );
      // Every list scrolls, and shows its scrollbar, inside the outline.
      final inside = frame.deflate(1);
      final lists = find.descendant(
        of: find.byType(Settings),
        // The lists, not a text field's own line.
        matching: find.byWidgetPredicate(
          (w) => w is Scrollable && w.axisDirection == AxisDirection.down,
        ),
      );
      expect(lists, findsWidgets);
      for (final list in lists.evaluate()) {
        // One inside a list (a text field's lines) scrolls with it, clipped by it.
        final nested = find.ancestor(
          of: find.byWidget(list.widget),
          matching: find.byType(Scrollable),
        );
        if (nested.evaluate().any(
          (e) => find
              .descendant(
                of: find.byType(Settings),
                matching: find.byWidget(e.widget),
              )
              .evaluate()
              .isNotEmpty,
        )) {
          continue;
        }
        final box = tester.getRect(find.byWidget(list.widget));
        expect(
          inside.expandToInclude(box) == inside,
          isTrue,
          reason: '$box is not inside $inside',
        );
      }
      final boundary =
          tester.renderObject(find.byKey(shot)) as RenderRepaintBoundary;
      final bytes = await tester.runAsync(() async {
        final image = await boundary.toImage();
        final data = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
        image.dispose();
        return data!;
      });
      Color at(Offset p) {
        final i = (p.dy.floor() * 1440 + p.dx.floor()) * 4;
        return Color.fromARGB(
          bytes!.getUint8(i + 3),
          bytes.getUint8(i),
          bytes.getUint8(i + 1),
          bytes.getUint8(i + 2),
        );
      }

      double apart(Color a, Color b) =>
          (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
      const radius = 22.0;
      // On the outline halfway round each left corner, and a little inside it, where the
      // column of parts is.
      for (final (corner, down) in [
        (frame.topLeft, 1.0),
        (frame.bottomLeft, -1.0),
      ]) {
        final centre = corner + Offset(radius, down * radius);
        Offset along(double r) =>
            centre + Offset(-r / math.sqrt2, -down * r / math.sqrt2);
        final line = at(along(radius - .5));
        final column = at(along(radius - 4));
        expect(
          apart(line, column),
          greaterThan(.02),
          reason: 'the outline at $corner is the column\'s colour: $line',
        );
      }
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    },
  );
}
