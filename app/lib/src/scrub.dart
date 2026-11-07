import 'dart:io';
import 'dart:math' as math;

/// Takes out of a log what is personal, for the log a problem report carries. This is the
/// one place a report is scrubbed; the core scrubs its lines once before they reach the
/// disk (crates/fogata-ffi/src/logs/scrub.rs), and a report goes through here again with
/// what only the window knows: the names of people and groups, this computer's user and
/// name, and its folders.
///
/// A log is scrubbed line by line, each cut at [longestLine]: no rule looks past its line,
/// and none takes more than a moment on a line however it was made.
///
/// What goes, in this order:
/// - Tricks of Unicode: marks that turn text around or hide it (bidi overrides, zero-width
///   characters, soft hyphens, tags), control characters, colours of a terminal, unusual
///   spaces, stacks of combining marks; full-width letters become plain ones, and Korean
///   written as separate jamo (as a Mac names files) is put together.
/// - The folders of this computer (the data folder, home and temporary folders), however
///   their separators are written.
/// - Text in quotes, links and e-mail addresses.
/// - The names known here, also written with look-alike letters of other alphabets,
///   accents or marks (or without them, or spelled out: `ü` as `ue`, `ß` as `ss`), hidden
///   characters between their letters, other spaces, hyphens or nothing between their
///   words, or digits after them. A name that is also a common word ("Will", "Rose") goes
///   only as it is written, so the word stays.
/// - What the core says with a name in it, network addresses and ports, host names (of a
///   local network, of computers, and domains), paths (with spaces in them too) and file
///   names.
/// - Ids: peers, device keys, UUIDs and other long hex, and fingerprints become a number
///   of this report (`‹peer 1›`, `‹id 2›`, `‹fingerprint 1›`): the steps of one device can
///   still be followed, and nothing in the report ties them to a device outside it. Codes
///   and keys go whole.
/// - Words that mix alphabets or use letters made to look like others (`Mаrta` with a
///   Cyrillic `а`, `𝐌𝐚𝐫𝐭𝐚`, `Ⓜarta`) or heap marks on them, which are names in
///   disguise.
///
/// What stays: the time and level of each line, the parts of the code, the words of the
/// messages, error kinds, versions, numbers and durations, where a panic of Rust happened
/// in a library or in Rust itself, and the names Dart and Rust give to types and threads.
class Scrubber {
  /// [names] are the names of people, groups and rooms known here, [paths] the folders of
  /// this computer. With [shortIds], ids keep the short forms the core writes instead of
  /// numbers, for lines written to the disk, which a report numbers later.
  Scrubber(
    Iterable<String> names, {
    Iterable<String> paths = const [],
    this.shortIds = false,
  }) : _names = _Names(names),
       _paths = _pathsRule(paths);

  /// A scrubber for this computer: [names] and its user, its name and its folders, the
  /// data folder [dataDir] among them. It asks the system for them: not on the window's
  /// thread.
  factory Scrubber.here(
    Iterable<String> names, {
    required String dataDir,
    bool shortIds = false,
  }) => Scrubber(
    [...names, ...machineNames()],
    paths: machinePaths(dataDir),
    shortIds: shortIds,
  );

  /// The longest line scrubbed, in UTF-16 code units; the rest of a longer one is cut.
  static const longestLine = 4096;

  final bool shortIds;
  final _Names _names;
  final RegExp? _paths;

  /// The number each id has in this report, by kind and by the part of the id kept.
  final _ids = <String, Map<String, int>>{};

  /// [log] without what is personal, line by line.
  String scrub(String log) {
    final out = StringBuffer();
    var first = true;
    for (final line in log.split('\n')) {
      if (!first) out.write('\n');
      first = false;
      out.write(_line(line));
    }
    return out.toString();
  }

  /// One line of a log.
  String line(String line) => scrub(line);

  String _line(String line) {
    var s = normalise(line);
    if (s.length > longestLine) {
      var end = longestLine;
      // Not between the two halves of a character.
      if (_isLowSurrogate(s.codeUnitAt(end))) end--;
      s = '${s.substring(0, end)} …‹cut›';
    }
    if (s.isEmpty) return s;
    // What must stay as it is waits aside, as marks no rule touches.
    final kept = <String>[];
    String keep(String text) {
      kept.add(text);
      return '\u{E000}${String.fromCharCode(0xE100 + kept.length - 1)}\u{E001}';
    }

    for (final (needs, rule, replace) in _kept) {
      if (_holds(s, needs)) {
        s = s.replaceAllMapped(rule, (m) => keep(replace(m)));
      }
    }
    final paths = _paths;
    if (paths != null) s = s.replaceAll(paths, '‹path›');
    s = _names.replace(_apply(s, _before));
    s = _apply(_files(_apply(s, _after)), _last);
    if (_notAscii.hasMatch(s)) {
      s = s.replaceAllMapped(
        _word,
        (m) => _disguised(m[0]!) ? '‹name›' : m[0]!,
      );
    }
    if (kept.isNotEmpty) {
      s = s.replaceAllMapped(
        _keptMark,
        (m) => kept[m[1]!.codeUnitAt(0) - 0xE100],
      );
    }
    return s;
  }

  /// [s] after each of [rules] whose text it holds.
  static String _apply(String s, List<_Rule> rules) {
    for (final (needs, rule, replace) in rules) {
      if (_holds(s, needs)) s = s.replaceAllMapped(rule, replace);
    }
    return s;
  }

  /// Whether [s] holds what a rule [needs] (anything, when null).
  static bool _holds(String s, Pattern? needs) => switch (needs) {
    null => true,
    _RunOf() => needs.foundIn(s),
    _ => s.contains(needs),
  };

  static bool _isLowSurrogate(int unit) => unit >= 0xDC00 && unit < 0xE000;

  /// Anything but plain ASCII and the marks the core writes in place of what it took out
  /// (`…`, `‹`, `›`, `·`): lines without it need no folding nor a look at their words.
  static final _notAscii = RegExp(r'[^\x00-\x7F…‹›·]');
  static final _keptMark = RegExp('\u{E000}(.)\u{E001}');

  /// [s] with the tricks of Unicode taken out (see the class's text): what is left reads
  /// as it shows, and the rules after see plain letters.
  static String normalise(String s) {
    if (!_odd.hasMatch(s) || !_changes.hasMatch(s)) return s;
    return _composeHangul(
      s
          .replaceAll(_terminal, '')
          .replaceAll(_controls, '')
          .replaceAll(_invisible, '')
          .replaceAll(_stacked, '')
          .replaceAll(_spaces, ' ')
          .replaceAllMapped(
            _fullWidth,
            (m) => String.fromCharCode(m[0]!.codeUnitAt(0) - 0xFEE0),
          ),
    );
  }

