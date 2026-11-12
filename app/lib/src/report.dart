import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:isolate';

import 'package:flutter/foundation.dart';

import 'private_file.dart';
import 'scrub.dart';
import 'test_switches.dart';
import 'version.dart';
import 'zip.dart';

export 'scrub.dart' show Scrubber;

/// Where a problem report goes.
const reportAddress = 'contact@holypanda.dev';

/// The folder of the data folder where the core and the window keep their log of the
/// last days (see crates/fogata-ffi/src/logs.rs).
const logFolder = 'diagnostics';

/// How far back the log of a report goes.
const reportLogSpan = Duration(days: 2);

/// The most a report's archive takes.
const reportMaxBytes = 5 * 1024 * 1024;

/// What a report says about this copy of Fogata, besides what the person wrote.
class ReportFacts {
  const ReportFacts({
    required this.build,
    required this.language,
    required this.at,
  });

  /// The build of this copy, when the updater says it.
  final int? build;
  final String language;
  final DateTime at;

  /// The file name of a report made then: `fogata-report-2026-10-30.zip`.
  String get fileName => 'fogata-report-${_date(at)}.zip';

  /// The report's text: what the person wrote, word for word, and the facts, with
  /// [system] the system and its version ([systemLine]).
  String text(String what, {required bool withLog, String? system}) {
    final lines = [
      'Fogata problem report',
      'Version: $appVersion${build == null ? '' : ' (build $build)'}',
      'System: ${system ?? systemLine()}',
      'Language: $language',
      'Made: ${at.toUtc().toIso8601String()}',
      'Log of the last 2 days: ${withLog ? 'included' : 'not included'}',
      '',
      'What happened:',
      what.trim(),
      '',
    ];
    return lines.join('\n');
  }
}

/// The system and its version, as a report says them: `linux, Linux 6.17.7-arch1-1`.
String systemLine() =>
    '${Platform.operatingSystem}, ${systemVersion(Platform.operatingSystemVersion)}';

/// A system's version without what follows it on Linux (`#1 SMP PREEMPT_DYNAMIC` and the
/// day the kernel was built, sometimes by whom and on which computer).
@visibleForTesting
String systemVersion(String version) {
  final hash = version.indexOf(' #');
  return (hash < 0 ? version : version.substring(0, hash)).trim();
}

String _date(DateTime t) =>
    '${t.year.toString().padLeft(4, '0')}-'
    '${t.month.toString().padLeft(2, '0')}-'
    '${t.day.toString().padLeft(2, '0')}';

/// A report made and ready to go: its files as the person reads them before sending, and
/// the archive made of exactly those.
class PreparedReport {
  const PreparedReport({
    required this.facts,
    required this.what,
    required this.withLog,
    required this.files,
    required this.bytes,
  });

  final ReportFacts facts;

  /// What the person wrote and whether the log goes, as they were when this was made.
  final String what;
  final bool withLog;

  /// Each file of the archive by its name in it: `report.txt`, then the logs.
  final Map<String, String> files;

  /// The archive, at most [reportMaxBytes].
  final Uint8List bytes;

  /// Whether this is the report of [what] and [withLog].
  bool isFor(String what, bool withLog) =>
      this.what == what && this.withLog == withLog;
}

/// What a report carries besides what the person wrote, with what is personal taken out:
/// the system's line and, when the log goes, its day files by name.
class ReportParts {
  const ReportParts({required this.system, required this.logs});
  final String system;
  final Map<String, String>? logs;

  /// Whether these hold the log.
  bool get withLog => logs != null;
}

/// The parts of a report from [dataDir] ([ReportParts]), the logs of the two days before
/// [now] only [withLog]; [names] are the names of people and groups known here, taken
/// out wherever they show ([Scrubber]). Reading and scrubbing two days of log takes a
/// moment, and the system is asked for this computer's names: never on the window's
/// thread.
Future<ReportParts> scrubReportParts({
  required String dataDir,
  required bool withLog,
  required DateTime now,
  Iterable<String> names = const [],
}) async {
  final scrubber = Scrubber.here(names, dataDir: dataDir);
  return ReportParts(
    system: scrubber.scrub(systemLine()),
    logs: withLog
        ? await collectLogs(dataDir, now: now, scrubber: scrubber)
        : null,
  );
}

