import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/host.dart';

/// A shared screen's capture, with a sound track or without.
class _Screen implements MediaStream {
  _Screen({required this.sound});
  final bool sound;

  @override
  List<MediaStreamTrack> getAudioTracks() => [if (sound) _Track()];

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Track implements MediaStreamTrack {
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

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

  // Linux gives a shared window no sound (every other application's would go with it),
  // nor a screen without PipeWire (the call's own sound would): the button would offer
  // what is not there.
  for (final system in [HostSystem.linux, HostSystem.windows]) {
    test(
      'on ${system.name} the screen\'s sound is offered only when the capture '
      'brought it',
      () {
        pinnedHost = system;
        addTearDown(() => pinnedHost = HostSystem.linux);
        final without = Call(group: 'g', me: 'me', mic: false)
          ..holdForTests(streams: [null, null, _Screen(sound: false)]);
        expect(without.canShareSound, isFalse);
        final withSound = Call(group: 'g', me: 'me', mic: false)
          ..holdForTests(streams: [null, null, _Screen(sound: true)]);
        expect(withSound.canShareSound, isTrue);
      },
    );
  }

  // Linux without PipeWire gives a screen no sound: the share says why, and only then.
  test('a screen shared without sound for want of PipeWire is told apart', () {
    bool needs({
      bool linux = true,
      bool sound = false,
      bool window = false,
      String? server = 'none',
    }) => Call.needsPipeWire(
      linux: linux,
      sound: sound,
      window: window,
      server: server,
    );
    expect(needs(), isTrue);
    // A server that takes the connection, a library too old to say, a capture with
    // sound, a window (no sound whatever the server) or another system: no reason to give.
    expect(needs(server: 'pipewire'), isFalse);
    expect(needs(server: null), isFalse);
    expect(needs(sound: true), isFalse);
    expect(needs(window: true), isFalse);
    expect(needs(linux: false), isFalse);
  });

  test('macOS takes the screen\'s sound itself, whatever the capture', () {
    pinnedHost = HostSystem.macos;
    addTearDown(() => pinnedHost = HostSystem.linux);
    final call = Call(group: 'g', me: 'me', mic: false)
      ..holdForTests(streams: [null, null, _Screen(sound: false)]);
    expect(call.canShareSound, isTrue);
  });
}