  /// Whatever [normalise] may change: anything but printable ASCII, the tab and the marks
  /// the core writes, and, to look for at once, each thing it changes.
  static final _odd = RegExp(r'[^\x20-\x7E\t…‹›·]');
  static final _changes = RegExp(
    r'[\x00-\x08\x0B-\x1F\x7F-\x9F\p{Cf}\u{115F}\u{1160}\u{3164}\u{FFA0}\u{2800}'
    r'\u{E000}-\u{F8FF}\u{00A0}\u{1680}\u{2000}-\u{200A}\u{2028}\u{2029}\u{202F}'
    r'\u{205F}\u{3000}\u{FF01}-\u{FF5E}\u{1100}-\u{1112}]|\p{Mn}{4,}',
    unicode: true,
  );

  /// What a terminal reads as colours and titles.
  static final _terminal = RegExp(
    r'\x1B(?:\[[0-?]*[ -/]*[@-~]|\][^\x07\x1B\n]*(?:\x07|\x1B\\)?)?',
  );

  /// Control characters but the tab and the end of a line.
  static final _controls = RegExp(r'[\x00-\x08\x0B-\x1F\x7F-\x9F]');

  /// Characters that do not show: format characters (bidi marks and overrides, zero-width
  /// spaces and joiners, the soft hyphen, tags), the letters made to be blank, and the
  /// characters for private use, with which the scrubber marks what it keeps aside.
  static final _invisible = RegExp(
    r'[\p{Cf}\u{115F}\u{1160}\u{3164}\u{FFA0}\u{2800}\u{E000}-\u{F8FF}]',
    unicode: true,
  );

  /// Four combining marks and more on one letter: text made to spill over the lines.
  static final _stacked = RegExp(r'\p{Mn}{4,}', unicode: true);

  /// Spaces of other widths, and the separators of lines and paragraphs, which would start
  /// a line of their own where the log has none.
  static final _spaces = RegExp(
    r'[\u{00A0}\u{1680}\u{2000}-\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}]',
    unicode: true,
  );

  static final _fullWidth = RegExp(r'[\u{FF01}-\u{FF5E}]', unicode: true);

  /// [s] with Korean written as separate jamo (a leading consonant, a vowel and maybe a
  /// final consonant, as a Mac writes file names) put together into syllables, by the
  /// arithmetic Unicode gives for them.
  static String _composeHangul(String s) {
    if (!_jamo.hasMatch(s)) return s;
    final out = StringBuffer();
    var i = 0;
    while (i < s.length) {
      final l = s.codeUnitAt(i);
      final v = i + 1 < s.length ? s.codeUnitAt(i + 1) : 0;
      if (l >= 0x1100 && l <= 0x1112 && v >= 0x1161 && v <= 0x1175) {
        final t = i + 2 < s.length ? s.codeUnitAt(i + 2) : 0;
        final hasT = t >= 0x11A8 && t <= 0x11C2;
        out.writeCharCode(
          0xAC00 +
              ((l - 0x1100) * 21 + (v - 0x1161)) * 28 +
              (hasT ? t - 0x11A7 : 0),
        );
        i += hasT ? 3 : 2;
      } else {
        out.writeCharCode(l);
        i++;
      }
    }
    return out.toString();
  }

  static final _jamo = RegExp(r'[ᄀ-ᄒ]');

  static RegExp _r(String pattern, {bool caseSensitive = true}) =>
      RegExp(pattern, caseSensitive: caseSensitive, unicode: true);

  static String Function(Match) _to(String text) =>
      (_) => text;

  /// The folders of this computer, each as written with either slash and with its
  /// backslashes doubled (as a debug print writes them), the longest first; a folder is
  /// taken out only whole, not as the start of a longer name.
  static RegExp? _pathsRule(Iterable<String> paths) {
    final all = <String>{};
    for (final path in paths) {
      final p = path.trim().replaceAll(RegExp(r'[\\/]+$'), '');
      // A folder of one letter, or a drive, is no one's.
      if (!RegExp(r'[^\\/:.~]{2}').hasMatch(p)) continue;
      all.addAll([
        p,
        p.replaceAll('\\', '/'),
        p.replaceAll('/', '\\'),
        p.replaceAll('/', '\\').replaceAll('\\', '\\\\'),
      ]);
    }
    if (all.isEmpty) return null;
    final sorted = all.toList()..sort((a, b) => b.length.compareTo(a.length));
    // The rest of a path that starts with one goes with it.
    return _r(
      '(?<![\\w.\\-])(?:${sorted.map(RegExp.escape).join('|')})'
      '(?=\$|[\\s\\\\/"\'<>,;:)\\]}]|\\.(?:\\s|\$))'
      '(?:$_sep${_parts(_winPart)})?',
      caseSensitive: false,
    );
  }

  /// What stays as written, before anything else is looked at: where a panic of Rust
  /// happened in a library (after the folder of its sources, which goes) or in Rust itself,
  /// the names Dart and Rust give to types and threads in quotes, and the edition of
  /// Windows in the system's line.
  static final _kept = <_Rule>[
    (
      'registry',
      _r(
        r'''(?<![^\s'"(\[])[^\s'"(\[]*?[\\/]registry[\\/]src[\\/][^\\/\s]+[\\/]([^\s:'")\]]+(?::\d+){0,2})''',
      ),
      (m) => '‹cargo›/${m[1]!.replaceAll('\\', '/')}',
    ),
    (
      '/rustc/',
      _r(r'''/rustc/[0-9a-f]{7,40}/[^\s:'")\]]+(?::\d+){0,2}'''),
      (m) => m[0]!,
    ),
    ("'", _r(r"\b(?:Instance of|thread) '[\w\-:.<>, ]{1,80}'"), (m) => m[0]!),
    ('Windows', _r(r'"Windows [^"\n]{1,40}"'), (m) => m[0]!),
  ];

  /// Before the names: text in quotes (whatever a library printed of data, a message
  /// among it), links (invitations carry their secret in them), e-mail addresses and
  /// domains, which may hold a name.
  static final _before = <_Rule>[
    ('"', _r(r'"(?:[^"\\\n]|\\.)*"'), _to('"‹text›"')),
    ('“', _r(r'“[^”\n]*”'), _to('“‹text›”')),
    ('«', _r(r'«[^»\n]*»'), _to('«‹text›»')),
    (
      "'",
      _r(r"(?<![\p{L}\p{N}])'(?=\S)[^'\n]*'(?![\p{L}\p{N}])"),
      _to("'‹text›'"),
    ),
    (
      '://',
      _r(r'(?<![a-zA-Z0-9+.\-])[a-zA-Z][a-zA-Z0-9+.\-]*://\S+'),
      _to('‹link›'),
    ),
    (null, _r(r'\bfogata:\S+', caseSensitive: false), _to('‹link›')),
    (
      '@',
      _r(
        r'(?<![\p{L}\p{N}_.+\-])[\p{L}\p{N}_.+\-]+@\p{L}[\p{L}\p{N}\-]*(?:\.[\p{L}\p{N}\-]+)*',
      ),
      _to('‹email›'),
    ),
    // Domains, before the names known here can be found in them.
    (
      _dotLetters,
      _r(
        r'(?<![\p{L}\p{N}_\-.@/\\:‹])((?:[\p{L}\p{N}][\p{L}\p{N}\-]{0,62}\.){1,8})(\p{L}{2,24})(?![\p{L}\p{N}_\-]|\.[\p{L}\p{N}]|::)',
      ),
      (m) => _isDomain(m[1]!, m[2]!) ? '‹host›' : m[0]!,
    ),
  ];

