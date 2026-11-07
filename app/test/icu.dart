// The messages of lib/l10n's ARB files read in tests: their ICU syntax checked, and English
// said with a pseudo-language that makes every word longer.

import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;

import 'package:fogata/src/l10n.dart';

/// A piece of a message: text, a placeholder, or a plural or select with its cases.
sealed class Node {}

class TextNode extends Node {
  TextNode(this.text);
  final String text;
}

class ArgNode extends Node {
  ArgNode(this.name);
  final String name;
}

class ChoiceNode extends Node {
  ChoiceNode(this.name, this.kind, this.cases);
  final String name;

  /// `plural` or `select`.
  final String kind;
  final Map<String, List<Node>> cases;
}

/// Reads [message]; throws a [FormatException] for broken syntax.
List<Node> parseMessage(String message) {
  final (nodes, end) = _parse(message, 0, 0);
  if (end != message.length) {
    throw FormatException('stray } in "$message"', message, end);
  }
  return nodes;
}

final _head = RegExp(r'\{\s*([A-Za-z_]\w*)\s*(?:,\s*(plural|select)\s*,)?');
final _case = RegExp(r'\s*(=\d+|[A-Za-z_]\w*)\s*\{');

(List<Node>, int) _parse(String s, int i, int depth) {
  final nodes = <Node>[];
  final text = StringBuffer();
  void flush() {
    if (text.isNotEmpty) nodes.add(TextNode(text.toString()));
    text.clear();
  }

  while (i < s.length) {
    final c = s[i];
    if (c == '}') {
      if (depth == 0) throw FormatException('stray }', s, i);
      flush();
      return (nodes, i);
    }
    if (c != '{') {
      text.write(c);
      i++;
      continue;
    }
    flush();
    final head = _head.matchAsPrefix(s, i);
    if (head == null) throw FormatException('bad placeholder', s, i);
    final name = head.group(1)!;
    final kind = head.group(2);
    i = head.end;
    if (kind == null) {
      if (i >= s.length || s[i] != '}') {
        throw FormatException('unclosed {$name', s, i);
      }
      nodes.add(ArgNode(name));
      i++;
      continue;
    }
    final cases = <String, List<Node>>{};
    while (true) {
      while (i < s.length && s[i].trim().isEmpty) {
        i++;
      }
      if (i < s.length && s[i] == '}') {
        i++;
        break;
      }
      final m = _case.matchAsPrefix(s, i);
      if (m == null) throw FormatException('bad case in $name', s, i);
      final key = m.group(1)!;
      if (cases.containsKey(key)) {
        throw FormatException('case $key twice in $name', s, i);
      }
      final (inner, end) = _parse(s, m.end, depth + 1);
      if (end >= s.length || s[end] != '}') {
        throw FormatException('unclosed case $key of $name', s, end);
      }
      cases[key] = inner;
      i = end + 1;
    }
    nodes.add(ChoiceNode(name, kind, cases));
  }
  if (depth > 0) throw FormatException('unclosed {', s, i);
  flush();
  return (nodes, i);
}

/// The placeholders a message uses, and the cases of each plural and select.
({
  Set<String> args,
  Map<String, Set<String>> plurals,
  Map<String, Set<String>> selects,
})
shape(List<Node> nodes) {
  final args = <String>{};
  final plurals = <String, Set<String>>{};
  final selects = <String, Set<String>>{};
  void walk(List<Node> nodes) {
    for (final n in nodes) {
      switch (n) {
        case TextNode():
          break;
        case ArgNode(:final name):
          args.add(name);
        case ChoiceNode(:final name, :final kind, :final cases):
          args.add(name);
          (kind == 'plural' ? plurals : selects)
              .putIfAbsent(name, () => {})
              .addAll(cases.keys);
          cases.values.forEach(walk);
      }
    }
  }

  walk(nodes);
  return (args: args, plurals: plurals, selects: selects);
}

