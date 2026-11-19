import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/host.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/types.dart';

import 'fake_room.dart';
import 'fake_time.dart';

/// A connection that only knows whether it was closed.
class _Connection implements RTCPeerConnection {
  bool closed = false;

  @override
  Future<void> close() async => closed = true;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

const me = '0000000000000000';
const ana = 'a000000000000000';

MediaDeviceInfo input(String id, String label) =>
    MediaDeviceInfo(kind: 'audioinput', label: label, deviceId: id);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('someone in the room without a call', () {
    // What the call would send goes nowhere.
    setUp(() => signalsForTests = (to, message) {});
    tearDown(() => signalsForTests = null);

    Call call() => Call(group: 'g', me: me)
      ..bridgeForTests = const UiMediaBridge(
        host: '127.0.0.1',
        port: 3478,
        username: 'u',
        password: 'p',
      );

    test('is no longer connected to, and shows without sound', () async {
      final c = call();
      final connection = _Connection();
      c.peers[ana] = CallPeer(ana)..connection = connection;
      await c.onSignal(ana, jsonEncode({'t': Call.noVoice}));
      final peer = c.peers[ana]!;
      expect(peer.voiceless, isTrue);
      expect(peer.connection, isNull);
      expect(connection.closed, isTrue);
    });

    test('counts as in the call again once they send anything else', () async {
      final c = call();
      c.peers[ana] = CallPeer(ana);
      await c.onSignal(ana, jsonEncode({'t': Call.noVoice}));
      await c.onSignal(
        ana,
        jsonEncode({
          't': 'state',
          'mic': true,
          'cam': false,
          'screen': false,
          'sound': false,
          'watch': '',
        }),
      );
      expect(c.peers[ana]!.voiceless, isFalse);
      expect(c.peers[ana]!.mic, isTrue);
    });
  });

  test(
    'coming and going without a call leaves one round of tries',
    () => inCallTime(() async {
      // This device has the larger id here, so its tries are "again" signals, not offers.
      const late = 'f000000000000000';
      final sent = <String>[];
      signalsForTests = (to, message) => sent.add('${message['t']}');
      addTearDown(() => signalsForTests = null);
      final c = Call(group: 'g', me: late)
        ..bridgeForTests = const UiMediaBridge(
          host: '127.0.0.1',
          port: 3478,
          username: 'u',
          password: 'p',
        );
      c.peers[ana] = CallPeer(ana);
      final state = jsonEncode({
        't': 'state',
        'mic': true,
        'cam': false,
        'screen': false,
        'sound': false,
        'watch': '',
      });
      for (var i = 0; i < 3; i++) {
        await c.onSignal(ana, jsonEncode({'t': Call.noVoice}));
        await c.onSignal(ana, state);
      }
      expect(c.peers[ana]!.watchRounds, 3);
      sent.clear();
      // Only the last round tries, once, 5 s on.
      await Future<void>.delayed(const Duration(milliseconds: 5600));
      expect(sent.where((t) => t == 'again'), hasLength(1));
      await c.stop();
    }),
  );

  test('a refused call tells everyone else in the room, once', () async {
    final state = fakeRoom(people: 3)..callState.call = null;
    final sent = <(String, String, String)>[];
    state.sendSignal = (group, device, payload) =>
        sent.add((group, device, payload));
    state.soundServer = () async => false;
    await state.startCallForTests('g');
    expect(state.callState.call, isNull);
    expect(sent, [
      ('g', 'p0', jsonEncode({'t': Call.noVoice})),
      ('g', 'p1', jsonEncode({'t': Call.noVoice})),
    ]);
  });