  late final _after = <_Rule>[
    // What the core says with the name of a person or a group in it.
    (
      'asks to join',
      _r(r'(: |^)(?:(?!: ).){1,200}? asks to join a group'),
      (m) => '${m[1]}‹name› asks to join a group',
    ),
    (
      'joined the group',
      _r(r'joined the group .*$'),
      _to('joined the group ‹name›'),
    ),
    (
      'not loaded:',
      _r(r'\b(group|chat) .{1,200}? (not loaded:)'),
      (m) => '${m[1]} ‹name› ${m[2]}',
    ),
    (
      ' group ',
      _r(r'\b(cannot (?:save|read|run) group) .{1,200}?( again)?:'),
      (m) => '${m[1]} ‹name›${m[2] ?? ''}:',
    ),
    // Network addresses, as multiaddrs and plain, and ports.
    (
      '/',
      _r(r'/(ip4|ip6|dns|dns4|dns6|dnsaddr)/[^/\s,)\]]+'),
      (m) => '/${m[1]}/‹ip›',
    ),
    ('/', _r(r'/(udp|tcp)/\d+'), (m) => '/${m[1]}/‹port›'),
    ('/certhash/', _r(r'/certhash/[^/\s,)\]]+'), _to('/certhash/‹key›')),
    // UUIDs, before the parts of one are taken for an address.
    (
      '-',
      _r(
        r'\b([0-9a-fA-F]{8})-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b',
      ),
      (m) => _id('id', m[1]!.toLowerCase(), '${m[1]}…'),
    ),
    // Four numbers after the word "version" are a version.
    (
      _fourNumbers,
      _r(
        r'(?<![\w.])(?<!\b[Vv]er(?:sion)?:? ?)(?:\d{1,3}\.){3}\d{1,3}(?::\d{1,5})?(?![\w.])',
      ),
      _to('‹ip›'),
    ),
    (
      ':',
      _r(
        r'(?<![\w:‹])\[?((?:[0-9a-fA-F]{0,4}:){2,7}[0-9a-fA-F]{0,4})(?:%\w+)?\]?(?::\d{1,5})?(?![\w:])',
      ),
      (m) => _isIpv6(m[1]!) ? '‹ip›' : m[0]!,
    ),
    (
      null,
      _r(r'\b(port:? ?)\d{1,5}\b', caseSensitive: false),
      (m) => '${m[1]}‹port›',
    ),
    // Host names on a local network and names of computers, often a person's name.
    (
      _localEnding,
      _r(
        r'(?<![\p{L}\p{N}_\-.])[\p{L}\p{N}_\-.]+\.(?:local|lan|home|internal|localdomain|home\.arpa)(?![\p{L}\p{N}])',
        caseSensitive: false,
      ),
      _to('‹host›'),
    ),
    (
      _computer,
      _r(
        r"(?<![\p{L}\p{N}_'’\-])[\p{L}\p{N}_'’\-]*(?:MacBook|iMac|Mac-?mini|Mac-?Studio|iPhone|iPad)[\p{L}\p{N}_\-]*",
      ),
      _to('‹host›'),
    ),
    ('-', _r(r'\b(?:DESKTOP|LAPTOP)-[A-Z0-9]{5,}\b'), _to('‹host›')),
    // Paths of Windows (drives, shared folders, long paths) and of the others, with spaces
    // in their folders' names too.
    (':', _r('(?<![\\w‹›])[A-Za-z]:$_sep${_parts(_winPart)}'), _to('‹path›')),
    (
      '\\\\',
      _r(
        '(?<![\\w\\\\‹›])\\\\{2,}(?:[?.]\\\\+(?:[A-Za-z]:\\\\+)?)?${_parts(_winPart)}',
      ),
      _to('‹path›'),
    ),
    (
      null,
      _r(
        '(?<![\\w/.~‹›\\-])(?:~|/(?!(?:$_protocols)(?:/|\\s|\$))$_unixPart(?: $_unixPart)*?(?=/))'
        '/${_parts(_unixPart, sep: '/')}',
      ),
      _to('‹path›'),
    ),
  ];

  /// After the paths and the file names: ports after a host, then ids.
  late final _last = <_Rule>[
    // A port is a number after a host, an address in brackets or what took their place;
    // not after a file of code (`lib.rs:12`), nor after a word (`status:404`).
    (
      ':',
      _r(r':\d{2,5}\b'),
      (m) => _host.hasMatch(m.input.substring(0, m.start)) ? ':‹port›' : m[0]!,
    ),
    // Ids, whole as a library prints them or in the short form the core writes, become a
    // number of this report.
    (
      const _RunOf(40),
      _r(r'\b(?:12D3KooW|Qm)[1-9A-HJ-NP-Za-km-z]{38,50}\b'),
      (m) {
        final tail = m[0]!.substring(m[0]!.length - 6);
        return _id('peer', tail, '…$tail');
      },
    ),
    (
      '…',
      _r(r'…([1-9A-HJ-NP-Za-km-z]{6})(?![\w])'),
      (m) => _id('peer', m[1]!, m[0]!),
    ),
    (
      const _RunOf(8),
      _r(r'\b0[xX]([0-9a-fA-F]{6})(?:[0-9a-fA-F]{10,}\b|…)'),
      (m) => _id('id', m[1]!.toLowerCase(), '${m[0]!.substring(0, 8)}…'),
    ),
    // Long hex with a letter in it: a long number is a number.
    (
      const _RunOf(16),
      _r(r'\b([0-9a-fA-F]{6})[0-9a-fA-F]{10,}\b'),
      (m) => m[0]!.contains(_hexLetter)
          ? _id('id', m[1]!.toLowerCase(), '${m[1]}…')
          : m[0]!,
    ),
    ('…', _r(r'\b([0-9a-f]{6})…'), (m) => _id('id', m[1]!, m[0]!)),
    (
      null,
      _r(
        r'\b([0-9A-HJKMNP-TV-Z]{4}) ([0-9A-HJKMNP-TV-Z]{4}) [0-9A-HJKMNP-TV-Z]{4} [0-9A-HJKMNP-TV-Z]{4}\b',
      ),
      (m) => _id('fingerprint', '${m[1]}${m[2]}', '${m[1]}·${m[2]}'),
    ),
    (
      const _RunOf(16),
      _r(
        r'\b([0-9A-HJKMNP-TV-Z]{4})([0-9A-HJKMNP-TV-Z]{4})[0-9A-HJKMNP-TV-Z]{8}\b',
      ),
      (m) {
        final f = m[0]!;
        final mixed = f.contains(RegExp(r'\d')) && f.contains(RegExp('[A-Z]'));
        return mixed
            ? _id('fingerprint', '${m[1]}${m[2]}', '${m[1]}·${m[2]}')
            : f;
      },
    ),
    (
      '·',
      _r(r'\b([0-9A-HJKMNP-TV-Z]{4})·([0-9A-HJKMNP-TV-Z]{4})\b'),
      (m) => _id('fingerprint', '${m[1]}${m[2]}', m[0]!),
    ),
    // Codes go whole: groups of capitals and digits with dashes, one of the groups
    // mixing both (`ISO-8859-1` and `RFC-9000` are no codes).
    (
      '-',
      _r(r'\b[0-9A-Z]{3,}(?:-[0-9A-Z]{4,})+\b'),
      (m) => m[0]!.split('-').skip(1).any(_mixed) ? '‹code›' : m[0]!,
    ),
    // A move's code typed without its dash: ten characters of Crockford base32, digits
    // and letters of one case.
    (
      const _RunOf(10),
      _r(r'\b(?:[0-9A-HJKMNP-TV-Z]{10}|[0-9a-hjkmnp-tv-z]{10})\b'),
      (m) {
        final t = m[0]!;
        return t.contains(RegExp(r'\d')) && t.contains(RegExp('[A-Za-z]'))
            ? '‹code›'
            : t;
      },
    ),
    (
      const _RunOf(32),
      _r(r'(?<![A-Za-z0-9+/_\-])[A-Za-z0-9+/_\-]{32,}={0,2}'),
      (m) {
        final t = m[0]!;
        final random =
            t.contains(RegExp(r'\d')) &&
            t.contains(RegExp('[A-Z]')) &&
            t.contains(RegExp('[a-z]'));
        return random ? '‹key›' : t;
      },
    ),
    // 16 bytes in base64: 22 characters, and the padding when there is one.
    (
      const _RunOf(22),
      _r(
        r'(?<![A-Za-z0-9+/_\-])[A-Za-z0-9+/_\-]{22,23}(={1,2})?(?![A-Za-z0-9+/_\-=])',
      ),
      (m) {
        final t = m[0]!.replaceAll('=', '');
        final random =
            t.contains(RegExp(r'\d')) &&
            RegExp('[A-Z]').allMatches(t).length >= 2 &&
            RegExp('[a-z]').allMatches(t).length >= 2 &&
            (m[1] != null || !t.contains('_'));
        return random ? '‹key›' : m[0]!;
      },
    ),
  ];