/// The report of what the person wrote, [what], and [parts], as files and as the archive
/// of exactly those, at most [reportMaxBytes]: past that, the oldest part of each log goes
/// first.
PreparedReport assembleReport({
  required ReportFacts facts,
  required String what,
  required ReportParts parts,
}) {
  final text = facts.text(what, withLog: parts.withLog, system: parts.system);
  final logs = parts.logs ?? const <String, String>{};
  var keep = 1.0;
  while (true) {
    final files = {'report.txt': text};
    for (final MapEntry(key: name, value: log) in logs.entries) {
      var start = (log.length * (1 - keep)).floor();
      // From the start of a line.
      if (start > 0) start = log.indexOf('\n', start) + 1;
      files['log/$name'] = log.substring(start);
    }
    final zip = ZipWriter();
    files.forEach(
      (name, text) => zip.add(name, utf8.encode(text), time: facts.at),
    );
    final bytes = zip.bytes();
    if (bytes.length <= reportMaxBytes || keep < .01) {
      return PreparedReport(
        facts: facts,
        what: what,
        withLog: parts.withLog,
        files: files,
        bytes: bytes,
      );
    }
    keep *= (reportMaxBytes / bytes.length) * .9;
  }
}

/// The report of [what], [facts] and, [withLog], the log of [dataDir], made at once off
/// the window's thread ([scrubReportParts], [assembleReport]).
Future<PreparedReport> prepareReport({
  required String dataDir,
  required String what,
  required bool withLog,
  required ReportFacts facts,
  Iterable<String> names = const [],
}) {
  final known = names.toList();
  return Isolate.run(
    () async => assembleReport(
      facts: facts,
      what: what,
      parts: await scrubReportParts(
        dataDir: dataDir,
        withLog: withLog,
        now: facts.at,
        names: known,
      ),
    ),
  );
}

/// Work done in an isolate of its own, which is stopped when its result is no longer
/// wanted ([cancel]): a report made again while the page changes leaves none behind.
class Cancellable<T> {
  Cancellable._(this.future, this.cancel);

  /// The result; [Cancelled] once [cancel] stopped the work.
  final Future<T> future;
  final void Function() cancel;
}

/// What a [Cancellable] ends with when it was stopped.
class Cancelled implements Exception {
  const Cancelled();
}

/// What an isolate of [runCancellable] runs: made apart, so that it takes nothing of
/// that function's along to the isolate but [work].
void Function(SendPort) _entry<T>(FutureOr<T> Function() work) =>
    (reply) async {
      try {
        reply.send((await work(),));
      } catch (e, stack) {
        reply.send(('$e', '$stack'));
      }
    };

/// [scrubReportParts] in an isolate that can be stopped.
Cancellable<ReportParts> scrubReportPartsApart({
  required String dataDir,
  required bool withLog,
  required DateTime now,
  required List<String> names,
}) => runCancellable(
  () => scrubReportParts(
    dataDir: dataDir,
    withLog: withLog,
    now: now,
    names: names,
  ),
);

/// [assembleReport] in an isolate that can be stopped.
Cancellable<PreparedReport> assembleReportApart({
  required ReportFacts facts,
  required String what,
  required ReportParts parts,
}) => runCancellable(
  () => assembleReport(facts: facts, what: what, parts: parts),
);

/// Runs [work] in an isolate of its own ([Cancellable]).
Cancellable<T> runCancellable<T>(FutureOr<T> Function() work) {
  final done = Completer<T>();
  final port = RawReceivePort();
  Isolate? isolate;
  var stopped = false;
  void end(Object? message) {
    port.close();
    isolate = null;
    if (done.isCompleted) return;
    switch (message) {
      case (T value,):
        done.complete(value);
      case (String error, String stack):
        done.completeError(RemoteError(error, stack));
      default:
        done.completeError(const Cancelled());
    }
  }

  port.handler = end;
  Isolate.spawn<SendPort>(
    _entry(work),
    port.sendPort,
    // An isolate that ends with nothing said ends the work too.
    onExit: port.sendPort,
  ).then(
    (spawned) {
      if (stopped) {
        spawned.kill(priority: Isolate.immediate);
      } else {
        isolate = spawned;
      }
    },
    onError: (Object e, StackTrace stack) {
      port.close();
      if (!done.isCompleted) done.completeError(e, stack);
    },
  );
  return Cancellable._(done.future, () {
    stopped = true;
    isolate?.kill(priority: Isolate.immediate);
    end(null);
  });
}

/// Writes [report] into [folder] under a name not taken there, and returns its path.
Future<String> writeReport({
  required String folder,
  required PreparedReport report,
}) async {
  await Directory(folder).create(recursive: true);
  final path = await _takeName(folder, report.facts.fileName);
  await writePrivate(path, report.bytes);
  return path;
}

