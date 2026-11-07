import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/types.dart';

import 'fake_room.dart';

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

  test('coming and going without a call leaves one round of tries', () async {
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
  });

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