  static final _hexLetter = RegExp('[a-fA-F]');

  /// What comes before a port: a host, an address in brackets or what took their place;
  /// not a file of code (`lib.rs:12`), nor a word (`status:404`).
  static final _host = RegExp(
    r'(?:\.[A-Za-z]{2,24}|\]|‹host›|\blocalhost)(?<!\.(?:rs|dart|go|py|js|ts|c|h|cc|cpp|kt|swift|java|m|mm))$',
  );

  // What a line must hold for some rules to be tried on it.
  static final _fourNumbers = RegExp(r'\d\.\d{1,3}\.\d{1,3}\.\d');
  static final _localEnding = RegExp(
    r'\.(?:local|lan|home|internal|localdomain)',
    caseSensitive: false,
  );
  static final _computer = RegExp(r'Mac|iP(?:hone|ad)');
  static final _dotLetters = RegExp(r'\.\p{L}{2}', unicode: true);
  static final _letter = RegExp(r'\p{L}', unicode: true);

  static bool _mixed(String group) =>
      group.contains(RegExp(r'\d')) && group.contains(RegExp('[A-Z]'));

  /// The id of [kind] whose kept part is [key], as this report numbers it, or [short] as
  /// the core writes it ([shortIds]).
  String _id(String kind, String key, String short) {
    if (shortIds) return short;
    final seen = _ids.putIfAbsent(kind, () => {});
    final n = seen.putIfAbsent(key, () => seen.length + 1);
    return '‹$kind $n›';
  }

  /// Whether [labels] (each with its dot) and [end] make a domain of the Internet: one
  /// whose last part is a domain's ending, not a word of the code (`State.build`) or the
  /// ending of a file of code (`net.rs`), and not one of Fogata's own or of the services
  /// a log names.
  static bool _isDomain(String labels, String end) {
    final tld = end.toLowerCase();
    final ok = tld.length == 2
        ? !_notCountries.contains(tld)
        : _endings.contains(tld);
    // A number with a unit (`1.5ms`) is no domain.
    if (!ok || !labels.contains(_letter)) return false;
    final domain = '${labels.toLowerCase()}$tld';
    return !_public.any((p) => domain == p || domain.endsWith('.$p'));
  }

  /// Two letters that end a domain but also a file of code or a short word of a message.
  static const _notCountries = {
    'rs',
    'md',
    'sh',
    'py',
    'js',
    'ts',
    'so',
    'cc',
    'hh',
    'mm',
    'pl',
    'pm',
    'rb',
    'cs',
    'fs',
    'ml',
    'go',
    'mk',
    'in',
    'is',
    'it',
    'at',
    'be',
    'me',
    'do',
    'no',
    'us',
    'as',
    'by',
    'on',
    'to',
    'an',
    'am',
    'my',
    'up',
    'we',
    'or',
    'of',
    'if',
    'id',
    'ok',
    'ms',
    'gb',
  };

  /// Endings of domains longer than two letters.
  static const _endings = {
    'com',
    'net',
    'org',
    'edu',
    'gov',
    'mil',
    'int',
    'biz',
    'pro',
    'io',
    'dev',
    'xyz',
    'online',
    'site',
    'tech',
    'cloud',
    'ai',
    'blog',
    'shop',
    'club',
    'website',
    'family',
    'onion',
    'arpa',
    'icu',
    'vip',
    'art',
    'design',
    'studio',
    'social',
    'games',
    'world',
    'cat',
    'eus',
    'gal',
    'moe',
  };

  /// Domains a log names that are no one's: Fogata's own and the services it uses.
  static const _public = {
    'fogata.app',
    'holypanda.dev',
    'crates.io',
    'github.com',
    'githubusercontent.com',
    'ko-fi.com',
    'flutter.dev',
    'dart.dev',
    'rust-lang.org',
  };

  /// What separates the folders of a path on Windows, doubled in a debug print.
  static const _sep = r'[\\/]+';

  /// A folder's name, between separators.
  static const _winPart = r'[^\s\\/"<>,;:|*?)\]]+';
  static const _unixPart = r'[^\s/\\"<>,;:|)\]]+';

  /// The folders of a path after its start, separated by [sep]: a folder's name may hold
  /// spaces when another folder follows it, and the last part when it ends as a file's name.
  static String _parts(String part, {String sep = _sep}) =>
      '(?:$part(?: $part)*?(?=$sep)$sep)*'
      '(?:$part(?: $part){0,4}?\\.(?:$_fileEnds)(?![\\p{L}\\p{N}])|$part)?';

