part of '../call.dart';

/// Puts VP8 first among the video codecs of a description, followed by its retransmission
/// codec; the others keep their order. The side that offers decides the codec, so video
/// goes in VP8, the same software codec on every system. Between Linux and macOS, H.264
/// (in hardware on macOS) sent too little for the connection to ever raise its bitrate,
/// and in the other direction showed no picture at all. (macOS refuses
/// setCodecPreferences, so the description is reordered instead.)
@visibleForTesting
String vp8First(String sdp) {
  final lines = sdp.split('\r\n');
  for (var i = 0; i < lines.length; i++) {
    if (!lines[i].startsWith('m=video ')) continue;
    var end = i + 1;
    while (end < lines.length && !lines[end].startsWith('m=')) {
      end++;
    }
    final section = lines.sublist(i + 1, end);
    final vp8 = {
      for (final line in section)
        if (RegExp(r'^a=rtpmap:(\d+) VP8/').firstMatch(line) case final m?)
          m.group(1)!,
    };
    final rtx = {
      for (final line in section)
        if (RegExp(r'^a=fmtp:(\d+) apt=(\d+)$').firstMatch(line) case final m?
            when vp8.contains(m.group(2)))
          m.group(1)!,
    };
    final words = lines[i].split(' ');
    final types = words.sublist(3);
    final first = [...types.where(vp8.contains), ...types.where(rtx.contains)];
    lines[i] = [
      ...words.take(3),
      ...first,
      ...types.where((t) => !first.contains(t)),
    ].join(' ');
    i = end - 1;
  }
  return lines.join('\r\n');
}

/// The payload types an `m=` line lists, as numbers; WebRTC takes no codec of a section
/// whose number is not among them, whatever its `a=rtpmap` line says.
Set<int> _formats(String mLine) => {
  for (final word in mLine.split(' ').skip(3)) ?_number(word),
};

/// A payload type or other number of a description, digits only; null for anything else
/// (WebRTC refuses a description with such a payload type).
int? _number(String? digits) =>
    digits != null && RegExp(r'^\d{1,9}$').hasMatch(digits)
    ? int.parse(digits)
    : null;

/// The lines of a description as WebRTC reads them: ending at each line feed, a carriage
/// return before it left out.
List<String> _sdpLines(String sdp) => [
  for (final line in sdp.split('\n'))
    line.endsWith('\r') ? line.substring(0, line.length - 1) : line,
];

/// Whether `line` is `a=bundle-only`, as WebRTC reads it (with a value or not).
bool _bundleOnly(String line) =>
    line == 'a=bundle-only' ||
    line.startsWith('a=bundle-only:') ||
    line.startsWith('a=bundle-only ');

/// The codec names (lower case) of a section's `a=rtpmap` lines, by payload type, for
/// the types its `m=` line lists; with [call], only Opus at 48 kHz in stereo and VP8 at
/// 90 kHz, as every copy offers them, are named.
Map<int, String> _codecNames(List<String> section, {bool call = false}) {
  final formats = _formats(section.first);
  final rtpmap = RegExp(r'^a=rtpmap:(\d+) ([^/\s]+)/(\d+)(?:/(\d+))?$');
  return {
    for (final line in section.skip(1))
      if (rtpmap.firstMatch(line) case final m?)
        if (_number(m.group(1)) case final type? when formats.contains(type))
          if (m.group(2)!.toLowerCase() case final name
              when !call ||
                  switch (name) {
                    'opus' => m.group(3) == '48000' && m.group(4) == '2',
                    'vp8' => m.group(3) == '90000' && m.group(4) == null,
                    _ => false,
                  })
            type: name,
  };
}

/// Whether a section is rejected (port 0, and not left open by `a=bundle-only`): WebRTC
/// makes no channel for it and decodes nothing on it.
bool _rejected(List<String> section) =>
    section.first.split(' ').elementAtOrNull(1) == '0' &&
    !section.any(_bundleOnly);

