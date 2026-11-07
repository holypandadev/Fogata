import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/rust/frb_generated.dart';

import 'fake_room.dart';

/// A core that stops, but whose data folder stays held.
class FakeCore implements RustLibApi {
  @override
  Future<void> crateApiAppStopCore({required String dataDir}) async =>
      throw AnyhowException(
        'the data folder /data is still in use 10s after Fogata stopped: '
        'try again in a moment',
      );

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

void main() {
  setUpAll(() => RustLib.initMock(api: FakeCore()));

  test('stopping for a move says when the data folder is still held', () async {
    final state = fakeRoom(people: 1)..started = true;
    await state.stopForMove();
    expect(state.offlineForMove, isTrue);
    expect(
      state.toasts.list.map((t) => (t.text, t.error)),
      contains((
        'The data folder /data is still in use 10s after Fogata stopped: '
            'try again in a moment.',
        true,
      )),
    );
    state.toasts.dispose();
  });
}