  /// The protocols of a multiaddr, which look like the start of a path.
  static const _protocols =
      'ip4|ip6|dns|dns4|dns6|dnsaddr|udp|tcp|quic|quic-v1|p2p|p2p-circuit|webrtc|'
      'webrtc-direct|certhash|tls|ws|wss|http';

  /// [s] with the names of files (what goes with an ending of [_fileEnds]) as `‹file›`. A
  /// name may hold spaces: the words before its last go too, back to a word of the message
  /// ([_beforeNames]). Found by hand, each run of a name's characters once.
  static String _files(String s) {
    if (!_anyFileEnd.hasMatch(s)) return s;
    final out = StringBuffer();
    var at = 0;
    for (final run in _nameRun.allMatches(s)) {
      final text = run[0]!;
      // The last ending in the run makes the longest name.
      var end = -1;
      for (
        var dot = text.lastIndexOf('.');
        dot > 0;
        dot = text.lastIndexOf('.', dot - 1)
      ) {
        final ext = _extension.matchAsPrefix(text, dot + 1);
        final after = dot + 1 + (ext?[0]!.length ?? 0);
        // An ending followed by more of a domain (`relay.fogata.app`) ends no file.
        if (ext != null &&
            _fileEndSet.contains(ext[0]!.toLowerCase()) &&
            (after + 1 >= text.length || text[after] != '.')) {
          end = run.start + after;
          break;
        }
      }
      if (end < 0) continue;
      // Back over the words before it, each after one space, to a word of the message.
      var start = run.start;
      while (start >= 2 && s[start - 1] == ' ' && start - 1 > at) {
        final word = _lastWord(s, start - 1, at);
        if (word == null || _beforeNames.contains(word.$2.toLowerCase())) break;
        start = word.$1;
      }
      out
        ..write(s.substring(at, start))
        ..write('‹file›');
      at = end;
    }
    out.write(s.substring(at));
    return out.toString();
  }

  /// The word of a name's characters that ends at [end] in [s] and starts after a space
  /// or the start of the line (not before [from]): where it starts, and the word.
  static (int, String)? _lastWord(String s, int end, int from) {
    var start = end;
    while (start > from && _nameChar.hasMatch(s[start - 1])) {
      start--;
    }
    if (start == end) return null;
    if (start > 0 && s[start - 1] != ' ' && s[start - 1] != '\t') return null;
    return (start, s.substring(start, end));
  }

  static final _anyFileEnd = RegExp(
    '\\.(?:$_fileEnds)(?![\\p{L}\\p{N}])',
    caseSensitive: false,
    unicode: true,
  );
  static final _nameRun = RegExp(r'[\p{L}\p{N}_()+\-.]+', unicode: true);
  static final _nameChar = RegExp(r'[\p{L}\p{N}_()+\-.]', unicode: true);
  static final _extension = RegExp(r'[\p{L}\p{N}]+', unicode: true);
  static final _fileEndSet = _fileEnds
      .replaceAll('jpe?g', 'jpg|jpeg')
      .replaceAll('tiff?', 'tif|tiff')
      .replaceAll('mpe?g', 'mpg|mpeg')
      .replaceAll('docx?', 'doc|docx')
      .replaceAll('xlsx?', 'xls|xlsx')
      .replaceAll('pptx?', 'ppt|pptx')
      .replaceAll('html?', 'htm|html')
      .replaceAll('ya?ml', 'yml|yaml')
      .split('|')
      .toSet();

  /// Words a message says before a file's name, where the name cannot go on (as in the
  /// core's scrubber, crates/fogata-ffi/src/logs/scrub.rs).
  static const _beforeNames = {
    'a',
    'an',
    'and',
    'as',
    'at',
    'attach',
    'attached',
    'attachment',
    'big',
    'by',
    'cannot',
    'copied',
    'copy',
    'could',
    'decode',
    'decoding',
    'delete',
    'deleted',
    'download',
    'downloaded',
    'downloading',
    'drop',
    'dropped',
    'error',
    'failed',
    'fetch',
    'fetched',
    'fetching',
    'file',
    'for',
    'from',
    'got',
    'image',
    'in',
    'into',
    'is',
    'kept',
    'large',
    'message',
    'move',
    'moved',
    'named',
    'not',
    'of',
    'on',
    'open',
    'opened',
    'opening',
    'or',
    'photo',
    'picked',
    'picture',
    'read',
    'reading',
    'received',
    'remove',
    'removed',
    'save',
    'saved',
    'send',
    'sending',
    'sent',
    'share',
    'shared',
    'skipped',
    'the',
    'to',
    'too',
    'video',
    'voice',
    'was',
    'with',
    'write',
    'writing',
    'wrote',
  };

  static bool _isIpv6(String s) {
    final colons = ':'.allMatches(s).length;
    // A time of day (12:34:56) is no address.
    if (!s.contains('::') && RegExp(r'^\d{1,2}(:\d{1,2}){1,2}$').hasMatch(s)) {
      return false;
    }
    return s.contains('::') || colons >= 3 || RegExp('[a-fA-F]').hasMatch(s);
  }

  /// A word with a letter that is not plain ASCII, the only words [_disguised] may find.
  static final _word = _r(
    r'(?<![\p{L}\p{M}\p{N}\u{2460}-\u{24FF}\u{1F100}-\u{1F1FF}])'
    r'(?=[\p{L}\p{M}\p{N}\u{2460}-\u{24FF}\u{1F100}-\u{1F1FF}]*[^\x00-\x7F])'
    r'[\p{L}\p{M}\p{N}\u{2460}-\u{24FF}\u{1F100}-\u{1F1FF}]+',
  );

  static final _combining = RegExp(r'\p{Mn}', unicode: true);

  /// Whether [word] mixes alphabets that look alike, has letters only made to look like
  /// others, or marks heaped on its letters: a name in disguise, never a word of the code
  /// or of a system's message.
  static bool _disguised(String word) {
    // Marks heaped on its letters: a name made to spill over the lines around it.
    if (_combining.allMatches(word).length >= 3) return true;
    final scripts = <_Script>{};
    for (final rune in word.runes) {
      final script = _scriptOf(rune);
      if (script == _Script.lookAlike) return true;
      if (script != _Script.other) scripts.add(script);
    }
    return scripts.length > 1;
  }
}

/// What ids, keys and codes are made of, and some rules need a run of: letters and digits
/// of ASCII, and `+`, `/`, `_` and `-`. Looked for by hand, a look at each character of a
/// line, where a rule's expression takes several times as long to find nothing; most
/// lines hold no run long enough for a key, and a key's rules are not tried on them.
class _RunOf implements Pattern {
  const _RunOf(this.length);

  /// The run's least length.
  final int length;

  RegExp get _rule => RegExp('[A-Za-z0-9+/_\\-]{$length}');