  group('the microphone in use', () {
    final chosen = input('headset', 'USB Headset');
    final builtIn = input('default', 'default: Built-in Microphone');

    test('going away and coming back is told once each way', () {
      final watch = MicWatch();
      expect(watch.take('headset', [builtIn, chosen]), isNull);
      expect(
        watch.take('headset', [builtIn]),
        l10n.callMicGone('Built-in Microphone'),
      );
      // Other changes while it is gone say nothing more.
      expect(
        watch.take('headset', [builtIn, input('x', 'Webcam Mic')]),
        isNull,
      );
      expect(watch.take('headset', [builtIn, chosen]), l10n.callMicBack);
      expect(watch.take('headset', [builtIn, chosen]), isNull);
    });

    test('another microphone chosen is not told as the last one back', () {
      final watch = MicWatch();
      expect(watch.take('headset', [builtIn]), isNotNull);
      // The person picks the built-in one: it is there, which is no coming back.
      expect(watch.take('default', [builtIn]), isNull);
      expect(watch.gone, isFalse);
    });

    test('a microphone missing when the call starts is told', () async {
      final c = Call(group: 'g', me: me, input: 'headset');
      final told = <String>[];
      c.onNotice = told.add;
      c.takeDevices([builtIn]);
      expect(told, [l10n.callMicGone('Built-in Microphone')]);
    });

    test('changes told while the list is read are read again', () async {
      final c = Call(group: 'g', me: me, input: 'headset');
      final told = <String>[];
      c.onNotice = told.add;
      final lists = [
        Completer<List<MediaDeviceInfo>>(),
        Completer<List<MediaDeviceInfo>>(),
      ];
      var read = 0;
      c.listDevices = () => lists[read++].future;
      c.devicesChangedForTests();
      // A burst is read once, after it settles.
      c.devicesChangedForTests();
      await Future<void>.delayed(const Duration(milliseconds: 400));
      expect(read, 1);
      // Another change while that list is read: read again once it is done.
      c.devicesChangedForTests();
      await Future<void>.delayed(const Duration(milliseconds: 400));
      lists[0].complete([builtIn]);
      await Future<void>.delayed(Duration.zero);
      expect(read, 2);
      lists[1].complete([builtIn, chosen]);
      await Future<void>.delayed(Duration.zero);
      expect(told, [l10n.callMicGone('Built-in Microphone'), l10n.callMicBack]);
    });

    test('with the system default chosen there is nothing to tell', () {
      final watch = MicWatch();
      expect(watch.take(null, [builtIn]), isNull);
      expect(watch.take(null, const []), isNull);
    });

    test('the default is named as the system lists it, or as the default', () {
      expect(
        MicWatch.defaultInputName([
          input('d', 'Default - Microphone (USB Audio)'),
        ]),
        'Microphone (USB Audio)',
      );
      expect(MicWatch.defaultInputName(const []), l10n.devicesSystemDefault);
      final watch = MicWatch();
      expect(
        watch.take('headset', const []),
        l10n.callMicGone(l10n.devicesSystemDefault),
      );
    });
  });

