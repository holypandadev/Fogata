import 'dart:io';

import 'package:flutter/foundation.dart';

import 'l10n.dart';
import 'rust/api/app.dart' as api;

/// Opens the system's file manager at [path], with the file selected where the system
/// allows it. False if no file manager could be reached.
Future<bool> showInFolder(String path) async {
  // Tests write what would open into a file instead of opening anything.
  final record = Platform.environment['FOGATA_TEST_OPENED'];
  if (record != null) {
    await File(record).writeAsString('$path\n', mode: FileMode.append);
    return true;
  }
  try {
    if (Platform.isMacOS) {
      return (await Process.run('open', ['-R', path])).exitCode == 0;
    } else if (Platform.isWindows) {
      // Explorer reads its command line itself and splits it at commas: the path goes in
      // quotes after `/select,`, which Dart's own quoting of arguments would not keep.
      api.revealInExplorer(path: path);
      return true;
    } else {
      return await showInFolderFreedesktop(path);
    }
  } catch (_) {
    return false;
  }
}

/// [showInFolder], and where no file manager could be reached, a note through [say] that
/// tells where the file is.
Future<void> showInFolderOrSay(
  String path,
  void Function(String, {bool error}) say,
) async {
  if (!await showInFolder(path)) {
    say(l10n.revealFailed(path), error: true);
  }
}

/// Linux and the BSDs: the freedesktop call selects the file in Dolphin, Files, Nemo and
/// others; where it fails, or `dbus-send` is not installed, the folder opens. False when
/// neither worked.
@visibleForTesting
Future<bool> showInFolderFreedesktop(
  String path, {
  Future<ProcessResult> Function(String, List<String>) run = Process.run,
}) async {
  var shown = false;
  try {
    final reply = await run('dbus-send', [
      '--session',
      '--print-reply',
      '--dest=org.freedesktop.FileManager1',
      '--type=method_call',
      '/org/freedesktop/FileManager1',
      'org.freedesktop.FileManager1.ShowItems',
      'array:string:${fileManagerItem(path)}',
      'string:',
    ]);
    shown = reply.exitCode == 0;
  } catch (_) {}
  if (shown) return true;
  try {
    return (await run('xdg-open', [File(path).parent.path])).exitCode == 0;
  } catch (_) {
    return false;
  }
}

/// [path] as an item of the freedesktop file manager call: its address, with commas written
/// as `%2C` (as an address may), since dbus-send splits an array's items at commas.
@visibleForTesting
String fileManagerItem(String path) =>
    Uri.file(path).toString().replaceAll(',', '%2C');