  bool foundIn(String s) {
    var run = 0;
    for (var i = 0; i < s.length; i++) {
      final c = s.codeUnitAt(i);
      if ((c >= 0x61 && c <= 0x7A) ||
          (c >= 0x41 && c <= 0x5A) ||
          (c >= 0x2F && c <= 0x39) ||
          c == 0x2B ||
          c == 0x5F ||
          c == 0x2D) {
        if (++run >= length) return true;
      } else {
        run = 0;
      }
    }
    return false;
  }

  @override
  Iterable<Match> allMatches(String string, [int start = 0]) =>
      _rule.allMatches(string, start);

  @override
  Match? matchAsPrefix(String string, [int start = 0]) =>
      _rule.matchAsPrefix(string, start);
}

/// A rule of [Scrubber]: what a line must hold for the rule to be tried there, quick to
/// look for (or null to try it on every line), the rule, and what takes the place of what it finds.
typedef _Rule = (Pattern?, RegExp, String Function(Match));

/// The alphabets whose letters look like each other's, and letters made to look like them.
enum _Script { latin, greek, cyrillic, armenian, cherokee, lookAlike, other }

/// Greek letters that look like no Latin one and that measures are written with
/// (`120μs`, `Δt`, `λ`, `Ω`): they mix with Latin letters in no disguise.
const _measures = 'μΔδλπθΣσΩωφψ';

_Script _scriptOf(int c) {
  bool within(int from, int to) => c >= from && c <= to;
  if (within(0x41, 0x5A) ||
      within(0x61, 0x7A) ||
      (within(0xC0, 0x24F) && c != 0xD7 && c != 0xF7) ||
      within(0x1E00, 0x1EFF) ||
      within(0x2C60, 0x2C7F) ||
      within(0xA720, 0xA7FF)) {
    return _Script.latin;
  }
  if (within(0x370, 0x3FF) || within(0x1F00, 0x1FFF)) {
    return _measures.codeUnits.contains(c) ? _Script.other : _Script.greek;
  }
  if (within(0x400, 0x52F) ||
      within(0x1C80, 0x1C8F) ||
      within(0x2DE0, 0x2DFF) ||
      within(0xA640, 0xA69F)) {
    return _Script.cyrillic;
  }
  if (within(0x530, 0x58F)) return _Script.armenian;
  if (within(0x13A0, 0x13FF) || within(0xAB70, 0xABBF)) {
    return _Script.cherokee;
  }
  // Phonetic letters and small capitals, letter-like symbols, letters in circles and
  // squares, and the mathematical alphabets.
  if (within(0x250, 0x2AF) ||
      within(0x1D00, 0x1DBF) ||
      within(0x2100, 0x214F) ||
      within(0x2460, 0x24FF) ||
      within(0x1D400, 0x1D7FF) ||
      within(0x1F100, 0x1F1FF)) {
    return _Script.lookAlike;
  }
  return _Script.other;
}

/// The names known here and how they are found in a log: each folded ([_fold]), so a
/// name is found with look-alike letters, with accents and marks or without them, or with
/// hidden characters; its words may be apart by other spaces, hyphens or underscores, or
/// together, and digits may follow it.
class _Names {
  _Names(Iterable<String> names) {
    final folded = <String>{};
    for (final name in names) {
      final plain = Scrubber.normalise(name).trim();
      // A name with umlauts is also written with an `e` after the letter instead.
      for (final spelled in {plain, _spelledOut(plain)}) {
        final f = _fold(spelled).text.trim();
        if (f.runes.length < 2) continue;
        folded.add(f);
        // A name that is also a common word goes only as it is written.
        if (_commonWords.contains(f)) {
          (_asWritten[f] ??= {}).add(_withoutMarks(spelled));
        }
      }
    }
    if (folded.isEmpty) return;
    for (final f in folded) {
      final first = f
          .split(_between)
          .firstWhere((w) => w.isNotEmpty, orElse: () => '');
      // A name that starts with something else than a letter or a digit (`'Ana`) starts
      // no word: every line is looked at whole for it.
      if (!_letterOrDigit.hasMatch(first)) {
        _starts = null;
        break;
      }
      _starts?.add(first.substring(0, math.min(_startLength, first.length)));
      _firstUnits.add(first.codeUnitAt(0));
    }
    final sorted = folded.toList()
      ..sort((a, b) => b.length.compareTo(a.length));
    final alternatives = [
      for (final f in sorted)
        f
            .split(_between)
            .where((w) => w.isNotEmpty)
            .map(RegExp.escape)
            .join(r'[\s\-_]*'),
    ];
    _rule = RegExp(
      '(?<![\\p{L}\\p{N}])(?:${alternatives.join('|')})\\d*(?![\\p{L}\\p{N}])',
      unicode: true,
    );
  }

  RegExp? _rule;

  /// The first letters ([_startLength] at most) of the first word of each name, folded:
  /// a line none of whose words starts with one holds no name, and the long rule of all
  /// the names, which takes most of the time a line takes, is not tried on it. Null when
  /// some name starts no word.
  Set<String>? _starts = {};
  static const _startLength = 3;

  /// A letter or a digit, the only characters a name starts with here.
  static final _letterOrDigit = RegExp(r'^[\p{L}\p{N}]', unicode: true);

  /// Whether some word of [text] (folded) may start as a name does. Found by hand, as a
  /// look at each character: only where the character after a plain letter or digit
  /// (which goes on a word, never starts one) begins as some name does.
  bool _mayHoldName(String text) {
    final starts = _starts;
    if (starts == null) return true;
    for (var i = 0; i < text.length; i++) {
      if (!_firstUnits.contains(text.codeUnitAt(i))) continue;
      if (i > 0 && _isAsciiLetterOrDigit(text.codeUnitAt(i - 1))) continue;
      for (var length = 1; length <= _startLength; length++) {
        if (i + length > text.length) break;
        if (starts.contains(text.substring(i, i + length))) return true;
      }
    }
    return false;
  }

  /// The first code unit of each of [_starts].
  final _firstUnits = <int>{};

  static bool _isAsciiLetterOrDigit(int unit) =>
      (unit >= 0x30 && unit <= 0x39) ||
      (unit >= 0x61 && unit <= 0x7A) ||
      (unit >= 0x41 && unit <= 0x5A);

  /// The common words among the names, folded, with the ways each name is written.
  final _asWritten = <String, Set<String>>{};

  static final _between = RegExp(r'[\s\-_]+');
  static final _digitsAtEnd = RegExp(r'\d+$');

  /// [s] with each name known here as `‹name›`.
  String replace(String s) {
    final rule = _rule;
    if (rule == null) return s;
    final f = _fold(s);
    if (!_mayHoldName(f.text)) return s;
    final out = StringBuffer();
    var at = 0;
    for (final m in rule.allMatches(f.text)) {
      final start = f.from(m.start);
      var end = f.to(m.end - 1);
      final written = _asWritten[m[0]!.replaceAll(_digitsAtEnd, '')];
      if (written != null &&
          !written.contains(
            _withoutMarks(s.substring(start, end)).replaceAll(_digitsAtEnd, ''),
          )) {
        continue;
      }
      // The marks that went on the name's last letter go with it.
      while (end < s.length && _mark.hasMatch(s[end])) {
        end++;
      }
      if (start < at) continue;
      out
        ..write(s.substring(at, start))
        ..write('‹name›');
      at = end;
    }
    if (at == 0) return s;
    out.write(s.substring(at));
    return out.toString();
  }
}

