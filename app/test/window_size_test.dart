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
}
