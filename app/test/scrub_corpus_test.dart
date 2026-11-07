// What a problem report's log loses and keeps, line by line, over a corpus of tricky
// paths and names: folders of every system, names in other alphabets, written
// backwards, with hidden characters or look-alike letters, very long, or the same as
// common words.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/report.dart';

const _peer = '12D3KooWGRUVh7MLpAFtJ6tXc1oFmDy2X1WbSmN4QZbWq9VtXk3A';
const _otherPeer = '12D3KooWD3eckifWpRn9wQpMG9R9hX3sD158z7EqHWmweQAJU5SA';
const _device =
    'b1946ac92492d2347c6235b4d2611184e2b6a1bda1c0e2c8a1f9d5c3b4a2e1f0';

/// A name of 120 characters, as a group may have.
final _longName = 'The hilltop crew ${'forever and ' * 10}beyond';

/// The names of people, groups and rooms known here.
final _names = [
  'Marta Ruiz',
  'Leo',
  'Sofía',
  'Will',
  'Rose',
  'Al',
  'Campfire Crew 🔥',
  'محمد',
  'Дмитрий',
  '李雷',
  'Ana-María',
  // Shown as "marta": the override turns the letters around.
  '\u202Eatram',
  // Hidden characters between the letters.
  'Lu\u200Bcas',
  'mruiz',
  'Martas-Arch',
  _longName,
  'Nguyễn Văn An',
  'Jürgen',
  'Straße',
  'Æsa',
  'Þór',
  '김민수',
];

/// This computer's folders: the data folder of the Flatpak, the homes of each system and
/// a Mac's temporary folder.
const _paths = [
  '/home/marta/.var/app/dev.holypanda.fogata/data/fogata',
  '/home/marta',
  '/Users/Marta Ruiz',
  r'C:\Users\Marta Ruiz',
  '/var/folders/x1/abc123def456/T',
];

/// A case of the corpus: a line, what must not be left of it, and what must.
typedef _Case = ({String line, List<String> gone, List<String> kept});

_Case _c(
  String line, {
  List<String> gone = const [],
  List<String> kept = const [],
}) => (line: line, gone: gone, kept: kept);