/// [name] with its umlauts spelled out as German does when it cannot write them
/// (`Jürgen` as `Juergen`).
String _spelledOut(String name) => name.replaceAllMapped(
  RegExp('[äöüÄÖÜ]'),
  (m) => const {
    'ä': 'ae',
    'ö': 'oe',
    'ü': 'ue',
    'Ä': 'Ae',
    'Ö': 'Oe',
    'Ü': 'Ue',
  }[m[0]]!,
);

/// Combining marks and characters that do not show.
final _mark = RegExp(r'[\p{M}\p{Cf}]', unicode: true);

String _withoutMarks(String s) => s.replaceAll(_mark, '');

/// Text folded to find names in it, with where each of its characters came from.
class _Folded {
  _Folded(this.text, [this._from, this._to]);
  final String text;

  /// For each code unit of [text], where in the text folded its character starts and
  /// ends; none when each is where it was.
  final List<int>? _from, _to;

  int from(int i) => _from?[i] ?? i;
  int to(int i) => _to?[i] ?? i + 1;
}

/// [s] in small letters, without marks, accents or hidden characters, with full-width
/// letters, letters spelled with two (`ß`, `æ`, `þ`) and the letters of Greek and Cyrillic
/// that look like Latin ones as plain Latin letters.
_Folded _fold(String s) {
  // Plain ASCII and the core's marks fold to their small letters, each where it was.
  if (!Scrubber._notAscii.hasMatch(s)) {
    return _Folded(s.toLowerCase());
  }
  final out = StringBuffer();
  final from = <int>[], to = <int>[];
  var i = 0;
  while (i < s.length) {
    var c = s.codeUnitAt(i);
    var n = 1;
    if (c >= 0xD800 && c < 0xDC00 && i + 1 < s.length) {
      final low = s.codeUnitAt(i + 1);
      if (low >= 0xDC00 && low < 0xE000) {
        c = 0x10000 + ((c - 0xD800) << 10) + (low - 0xDC00);
        n = 2;
      }
    }
    final String f;
    if (c < 0x80) {
      f = String.fromCharCode(c >= 0x41 && c <= 0x5A ? c + 32 : c);
    } else if (c >= 0xFF01 && c <= 0xFF5E) {
      f = String.fromCharCode(c - 0xFEE0).toLowerCase();
    } else {
      final char = String.fromCharCode(c);
      f = _mark.hasMatch(char) ? '' : (_folds[c] ?? char.toLowerCase());
    }
    for (final unit in f.codeUnits) {
      out.writeCharCode(unit);
      from.add(i);
      to.add(i + n);
    }
    i += n;
  }
  return _Folded(out.toString(), from, to);
}

/// What letters fold to besides their small letter: the Latin letters with accents (made
/// from Unicode's decompositions of Latin-1, Latin Extended-A and B and Latin Extended
/// Additional), letters written with two, and the letters of Greek and Cyrillic that look
/// like a Latin one.
final _folds = <int, String>{
  for (var i = 0; i < _accented.length; i++)
    _accented.codeUnitAt(i): _unaccented[i],
  for (final MapEntry(key: plain, value: like) in const {
    'a': 'АаΑαɑ',
    'ae': 'ÆæǼǽ',
    'b': 'ВΒЬ',
    'c': 'Ссϲ',
    'd': 'ĐđÐðԁ',
    'e': 'ЕеЁёΕ',
    'g': 'ɡ',
    'h': 'ĦħНΗһ',
    'i': 'ıІіΙιӀӏ',
    'j': 'Јј',
    'k': 'КкΚκ',
    'l': 'ŁłĿŀ',
    'm': 'МΜ',
    'n': 'Ν',
    'o': 'ØøǾǿОоΟο',
    'oe': 'Œœ',
    'p': 'РрΡρ',
    's': 'Ѕѕ',
    'ss': 'ßẞ',
    't': 'ŦŧТΤ',
    'th': 'Þþ',
    'x': 'ХхΧχ',
    'y': 'УуΥ',
    'z': 'Ζ',
  }.entries)
    for (final rune in like.runes) rune: plain,
};

/// Latin letters with accents, and below the letters they fold to, one for one; made with
/// Python's `unicodedata.normalize('NFD', …)` over U+00C0–U+024F and U+1E00–U+1EFF.
const _accented =
    'ÀÁÂÃÄÅÇÈÉÊËÌÍÎÏÑÒÓÔÕÖÙÚÛÜÝàáâãäåçèéêëìíîïñòóôõöùúûüýÿĀāĂăĄąĆćĈĉĊċČčĎďĒēĔĕĖėĘęĚ'
    'ěĜĝĞğĠġĢģĤĥĨĩĪīĬĭĮįİĴĵĶķĹĺĻļĽľŃńŅņŇňŌōŎŏŐőŔŕŖŗŘřŚśŜŝŞşŠšŢţŤťŨũŪūŬŭŮůŰűŲųŴŵŶŷŸŹ'
    'źŻżŽžƠơƯưǍǎǏǐǑǒǓǔǕǖǗǘǙǚǛǜǞǟǠǡǦǧǨǩǪǫǬǭǰǴǵǸǹǺǻȀȁȂȃȄȅȆȇȈȉȊȋȌȍȎȏȐȑȒȓȔȕȖȗȘșȚțȞȟȦȧȨȩ'
    'ȪȫȬȭȮȯȰȱȲȳḀḁḂḃḄḅḆḇḈḉḊḋḌḍḎḏḐḑḒḓḔḕḖḗḘḙḚḛḜḝḞḟḠḡḢḣḤḥḦḧḨḩḪḫḬḭḮḯḰḱḲḳḴḵḶḷḸḹḺḻḼḽḾḿṀṁṂṃ'
    'ṄṅṆṇṈṉṊṋṌṍṎṏṐṑṒṓṔṕṖṗṘṙṚṛṜṝṞṟṠṡṢṣṤṥṦṧṨṩṪṫṬṭṮṯṰṱṲṳṴṵṶṷṸṹṺṻṼṽṾṿẀẁẂẃẄẅẆẇẈẉẊẋẌẍẎẏẐẑ'
    'ẒẓẔẕẖẗẘẙẠạẢảẤấẦầẨẩẪẫẬậẮắẰằẲẳẴẵẶặẸẹẺẻẼẽẾếỀềỂểỄễỆệỈỉỊịỌọỎỏỐốỒồỔổỖỗỘộỚớỜờỞởỠỡỢợỤụ'
    'ỦủỨứỪừỬửỮữỰựỲỳỴỵỶỷỸỹ';
