import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';

void main() {
  test('a shared screen is heard only while this device watches it', () {
    final call = Call(group: 'g', me: 'me', screenGainOf: (id) => 0.5)
      ..peers['ana'] = CallPeer('ana')
      ..peers['leo'] = CallPeer('leo');
    expect(call.screenSoundGain('ana'), 0);
    expect(call.screenSoundGain('leo'), 0);
    call.watching = 'ana';
    expect(call.screenSoundGain('ana'), 0.5);
    expect(call.screenSoundGain('leo'), 0);
    call.watch('leo');
    expect(call.screenSoundGain('ana'), 0);
    expect(call.screenSoundGain('leo'), 0.5);
    call.watch(null);
    expect(call.screenSoundGain('leo'), 0);
  });
}