/// Keeps only the codecs calls use: VP8 with its retransmission codec for video and Opus
/// for sound. Every other codec, and FEC, is one less decoder for what others send. A
/// section with neither is rejected (port 0, and out of the BUNDLE group), so nothing on
/// it is decoded; no offer with one is taken ([callCodecs]), and WebRTC answers an offer
/// with the codecs it lists, so only an offer it could not match (Opus in mono, say)
/// leaves one. (libwebrtc takes a local description that rejects a section it offered:
/// it stops that section's transceiver; read in M156's `SdpOfferAnswerHandler`, where
/// this is meant for apps that munge.) With [voiceRed], the voice channel also keeps RED over Opus (see
/// [voiceRedFirst]).
@visibleForTesting
String onlyCallCodecs(String sdp, {bool voiceRed = false}) {
  final lines = sdp.split('\r\n');
  final out = <String>[];
  final rejected = <String>{};
  var i = 0;
  while (i < lines.length) {
    final kind = RegExp(r'^m=(audio|video) ').firstMatch(lines[i])?.group(1);
    if (kind == null) {
      out.add(lines[i++]);
      continue;
    }
    var end = i + 1;
    while (end < lines.length && !lines[end].startsWith('m=')) {
      end++;
    }
    final section = lines.sublist(i, end);
    i = end;
    final wanted = kind == 'audio' ? 'opus' : 'vp8';
    final names = _codecNames(section);
    final main = {
      for (final e in _codecNames(section, call: true).entries)
        if (e.value == wanted) e.key,
    };
    if (main.isEmpty) {
      final words = section.first.split(' ');
      out.add([words[0], '0', ...words.skip(2)].join(' '));
      for (final line in section.skip(1)) {
        if (_bundleOnly(line)) continue;
        if (line.startsWith('a=mid:')) rejected.add(line.substring(6));
        out.add(line);
      }
      continue;
    }
    final keep = {
      ...main,
      for (final line in section)
        if (RegExp(r'^a=fmtp:(\d+) apt=(\d+)$').firstMatch(line) case final m?
            when main.contains(_number(m.group(2))) &&
                names[_number(m.group(1))] == 'rtx')
          ?_number(m.group(1)),
      if (voiceRed && kind == 'audio' && section.contains('a=mid:$_audio'))
        ..._redOver(section, names, main),
    };
    final words = section.first.split(' ');
    out.add(
      [
        ...words.take(3),
        ...words.skip(3).where((w) => keep.contains(_number(w))),
      ].join(' '),
    );
    for (final line in section.skip(1)) {
      final type = RegExp(r'^a=(?:rtpmap|fmtp|rtcp-fb):(\d+)')
          .firstMatch(line)
          ?.group(1);
      if (type == null || keep.contains(_number(type))) out.add(line);
    }
  }
  // A section rejected leaves the BUNDLE group: WebRTC takes no group that names one.
  return [
    for (final line in out)
      if (rejected.isNotEmpty && line.startsWith('a=group:BUNDLE'))
        line.split(' ').where((w) => !rejected.contains(w)).join(' ')
      else
        line,
  ].join('\r\n');
}

/// The RED types of a sound section whose redundant encodings are all one of [opus]
/// (`a=fmtp:63 111/111`).
Iterable<int> _redOver(
  List<String> section,
  Map<int, String> names,
  Set<int> opus,
) sync* {
  for (final line in section) {
    final m = RegExp(r'^a=fmtp:(\d+) (\d+(?:/\d+)*)$').firstMatch(line);
    if (m != null &&
        names[_number(m.group(1))] == 'red' &&
        m.group(2)!.split('/').every((t) => opus.contains(_number(t)))) {
      yield _number(m.group(1))!;
    }
  }
}

/// Puts RED over Opus first in the voice channel, so each voice packet also carries the
/// one before it and a lost packet is made up from the next. WebRTC sends the first codec
/// it knows in the other side's description: a side that leaves RED out (an older copy)
/// gets plain Opus, and RED is only sent to a side that listed it, so it can decode it.
@visibleForTesting
String voiceRedFirst(String sdp) {
  final lines = sdp.split('\r\n');
  for (var i = 0; i < lines.length; i++) {
    if (!lines[i].startsWith('m=audio ')) continue;
    var end = i + 1;
    while (end < lines.length && !lines[end].startsWith('m=')) {
      end++;
    }
    final section = lines.sublist(i, end);
    if (section.contains('a=mid:$_audio')) {
      final names = _codecNames(section);
      final opus = {
        for (final e in names.entries)
          if (e.value == 'opus') e.key,
      };
      final red = _redOver(section, names, opus).map((t) => '$t').toSet();
      final words = lines[i].split(' ');
      final types = words.sublist(3);
      lines[i] = [
        ...words.take(3),
        ...types.where(red.contains),
        ...types.where((t) => !red.contains(t)),
      ].join(' ');
    }
    i = end - 1;
  }
  return lines.join('\r\n');
}

