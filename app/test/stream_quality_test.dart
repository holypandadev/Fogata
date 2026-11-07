import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/photo.dart';

void main() {
  test('the quality falls with the delay and the losses', () {
    expect(StreamQuality.levelOf(null, null), 2);
    expect(StreamQuality.levelOf(40, 0.5), 2);
    expect(StreamQuality.levelOf(200, 0), 1);
    expect(StreamQuality.levelOf(60, 3), 1);
    expect(StreamQuality.levelOf(450, 0), 0);
    expect(StreamQuality.levelOf(60, 9), 0);
  });

  test('pictures are told by their names', () {
    expect(isPicture('lake.JPG'), true);
    expect(isPicture('screenshot.png'), true);
    expect(isPicture('plan.pdf'), false);
    expect(isPicture('png'), false);
  });
}