final _corpus = <String, _Case>{
  // ----- Paths -----
  'a Linux home folder': _c(
    'WARN fogata_core: cannot read /home/marta/.local/share/fogata/groups/x.json: denied',
    gone: ['marta', '.local', 'x.json'],
    kept: ['WARN fogata_core: cannot read ‹path›: denied'],
  ),
  'the data folder of the Flatpak': _c(
    'INFO fogata_ffi: log in /home/marta/.var/app/dev.holypanda.fogata/data/fogata/diagnostics, 3 files',
    gone: ['marta', 'diagnostics'],
    kept: ['log in ‹path›, 3 files'],
  ),
  'a home not known here, with spaces, accents and emoji': _c(
    'WARN x: cannot open /home/josé luis/Music/🎉 final party.mp3: busy',
    gone: ['josé', 'luis', 'Music', 'party', '🎉'],
    kept: ['cannot open ‹path›: busy'],
  ),
  'a Mac container, its folder with a space': _c(
    'ERROR fogata_core: /Users/Marta Ruiz/Library/Containers/dev.holypanda.fogata/Data/Library/Application Support/fogata/core.db failed',
    gone: ['Marta', 'Ruiz', 'core.db'],
    kept: ['ERROR fogata_core: ‹path› failed'],
  ),
  'a Mac home not known here': _c(
    'INFO x: cache at /Users/kenji.tanaka/Library/Caches/fogata ok',
    gone: ['kenji', 'tanaka'],
    kept: ['cache at ‹path› ok'],
  ),
  "a Mac's temporary folder, as written and as resolved": _c(
    'shared /var/folders/x1/abc123def456/T/fogata-share-9/photo.png and '
    '/private/var/folders/x1/abc123def456/T/clip 2.png',
    gone: ['abc123def456', 'photo', 'clip'],
    kept: ['shared ‹path› and ‹path›'],
  ),
  'a Windows home with a space': _c(
    r'WARN fogata_core: C:\Users\Marta Ruiz\AppData\Roaming\fogata\fogata.db locked',
    gone: ['Marta', 'Ruiz', 'Roaming'],
    kept: ['WARN fogata_core: ‹path› locked'],
  ),
  'a Windows path as a debug print writes it': _c(
    r'at C:\\Users\\Ömer Faruk\\Desktop\\Rapor son.docx now',
    gone: ['Ömer', 'Faruk', 'Rapor', 'Desktop'],
    kept: ['at ‹path› now'],
  ),
  'a Windows path with forward slashes': _c(
    'saved C:/Users/marta/Downloads/a b.pdf done',
    gone: ['marta', 'Downloads', 'a b.pdf'],
    kept: ['saved ‹path› done'],
  ),
  'a shared folder of the network': _c(
    r'WARN x: \\NAS-MARTA\photos\2024\summer at the beach.jpg missing',
    gone: ['NAS-MARTA', 'photos', 'summer', 'beach'],
    kept: ['WARN x: ‹path› missing'],
  ),
  'a long path of Windows': _c(
    r'open \\?\C:\Users\Marta\very long folder\x.txt failed',
    gone: ['Marta', 'very long', 'x.txt'],
    kept: ['open ‹path› failed'],
  ),
  'a Windows temporary folder with a short name': _c(
    r'copy C:\Users\MARTA~1\AppData\Local\Temp\fogata-1\clip.png gone',
    gone: ['MARTA', 'clip'],
    kept: ['copy ‹path› gone'],
  ),
  'a very long path': _c(
    'ERROR x: /home/marta/${'folder/' * 60}x.txt: too long',
    gone: ['marta', 'folder'],
    kept: ['ERROR x: ‹path›: too long'],
  ),
  'a path from the home folder': _c(
    "kept ~/Documents/Marta's notes.txt",
    gone: ['Documents', 'notes', 'Marta'],
    kept: ['kept ‹path›'],
  ),
  'a file at the end of a sentence': _c(
    'INFO x: sent beach-photo.jpg.',
    gone: ['beach', 'photo'],
    kept: ['sent ‹file›.'],
  ),
  'a file sent, with spaces in its name': _c(
    'INFO fogata_core::direct: sent Final report Marta (v2).pdf in 3.1 s',
    gone: ['Final', 'report', 'Marta'],
    kept: ['sent ‹file› in 3.1 s'],
  ),

  // ----- Names -----
  'a name known here': _c(
    'INFO fogata_net: call with Leo failed: timeout',
    gone: ['Leo'],
    kept: ['call with ‹name› failed: timeout'],
  ),
  'a name with a hidden character between its letters': _c(
    'Le\u200Bo joined',
    gone: ['Le', '\u200B'],
    kept: ['‹name› joined'],
  ),
  'a name with hidden characters, as it was saved': _c(
    'Lucas left; Lu\u200Bcas came back',
    gone: ['Lucas', 'Lu'],
    kept: ['‹name› left; ‹name› came back'],
  ),
  'a name written backwards': _c(
    'member \u202Eatram\u202C dialed',
    gone: ['atram', '\u202E', '\u202C'],
    kept: ['member ‹name› dialed'],
  ),
  'a name with a look-alike letter of another alphabet': _c(
    'M\u0430rta joined',
    gone: ['M\u0430rta', 'rta'],
    kept: ['‹name› joined'],
  ),
  'a name not known here, made of two alphabets': _c(
    'friend request from \u0420\u0430yroll Support',
    gone: ['yroll'],
    kept: ['friend request from ‹name› Support'],
  ),
  'a name in mathematical letters': _c(
    '𝐌𝐚𝐫𝐭𝐚 left the room',
    gone: ['𝐌', '𝐚'],
    kept: ['‹name› left the room'],
  ),
  'a name in full-width letters': _c(
    'Ｍａｒｔａ Ｒｕｉｚ joined',
    gone: ['Ｍ', 'Marta', 'Ruiz'],
    kept: ['‹name› joined'],
  ),
  'a name in circled letters': _c(
    'Ⓜⓐⓡⓣⓐ is typing',
    gone: ['Ⓜ', 'ⓐ'],
    kept: ['‹name› is typing'],
  ),
  'a name with its accent as a separate mark': _c(
    'Sofi\u0301a sent a voice note',
    gone: ['Sofi', '\u0301'],
    kept: ['‹name› sent a voice note'],
  ),
  'a name in capitals and without its accent': _c(
    'SOFIA and ana-maria are online',
    gone: ['SOFIA', 'ana-maria'],
    kept: ['‹name› and ‹name› are online'],
  ),
  'a group with an emoji': _c(
    'group Campfire Crew 🔥 synced',
    gone: ['Campfire', '🔥'],
    kept: ['group ‹name› synced'],
  ),
  'a name written right to left': _c(
    'peer محمد online',
    gone: ['محمد'],
    kept: ['peer ‹name› online'],
  ),
  'names in Cyrillic and Chinese': _c(
    'Дмитрий called 李雷',
    gone: ['Дмитрий', '李雷'],
    kept: ['‹name› called ‹name›'],
  ),
  'a very long name': _c(
    'cannot save group $_longName: disk full',
    gone: ['hilltop', 'forever', 'beyond'],
    kept: ['cannot save group ‹name›: disk full'],
  ),
  'names that are also common words': _c(
    'Will joined; will retry in 5 s; latency rose to 80 ms; Rose left; Al and all',
    gone: ['Will joined', 'Rose left', 'Al and'],
    kept: [
      '‹name› joined',
      'will retry in 5 s',
      'latency rose to 80 ms',
      '‹name› left',
      '‹name› and all',
    ],
  ),
  'a name made of blank letters and stacked marks': _c(
    'name \u3164\u3164 and M\u0337\u0322\u031b\u0328a\u0338\u0321r\u0334t\u0335a\u0336 set',
    gone: ['\u3164', '\u0337', 'M'],
    kept: ['name  and ‹name› set'],
  ),
  'colours of a terminal and control characters': _c(
    '\x1B[31mERROR\x1B[0m x: bad\x07 state\r',
    gone: ['\x1B', '[31m', '\x07', '\r'],
    kept: ['ERROR x: bad state'],
  ),
  'a separator of lines inside a message': _c(
    'ok\u2028ERROR fake: line',
    gone: ['\u2028'],
    kept: ['ok ERROR fake: line'],
  ),

  // ----- This computer -----
  "this computer's user and name": _c(
    'connected Martas-Arch as mruiz',
    gone: ['Martas', 'mruiz'],
    kept: ['connected ‹name› as ‹name›'],
  ),
  'names of computers on the network': _c(
    'mdns: Kenjis-MacBook-Pro and DESKTOP-ABC1234 and marta-pc.local announced',
    gone: ['Kenjis', 'MacBook', 'DESKTOP-ABC1234', 'marta-pc'],
    kept: ['mdns: ‹host› and ‹host› and ‹host› announced'],
  ),

  // ----- Addresses -----
  'a multiaddr with a domain': _c(
    'dial /dns4/marta-home.example-dns.org/tcp/443/wss/p2p/$_peer',
    gone: ['marta-home', 'example-dns', '443', _peer],
    kept: ['dial /dns4/‹ip›/tcp/‹port›/wss/p2p/‹peer 1›'],
  ),
  'addresses of each kind': _c(
    'via 192.168.1.20:4001, [2001:db8::42]:9000, fe80::1%en0 and a4:83:e7:12:34:56',
    gone: ['192.168', '2001:db8', 'fe80', 'a4:83', '4001', '9000'],
    kept: ['via ‹ip›, ‹ip›, ‹ip› and ‹ip›'],
  ),
  'links and e-mail addresses': _c(
    'invite fogata://join/AbCd?k=x from marta.ruiz+fogata@mail.example.org',
    gone: ['AbCd', 'marta', 'mail.example'],
    kept: ['invite ‹link› from ‹email›'],
  ),

  // ----- Ids -----
  'one peer whole and short is one number, another peer another': _c(
    'peer=$_peer then …VtXk3A then $_otherPeer',
    gone: [_peer, 'VtXk3A', _otherPeer, 'AJU5SA'],
    kept: ['peer=‹peer 1› then ‹peer 1› then ‹peer 2›'],
  ),
  'a device key whole and short, and a fingerprint grouped and short': _c(
    'device $_device is b1946a… with 7Q4M K2XR 9PMD 3VHC and 7Q4M·K2XR',
    gone: ['b1946a', '7Q4M', 'K2XR', '9PMD'],
    kept: ['device ‹id 1› is ‹id 1› with ‹fingerprint 1› and ‹fingerprint 1›'],
  ),

  // ----- What a developer needs -----
  'an error, its kind and its timings': _c(
    'ERROR fogata_net::node: dial failed: Timeout after 5.2s (attempt 3/5), os error 111',
    kept: [
      'ERROR fogata_net::node: dial failed: Timeout after 5.2s (attempt 3/5), os error 111',
    ],
  ),
  'versions and the system': _c(
    'INFO fogata_core::updates: build 3 (0.1.0-beta.3), linux-flatpak, macOS 15.1, webrtc m125',
    kept: ['build 3 (0.1.0-beta.3), linux-flatpak, macOS 15.1, webrtc m125'],
  ),
  'words of the code': _c(
    'WARN libp2p_quic: Ed25519PublicKey in QuicTransport: ConnectionRefused at 12:30:45',
    kept: ['Ed25519PublicKey in QuicTransport: ConnectionRefused at 12:30:45'],
  ),
  "a system's error in its own language": _c(
    'ERROR app: FileSystemException: ファイルが見つかりません。 Не удается найти указанный файл. Accès refusé',
    kept: ['ファイルが見つかりません', 'Не удается найти указанный файл', 'Accès refusé'],
  ),
  'durations in microseconds': _c(
    'frame took 120µs and 3 ms',
    kept: ['frame took 120µs and 3 ms'],
  ),

  // ----- Names written another way -----
  'a name with its accents as separate marks (NFD)': _c(
    'Nguyễn Văn An joined',
    gone: ['Nguy', 'Văn', '̂'],
    kept: ['‹name› joined'],
  ),
  'a name without its accents': _c(
    'Nguyen Van An and NGUYỄN VĂN AN joined',
    gone: ['Nguyen', 'NGUY', 'Van'],
    kept: ['‹name› and ‹name› joined'],
  ),
  'Korean written as separate jamo, as a Mac names files': _c(
    '김민수 joined',
    gone: ['ᄀ', 'ᅵ'],
    kept: ['‹name› joined'],
  ),
  'names spelled out without their letters': _c(
    'Strasse, STRASSE, STRAẞE, Aesa, ÆSA, Thor, ÞÓR, Juergen and Jurgen',
    gone: [
      'Strasse',
      'STRA',
      'Aesa',
      'ÆSA',
      'Thor',
      'ÞÓR',
      'Juergen',
      'Jurgen',
    ],
    kept: [
      '‹name›, ‹name›, ‹name›, ‹name›, ‹name›, ‹name›, ‹name›, ‹name› and ‹name›',
    ],
  ),
  'the words of a name apart in other ways, or together': _c(
    'Marta  Ruiz, Marta\tRuiz, Marta-Ruiz, marta_ruiz and MartaRuiz left',
    gone: ['Marta', 'marta', 'Ruiz', 'ruiz'],
    kept: ['‹name›, ‹name›, ‹name›, ‹name› and ‹name› left'],
  ),
  'a name with digits after it': _c(
    'Marta Ruiz2 and leo99 joined',
    gone: ['Marta', 'Ruiz', 'leo'],
    kept: ['‹name› and ‹name› joined'],
  ),

  // ----- Addresses and ids of other kinds -----
  'e-mail addresses with letters of other languages, or no domain ending': _c(
    'mail josé@post.es, info@peña.es and marta@mailhost failed',
    gone: ['josé', 'peña', 'mailhost', 'post.es'],
    kept: ['mail ‹email›, ‹email› and ‹email› failed'],
  ),
  'domains without a link': _c(
    'resolve relay.myhost.com and www.ruiz-home.es failed; via relay.fogata.app ok',
    gone: ['myhost', 'ruiz-home'],
    kept: ['resolve ‹host› and ‹host› failed; via relay.fogata.app ok'],
  ),
  'a host with a port, an address in brackets with one': _c(
    'connect home-server.net:8443 and [::1]:9000',
    gone: ['home-server', '8443', '9000'],
    kept: ['connect ‹host›:‹port› and ‹ip›'],
  ),
  'UUIDs, each one number': _c(
    'room 123e4567-e89b-12d3-a456-426614174000 and 123E4567-E89B-12D3-A456-426614174000',
    gone: ['123e4567', 'e89b', '426614174000'],
    kept: ['room ‹id 1› and ‹id 1›'],
  ),
  'hex without digits in its first six': _c(
    'device abcdef… then abcdef1234567890ab',
    gone: ['abcdef'],
    kept: ['device ‹id 1› then ‹id 1›'],
  ),

  // ----- What a developer needs, again -----
  'where a panic of Rust happened in a library': _c(
    "thread 'tokio-runtime-worker' panicked at /home/runner/.cargo/registry/src/index.crates.io-6f17d22bba15001f/libp2p-swarm-0.45.1/src/lib.rs:1234:5:",
    gone: ['runner', '6f17d22bba15001f'],
    kept: [
      "thread 'tokio-runtime-worker' panicked at ‹cargo›/libp2p-swarm-0.45.1/src/lib.rs:1234:5:",
    ],
  ),
  'where a panic of Rust happened in Rust itself': _c(
    'panicked at /rustc/90b35a6239c3d8bdabc530a6a0816f7ff89a0aaf/library/core/src/option.rs:931:5',
    kept: [
      'panicked at /rustc/90b35a6239c3d8bdabc530a6a0816f7ff89a0aaf/library/core/src/option.rs:931:5',
    ],
  ),
  'where a panic happened in Fogata, and a Dart frame': _c(
    'panicked at crates/fogata-core/src/net.rs:120:9 in ReportPage._prepare (package:fogata/src/screens/report.dart:653:20)',
    kept: [
      'panicked at crates/fogata-core/src/net.rs:120:9 in ReportPage._prepare (package:fogata/src/screens/report.dart:653:20)',
    ],
  ),
  'names of types and threads in quotes': _c(
    "Instance of 'FfiException' in thread 'tokio-runtime-worker'",
    kept: ["Instance of 'FfiException' in thread 'tokio-runtime-worker'"],
  ),
  'numbers after a word and a colon are no ports': _c(
    'HTTP status:404 after timeout:30 retries:10',
    kept: ['HTTP status:404 after timeout:30 retries:10'],
  ),
  'measures in Greek letters': _c(
    'took 120μs; Δt=5ms; λ=0.5',
    kept: ['took 120μs; Δt=5ms; λ=0.5'],
  ),
  'a version of four numbers, and long numbers': _c(
    'version 1.2.3.4 build 20261105123456789 seq 1730800000000000000',
    kept: ['version 1.2.3.4 build 20261105123456789 seq 1730800000000000000'],
  ),
  'names of standards with dashes': _c(
    'ISO-8859-1 and AES-256-GCM and RFC-9000',
    kept: ['ISO-8859-1 and AES-256-GCM and RFC-9000'],
  ),
  "the edition of Windows in the system's line": _c(
    'windows, "Windows 11 Pro" 10.0 (Build 26100)',
    kept: ['windows, "Windows 11 Pro" 10.0 (Build 26100)'],
  ),
  'numbers with units and words of the code with dots': _c(
    'took 1.5ms in State.build and Level.info',
    kept: ['took 1.5ms in State.build and Level.info'],
  ),
};