/// Makes an empty file in [folder] under [name] or a name not taken there ([freePath]),
/// before anything is written: writing would go through a link, to wherever it points,
/// and one put there meanwhile makes this fail and try the next name.
Future<String> _takeName(String folder, String name) async {
  for (var tries = 0; ; tries++) {
    final path = freePath(folder, name);
    try {
      await File(path).create(exclusive: true);
      return path;
    } on FileSystemException {
      if (tries >= 3) rethrow;
    }
  }
}

/// A path in [folder] for [name] that nothing takes yet, a link to nothing included:
/// `name.zip`, `name-2.zip`…
String freePath(String folder, String name) {
  final dot = name.lastIndexOf('.');
  final (stem, ext) = dot <= 0
      ? (name, '')
      : (name.substring(0, dot), name.substring(dot));
  bool taken(String path) =>
      FileSystemEntity.typeSync(path, followLinks: false) !=
      FileSystemEntityType.notFound;
  var path = '$folder${Platform.pathSeparator}$name';
  for (var n = 2; taken(path) && n < 1000; n++) {
    path = '$folder${Platform.pathSeparator}$stem-$n$ext';
  }
  return path;
}

/// The day files of the log in [dataDir] from the last two days, each scrubbed by
/// [scrubber], by file name. Only lines written within [reportLogSpan] of [now] are kept.
Future<Map<String, String>> collectLogs(
  String dataDir, {
  required DateTime now,
  required Scrubber scrubber,
}) async {
  final dir = Directory('$dataDir${Platform.pathSeparator}$logFolder');
  final out = <String, String>{};
  if (!await dir.exists()) return out;
  final since = now.toUtc().subtract(reportLogSpan);
  final files = <File>[];
  await for (final f in dir.list()) {
    final name = f.uri.pathSegments.last;
    if (f is File &&
        RegExp(r'^(fogata|app)-\d{4}-\d\d-\d\d\.log$').hasMatch(name)) {
      final day = DateTime.tryParse(
        '${name.substring(name.indexOf('-') + 1, name.length - 4)}T23:59:59Z',
      );
      if (day != null && !day.isBefore(since)) files.add(f);
    }
  }
  files.sort((a, b) => a.path.compareTo(b.path));
  for (final f in files) {
    String text;
    try {
      text = utf8.decode(await f.readAsBytes(), allowMalformed: true);
    } catch (_) {
      continue;
    }
    final kept = recentLines(text, since);
    if (kept.isEmpty) continue;
    out[f.uri.pathSegments.last] = scrubber.scrub(kept);
  }
  return out;
}

/// The lines of [log] written at [since] or later, by the time each line starts with; a
/// line without one goes with the line before it.
@visibleForTesting
String recentLines(String log, DateTime since) {
  final out = StringBuffer();
  var keep = false;
  for (final line in const LineSplitter().convert(log)) {
    final stamp = RegExp(r'^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d').hasMatch(line)
        ? DateTime.tryParse('${line.substring(0, 19)}Z')
        : null;
    if (stamp != null) {
      keep = !stamp.isBefore(since);
    }
    if (keep) out.writeln(line);
  }
  return out.toString();
}

/// Opens [uri] (an e-mail to write, a page) with the system's own application for it.
/// False when none could be reached, and for any other kind of address: a file, or a
/// scheme some program registered, could run something on this computer.
Future<bool> openExternal(
  Uri uri, {
  @visibleForTesting
  Future<ProcessResult> Function(String, List<String>) run = Process.run,
}) async {
  if (!const {'https', 'mailto'}.contains(uri.scheme.toLowerCase())) {
    return false;
  }
  // Tests write what would open into a file instead of opening anything.
  final record = testSwitch('FOGATA_TEST_OPENED');
  if (record != null) {
    await File(record).writeAsString('$uri\n', mode: FileMode.append);
    return true;
  }
  try {
    final ProcessResult result;
    if (Platform.isMacOS) {
      result = await run('open', [uri.toString()]);
    } else if (Platform.isWindows) {
      result = await run('rundll32', [
        'url.dll,FileProtocolHandler',
        uri.toString(),
      ]);
    } else {
      result = await run('xdg-open', [uri.toString()]);
    }
    return result.exitCode == 0;
  } catch (_) {
    return false;
  }
}

/// The e-mail of a report: to [reportAddress], with a subject and the name of the file to
/// attach. What the person wrote goes only in the file: the address is handed to another
/// program on its command line, which other programs on the computer can read, and the
/// mail application may keep it.
Uri reportMail(String fileName) => Uri(
  scheme: 'mailto',
  path: reportAddress,
  query: _query({
    'subject': 'Fogata $appVersion: problem report',
    'body': 'Problem report: $fileName',
  }),
);

