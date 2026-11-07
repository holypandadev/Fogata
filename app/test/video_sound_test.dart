import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/video_sound.dart';

import 'fake_room.dart';

void main() {
  test('a chat video goes quiet while deafened and comes back after', () async {
    final state = fakeRoom(people: 2);
    final volumes = <double>[];
    var changes = 0;
    final sound = VideoSound(state, () => changes++);
    await sound.attachVolume((v) async => volumes.add(v));
    expect(sound.muted, isFalse);
    expect(volumes, isEmpty);

    void deafen(bool on) {
      state.callState.deafened = on;
      state.notifyListeners();
    }

    // In the middle of the video.
    deafen(true);
    await pumpEventQueue();
    expect(sound.muted, isTrue);
    expect(volumes, [0]);
    deafen(false);
    await pumpEventQueue();
    expect(sound.muted, isFalse);
    expect(volumes, [0, 1]);

    // Muted by hand, it stays muted when the person hears again.
    await sound.toggle();
    deafen(true);
    deafen(false);
    await pumpEventQueue();
    expect(sound.muted, isTrue);
    expect(volumes, [0, 1, 0]);
    expect(changes, 3);
    sound.dispose();
    deafen(true);
    expect(volumes, [0, 1, 0]);
  });

  test('a video opened while deafened starts quiet', () async {
    final state = fakeRoom(people: 2)..callState.deafened = true;
    final volumes = <double>[];
    final sound = VideoSound(state, () {});
    await sound.attachVolume((v) async => volumes.add(v));
    expect(sound.muted, isTrue);
    expect(volumes, [0]);
    // Its own Sound on plays it, deafened or not.
    await sound.toggle();
    expect(volumes, [0, 1]);
    sound.dispose();
  });
}