  group('devices the voice engine does not use (macOS)', () {
    // As the engine names devices: by their unique ids, the system's ones too.
    const monitor = 'monitor-uid',
        speakers = 'speakers-uid',
        headset = 'headset-uid';
    const names = {
      monitor: 'Monitor',
      speakers: 'Mac mini Speakers',
      headset: 'USB Headset',
    };
    Map<String, Object?> facts({
      String? chosen = monitor,
      String inUse = monitor,
      bool present = true,
      bool changing = false,
      String? reason,
      String? inputChosen,
      String inputInUse = headset,
      bool inputPresent = true,
    }) => {
      'chosen': ?chosen,
      'chosenName': chosen == null ? null : names[chosen],
      'chosenPresent': present,
      'inUse': inUse,
      'inUseName': names[inUse],
      'reason': ?reason,
      'inputChosen': ?inputChosen,
      'inputChosenName': inputChosen == null ? null : 'Webcam Mic',
      'inputChosenPresent': inputPresent,
      'inputInUse': inputInUse,
      'inputInUseName': names[inputInUse] ?? 'Webcam Mic',
      'changing': changing,
    };
    DeviceTold none = (text: null, inUse: null);

    test(
      'another output in use, seen twice, is told once and shown beside',
      () {
        final watch = OutputWatch();
        expect(watch.take(facts(), const [], asked: monitor), none);
        final other = facts(inUse: speakers);
        // Once: a moment between (a device given back) says nothing.
        expect(watch.take(other, const [], asked: monitor), none);
        expect(watch.take(other, const [], asked: monitor), (
          text: l10n.callOutputUnusable('Monitor', 'Mac mini Speakers'),
          inUse: speakers,
        ));
        expect(watch.take(other, const [], asked: monitor), (
          text: null,
          inUse: speakers,
        ));
        // Back on the one chosen, then away again: told again.
        expect(watch.take(facts(), const [], asked: monitor), none);
        watch.take(other, const [], asked: monitor);
        expect(watch.take(other, const [], asked: monitor).text, isNotNull);
      },
    );

    test('with the rates different, it says so', () {
      final watch = OutputWatch();
      final other = facts(inUse: speakers, reason: 'rate');
      watch.take(other, const [], asked: monitor);
      expect(
        watch.take(other, const [], asked: monitor).text,
        l10n.callOutputNotWithMicrophone('Monitor', 'Mac mini Speakers'),
      );
    });

    test('the one chosen being the system\'s own, in use, says nothing', () {
      final watch = OutputWatch();
      for (var i = 0; i < 3; i++) {
        expect(watch.take(facts(), const [], asked: monitor), none);
      }
    });

    test(
      'facts about another choice, or while one is carried out, say nothing',
      () {
        final watch = OutputWatch();
        final other = facts(inUse: speakers);
        for (var i = 0; i < 2; i++) {
          expect(watch.take(other, const [], asked: speakers), none);
          expect(
            watch.take(
              facts(inUse: speakers, changing: true),
              const [],
              asked: monitor,
            ),
            none,
          );
        }
      },
    );

    test('an output unplugged is told at once each time it goes, then back says nothing', () {
      final watch = OutputWatch();
      final gone = facts(inUse: speakers, present: false);
      final told = l10n.callOutputGone('Monitor', 'Mac mini Speakers');
      expect(watch.take(gone, const [], asked: monitor).text, told);
      expect(watch.take(gone, const [], asked: monitor).text, isNull);
      // Plugged in again and given back to the engine: nothing said.
      expect(
        watch.take(facts(inUse: speakers), const [], asked: monitor).text,
        isNull,
      );
      expect(watch.take(facts(), const [], asked: monitor), none);
      expect(watch.take(gone, const [], asked: monitor).text, told);
    });

    test('nothing chosen, or no facts, says nothing', () {
      final watch = OutputWatch();
      expect(
        watch.take(facts(chosen: null, inUse: speakers), const [], asked: null),
        none,
      );
      expect(watch.take(null, const [], asked: monitor), none);
      expect(watch.take(true, const [], asked: monitor), none);
    });

    test(
      'another microphone in use, seen twice, is told once with the one in use',
      () {
        final watch = InputWatch();
        final other = facts(inputChosen: 'webcam', inputInUse: headset);
        expect(watch.take(other, const [], asked: 'webcam'), none);
        expect(watch.take(other, const [], asked: 'webcam'), (
          text: l10n.callInputUnusable('Webcam Mic', 'USB Headset'),
          inUse: headset,
        ));
        expect(watch.take(other, const [], asked: 'webcam').text, isNull);
        // Unplugged: MicWatch tells it, from the list.
        final gone = facts(
          inputChosen: 'webcam',
          inputInUse: headset,
          inputPresent: false,
        );
        final fresh = InputWatch();
        expect(fresh.take(gone, const [], asked: 'webcam').text, isNull);
        expect(fresh.take(gone, const [], asked: 'webcam').text, isNull);
        // About another choice: nothing.
        expect(InputWatch().take(other, const [], asked: null), none);
      },
    );

    test('a recording lost is recovered once, after two readings', () {
      final watch = RecordingWatch();
      final lost = {
        'recordingAsked': true,
        'recording': false,
        'inputUsable': true,
        'changing': false,
      };
      expect(watch.take(lost, mic: true, sending: true), isFalse);
      expect(watch.take(lost, mic: true, sending: true), isTrue);
      expect(watch.take(lost, mic: true, sending: true), isFalse);
      // Recording again: a later loss is recovered again.
      final back = {...lost, 'recording': true};
      expect(watch.take(back, mic: true, sending: true), isFalse);
      expect(watch.take(lost, mic: true, sending: true), isFalse);
      expect(watch.take(lost, mic: true, sending: true), isTrue);
    });

    test('a microphone chosen is judged only while something records', () {
      // Muted: the facts give no input in use; nothing is told, nothing shown beside.
      final watch = InputWatch();
      final muted = {
        'inputChosen': 'webcam',
        'inputChosenName': 'Webcam Mic',
        'inputChosenPresent': true,
        'changing': false,
      };
      for (var i = 0; i < 3; i++) {
        expect(watch.take(muted, const [], asked: 'webcam'), (
          text: null,
          inUse: null,
        ));
      }
    });

    test('the pair now refused lets a restart be tried again', () {
      // The first restart came while the engine still had the pair; the engine then
      // refused it and follows the system's devices: the next one may work.
      final watch = RecordingWatch();
      final lost = {
        'recordingAsked': false,
        'recording': false,
        'inputUsable': true,
        'changing': false,
        'combinable': true,
      };
      expect(watch.take(lost, mic: true, sending: true), isFalse);
      expect(watch.take(lost, mic: true, sending: true), isTrue);
      expect(watch.take(lost, mic: true, sending: true), isFalse);
      final refused = {...lost, 'combinable': false};
      expect(watch.take(refused, mic: true, sending: true), isTrue);
      expect(watch.take(refused, mic: true, sending: true), isFalse);
    });

    test('a loss seen as the pair is refused is recovered at once', () {
      // The engine's start with the pair failed and a new engine follows the system:
      // the first reading that shows both starts the recording again.
      final watch = RecordingWatch();
      final recording = {
        'recordingAsked': true,
        'recording': true,
        'inputUsable': true,
        'changing': false,
        'combinable': true,
      };
      expect(watch.take(recording, mic: true, sending: true), isFalse);
      final refusedAndLost = {
        ...recording,
        'recordingAsked': false,
        'recording': false,
        'combinable': false,
      };
      expect(watch.take(refusedAndLost, mic: true, sending: true), isTrue);
      expect(watch.take(refusedAndLost, mic: true, sending: true), isFalse);
    });

    test(
      'a refusal at an unmute is recovered at the first reading after it',
      () {
        // As on a device run: muted at the reading before (no voice sent then), the start
        // with the pair failed at the unmute and dropped the module's request.
        final watch = RecordingWatch();
        final muted = {
          'recordingAsked': false,
          'recording': false,
          'inputUsable': true,
          'changing': false,
          'combinable': true,
        };
        expect(watch.take(muted, mic: false, sending: false), isFalse);
        final refused = {...muted, 'combinable': false};
        expect(watch.take(refused, mic: true, sending: false), isTrue);
        expect(watch.take(refused, mic: true, sending: true), isFalse);
      },
    );

    test('a refusal while alone waits for someone the voice goes to', () {
      final watch = RecordingWatch();
      final playing = {
        'recordingAsked': false,
        'recording': false,
        'inputUsable': true,
        'changing': false,
        'combinable': true,
      };
      expect(
        watch.take(playing, mic: true, sending: false, listened: false),
        isFalse,
      );
      // Refused while the engine only plays, nobody in the call: nothing to start.
      final refused = {...playing, 'combinable': false};
      for (var i = 0; i < 3; i++) {
        expect(
          watch.take(refused, mic: true, sending: false, listened: false),
          isFalse,
        );
      }
      // Someone comes, and nothing records for them: started again at once.
      expect(
        watch.take(refused, mic: true, sending: false, listened: true),
        isTrue,
      );
    });

    test(
      'a refusal ends as something records, and is not taken while muted',
      () {
        final watch = RecordingWatch();
        final recording = {
          'recordingAsked': true,
          'recording': true,
          'inputUsable': true,
          'changing': false,
          'combinable': true,
        };
        expect(watch.take(recording, mic: true, sending: true), isFalse);
        final refused = {...recording, 'combinable': false};
        expect(watch.take(refused, mic: true, sending: true), isFalse);
        // Recording through the new engine: the refusal is over, a later loss needs two.
        final lost = {...refused, 'recording': false};
        expect(watch.take(lost, mic: true, sending: true), isFalse);
        expect(watch.take(lost, mic: true, sending: true), isTrue);
        // Refused while muted: nothing, and the unmute after it waits for two readings.
        final again = RecordingWatch();
        expect(again.take(recording, mic: true, sending: true), isFalse);
        expect(again.take(lost, mic: false, sending: true), isFalse);
        expect(again.take(lost, mic: true, sending: true), isFalse);
        expect(again.take(lost, mic: true, sending: true), isTrue);
      },
    );

    test(
      'a start that dropped the request counts as lost while voice is sent',
      () {
        final watch = RecordingWatch();
        final dropped = {
          'recordingAsked': false,
          'recording': false,
          'inputUsable': true,
          'changing': false,
        };
        expect(watch.take(dropped, mic: true, sending: false), isFalse);
        expect(watch.take(dropped, mic: true, sending: false), isFalse);
        expect(watch.take(dropped, mic: true, sending: true), isFalse);
        expect(watch.take(dropped, mic: true, sending: true), isTrue);
      },
    );

    test('nothing is recovered with the microphone off, its input out, or a choice going on', () {
      final watch = RecordingWatch();
      final lost = {
        'recordingAsked': true,
        'recording': false,
        'inputUsable': true,
        'changing': false,
      };
      for (var i = 0; i < 3; i++) {
        expect(watch.take(lost, mic: false, sending: true), isFalse);
        expect(
          watch.take({...lost, 'inputUsable': false}, mic: true, sending: true),
          isFalse,
        );
        expect(
          watch.take({...lost, 'changing': true}, mic: true, sending: true),
          isFalse,
        );
      }
    });
  });