const _unaccented =
    'aaaaaaceeeeiiiinooooouuuuyaaaaaaceeeeiiiinooooouuuuyyaaaaaaccccccccddeeeeeeeee'
    'egggggggghhiiiiiiiiijjkkllllllnnnnnnoooooorrrrrrssssssssttttuuuuuuuuuuuuwwyyyz'
    'zzzzzoouuaaiioouuuuuuuuuuaaaaggkkoooojggnnaaaaaaeeeeiiiioooorrrruuuusstthhaaee'
    'ooooooooyyaabbbbbbccddddddddddeeeeeeeeeeffgghhhhhhhhhhiiiikkkkkkllllllllmmmmmm'
    'nnnnnnnnoooooooopppprrrrrrrrssssssssssttttttttuuuuuuuuuuvvvvwwwwwwwwwwxxxxyyzz'
    'zzzzhtwyaaaaaaaaaaaaaaaaaaaaaaaaeeeeeeeeeeeeeeeeiiiioooooooooooooooooooooooouu'
    'uuuuuuuuuuuuyyyyyyyy';

/// Words a log says, in small letters: a name that is one of them is taken out only as
/// it is written, so the word stays readable.
const _commonWords = {
  'about',
  'accept',
  'active',
  'add',
  'address',
  'admin',
  'after',
  'again',
  'all',
  'app',
  'art',
  'ask',
  'audio',
  'away',
  'back',
  'bad',
  'bell',
  'best',
  'big',
  'bird',
  'blue',
  'book',
  'box',
  'bug',
  'busy',
  'call',
  'camera',
  'can',
  'cannot',
  'case',
  'chat',
  'check',
  'clear',
  'close',
  'closed',
  'code',
  'connect',
  'connected',
  'core',
  'could',
  'data',
  'day',
  'dead',
  'default',
  'device',
  'dial',
  'direct',
  'disk',
  'done',
  'down',
  'drop',
  'echo',
  'empty',
  'end',
  'error',
  'event',
  'fail',
  'failed',
  'faith',
  'false',
  'file',
  'files',
  'fine',
  'fire',
  'first',
  'fix',
  'friend',
  'friends',
  'from',
  'full',
  'game',
  'gaming',
  'gone',
  'good',
  'gray',
  'green',
  'group',
  'guest',
  'happy',
  'have',
  'heart',
  'hello',
  'help',
  'here',
  'home',
  'hope',
  'host',
  'idle',
  'info',
  'invite',
  'join',
  'joy',
  'just',
  'key',
  'king',
  'last',
  'late',
  'leave',
  'left',
  'level',
  'light',
  'line',
  'link',
  'list',
  'live',
  'load',
  'local',
  'lost',
  'love',
  'main',
  'mark',
  'max',
  'may',
  'me',
  'member',
  'message',
  'mic',
  'min',
  'mode',
  'more',
  'move',
  'music',
  'mute',
  'name',
  'net',
  'new',
  'next',
  'night',
  'node',
  'none',
  'not',
  'note',
  'off',
  'offer',
  'old',
  'one',
  'online',
  'only',
  'open',
  'out',
  'over',
  'owner',
  'page',
  'panic',
  'path',
  'pause',
  'peer',
  'phone',
  'pink',
  'play',
  'port',
  'rain',
  'ready',
  'red',
  'relay',
  'rest',
  'retry',
  'ring',
  'room',
  'root',
  'rose',
  'run',
  'safe',
  'save',
  'say',
  'screen',
  'see',
  'send',
  'sent',
  'server',
  'set',
  'share',
  'show',
  'sky',
  'slow',
  'snow',
  'sound',
  'start',
  'state',
  'step',
  'stop',
  'stream',
  'summer',
  'sun',
  'sync',
  'system',
  'team',
  'test',
  'text',
  'the',
  'time',
  'top',
  'trace',
  'tray',
  'true',
  'try',
  'up',
  'update',
  'user',
  'video',
  'voice',
  'wait',
  'warn',
  'water',
  'way',
  'well',
  'white',
  'will',
  'win',
  'window',
  'winter',
  'word',
  'work',
  'world',
  'yes',
  'you',
};

/// The user of this computer and its name, as the system says them: names a log must not
/// carry. One that is also a common word is left out: it says nothing of anyone.
List<String> machineNames() {
  final env = Platform.environment;
  final out = <String>[
    for (final v in const ['USER', 'USERNAME', 'LOGNAME']) ?env[v],
  ];
  for (final v in const ['HOME', 'USERPROFILE']) {
    final home = env[v]?.replaceAll(RegExp(r'[\\/]+$'), '');
    if (home != null && home.isNotEmpty) {
      out.add(home.split(RegExp(r'[\\/]')).last);
    }
  }
  // A computer's name without its domain (`marta-laptop.local` as `marta-laptop`).
  for (final host in [env['HOSTNAME'], env['COMPUTERNAME'], _localHostname()]) {
    if (host != null) out.add(host.split('.').first);
  }
  return [
    for (final name in out.map((n) => n.trim()).toSet())
      if (name.runes.length >= 2 && !_commonWords.contains(name.toLowerCase()))
        name,
  ];
}

String? _localHostname() {
  try {
    return Platform.localHostname;
  } catch (_) {
    return null;
  }
}

/// The folders of this computer a log may name: the data folder [dataDir], the home and
/// temporary folders and those of applications' data, each also as the system resolves it
/// (`/var/folders/…` is `/private/var/folders/…` on a Mac).
List<String> machinePaths(String dataDir) {
  final env = Platform.environment;
  final paths = <String>{
    if (dataDir.isNotEmpty) dataDir,
    for (final v in const [
      'HOME',
      'USERPROFILE',
      'TMPDIR',
      'TEMP',
      'TMP',
      'XDG_RUNTIME_DIR',
      'APPDATA',
      'LOCALAPPDATA',
    ])
      ?env[v],
  };
  try {
    paths.add(Directory.systemTemp.path);
  } catch (_) {}
  for (final path in paths.toList()) {
    try {
      paths.add(Directory(path).resolveSymbolicLinksSync());
    } catch (_) {}
  }
  return paths.toList();
}

/// The endings of files people have, taken out of a log wherever a name shows with one
/// (as in the core's scrubber).
const _fileEnds =
    'png|jpe?g|gif|webp|heic|heif|avif|bmp|tiff?|svg|ico|raw|dng|cr2|nef|mp4|m4v|mov|'
    'mkv|webm|avi|wmv|mpe?g|3gp|mp3|m4a|wav|flac|ogg|oga|opus|aac|wma|mid|pdf|docx?|'
    'xlsx?|pptx?|odt|ods|odp|rtf|txt|md|csv|epub|pages|numbers|json|xml|html?|ya?ml|toml|'
    'ini|zip|rar|7z|tar|gz|tgz|bz2|xz|zst|exe|msi|dmg|pkg|apk|deb|rpm|appimage|iso|'
    'flatpak|fogata|log|key|pem|ps1|sh|bat|py|js|db|sqlite|bak|tmp|part|ics|vcf|eml|srt|'
    'vtt|torrent';
