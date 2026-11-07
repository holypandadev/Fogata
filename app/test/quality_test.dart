import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart' show StatsReport;
import 'package:fogata/src/quality.dart';

/// Statistics as WebRTC gives them: a chosen candidate pair and the voice coming in.
List<StatsReport> stats({
  double rtt = .03,
  int heard = 100,
  int voiceLost = 0,
  int voiceIn = 0,
  int videoLost = 0,
  int videoIn = 0,
}) => [
  StatsReport('T01', 'transport', 0, {'selectedCandidatePairId': 'CPa'}),
  StatsReport('CPb', 'candidate-pair', 0, {
    'nominated': true,
    'currentRoundTripTime': .9,
    'requestsReceived': 1,
  }),
  StatsReport('CPa', 'candidate-pair', 0, {
    'nominated': true,
    'currentRoundTripTime': rtt,
    'requestsReceived': heard,
    'responsesReceived': 0,
    'bytesReceived': 0,
  }),
  StatsReport('IA', 'inbound-rtp', 0, {
    'kind': 'audio',
    'mid': '0',
    'packetsLost': voiceLost,
    'packetsReceived': voiceIn,
  }),
  // The sound of a shared screen is not the voice.
  StatsReport('IS', 'inbound-rtp', 0, {
    'kind': 'audio',
    'mid': '3',
    'packetsLost': 500,
    'packetsReceived': 1,
  }),
  StatsReport('IV', 'inbound-rtp', 0, {
    'kind': 'video',
    'mid': '1',
    'packetsLost': videoLost,
    'packetsReceived': videoIn,
  }),
];