  group('the sound server going away (Linux)', () {
    final mic = input('headset', 'USB Headset');
    final speaker = MediaDeviceInfo(
      kind: 'audiooutput',
      label: 'Speakers',
      deviceId: 'speakers',
    );
    final start = DateTime(2026, 11, 12, 12);
    DateTime at(int s) => start.add(Duration(seconds: s));

    test('one back within a moment is told nothing', () {
      final watch = SoundServerWatch();
      expect(watch.take([mic, speaker], linux: true, now: at(0)), isNull);
      // No device at all, not even the default: the voice engine reaches no server.
      expect(watch.take(const [], linux: true, now: at(1)), isNull);
      expect(watch.lost, isTrue);
      expect(watch.take(const [], linux: true, now: at(5)), isNull);
      expect(watch.take([mic, speaker], linux: true, now: at(5)), isNull);
      expect(watch.lost, isFalse);
    });

    test('one gone 5 s is told once, and told again when back', () {
      final watch = SoundServerWatch();
      expect(watch.take(const [], linux: true, now: at(0)), isNull);
      expect(
        watch.take(const [], linux: true, now: at(5)),
        l10n.callSoundSystemLost,
      );
      expect(watch.take(const [], linux: true, now: at(60)), isNull);
      expect(
        watch.take([speaker], linux: true, now: at(61)),
        l10n.callSoundSystemBack,
      );
      expect(watch.take([speaker], linux: true, now: at(62)), isNull);
    });

    test('elsewhere an empty list says nothing of a server', () {
      final watch = SoundServerWatch();
      expect(watch.take(const [], linux: false, now: at(0)), isNull);
      expect(watch.take(const [], linux: false, now: at(10)), isNull);
      expect(watch.lost, isFalse);
    });

    test('the call says nothing of its microphone while nothing is listed', () {
      final c = Call(group: 'g', me: me, input: 'headset');
      final told = <String>[];
      c.onNotice = told.add;
      c.takeDevices([mic, speaker]);
      c.takeDevices(const []);
      // Back with its microphone: nothing to tell either.
      c.takeDevices([mic, speaker]);
      expect(told, isEmpty);
    });

    test(
      'the call tells it once gone 5 s, reading the list again',
      () => inCallTime(() async {
        final c = Call(group: 'g', me: me, input: 'headset');
        final told = <String>[];
        c.onNotice = told.add;
        c.listDevices = () async => const <MediaDeviceInfo>[];
        c.takeDevices(const []);
        await Future<void>.delayed(const Duration(milliseconds: 5400));
        expect(told, [l10n.callSoundSystemLost]);
        c.takeDevices([mic, speaker]);
        expect(told, [l10n.callSoundSystemLost, l10n.callSoundSystemBack]);
        await c.stop();
      }),
    );
  });

