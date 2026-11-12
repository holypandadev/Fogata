import 'dart:ui';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/window.dart';

void main() {
  test('on a large screen, the window opens at 1440 by 900 and may shrink to '
      '960 by 640', () {
    final sizes = mainWindowSizes(
      screen: const Size(1920, 1080),
      work: const Size(1920, 1040),
    );
    expect(sizes.minimum, const Size(960, 640));
    expect(sizes.opening, const Size(1440, 900));
  });

  test('a 1440 by 900 Mac opens as before', () {
    final sizes = mainWindowSizes(
      screen: const Size(1440, 900),
      work: const Size(1440, 875),
    );
    expect(sizes.minimum, const Size(960, 640));
    expect(sizes.opening, const Size(1360, 820));
  });

  test('the window is never larger than the work area, nor must it be', () {
    for (final (screen, work) in const [
      // 1920 by 1080 at 175 %, less the taskbar.
      (Size(1097, 617), Size(1097, 590)),
      // 1280 by 800 at 125 %, less the taskbar.
      (Size(1024, 640), Size(1024, 592)),
      // A Mac's "Larger Text" 1024 by 640, less the menu bar and the Dock.
      (Size(1024, 640), Size(1024, 571)),
      // Narrower than 960.
      (Size(900, 700), Size(900, 660)),
    ]) {
      final sizes = mainWindowSizes(screen: screen, work: work);
      expect(sizes.minimum.width, lessThanOrEqualTo(work.width));
      expect(sizes.minimum.height, lessThanOrEqualTo(work.height));
      expect(sizes.opening.width, lessThanOrEqualTo(work.width));
      expect(sizes.opening.height, lessThanOrEqualTo(work.height));
      expect(sizes.opening.width, greaterThanOrEqualTo(sizes.minimum.width));
      expect(sizes.opening.height, greaterThanOrEqualTo(sizes.minimum.height));
    }
  });

  group('where the main window opens', () {
    // A 2560 by 1440 screen less the menu bar and the Dock.
    const area = Rect.fromLTWH(0, 25, 2560, 1345);
    const size = Size(1440, 900);

    test('around a splash in the middle of the screen, as before', () {
      final bounds = mainWindowBounds(
        center: const Offset(1280, 700),
        size: size,
        area: area,
      );
      expect(
        bounds,
        Rect.fromCenter(
          center: const Offset(1280, 700),
          width: 1440,
          height: 900,
        ),
      );
    });

    test('whole within the work area when the splash is near an edge', () {
      // A splash the system put at the bottom left: the window around it had its left
      // edge off the screen and its bottom under the Dock.
      final bounds = mainWindowBounds(
        center: const Offset(680, 1064),
        size: size,
        area: area,
      );
      expect(bounds.size, size);
      expect(bounds.left, 0);
      expect(bounds.bottom, area.bottom);
      for (final center in const [
        Offset(0, 0),
        Offset(2560, 0),
        Offset(2560, 1440),
        Offset(-500, 3000),
      ]) {
        final bounds = mainWindowBounds(center: center, size: size, area: area);
        expect(bounds.size, size);
        expect(area.contains(bounds.topLeft), isTrue, reason: '$center');
        expect(bounds.right, lessThanOrEqualTo(area.right), reason: '$center');
        expect(
          bounds.bottom,
          lessThanOrEqualTo(area.bottom),
          reason: '$center',
        );
      }
    });

    test('on a second screen, whose work area does not start at zero', () {
      const second = Rect.fromLTWH(-1920, 200, 1920, 1040);
      final bounds = mainWindowBounds(
        center: const Offset(-100, 300),
        size: size,
        area: second,
      );
      expect(bounds, const Rect.fromLTWH(-1440, 200, 1440, 900));
    });

    test('from the top left of a work area smaller than the window', () {
      final bounds = mainWindowBounds(
        center: const Offset(400, 300),
        size: size,
        area: const Rect.fromLTWH(0, 25, 1024, 571),
      );
      expect(bounds, const Rect.fromLTWH(0, 25, 1440, 900));
    });
  });
}