/// A mailto query: spaces as %20, which every mail application reads, not `+`.
String _query(Map<String, String> fields) => fields.entries
    .map((e) => '${e.key}=${Uri.encodeComponent(e.value)}')
    .join('&');

/// The names of people and groups the window knows, which its errors written to the log
/// never carry, as the core's lines never do. Set once the window's state is made.
Iterable<String> Function() appErrorNames = () => const [];

/// Every name [rememberNames] was given while Fogata runs: a name forgotten since (a group
/// left, a friend removed) is still taken out of what is written and reported.
final _namesSeen = <String>{};

/// [names] and every name seen before them while Fogata runs.
Set<String> rememberNames(Iterable<String> names) {
  _namesSeen.addAll(names.where((n) => n.trim().isNotEmpty));
  return {..._namesSeen};
}

/// What was noted since the last write, by data folder and then by text, with how many times
/// each came.
final _noted = <String, Map<String, int>>{};

/// Set from one write to the next second: errors that come meanwhile wait for its end.
Timer? _quiet;

/// The last write: each waits for the one before.
Future<void> _appended = Future.value();

/// Notes an error of the window in the day's log beside the core's, for reports, with
/// what is personal taken out before it is written ([Scrubber]). The first error of a quiet
/// moment is written at once, off the window's thread; those that follow wait for the next
/// second, and one that repeats is written once with how many times it came, so an error
/// every frame costs one small write a second.
void noteAppError(String dataDir, Object error, StackTrace? stack) =>
    _note(dataDir, 'ERROR app: $error\n${stack == null ? '' : '$stack\n'}');

/// Notes [line], something the window or a library printed, in the day's log as
/// [noteAppError] notes an error: a release build writes there, scrubbed, what it would
/// have printed as it is (see console.dart).
void noteAppLine(String dataDir, String line) =>
    _note(dataDir, 'INFO app: $line\n');

void _note(String dataDir, String text) {
  if (dataDir.isEmpty) return;
  final waiting = _noted.putIfAbsent(dataDir, () => {});
  waiting[text] = (waiting[text] ?? 0) + 1;
  if (_quiet == null) _writeNoted();
}

/// Writes what waits, and keeps others for a second.
void _writeNoted() {
  final now = DateTime.now().toUtc();
  final batch = {for (final e in _noted.entries) e.key: e.value};
  _noted.clear();
  _quiet?.cancel();
  _quiet = null;
  if (batch.isEmpty) return;
  _appended = _appended.then((_) => _append(now, batch));
  _quiet = Timer(const Duration(seconds: 1), () {
    _quiet = null;
    if (_noted.isNotEmpty) _writeNoted();
  });
}

/// Writes now what waits to be noted, and returns when everything noted is on the disk.
Future<void> flushAppErrors() {
  if (_noted.isNotEmpty) _writeNoted();
  return _appended;
}

/// [said] scrubbed, each after [stamp]: off the window's thread, as is asking the system
/// for this computer's names.
Future<String> _scrubAppErrors(
  String dataDir,
  List<String> names,
  String stamp,
  List<String> said,
) => Isolate.run(() {
  final scrubber = Scrubber.here(names, dataDir: dataDir, shortIds: true);
  return [for (final text in said) '$stamp ${scrubber.scrub(text)}'].join();
});

Future<void> _append(DateTime now, Map<String, Map<String, int>> batch) async {
  for (final MapEntry(key: dataDir, value: errors) in batch.entries) {
    try {
      final dir = await Directory('$dataDir${Platform.pathSeparator}$logFolder')
          .create(recursive: true);
      final file = File(
        '${dir.path}${Platform.pathSeparator}app-${_date(now)}.log',
      );
      // A day's file of the window stays small: errors that repeat stop being noted.
      if (await file.exists() && await file.length() > 2 * 1024 * 1024) {
        continue;
      }
      final stamp = now.toIso8601String();
      final said = [
        for (final MapEntry(key: text, value: count) in errors.entries)
          count > 1 ? '$text(this came $count times)\n' : text,
      ];
      final lines = await _scrubAppErrors(
        dataDir,
        appErrorNames().toList(),
        stamp,
        said,
      );
      // A day's file is made as the core makes its own, readable by the owner only; what
      // comes later that day is added to it.
      if (await file.exists()) {
        await file.writeAsString(lines, mode: FileMode.append, flush: true);
      } else {
        await writePrivate(file.path, utf8.encode(lines));
      }
    } catch (_) {}
  }
}