  group('a microphone that reads nothing', () {
    final start = DateTime(2026, 11, 5, 12);
    DateTime at(int s) => start.add(Duration(seconds: s));

    test('is told once, 8 s into sending with nothing read or sent', () {
      final check = MicCheck();
      bool read(int t) => check.take(
        on: true,
        sending: true,
        read: 0,
        sent: const {'ana': 0},
        now: at(t),
      );
      for (var t = 0; t < 8; t += 2) {
        expect(read(t), isFalse);
      }
      expect(read(8), isTrue);
      expect(read(10), isFalse);
      // Turned off and on again: judged anew.
      check.reset();
      expect(read(12), isFalse);
      expect(read(20), isTrue);
    });

    test(
      'another output chosen is judged from the start again on macOS',
      () async {
        // macOS makes its voice engine again for it, recording nothing for a moment.
        pinnedHost = HostSystem.macos;
        addTearDown(() => pinnedHost = null);
        final c = Call(group: 'g', me: me);
        bool read(int t) => c.micCheckForTests.take(
          on: true,
          sending: true,
          read: 0,
          sent: const {'ana': 0},
          now: at(t),
        );
        for (var t = 0; t <= 6; t += 2) {
          expect(read(t), isFalse);
        }
        await c.useOutput('monitor');
        expect(read(8), isFalse);
        expect(read(14), isFalse);
        expect(read(16), isTrue);
      },
    );

    test('not while the microphone waits for a new description', () {
      // Turned on mid-call: the connection sends voice only once the other side answered,
      // which may take a resend (10 s); nothing is judged until then.
      final check = MicCheck();
      for (var t = 0; t <= 12; t += 2) {
        expect(
          check.take(
            on: true,
            sending: false,
            read: 0,
            sent: const {'ana': 0},
            now: at(t),
          ),
          isFalse,
        );
      }
      for (var t = 14; t <= 60; t += 2) {
        expect(
          check.take(
            on: true,
            sending: true,
            read: (t - 14) * 1.0,
            sent: {'ana': (t - 14) * 50},
            now: at(t),
          ),
          isFalse,
        );
      }
    });

    test('a device that stops later is caught too', () {
      // Another input chosen, which cannot be opened: what is read stops growing.
      final check = MicCheck();
      for (var t = 0; t <= 10; t += 2) {
        expect(
          check.take(
            on: true,
            sending: true,
            read: t * 1.0,
            sent: {'ana': t * 50},
            now: at(t),
          ),
          isFalse,
        );
      }
      bool stopped(int t) => check.take(
        on: true,
        sending: true,
        read: 10,
        sent: const {'ana': 500},
        now: at(t),
      );
      expect(stopped(12), isFalse);
      expect(stopped(16), isFalse);
      expect(stopped(18), isTrue);
    });

    test('silence that still sends now and then is no failure', () {
      final check = MicCheck();
      for (var t = 0; t <= 60; t += 2) {
        expect(
          check.take(
            on: true,
            sending: true,
            read: null,
            sent: {'ana': t ~/ 2},
            now: at(t),
          ),
          isFalse,
        );
      }
    });

    test('nothing is judged without a count', () {
      final check = MicCheck();
      for (var t = 0; t <= 30; t += 2) {
        expect(
          check.take(
            on: true,
            sending: true,
            read: null,
            sent: const {},
            now: at(t),
          ),
          isFalse,
        );
      }
    });
  });