void main() {
  for (final MapEntry(key: what, value: c) in _corpus.entries) {
    test(what, () {
      // A scrubber of its own, as each report has: its numbers start at 1.
      final out = Scrubber(_names, paths: _paths).scrub(c.line);
      for (final gone in c.gone) {
        expect(out, isNot(contains(gone)), reason: '"$gone" is left in:\n$out');
      }
      for (final kept in c.kept) {
        expect(out, contains(kept), reason: '"$kept" is not in:\n$out');
      }
      // One line stays one line.
      expect('\n'.allMatches(out).length, '\n'.allMatches(c.line).length);
    });
  }

  test(
    'nothing identifying is left of the whole corpus, scrubbed as one log',
    () {
      final log = _corpus.values.map((c) => c.line).join('\n');
      final out = Scrubber(_names, paths: _paths).scrub(log);
      for (final gone in [
        'marta',
        'Marta',
        'Ruiz',
        'josé',
        'kenji',
        'Ömer',
        'Leo',
        'Sofía',
        'Campfire',
        'محمد',
        'Дмитрий',
        '李雷',
        'mruiz',
        'example-dns',
        '192.168',
        _peer,
        _device,
        '\u202E',
        '\u200B',
        '\x1B',
      ]) {
        expect(out, isNot(contains(gone)), reason: '"$gone" is left in:\n$out');
      }
      expect(
        '\n'.allMatches(out).length,
        '\n'.allMatches(log).length,
        reason: 'each line of the log stays a line',
      );
    },
  );

  test('a line too long is cut, and the next one stays', () {
    final out = Scrubber(const [])
        .scrub('${'x' * 10000}\n2026-10-30T09:12:01.120Z INFO a: next');
    final lines = out.split('\n');
    expect(lines.first, '${'x' * Scrubber.longestLine} …‹cut›');
    expect(lines.last, '2026-10-30T09:12:01.120Z INFO a: next');
  });

  test('lines written to the disk keep the short ids the core writes', () {
    final out = Scrubber(
      const [],
      shortIds: true,
    ).scrub('peer $_peer device $_device fingerprint 7Q4MK2XR9PMD3VHC');
    expect(out, 'peer …VtXk3A device b1946a… fingerprint 7Q4M·K2XR');
  });

  test("this computer's folders and user are taken out of a report", () {
    final env = Platform.environment;
    final home = env['HOME'] ?? env['USERPROFILE']!;
    final temp = Directory.systemTemp.path;
    final data = '$home${Platform.pathSeparator}fogata data';
    final out = Scrubber.here(
      const [],
      dataDir: data,
    ).scrub('read $data/x.db, $home/y and $temp/z');
    expect(out, 'read ‹path›, ‹path› and ‹path›');
    final user = home.split(RegExp(r'[\\/]')).last;
    if (user.length >= 2) {
      expect(
        Scrubber.here(const [], dataDir: data).scrub('as $user now'),
        'as ‹name› now',
      );
    }
  });

  test('the system says its version, not how its kernel was built', () {
    expect(
      systemVersion(
        'Linux 6.17.7-arch1-1 #1 SMP PREEMPT_DYNAMIC Sun, 02 Nov 2025 10:00:00 +0000',
      ),
      'Linux 6.17.7-arch1-1',
    );
    expect(
      systemVersion('Version 15.1 (Build 24B83)'),
      'Version 15.1 (Build 24B83)',
    );
    expect(
      systemVersion('"Windows 10 Pro" 10.0 (Build 19045)'),
      '"Windows 10 Pro" 10.0 (Build 19045)',
    );
  });
}