/// Raises the limit of what the other side sends in the screen channel to [screenMaxKbps]
/// for every VP8 type of that section. The sender takes the limit from the description it
/// receives, so this side's description sets what reaches this side.
@visibleForTesting
String screenBitrate(String sdp) =>
    _withParam(sdp, _screen, 'vp8', 'x-google-max-bitrate=$screenMaxKbps');

/// What WebRTC's bandwidth estimate starts at, in kbit/s, instead of its own 300. While
/// only voice flows, the estimate often stays at its start (a muted or quiet microphone
/// sends next to nothing, and nothing can probe for more before a picture has gone out),
/// and the camera's first pictures are sized for the bitrate the camera's channel had
/// then, about half the start, as the screen's channel takes the rest: under 500 kbit/s,
/// WebRTC drops them and makes the picture smaller each time, down to 320x180 at 10
/// frames a second, and grows it back a step every 8 to 10 s, to 1280x720 a minute later.
/// From 1200 the first pictures go out at the size the step asks for; the estimate then
/// finds the connection's own rate within a second or two, and a connection it already
/// found slower starts the camera smaller, as before.
const cameraStartKbps = 1200;

/// Limits what the other side sends in the camera channel to [cameraMaxKbps], as
/// [screenBitrate] does for the screen, and has its bandwidth estimate start at
/// [cameraStartKbps] (a value the sender takes once, from the first description with
/// it).
@visibleForTesting
String cameraBitrate(String sdp) => _withParam(
  _withParam(sdp, _camera, 'vp8', 'x-google-max-bitrate=$cameraMaxKbps'),
  _camera,
  'vp8',
  'x-google-start-bitrate=$cameraStartKbps',
);

/// The highest start of the bandwidth estimate taken from a participant's description, in
/// kbit/s: twice what copies ask for ([cameraStartKbps]).
const maxStartKbps = 2 * cameraStartKbps;

/// A participant's description as this device takes it: the format parameters by which
/// it could set this device's sending (`x-google-min-bitrate`, `x-google-start-bitrate`)
/// kept within what copies ask for. No copy asks for a lowest bitrate; given one, WebRTC
/// keeps its bandwidth estimate of that connection above it however congested this
/// device's network gets, and every voice it sends suffers. A start above
/// [maxStartKbps] starts at that (one that is not a number goes, as WebRTC would ignore
/// it). The highest bitrate stays as asked: it only limits what reaches that side, and
/// [_tune]'s limits of each picture hold anyway. A line without such parameters, as
/// every copy sends, stays as it is.
@visibleForTesting
String peerBitrates(String sdp) {
  final lines = sdp.split('\r\n');
  final out = <String>[];
  for (final line in lines) {
    final fmtp = RegExp(r'^(a=fmtp:\d+ )(.*)$').firstMatch(line);
    if (fmtp == null) {
      out.add(line);
      continue;
    }
    final kept = <String>[];
    var changed = false;
    for (final param in fmtp.group(2)!.split(';')) {
      final name = param.split('=').first.trim().toLowerCase();
      if (name == 'x-google-min-bitrate') {
        changed = true;
        continue;
      }
      if (name == 'x-google-start-bitrate') {
        final kbps = int.tryParse(param.split('=').skip(1).join('=').trim());
        if (kbps == null || kbps > maxStartKbps) {
          changed = true;
          if (kbps != null) kept.add('x-google-start-bitrate=$maxStartKbps');
          continue;
        }
      }
      kept.add(param);
    }
    if (!changed) {
      out.add(line);
    } else if (kept.isNotEmpty) {
      out.add('${fmtp.group(1)}${kept.join(';')}');
    }
  }
  return out.join('\r\n');
}

/// Lets the other side's voice send next to nothing while it is silent (Opus DTX): in a
/// large room most people say nothing most of the time.
@visibleForTesting
String voiceDtx(String sdp) => _withParam(sdp, _audio, 'opus', 'usedtx=1');

/// Has a shared screen's sound go in two channels, in the section with [mid] (the fourth
/// channel of a call's connection, or the one section of a screen's sound connection):
/// `stereo=1` says this side takes two channels, `sprop-stereo=1` that it may send them.
/// WebRTC encodes two channels only towards a side whose own description says
/// `stereo=1`, so an older copy, which never says it, is sent one, mixed down by the
/// sender's WebRTC; and an older copy takes this side's description as before (its
/// checks read no format parameters), sending two channels where it captures them. The
/// voice stays in one channel.
@visibleForTesting
String screenSoundStereo(String sdp, int mid) {
  if (!screenSoundInStereo) return sdp;
  return _withParam(
    _withParam(sdp, mid, 'opus', 'stereo=1'),
    mid,
    'opus',
    'sprop-stereo=1',
  );
}