  group('what is said of a microphone that reads nothing', () {
    final input = MediaDeviceInfo(
      kind: 'audioinput',
      label: 'USB Headset',
      deviceId: 'headset',
    );
    final output = MediaDeviceInfo(
      kind: 'audiooutput',
      label: 'Speakers',
      deviceId: 'speakers',
    );

    test('no input listed at all: none is connected, on every system', () {
      for (final system in ['macos', 'windows', 'other']) {
        expect(Call.micUnusable([output], system), l10n.callNoMicrophone);
        expect(Call.micUnusable(const [], system), l10n.callNoMicrophone);
      }
    });

    test('an input listed: it could not be used, and where to look', () {
      expect(
        Call.micUnusable([input, output], 'windows'),
        l10n.callMicSilent('windows'),
      );
      expect(
        l10n.callMicSilent('windows'),
        'Could not use the microphone. Check that it is connected and allowed in '
        'Settings › Privacy & security › Microphone.',
      );
      expect(
        Call.micUnusable([input], 'macos'),
        contains('System Settings › Privacy & Security'),
      );
      expect(
        Call.micUnusable([input], 'other'),
        'Could not use the microphone. Check that it is connected.',
      );
    });

    test('a list that could not be read says it could not be used', () {
      expect(Call.micUnusable(null, 'windows'), l10n.callMicSilent('windows'));
    });
  });

  test(
    'a microphone that reads nothing goes off in the call, said once a session',
    () async {
      final state = fakeRoom(people: 2);
      state.prefs.joinWithMic = true;
      final first = Call(group: 'g', me: me, mic: true);
      await state.micUnusable(first, 'nothing read');
      expect(first.mic, isFalse);
      // The preference stays: the input may work again.
      expect(state.prefs.joinWithMic, isTrue);
      expect(state.toasts.list.map((t) => t.text), ['nothing read']);
      // Turned on again in the same call, and still nothing: said again.
      await state.micUnusable(first, 'nothing read');
      expect(state.toasts.list, hasLength(2));
      // A later call with the same input: off, without a word more.
      final later = Call(group: 'g', me: me, mic: true);
      await state.micUnusable(later, 'nothing read');
      expect(later.mic, isFalse);
      expect(state.toasts.list, hasLength(2));
    },
  );
}