void main() {
  test('levels follow the delay and the losses', () {
    expect(NetLevel.of(), NetLevel.good);
    expect(NetLevel.of(delayMs: 149, lossPercent: 1.9), NetLevel.good);
    expect(NetLevel.of(delayMs: 150), NetLevel.okay);
    expect(NetLevel.of(lossPercent: 2), NetLevel.okay);
    expect(NetLevel.of(delayMs: 299, lossPercent: 4.9), NetLevel.okay);
    expect(NetLevel.of(delayMs: 300), NetLevel.weak);
    expect(NetLevel.of(lossPercent: 5), NetLevel.weak);
    expect(NetLevel.of(delayMs: 40, lossPercent: 9), NetLevel.weak);
    expect(NetLevel.good.worse(NetLevel.okay), NetLevel.okay);
    expect(NetLevel.lost.better(NetLevel.weak), NetLevel.weak);
    expect(NetLevel.named('weak'), NetLevel.weak);
    expect(NetLevel.named('bad'), isNull);
    expect(NetLevel.named(null), isNull);
  });

  test(
    'a new level needs two readings in a row; losing someone is at once',
    () {
      final gate = LevelGate();
      expect(gate.take(NetLevel.good), true);
      expect(gate.take(NetLevel.weak), false);
      expect(gate.level, NetLevel.good);
      // A good reading in between starts over.
      expect(gate.take(NetLevel.good), false);
      expect(gate.take(NetLevel.weak), false);
      expect(gate.take(NetLevel.okay), false);
      expect(gate.level, NetLevel.good);
      expect(gate.take(NetLevel.okay), true);
      expect(gate.level, NetLevel.okay);
      expect(gate.take(NetLevel.lost), true);
      expect(gate.level, NetLevel.lost);
      // Coming back shows at once too.
      expect(gate.take(NetLevel.good), true);
      expect(gate.level, NetLevel.good);
    },
  );

  test('statistics are read from the chosen path and the voice', () {
    final sample = LinkSample.of(
      stats(rtt: .042, heard: 7, voiceLost: 3, voiceIn: 97, videoIn: 10),
    );
    expect(sample.delayMs, 42);
    expect(sample.heard, 8);
    expect(sample.voice, (3, 97));
    expect(sample.video, (0, 10));
    // Without the transport's choice, the nominated pair.
    final nominated = LinkSample.of([
      StatsReport('CP', 'candidate-pair', 0, {
        'nominated': true,
        'currentRoundTripTime': .2,
      }),
    ]);
    expect(nominated.delayMs, 200);
    expect(roundTripMs(const []), isNull);
  });

  group('a call', () {
    late CallQuality quality;
    final start = DateTime(2026, 10, 25, 12);
    DateTime at(int seconds) => start.add(Duration(seconds: seconds));
    var heard = 0;

    setUp(() {
      quality = CallQuality(me: 'me');
      heard = 0;
    });

    /// One reading of [id] at second [s]: something arrived unless [silent].
    void read(
      String id,
      int s, {
      double rtt = .03,
      int voiceLost = 0,
      int voiceIn = 0,
      bool silent = false,
    }) {
      if (!silent) heard++;
      quality.take(
        id,
        LinkSample.of(
          stats(rtt: rtt, heard: heard, voiceLost: voiceLost, voiceIn: voiceIn),
        ),
        at(s),
      );
    }

    test('levels change only on the second reading', () {
      var changes = 0;
      quality.addListener(() => changes++);
      read('leo', 0);
      expect(quality.linkOf('leo'), NetLevel.good);
      read('leo', 2, rtt: .2);
      expect(quality.linkOf('leo'), NetLevel.good);
      read('leo', 4, rtt: .2);
      expect(quality.linkOf('leo'), NetLevel.okay);
      read('leo', 6, rtt: .35);
      read('leo', 8, rtt: .35);
      expect(quality.linkOf('leo'), NetLevel.weak);
      expect(quality.delayOf('leo'), 350);
      // Only the level changes told anyone: no full redraw on each reading.
      expect(changes, 3);
      expect(quality.readings.value, 5);
    });

    test('losses count over enough packets', () {
      read('leo', 0, voiceIn: 1000);
      // Five packets, one lost: too few to tell.
      read('leo', 2, voiceLost: 1, voiceIn: 1004);
      expect(quality.lossOf('leo'), isNull);
      read('leo', 4, voiceLost: 10, voiceIn: 1100);
      expect(quality.lossOf('leo'), closeTo(9.09, .01));
      read('leo', 6, voiceLost: 20, voiceIn: 1190);
      expect(quality.linkOf('leo'), NetLevel.weak);
      // A quiet voice: 20 packets over 6 s are enough.
      read('leo', 8, voiceLost: 20, voiceIn: 1200);
      read('leo', 10, voiceLost: 20, voiceIn: 1205);
      read('leo', 12, voiceLost: 20, voiceIn: 1210);
      expect(quality.lossOf('leo'), 0);
      read('leo', 14, voiceLost: 20, voiceIn: 1300);
      expect(quality.linkOf('leo'), NetLevel.good);
      // A new connection counts from zero again.
      read('leo', 16, voiceIn: 3);
      expect(quality.lossOf('leo'), 0);
    });

    test('nothing for 3 s is lost, and coming back is at once', () {
      read('leo', 0);
      read('leo', 2, silent: true);
      expect(quality.linkOf('leo'), NetLevel.good);
      read('leo', 3, silent: true);
      expect(quality.linkOf('leo'), NetLevel.lost);
      expect(quality.silenceOf('leo', at(9)), const Duration(seconds: 9));
      // What went missing meanwhile does not count once they are back.
      read('leo', 9, voiceLost: 300, voiceIn: 100);
      expect(quality.linkOf('leo'), NetLevel.good);
      read('leo', 11, voiceLost: 300, voiceIn: 200);
      expect(quality.lossOf('leo'), 0);
      expect(quality.linkOf('leo'), NetLevel.good);
      quality.cut('leo');
      expect(quality.linkOf('leo'), NetLevel.lost);
      // Someone never reached has nothing to lose.
      quality.cut('ana');
      expect(quality.linkOf('ana'), isNull);
    });

    test('the own network is the best connection, told apart from two', () {
      expect(quality.mine, isNull);
      read('leo', 0, rtt: .4);
      expect(quality.mine, NetLevel.weak);
      expect(quality.network, isNull);
      read('marta', 0);
      expect(quality.mine, NetLevel.good);
      expect(quality.network, NetLevel.good);
      expect(quality.shownOf('me'), NetLevel.good);
      // Sofía says her network is poor: her tile shows the worse of the two.
      read('sofia', 0);
      quality.theirs('sofia', NetLevel.weak);
      expect(quality.shownOf('sofia'), NetLevel.weak);
      expect(quality.shownOf('marta'), NetLevel.good);
      quality.theirs('sofia', null);
      expect(quality.shownOf('sofia'), NetLevel.good);
      quality.keep({'leo'});
      expect(quality.measured, ['leo']);
      expect(quality.network, isNull);
    });
  });

  test('figures read as in the details', () {
    expect(delayText(32), '32 ms');
    expect(lossText(0), '0 %');
    expect(lossText(.1), '0.1 %');
    expect(lossText(2.84), '2.8 %');
    expect(lossText(9), '9 %');
    expect(lossText(12.6), '13 %');
  });

  test("the hint says whose side the trouble is on", () {
    expect(
      hintAbout(name: 'Marta', link: NetLevel.good, theirs: NetLevel.good),
      'Marta hears you and you hear them without trouble.',
    );
    expect(
      hintAbout(
        name: 'Sofía',
        link: NetLevel.weak,
        theirs: NetLevel.weak,
        mine: NetLevel.good,
      ),
      "The trouble is on Sofía’s side: they sound choppy to everyone. Their Wi-Fi or "
      'mobile data is poor right now.',
    );
    expect(
      hintAbout(
        name: 'Leo',
        link: NetLevel.okay,
        theirs: NetLevel.good,
        mine: NetLevel.good,
        throughMember: true,
      ),
      "Leo’s own network is fine; the way between you two is slower because it goes "
      'through another member. They may sound choppy to you now and then; the others '
      'hear them well.',
    );
    // The member that carries them is named once known.
    expect(
      hintAbout(
        name: 'Bea',
        link: NetLevel.okay,
        theirs: NetLevel.good,
        mine: NetLevel.good,
        throughMember: true,
        through: 'Ana',
      ),
      "Bea’s own network is fine; the way between you two is slower because it goes "
      'through Ana. They may sound choppy to you now and then; the others hear them '
      'well.',
    );
    expect(
      hintAbout(
        name: 'Bea',
        link: NetLevel.weak,
        throughMember: true,
        through: 'Ana',
      ),
      'The way between you and Bea is poor right now; it may be either network. You '
      'may sound choppy to each other. It goes through Ana.',
    );
    expect(
      hintAbout(
        name: 'Leo',
        link: NetLevel.weak,
        theirs: NetLevel.good,
        mine: NetLevel.weak,
      ),
      startsWith('The trouble is on your side'),
    );
    expect(
      hintAbout(
        name: 'Sofía',
        link: NetLevel.lost,
        silence: const Duration(seconds: 6),
      ),
      startsWith('Nothing has come from Sofía for 6 s.'),
    );
    // Two people: a poor connection may be either network.
    expect(
      hintAbout(name: 'Leo', link: NetLevel.weak),
      'The way between you and Leo is poor right now; it may be either network. You '
      'may sound choppy to each other.',
    );
    expect(
      hintAboutMe(
        network: NetLevel.good,
        others: [
          (
            name: 'Marta',
            link: NetLevel.good,
            theirs: NetLevel.good,
            throughMember: false,
            through: null,
            silence: Duration.zero,
          ),
          (
            name: 'Leo',
            link: NetLevel.okay,
            theirs: NetLevel.good,
            throughMember: true,
            through: null,
            silence: Duration.zero,
          ),
          (
            name: 'Sofía',
            link: NetLevel.weak,
            theirs: NetLevel.weak,
            throughMember: false,
            through: null,
            silence: Duration.zero,
          ),
        ],
      ),
      'All is well on your side. Leo comes to you through another member; '
      "Sofía’s network is poor, so they sound choppy to everyone.",
    );
    expect(
      hintAboutMe(
        network: NetLevel.good,
        others: [
          (
            name: 'Bea',
            link: NetLevel.okay,
            theirs: NetLevel.good,
            throughMember: true,
            through: 'Ana',
            silence: Duration.zero,
          ),
        ],
      ),
      'All is well on your side. Bea comes to you through Ana.',
    );
    expect(
      hintAboutMe(network: NetLevel.weak, others: const []),
      startsWith('The trouble is on your side'),
    );
    expect(
      hintAboutMe(
        others: [
          (
            name: 'Leo',
            link: NetLevel.good,
            theirs: null,
            throughMember: false,
            through: null,
            silence: Duration.zero,
          ),
        ],
      ),
      'All is well: you and Leo hear each other without trouble.',
    );
  });
}
