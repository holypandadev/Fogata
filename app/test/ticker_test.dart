import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/screens/home.dart' show Ticker;

void main() {
  testWidgets('a ticker builds again every second on its own', (tester) async {
    var builds = 0;
    await tester.pumpWidget(
      Ticker(
        builder: (context) {
          builds++;
          return const SizedBox();
        },
      ),
    );
    expect(builds, 1);
    await tester.pump(const Duration(seconds: 1));
    expect(builds, 2);
    await tester.pump(const Duration(seconds: 3));
    expect(builds, greaterThanOrEqualTo(3));
  });
}