/// Whether a shared screen's sound is asked for in two channels ([screenSoundStereo]);
/// tests turn it off, to compare what the sharer's microphone goes through either way
/// (tools/call-lab/mic.sh echo with ECHO_MONO=1).
@visibleForTesting
bool screenSoundInStereo = true;

/// Adds `param` (`name=value`) to the format parameters of every `codec` type in the
/// section with `mid`, unless they have one with that name already.
String _withParam(String sdp, int mid, String codec, String param) {
  final name = param.split('=').first;
  final lines = sdp.split('\r\n');
  final out = <String>[];
  var i = 0;
  while (i < lines.length) {
    if (!lines[i].startsWith('m=')) {
      out.add(lines[i++]);
      continue;
    }
    var end = i + 1;
    while (end < lines.length && !lines[end].startsWith('m=')) {
      end++;
    }
    final section = lines.sublist(i, end);
    i = end;
    if (!section.contains('a=mid:$mid')) {
      out.addAll(section);
      continue;
    }
    final types = {
      for (final line in section)
        if (RegExp(r'^a=rtpmap:(\d+) ([^/]+)/').firstMatch(line) case final m?
            when m.group(2)!.toLowerCase() == codec)
          m.group(1)!,
    };
    for (final line in section) {
      final fmtp = RegExp(r'^a=fmtp:(\d+) (.*)$').firstMatch(line);
      if (fmtp != null && types.contains(fmtp.group(1))) {
        // By its whole name: `stereo` is not `sprop-stereo`.
        final has = fmtp
            .group(2)!
            .split(';')
            .any((p) => p.trim().split('=').first == name);
        out.add(has ? line : '$line;$param');
        types.remove(fmtp.group(1));
        continue;
      }
      out.add(line);
    }
    // A codec without parameters gets them right after its rtpmap line.
    for (final type in types) {
      final at = out.lastIndexWhere((l) => l.startsWith('a=rtpmap:$type '));
      if (at >= 0) out.insert(at + 1, 'a=fmtp:$type $param');
    }
  }
  return out.join('\r\n');
}

/// Longest description of a call taken from a participant, and most media sections in it: a
/// call with a camera, a screen and its sound takes a few kilobytes and four sections.
const maxSdpBytes = 64 * 1024;
const maxSdpSections = 4;

/// Whether a signal's description, if it has one, is one a participant would send: a
/// larger one would only make this device work on it, and the call's own offers and
/// answers have its channels and no others ([callSections]).
@visibleForTesting
bool fairSignal(Map<String, dynamic> message) {
  // The sender's call (see Call's `_callId`): 16 hexadecimal digits from this version on.
  final call = message['c'];
  if (call != null && (call is! String || call.length > maxCallIdChars)) {
    return false;
  }
  if (message['t'] == 'ice') return _fairCandidate(message);
  final sdp = message['sdp'];
  if (sdp == null) return true;
  if (sdp is! String || sdp.length > maxSdpBytes) return false;
  final t = message['t'];
  if (t == 'offer' || t == 'answer') return callSections(sdp);
  return '\nm='.allMatches(sdp).length + (sdp.startsWith('m=') ? 1 : 0) <=
      maxSdpSections;
}

/// Longest call id taken in a signal.
const maxCallIdChars = 32;

/// Longest candidate taken from a participant: one names an address, a port and a few
/// words about them, some 200 bytes. This device sends its candidates inside its
/// descriptions; only older copies send them one by one.
const maxCandidateBytes = 512;

/// A candidate sent alone: its line no longer than [maxCandidateBytes], and the channel
/// it is for named as the call names them (its number, and its place among at most
/// [maxSdpSections]).
bool _fairCandidate(Map<String, dynamic> message) {
  final candidate = message['candidate'];
  final mid = message['mid'];
  final index = message['index'];
  return (candidate == null ||
          (candidate is String && candidate.length <= maxCandidateBytes)) &&
      (mid == null || (mid is String && mid.length <= 4)) &&
      (index == null || (index is int && index >= 0 && index < maxSdpSections));
}

