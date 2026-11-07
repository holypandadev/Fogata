// The chat shown is read again on many events; its friend's banner only when it may have
// changed: when the chat is opened, and after the friends changed.

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';

class _Core implements RustLibApi {
  final banners = <String>[];

  @override
  Future<UiBanner?> crateApiAppFriendBanner({required String deviceId}) async {
    banners.add(deviceId);
    return null;
  }

  @override
  Future<List<UiDirectChat>> crateApiDirectDirectChats() async => const [];

  @override
  Future<UiDirectPage> crateApiDirectDirectChat({
    required String deviceId,
    required int limit,
  }) async => throw StateError('no page here');

  @override
  Future<void> crateApiDirectMarkDirectRead({required String deviceId}) async {}

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final core = _Core();
  setUpAll(() => RustLib.initMock(api: core));

  test('the banner is read on opening the chat and after the friends change, '
      'not on every refresh', () async {
    final state = AppState('/nonexistent/fogata-friend-banner-reads-test');
    await state.direct.show('ana');
    expect(core.banners, ['ana']);
    // Messages come and go: the chat is read again, the banner not.
    for (var i = 0; i < 5; i++) {
      await state.direct.refresh();
    }
    expect(core.banners, ['ana']);
    // The friends changed, the banner among what may have: read once more.
    state.direct.friendsChanged();
    await state.direct.refresh();
    await state.direct.refresh();
    expect(core.banners, ['ana', 'ana']);
    // Opened again: read again.
    await state.direct.show('ana');
    expect(core.banners, ['ana', 'ana', 'ana']);
    state.dispose();
  });
}
