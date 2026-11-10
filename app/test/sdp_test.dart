import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';

void main() {
  test('VP8 and its retransmission codec go first in every video section', () {
    // As macOS offers it: H.264 first.
    final offer = [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111 63',
      'a=rtpmap:111 opus/48000/2',
      'a=rtpmap:63 red/48000/2',
      'm=video 9 UDP/TLS/RTP/SAVPF 96 97 98 99 100 101 127',
      'a=rtpmap:96 H264/90000',
      'a=rtpmap:97 rtx/90000',
      'a=fmtp:97 apt=96',
      'a=rtpmap:98 VP8/90000',
      'a=rtpmap:99 rtx/90000',
      'a=fmtp:99 apt=98',
      'a=rtpmap:100 VP9/90000',
      'a=rtpmap:101 rtx/90000',
      'a=fmtp:101 apt=100',
      'a=rtpmap:127 red/90000',
      'm=video 9 UDP/TLS/RTP/SAVPF 102 103 104',
      'a=rtpmap:102 H264/90000',
      'a=rtpmap:103 VP8/90000',
      'a=rtpmap:104 rtx/90000',
      'a=fmtp:104 apt=103',
      '',
    ].join('\r\n');
    final lines = vp8First(offer).split('\r\n');
    expect(lines[1], 'm=audio 9 UDP/TLS/RTP/SAVPF 111 63');
    expect(lines[4], 'm=video 9 UDP/TLS/RTP/SAVPF 98 99 96 97 100 101 127');
    expect(lines[15], 'm=video 9 UDP/TLS/RTP/SAVPF 103 104 102');
    expect(lines.length, offer.split('\r\n').length);
  });

  test('only the screen channel gets the higher limit', () {
    final sdp = [
      'v=0',
      'm=video 9 UDP/TLS/RTP/SAVPF 98 99',
      'a=mid:1',
      'a=rtpmap:98 VP8/90000',
      'a=rtpmap:99 rtx/90000',
      'a=fmtp:99 apt=98',
      'm=video 9 UDP/TLS/RTP/SAVPF 98 99 100',
      'a=mid:2',
      'a=rtpmap:98 VP8/90000',
      'a=rtpmap:99 rtx/90000',
      'a=fmtp:99 apt=98',
      'a=rtpmap:100 VP8/90000',
      'a=fmtp:100 max-fr=30',
      '',
    ].join('\r\n');
    final lines = screenBitrate(sdp).split('\r\n');
    // The camera's section is untouched.
    expect(lines.sublist(0, 6), sdp.split('\r\n').sublist(0, 6));
    expect(lines.sublist(6), [
      'm=video 9 UDP/TLS/RTP/SAVPF 98 99 100',
      'a=mid:2',
      'a=rtpmap:98 VP8/90000',
      'a=fmtp:98 x-google-max-bitrate=$screenMaxKbps',
      'a=rtpmap:99 rtx/90000',
      'a=fmtp:99 apt=98',
      'a=rtpmap:100 VP8/90000',
      'a=fmtp:100 max-fr=30;x-google-max-bitrate=$screenMaxKbps',
      '',
    ]);
    // Twice is the same as once.
    expect(screenBitrate(screenBitrate(sdp)), screenBitrate(sdp));
  });

  test('the camera gets its limit and start and the voice may go quiet', () {
    final sdp = [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      'a=mid:0',
      'a=rtpmap:111 opus/48000/2',
      'a=fmtp:111 minptime=10;useinbandfec=1',
      'm=video 9 UDP/TLS/RTP/SAVPF 98 99',
      'a=mid:1',
      'a=rtpmap:98 VP8/90000',
      'a=rtpmap:99 rtx/90000',
      'a=fmtp:99 apt=98',
      'm=video 9 UDP/TLS/RTP/SAVPF 98',
      'a=mid:2',
      'a=rtpmap:98 VP8/90000',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      'a=mid:3',
      'a=rtpmap:111 opus/48000/2',
      'a=fmtp:111 minptime=10',
      '',
    ].join('\r\n');
    final lines = voiceDtx(cameraBitrate(sdp)).split('\r\n');
    expect(lines[4], 'a=fmtp:111 minptime=10;useinbandfec=1;usedtx=1');
    expect(
      lines[8],
      'a=fmtp:98 x-google-max-bitrate=$cameraMaxKbps;'
      'x-google-start-bitrate=$cameraStartKbps',
    );
    // Neither the screen nor its sound.
    expect(lines.sublist(12), sdp.split('\r\n').sublist(11));
    // Twice is the same as once.
    expect(voiceDtx(voiceDtx(sdp)), voiceDtx(sdp));
    expect(cameraBitrate(cameraBitrate(sdp)), cameraBitrate(sdp));
  });

  test('only the screen\'s sound goes in stereo, the voice in one channel', () {
    final sdp = [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      'a=mid:0',
      'a=rtpmap:111 opus/48000/2',
      'a=fmtp:111 minptime=10;useinbandfec=1',
      'm=video 9 UDP/TLS/RTP/SAVPF 98',
      'a=mid:1',
      'a=rtpmap:98 VP8/90000',
      'm=video 9 UDP/TLS/RTP/SAVPF 98',
      'a=mid:2',
      'a=rtpmap:98 VP8/90000',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      'a=mid:3',
      'a=rtpmap:111 opus/48000/2',
      'a=fmtp:111 minptime=10;useinbandfec=1',
      '',
    ].join('\r\n');
    final stereo = screenSoundStereo(voiceDtx(sdp), 3);
    final lines = stereo.split('\r\n');
    expect(lines[4], 'a=fmtp:111 minptime=10;useinbandfec=1;usedtx=1');
    expect(
      lines[14],
      'a=fmtp:111 minptime=10;useinbandfec=1;stereo=1;sprop-stereo=1',
    );
    expect(lines.where((l) => l.contains('stereo')), hasLength(1));
    // Twice is the same as once, whichever was there first.
    expect(screenSoundStereo(stereo, 3), stereo);
    final sprop = sdp.replaceFirst(
      'a=mid:3\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;useinbandfec=1',
      'a=mid:3\r\na=rtpmap:111 opus/48000/2\r\n'
          'a=fmtp:111 minptime=10;useinbandfec=1;sprop-stereo=1',
    );
    expect(
      screenSoundStereo(sprop, 3).split('\r\n')[14],
      'a=fmtp:111 minptime=10;useinbandfec=1;sprop-stereo=1;stereo=1',
    );
    // A screen's sound connection has its one section first, and Opus without
    // parameters gets them after its rtpmap line.
    const own =
        'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=mid:0\r\n'
        'a=rtpmap:111 opus/48000/2\r\n';
    expect(
      screenSoundStereo(own, 0),
      'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=mid:0\r\n'
      'a=rtpmap:111 opus/48000/2\r\na=fmtp:111 stereo=1;sprop-stereo=1\r\n',
    );
    // What an older copy checks of a description it takes reads no format parameters:
    // it takes these as before, and its own, without `stereo=1`, has WebRTC send it one
    // channel.
    expect(callSections(stereo), isTrue);
    expect(callCodecs(stereo), isTrue);
    expect(fairSignal({'t': 'offer', 'sdp': stereo}), isTrue);
    expect(soundDescription(screenSoundStereo(own, 0)), isTrue);
    expect(onlyCallCodecs(stereo), stereo);
  });

  test('a description without VP8 stays as it was', () {
    const sdp =
        'v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 H264/90000\r\n';
    expect(vp8First(sdp), sdp);
  });
  test('only VP8 with its retransmission and Opus stay', () {
    final offer = [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111 63 9 126',
      'a=rtpmap:111 opus/48000/2',
      'a=rtcp-fb:111 transport-cc',
      'a=fmtp:111 minptime=10;useinbandfec=1',
      'a=rtpmap:63 red/48000/2',
      'a=fmtp:63 111/111',
      'a=rtpmap:9 G722/8000',
      'a=rtpmap:126 telephone-event/8000',
      'a=mid:0',
      'm=video 9 UDP/TLS/RTP/SAVPF 96 97 98 99 100 127',
      'a=rtpmap:96 H264/90000',
      'a=rtcp-fb:96 nack',
      'a=fmtp:96 profile-level-id=42e01f',
      'a=rtpmap:97 rtx/90000',
      'a=fmtp:97 apt=96',
      'a=rtpmap:98 VP8/90000',
      'a=rtcp-fb:98 nack pli',
      'a=rtpmap:99 rtx/90000',
      'a=fmtp:99 apt=98',
      'a=rtpmap:100 AV1/90000',
      'a=rtpmap:127 ulpfec/90000',
      'a=rtcp-fb:* ccm fir',
      'a=mid:1',
      'm=video 9 UDP/TLS/RTP/SAVPF 102',
      'a=rtpmap:102 H264/90000',
      'a=mid:2',
      '',
    ].join('\r\n');
    final lines = onlyCallCodecs(offer).split('\r\n');
    expect(lines, [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      'a=rtpmap:111 opus/48000/2',
      'a=rtcp-fb:111 transport-cc',
      'a=fmtp:111 minptime=10;useinbandfec=1',
      'a=mid:0',
      'm=video 9 UDP/TLS/RTP/SAVPF 98 99',
      'a=rtpmap:98 VP8/90000',
      'a=rtcp-fb:98 nack pli',
      'a=rtpmap:99 rtx/90000',
      'a=fmtp:99 apt=98',
      'a=rtcp-fb:* ccm fir',
      'a=mid:1',
      // Without VP8 the section is rejected: nothing on it is decoded.
      'm=video 0 UDP/TLS/RTP/SAVPF 102',
      'a=rtpmap:102 H264/90000',
      'a=mid:2',
      '',
    ]);
  });

  test('a section whose Opus or VP8 is not among its types is rejected', () {
    final answer = [
      'v=0',
      'a=group:BUNDLE 0 1 2 3',
      // Opus named, but not listed: WebRTC would take PCMU and G.722 here.
      'm=audio 9 UDP/TLS/RTP/SAVPF 0 9',
      'a=mid:0',
      'a=rtpmap:111 opus/48000/2',
      'a=rtpmap:9 G722/8000',
      // Fine.
      'm=video 9 UDP/TLS/RTP/SAVPF 98 99',
      'a=mid:1',
      'a=rtpmap:98 VP8/90000',
      'a=rtpmap:99 rtx/90000',
      'a=fmtp:99 apt=98',
      // VP8 named, H.264 listed; and left in the bundle with port 0.
      'm=video 9 UDP/TLS/RTP/SAVPF 102',
      'a=mid:2',
      'a=bundle-only',
      'a=rtpmap:98 VP8/90000',
      'a=rtpmap:102 H264/90000',
      // Static types only, PCMU and PCMA, without an rtpmap line.
      'm=audio 9 UDP/TLS/RTP/SAVPF 0 8',
      'a=mid:3',
      '',
    ].join('\r\n');
    expect(onlyCallCodecs(answer).split('\r\n'), [
      'v=0',
      'a=group:BUNDLE 1',
      'm=audio 0 UDP/TLS/RTP/SAVPF 0 9',
      'a=mid:0',
      'a=rtpmap:111 opus/48000/2',
      'a=rtpmap:9 G722/8000',
      'm=video 9 UDP/TLS/RTP/SAVPF 98 99',
      'a=mid:1',
      'a=rtpmap:98 VP8/90000',
      'a=rtpmap:99 rtx/90000',
      'a=fmtp:99 apt=98',
      'm=video 0 UDP/TLS/RTP/SAVPF 102',
      'a=mid:2',
      'a=rtpmap:98 VP8/90000',
      'a=rtpmap:102 H264/90000',
      'm=audio 0 UDP/TLS/RTP/SAVPF 0 8',
      'a=mid:3',
      '',
    ]);
    // Twice is the same as once.
    expect(onlyCallCodecs(onlyCallCodecs(answer)), onlyCallCodecs(answer));
    // Opus listed but in mono is not the Opus a call keeps either.
    expect(
      onlyCallCodecs(
        'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111 9\r\n'
        'a=rtpmap:111 opus/48000/1\r\na=rtpmap:9 G722/8000\r\n',
      ),
      startsWith('v=0\r\nm=audio 0 '),
    );
  });

  test('the voice channel keeps RED over Opus, first; nothing else does', () {
    final offer = [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111 63 9 62',
      'a=mid:0',
      'a=rtpmap:111 opus/48000/2',
      'a=fmtp:111 minptime=10;useinbandfec=1',
      'a=rtpmap:63 red/48000/2',
      'a=fmtp:63 111/111',
      'a=rtpmap:9 G722/8000',
      // RED over another codec is left out.
      'a=rtpmap:62 red/8000',
      'a=fmtp:62 9/9',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111 63',
      'a=mid:3',
      'a=rtpmap:111 opus/48000/2',
      'a=rtpmap:63 red/48000/2',
      'a=fmtp:63 111/111',
      '',
    ].join('\r\n');
    final lines = voiceDtx(voiceRedFirst(onlyCallCodecs(offer, voiceRed: true)))
        .split('\r\n');
    expect(lines, [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 63 111',
      'a=mid:0',
      'a=rtpmap:111 opus/48000/2',
      'a=fmtp:111 minptime=10;useinbandfec=1;usedtx=1',
      'a=rtpmap:63 red/48000/2',
      'a=fmtp:63 111/111',
      // The screen's sound stays plain Opus.
      'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      'a=mid:3',
      'a=rtpmap:111 opus/48000/2',
      '',
    ]);
    // Twice is the same as once.
    expect(voiceRedFirst(lines.join('\r\n')), lines.join('\r\n'));
    // Without it (a screen's sound connection), RED goes.
    expect(onlyCallCodecs(offer), isNot(contains('red/')));
  });

  test('a voice channel without RED (an older copy) stays plain Opus', () {
    final answer = [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      'a=mid:0',
      'a=rtpmap:111 opus/48000/2',
      '',
    ].join('\r\n');
    expect(voiceRedFirst(onlyCallCodecs(answer, voiceRed: true)), answer);
  });

  test('a shared screen larger than 1920 by 1080 is sent scaled to fit', () {
    expect(screenScale(1280, 720), 1);
    expect(screenScale(1920, 1080), 1);
    expect(screenScale(3840, 2160), 2);
    // Taller than 16:9: the height decides.
    expect(screenScale(2560, 1600), closeTo(1600 / 1080, 1e-9));
  });

  test('a shared screen fits the resolution chosen for it', () {
    expect(screenScale(1600, 900, lines: 720), 1.25);
    expect(screenScale(1280, 720, lines: 720), 1);
    expect(screenScale(3840, 2160, lines: null), 1);
    // Wider than 16:9: the width decides.
    expect(screenScale(3440, 1440, lines: 1080), closeTo(3440 / 1920, 1e-9));
  });

  test('the stream quality reads back what was saved, else the default', () {
    const chosen = ScreenQuality(lines: null, fps: 60);
    expect(ScreenQuality.fromJson(chosen.toJson()), chosen);
    expect(
      ScreenQuality.fromJson({'lines': 480, 'fps': 24}),
      const ScreenQuality(),
    );
    expect(ScreenQuality.fromJson(null), const ScreenQuality());
  });

  test('a screen sound description is one sound channel in Opus', () {
    const opus =
        'm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=mid:0\r\n'
        'a=rtpmap:111 opus/48000/2\r\n';
    expect(soundDescription('v=0\r\n$opus'), isTrue);
    for (final (what, sdp) in [
      (
        'with video',
        'v=0\r\n${opus}m=video 9 UDP/TLS/RTP/SAVPF 98\r\na=mid:1\r\n'
            'a=rtpmap:98 VP8/90000\r\n',
      ),
      (
        'two sound channels',
        'v=0\r\n$opus${opus.replaceAll('mid:0', 'mid:1')}',
      ),
      (
        'a data channel',
        'v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=mid:0\r\n',
      ),
      (
        'Opus not listed',
        'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 9\r\na=rtpmap:111 opus/48000/2\r\n',
      ),
      ('rejected', 'v=0\r\n${opus.replaceFirst(' 9 ', ' 0 ')}'),
      ('nothing', 'v=0\r\n'),
    ]) {
      expect(soundDescription(sdp), isFalse, reason: what);
    }
  });

  test('an offer with a channel in no codec a call keeps is told apart', () {
    const sounds = {
      'opus': (111, '48000/2'),
      'G722': (9, '8000'),
      'PCMU': (0, '8000'),
      'ILBC': (102, '8000'),
    };
    String offer(String video, String sound) => [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      'a=rtpmap:111 opus/48000/2',
      'a=mid:0',
      'm=video 9 UDP/TLS/RTP/SAVPF 98',
      'a=rtpmap:98 VP8/90000',
      'a=mid:1',
      'm=video 9 UDP/TLS/RTP/SAVPF 102',
      'a=rtpmap:102 $video/90000',
      'a=mid:2',
      'm=audio 9 UDP/TLS/RTP/SAVPF ${sounds[sound]!.$1}',
      'a=rtpmap:${sounds[sound]!.$1} $sound/${sounds[sound]!.$2}',
      'a=mid:3',
      '',
    ].join('\r\n');
    expect(callCodecs(offer('VP8', 'opus')), isTrue);
    // A screen's channel in H.264, AV1 or VP9 only, its sound in G.722 or PCMU only.
    for (final video in ['H264', 'AV1', 'VP9']) {
      expect(callCodecs(offer(video, 'opus')), isFalse, reason: video);
    }
    for (final sound in ['G722', 'PCMU', 'ILBC']) {
      expect(callCodecs(offer('VP8', sound)), isFalse, reason: sound);
    }
    // The connection of a screen's sound, with its one channel.
    expect(
      callCodecs(
        'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 9\r\na=rtpmap:9 G722/8000\r\n',
      ),
      isFalse,
    );
    expect(
      callCodecs(
        'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=rtpmap:111 opus/48000/2\r\n',
      ),
      isTrue,
    );
  });

  test('Opus and VP8 count only among the types the section lists', () {
    String offer({
      String sound = 'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      String opus = 'a=rtpmap:111 opus/48000/2',
      String video = 'm=video 9 UDP/TLS/RTP/SAVPF 98',
      String vp8 = 'a=rtpmap:98 VP8/90000',
      List<String> more = const [],
    }) => [
      'v=0',
      'a=group:BUNDLE 0 1',
      sound,
      'a=mid:0',
      opus,
      video,
      'a=mid:1',
      vp8,
      ...more,
      '',
    ].join('\r\n');
    expect(callCodecs(offer()), isTrue);
    for (final (what, sdp) in [
      // An rtpmap line for a number the m= line does not list: WebRTC ignores it.
      ('orphan Opus', offer(sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 0')),
      ('orphan VP8', offer(video: 'm=video 9 UDP/TLS/RTP/SAVPF 96')),
      // Static types need no rtpmap line: PCMU, PCMA and G.722.
      ('static types only', offer(sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 0 8 9')),
      // The Opus and VP8 every copy offers, and no other.
      ('Opus in mono', offer(opus: 'a=rtpmap:111 opus/48000/1')),
      ('Opus at 8 kHz', offer(opus: 'a=rtpmap:111 opus/8000/2')),
      ('VP8 at 8 kHz', offer(vp8: 'a=rtpmap:98 VP8/8000')),
      // The second of several sections falls short.
      (
        'a third section',
        offer(
          more: [
            'm=video 9 UDP/TLS/RTP/SAVPF 102',
            'a=mid:2',
            'a=rtpmap:98 VP8/90000',
            'a=rtpmap:102 H264/90000',
          ],
        ),
      ),
      // Port 0 with bundle-only is a section in the bundle, not a rejected one, also
      // with a value, as WebRTC reads it.
      for (final only in [
        'a=bundle-only',
        'a=bundle-only:1',
        'a=bundle-only x',
      ])
        (
          only,
          offer(
            more: [
              'm=video 0 UDP/TLS/RTP/SAVPF 102',
              'a=mid:2',
              only,
              'a=rtpmap:102 H264/90000',
            ],
          ),
        ),
      // One payload type named twice, the same way or not, or listed twice.
      (
        'Opus named twice',
        offer(opus: 'a=rtpmap:111 opus/48000/2\r\na=rtpmap:111 opus/48000/2'),
      ),
      (
        'Opus and G.722 on one number',
        offer(opus: 'a=rtpmap:111 opus/48000/2\r\na=rtpmap:111 G722/8000'),
      ),
      (
        'a type listed twice',
        offer(sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 111 111'),
      ),
      // A payload type WebRTC would not read as one.
      ('a number in hex', offer(sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 0x6f')),
    ]) {
      expect(callCodecs(sdp), isFalse, reason: what);
    }
    // A rejected section decodes nothing, whatever it lists.
    expect(
      callCodecs(
        offer(
          more: [
            'm=video 0 UDP/TLS/RTP/SAVPF 102',
            'a=mid:2',
            'a=rtpmap:102 H264/90000',
          ],
        ),
      ),
      isTrue,
    );
  });

  test('a section lists Opus or VP8 and only what goes with them, by number', () {
    String offer({
      String sound = 'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      List<String> soundLines = const ['a=rtpmap:111 opus/48000/2'],
      String video = 'm=video 9 UDP/TLS/RTP/SAVPF 96',
      List<String> videoLines = const ['a=rtpmap:96 VP8/90000'],
    }) => [
      'v=0',
      'a=group:BUNDLE 0 1',
      sound,
      'a=mid:0',
      ...soundLines,
      video,
      'a=mid:1',
      ...videoLines,
      '',
    ].join('\r\n');
    // What a copy sends: Opus with RED over it, VP8 with its RTX.
    final copy = offer(
      sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 63 111',
      soundLines: [
        'a=rtpmap:63 red/48000/2',
        'a=fmtp:63 111/111',
        'a=rtpmap:111 opus/48000/2',
      ],
      video: 'm=video 9 UDP/TLS/RTP/SAVPF 96 97',
      videoLines: [
        'a=rtpmap:96 VP8/90000',
        'a=rtpmap:97 rtx/90000',
        'a=fmtp:97 apt=96',
      ],
    );
    expect(callCodecs(copy), isTrue);
    expect(callCodecs(offer()), isTrue);
    for (final (what, sdp) in [
      // A static type listed beside Opus or VP8: WebRTC would take it without a line.
      ('PCMU beside Opus', offer(sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 111 0')),
      ('G.722 beside Opus', offer(sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 111 9')),
      (
        'H.264 beside VP8',
        offer(
          video: 'm=video 9 UDP/TLS/RTP/SAVPF 96 102',
          videoLines: ['a=rtpmap:96 VP8/90000', 'a=rtpmap:102 H264/90000'],
        ),
      ),
      // One payload type written two ways: 111 and 0111 are the same to WebRTC.
      (
        '0111 listed beside 111',
        offer(sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 111 0111'),
      ),
      (
        '0111 named G.722 beside 111 named Opus',
        offer(
          soundLines: ['a=rtpmap:111 opus/48000/2', 'a=rtpmap:0111 G722/8000'],
        ),
      ),
      (
        'Opus named as 0111',
        offer(
          sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 0111',
          soundLines: ['a=rtpmap:111 opus/48000/2', 'a=rtpmap:0111 G722/8000'],
        ),
      ),
      // RED over something that is not Opus, RED in a video section.
      (
        'RED over G.722',
        offer(
          sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 111 63 9',
          soundLines: [
            'a=rtpmap:111 opus/48000/2',
            'a=rtpmap:63 red/48000/2',
            'a=fmtp:63 9/9',
            'a=rtpmap:9 G722/8000',
          ],
        ),
      ),
      (
        'RED with no codec named',
        offer(
          sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 111 63',
          soundLines: ['a=rtpmap:111 opus/48000/2', 'a=rtpmap:63 red/48000/2'],
        ),
      ),
      // RTX of nothing, of another codec, or of two.
      (
        'RTX with no apt',
        offer(
          video: 'm=video 9 UDP/TLS/RTP/SAVPF 96 97',
          videoLines: ['a=rtpmap:96 VP8/90000', 'a=rtpmap:97 rtx/90000'],
        ),
      ),
      (
        'RTX of H.264',
        offer(
          video: 'm=video 9 UDP/TLS/RTP/SAVPF 96 97',
          videoLines: [
            'a=rtpmap:96 VP8/90000',
            'a=rtpmap:97 rtx/90000',
            'a=fmtp:97 apt=102',
          ],
        ),
      ),
      (
        'RTX with two apt lines',
        offer(
          video: 'm=video 9 UDP/TLS/RTP/SAVPF 96 97',
          videoLines: [
            'a=rtpmap:96 VP8/90000',
            'a=rtpmap:97 rtx/90000',
            'a=fmtp:97 apt=96',
            'a=fmtp:97 apt=102',
          ],
        ),
      ),
      // A codec named but not one a copy offers: FEC, comfort noise, DTMF.
      (
        'ulpfec beside VP8',
        offer(
          video: 'm=video 9 UDP/TLS/RTP/SAVPF 96 127',
          videoLines: ['a=rtpmap:96 VP8/90000', 'a=rtpmap:127 ulpfec/90000'],
        ),
      ),
      (
        'telephone-event beside Opus',
        offer(
          sound: 'm=audio 9 UDP/TLS/RTP/SAVPF 111 126',
          soundLines: [
            'a=rtpmap:111 opus/48000/2',
            'a=rtpmap:126 telephone-event/8000',
          ],
        ),
      ),
    ]) {
      expect(callCodecs(sdp), isFalse, reason: what);
    }
  });

  test('a screen shared as captured fits once WebRTC rounds its sides', () {
    for (var width = 7000; width <= 20000; width += 37) {
      for (var height = 600; height <= 12000; height += 53) {
        final scale = screenScale(
          width.toDouble(),
          height.toDouble(),
          lines: null,
        );
        // As WebRTC's encoder makes the size of a picture scaled down.
        final (w, h) = ((width / scale).round(), (height / scale).round());
        expect(
          fairPicture(2, w, h),
          isTrue,
          reason: '$width x $height went out at $w x $h',
        );
      }
    }
    // A screen within the limit goes out as it is.
    expect(screenScale(7680, 4320, lines: null), 1);
    expect(screenScale(8192, 2000, lines: null), 1);
  });
}
