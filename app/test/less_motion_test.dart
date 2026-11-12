import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/motion.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/typing.dart';
import 'package:fogata/src/widgets.dart';

/// What moves for as long as it shows, and a spring, in a window whose system asks for
/// less motion or not.
Widget _window({required bool less, Widget? child}) => MaterialApp(
  theme: Fg.lightTokens.themeData,
  builder: (context, child) => MediaQuery(
    data: MediaQuery.of(context).copyWith(disableAnimations: less),
    child: FollowMotion(child: child!),
  ),
  home:
      child ??
      const Material(
        child: Column(
          children: [
            Rings(color: Colors.green, size: 80, child: SizedBox()),
            SpeakingBars(),
            TypingDots(),
            Skeleton(width: 40, height: 10),
          ],
        ),
      ),
);

void main() {
  tearDown(() => lessMotionForTests = false);

  testWidgets('without less motion asked for, the springs bounce and the loops '
      'move as before', (tester) async {
    await tester.pumpWidget(_window(less: false));
    expect(
      spring.transform(.3),
      Curves.easeOutBack.transform(.3),
      reason: 'the same curve, a little past the target',
    );
    expect(tester.hasRunningAnimations, isTrue);
    expect(find.byType(SlowPulse), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
    'with less motion asked for, the springs land at once and nothing '
    'moves for ever',
    (tester) async {
      await tester.pumpWidget(_window(less: true));
      for (final t in [.01, .3, .9]) {
        expect(spring.transform(t), 1.0);
      }
      expect(spring.transform(0), 0.0);
      expect(tester.hasRunningAnimations, isFalse);
      // The talking bars hold still: no clock steps them.
      expect(find.byType(SlowPulse), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('a change of the setting takes effect while shown', (
    tester,
  ) async {
    await tester.pumpWidget(_window(less: false));
    expect(tester.hasRunningAnimations, isTrue);
    await tester.pumpWidget(_window(less: true));
    await tester.pump();
    expect(tester.hasRunningAnimations, isFalse);
    expect(spring.transform(.3), 1.0);
    await tester.pumpWidget(_window(less: false));
    await tester.pump();
    expect(tester.hasRunningAnimations, isTrue);
    expect(spring.transform(.3), Curves.easeOutBack.transform(.3));
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('a scale with a spring is at its end from the first frame', (
    tester,
  ) async {
    Widget scaled(double scale) => _window(
      less: true,
      child: Center(
        child: AnimatedScale(
          scale: scale,
          duration: springTime,
          curve: spring,
          child: const SizedBox(width: 10, height: 10, key: Key('box')),
        ),
      ),
    );
    await tester.pumpWidget(scaled(1));
    await tester.pumpWidget(scaled(2));
    await tester.pump(const Duration(milliseconds: 16));
    final transition = tester.widget<ScaleTransition>(
      find.ancestor(
        of: find.byKey(const Key('box')),
        matching: find.byType(ScaleTransition),
      ),
    );
    expect(transition.scale.value, 2.0);
    await tester.pumpAndSettle();
  });
}