/// [nodes] said with [values]; a plural takes `=n`, then `one` for 1, then `other`.
String say(List<Node> nodes, Map<String, Object?> values) {
  final out = StringBuffer();
  for (final n in nodes) {
    switch (n) {
      case TextNode(:final text):
        out.write(text);
      case ArgNode(:final name):
        out.write(values[name]);
      case ChoiceNode(:final name, :final kind, :final cases):
        final value = values[name];
        final List<Node>? chosen;
        if (kind == 'plural') {
          final count = value is num ? value : 0;
          chosen =
              cases['=$count'] ??
              (count == 1 ? cases['one'] : null) ??
              cases['other'];
        } else {
          chosen = cases['$value'] ?? cases['other'];
        }
        out.write(say(chosen ?? const [], values));
    }
  }
  return out.toString();
}

/// The ARB files: by language code (`en`, `es`, … as in the file names).
Map<String, Map<String, dynamic>> readArbs() => {
  for (final file in Directory(
    'lib/l10n',
  ).listSync().whereType<File>().where((f) => f.path.endsWith('.arb')))
    RegExp(r'app_(\w+)\.arb$').firstMatch(file.path)!.group(1)!:
        jsonDecode(file.readAsStringSync()) as Map<String, dynamic>,
};

/// English with every word made about 40 % longer and its letters accented, to see that
/// the screens still fit languages that take more room than English.
class PseudoL10n implements L10n {
  PseudoL10n() {
    final en = readArbs()['en']!;
    for (final MapEntry(:key, :value) in en.entries) {
      if (key.startsWith('@')) continue;
      _messages[key] = parseMessage(value as String);
      final placeholders =
          (en['@$key'] as Map<String, dynamic>)['placeholders']
              as Map<String, dynamic>?;
      _params[key] = placeholders?.keys.toList() ?? const [];
    }
  }

  final _messages = <String, List<Node>>{};
  final _params = <String, List<String>>{};

  @override
  String get localeName => 'en';

  @override
  dynamic noSuchMethod(Invocation invocation) {
    // Symbol("name") without mirrors.
    final name = RegExp(r'"(\w+)"')
        .firstMatch(invocation.memberName.toString())!
        .group(1)!;
    final nodes = _messages[name];
    if (nodes == null) return super.noSuchMethod(invocation);
    final params = _params[name]!;
    final values = {
      for (final (i, arg) in invocation.positionalArguments.indexed)
        params[i]: arg,
    };
    // Only the message's own words grow: what fills its placeholders comes in as it is.
    final grown = _grow(nodes);
    return say(grown, values);
  }

  static const _accents = {
    'a': 'á', 'e': 'é', 'i': 'í', 'o': 'ö', 'u': 'ü', 'c': 'ç', 'n': 'ñ', //
    'A': 'Å', 'E': 'É', 'I': 'Í', 'O': 'Ö', 'U': 'Ü', 'C': 'Ç', 'N': 'Ñ',
  };

  static List<Node> _grow(List<Node> nodes) => [
    for (final n in nodes)
      switch (n) {
        TextNode(:final text) => TextNode(
          text.replaceAllMapped(RegExp(r'[A-Za-z]+'), (m) {
            final word = m[0]!;
            final extra = (word.length * .4).ceil();
            final longer =
                word + word.toLowerCase() * (extra ~/ word.length + 1);
            return longer
                .substring(0, word.length + extra)
                .split('')
                .map((c) => _accents[c] ?? c)
                .join();
          }),
        ),
        ArgNode() => n,
        ChoiceNode(:final name, :final kind, :final cases) => ChoiceNode(
          name,
          kind,
          {
            for (final MapEntry(:key, :value) in cases.entries)
              key: _grow(value),
          },
        ),
      },
  ];
}

/// How much longer [PseudoL10n] makes English text, for a check that it really grows.
double pseudoGrowth(String english, String pseudo) =>
    pseudo.length / math.max(1, english.length);