/// Whether a description of a call has the call's channels, in order, and no others: at
/// most [maxSdpSections], each of its channel's kind and named by its number. WebRTC
/// takes one channel for each section of an offer, and decodes what comes on each of
/// them; the application draws only the camera and the screen.
@visibleForTesting
bool callSections(String sdp) {
  const kinds = ['audio', 'video', 'video', 'audio'];
  var sections = 0;
  String? mid;
  for (final line in _sdpLines(sdp)) {
    if (line.startsWith('m=')) {
      if (sections > 0 && mid != '${sections - 1}') return false;
      if (sections == kinds.length) return false;
      if (!line.startsWith('m=${kinds[sections]} ')) return false;
      sections++;
      mid = null;
    } else if (line.startsWith('a=mid:') && sections > 0) {
      mid = line.substring(6);
    }
  }
  return sections == 0 || mid == '${sections - 1}';
}

/// Whether every sound section of a description offers Opus and every video section VP8,
/// each among the payload types its `m=` line lists (WebRTC ignores an `a=rtpmap` line for
/// any other number, and takes a listed static type, such as 0 for PCMU or 9 for G.722,
/// without one), and lists nothing else: in a sound section only Opus and RED over Opus,
/// in a video section only VP8 and its retransmission codec (RTX), which is all a copy
/// offers or answers ([onlyCallCodecs]). The answer to an offer takes the codecs it lists,
/// and this device would then decode whatever codec its sender chose among the ones
/// WebRTC has (H.264, VP9, AV1, G.722, iLBC...). A rejected section (port 0) carries
/// nothing and needs neither.
@visibleForTesting
bool callCodecs(String sdp) {
  for (final section in _sectionsOf(sdp)) {
    final wanted = switch (section.first.split(' ').first) {
      'm=audio' => 'opus',
      'm=video' => 'vp8',
      _ => null,
    };
    if (wanted == null || _rejected(section)) continue;
    if (!_oneMeaning(section)) return false;
    final call = _codecNames(section, call: true);
    final main = {
      for (final e in call.entries)
        if (e.value == wanted) e.key,
    };
    if (main.isEmpty) return false;
    final names = _codecNames(section);
    for (final type in _formats(section.first)) {
      final fits =
          main.contains(type) ||
          switch (names[type]) {
            'red' =>
              wanted == 'opus' && _redOver(section, names, main).contains(type),
            'rtx' => wanted == 'vp8' && _repeats(section, type, main),
            _ => false,
          };
      if (!fits) return false;
    }
  }
  return true;
}

/// Whether RTX payload type [type] of a section repeats one of [main]: its `a=fmtp` line
/// names one of them (`a=fmtp:97 apt=96`), and no other line gives it another.
bool _repeats(List<String> section, int type, Set<int> main) {
  final apt = [
    for (final line in section)
      if (RegExp(r'^a=fmtp:(\d+) apt=(\d+)$').firstMatch(line) case final m?
          when _number(m.group(1)) == type)
        _number(m.group(2)),
  ];
  return apt.length == 1 && main.contains(apt.single);
}

/// Whether each payload type of a section is a number, listed once and named by one
/// `a=rtpmap` line at most, as numbers: `111` and `0111` are one payload type to WebRTC,
/// so what a payload type stands for is not left to whichever reading wins.
bool _oneMeaning(List<String> section) {
  final words = section.first.split(' ').skip(3).toList();
  final listed = words.map(_number).toList();
  if (listed.contains(null) || listed.toSet().length != listed.length) {
    return false;
  }
  final named = <int>{};
  for (final line in section.skip(1)) {
    final type = RegExp(r'^a=rtpmap:(\S*)').firstMatch(line)?.group(1);
    if (type == null) continue;
    final number = _number(type);
    if (number == null || !named.add(number)) return false;
  }
  return true;
}

/// The media sections of a description, each from its `m=` line on.
List<List<String>> _sectionsOf(String sdp) {
  final sections = <List<String>>[];
  for (final line in _sdpLines(sdp)) {
    if (line.startsWith('m=')) {
      sections.add([line]);
    } else if (sections.isNotEmpty) {
      sections.last.add(line);
    }
  }
  return sections;
}

/// Whether a description for the connection of a shared screen's sound, offer or answer,
/// is one a participant would send: one sound section, in Opus ([callCodecs]). That
/// connection carries nothing else.
@visibleForTesting
bool soundDescription(String sdp) {
  final sections = _sectionsOf(sdp);
  return sections.length == 1 &&
      sections.single.first.startsWith('m=audio ') &&
      !_rejected(sections.single) &&
      callCodecs(sdp);
}
