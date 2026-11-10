import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';

void main() {
  test('signals wait for their sender to be a participant, then are taken in order', () {
    final early = EarlySignals();
    expect(early.isParticipant('bea'), isFalse);
    early.keep('bea', {'t': 'offer', 'n': 1});
    early.keep('bea', {'t': 'state', 'n': 2});
    expect(early.waiting, 2);
    early.participants(['bea', 'cid']);
    expect(early.isParticipant('bea'), isTrue);
    expect(early.isParticipant('dan'), isFalse);
    expect(early.take('bea').map((m) => m['n']), [1, 2]);
    expect(early.take('bea'), isEmpty);
    expect(early.waiting, 0);
  });

  test('signals from someone who is not in the room are dropped', () {
    final early = EarlySignals();
    early.participants(['bea']);
    early.keep('mallory', {'t': 'offer'});
    // Mallory leaves (or never was): her signals go with her.
    early.participants(['bea']);
    expect(early.take('mallory'), isEmpty);
    expect(early.waiting, 0);
  });

  test('only the last few signals of a device are kept', () {
    final early = EarlySignals();
    for (var i = 0; i < 10; i++) {
      early.keep('bea', {'n': i});
    }
    expect(early.waiting, EarlySignals.perDevice);
    early.participants(['bea']);
    expect(early.take('bea').map((m) => m['n']), [6, 7, 8, 9]);
  });

  /// A description with a section of each kind, named by its place.
  String described(List<String> kinds) => [
    'v=0',
    for (final (i, kind) in kinds.indexed) ...[
      'm=$kind 9 UDP/TLS/RTP/SAVPF 96',
      'a=mid:$i',
    ],
    '',
  ].join('\r\n');

  test('descriptions too long or with too many sections are dropped', () {
    const section = 'm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=mid:0\r\n';
    final call = described(['audio', 'video', 'video', 'audio']);
    expect(fairSignal({'t': 'offer', 'sdp': call}), isTrue);
    expect(fairSignal({'t': 'answer', 'sdp': call}), isTrue);
    expect(fairSignal({'t': 'state', 'mic': true}), isTrue);
    expect(fairSignal({'t': 'sound', 'sdp': 'v=0\r\n$section'}), isTrue);
    expect(fairSignal({'t': 'offer', 'sdp': 'v=0\r\n${section * 9}'}), isFalse);
    expect(fairSignal({'t': 'sound', 'sdp': 'v=0\r\n${section * 5}'}), isFalse);
    expect(
      fairSignal({'t': 'sound', 'sdp': 'v=0\r\n${'a' * (maxSdpBytes + 1)}'}),
      isFalse,
    );
    expect(fairSignal({'t': 'answer', 'sdp': 7}), isFalse);
  });

  test('candidates too long or for no channel of a call are dropped', () {
    const line =
        'candidate:842163049 1 udp 1677729535 192.0.2.7 50000 typ srflx raddr 0.0.0.0 '
        'rport 0 generation 0 ufrag Ab3d network-id 1 network-cost 10';
    expect(
      fairSignal({'t': 'ice', 'candidate': line, 'mid': '0', 'index': 0}),
      isTrue,
    );
    expect(fairSignal({'t': 'ice', 'candidate': null}), isTrue);
    expect(
      fairSignal({
        't': 'ice',
        'candidate': 'a' * (maxCandidateBytes + 1),
        'mid': '0',
        'index': 0,
      }),
      isFalse,
    );
    expect(fairSignal({'t': 'ice', 'candidate': 7}), isFalse);
    expect(fairSignal({'t': 'ice', 'candidate': line, 'mid': 0}), isFalse);
    expect(
      fairSignal({'t': 'ice', 'candidate': line, 'mid': 'x' * 100}),
      isFalse,
    );
    expect(fairSignal({'t': 'ice', 'candidate': line, 'index': '0'}), isFalse);
    expect(fairSignal({'t': 'ice', 'candidate': line, 'index': -1}), isFalse);
    expect(
      fairSignal({'t': 'ice', 'candidate': line, 'index': maxSdpSections}),
      isFalse,
    );
  });

  test('a call\'s offer or answer has its channels and no others', () {
    // Older copies, and copies before the screen's sound, offer fewer.
    for (final kinds in [
      ['audio', 'video', 'video', 'audio'],
      ['audio', 'video', 'video'],
      <String>[],
    ]) {
      expect(callSections(described(kinds)), isTrue, reason: '$kinds');
    }
    for (final kinds in [
      // Four more pictures that nobody draws, decoded all the same.
      ['audio', 'video', 'video', 'audio', 'video', 'video', 'video', 'video'],
      ['audio', 'video', 'video', 'audio', 'audio'],
      ['video', 'video', 'video', 'audio'],
      ['audio', 'video', 'audio', 'audio'],
    ]) {
      expect(callSections(described(kinds)), isFalse, reason: '$kinds');
      expect(
        fairSignal({'t': 'offer', 'sdp': described(kinds)}),
        isFalse,
        reason: '$kinds',
      );
    }
    // Sections named out of their place.
    final swapped = described(['audio', 'video', 'video', 'audio'])
        .replaceFirst('a=mid:1', 'a=mid:2');
    expect(callSections(swapped), isFalse);
  });

  test(
    'pictures larger than any participant sends are seen in the statistics',
    () {
      StatsReport video(Object mid, int width, int height) =>
          StatsReport('in-$mid', 'inbound-rtp', 0, {
            'kind': 'video',
            'mid': '$mid',
            'frameWidth': width,
            'frameHeight': height,
          });
      expect(fairPicture(1, 1920, 1080), isTrue);
      // A few pixels more, as an encoder may round a scaled picture.
      expect(fairPicture(1, 1922, 1082), isTrue);
      expect(fairPicture(1, 1080, 1920), isTrue);
      expect(fairPicture(1, 2560, 1440), isFalse);
      expect(fairPicture(2, 7680, 4320), isTrue);
      expect(fairPicture(2, 8192, 1000), isTrue);
      expect(fairPicture(2, 8209, 100), isFalse);
      // A screen's has no margin: the decoder takes no larger one, which would show
      // black, so its sender is kept out with a note instead.
      expect(fairPicture(2, 8193, 100), isFalse);
      expect(fairPicture(2, 7680, 4321), isFalse);
      expect(fairPicture(2, 16383, 16383), isFalse);
      // No other channel brings pictures.
      expect(fairPicture(5, 16, 16), isFalse);
      expect(
        unfairPicture([video(1, 1280, 720), video(2, 3840, 2160)]),
        isNull,
      );
      // Nothing decoded yet.
      expect(unfairPicture([video(4, 0, 0)]), isNull);
      expect(
        unfairPicture([video(1, 1280, 720), video(1, 3840, 2160)]),
        'video on channel 1 at 3840x2160',
      );
      expect(unfairPicture([video(5, 320, 240)]), isNotNull);
      // Statistics that name no channel: a screen's limit, not kept out.
      StatsReport unnamed(int width, int height) => StatsReport(
        'in',
        'inbound-rtp',
        0,
        {'kind': 'video', 'frameWidth': width, 'frameHeight': height},
      );
      expect(unfairPicture([unnamed(3840, 2160)]), isNull);
      expect(unfairPicture([unnamed(16383, 16383)]), isNotNull);
      expect(
        unfairPicture([
          StatsReport('out', 'outbound-rtp', 0, {
            'kind': 'video',
            'mid': '1',
            'frameWidth': 3840,
            'frameHeight': 2160,
          }),
        ]),
        isNull,
      );
    },
  );
}
