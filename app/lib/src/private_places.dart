import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:flutter/foundation.dart';

import 'host.dart';

/// Whether the file at [path] is hidden or lies in a place with other people's secrets or the
/// system's: a folder or file whose name starts with a dot (`.ssh`, `.gnupg`, a browser's
/// profile, `.env`), the user's `Library` on macOS (keychains, browsers, mail) or `AppData`
/// on Windows, or a system folder. Dragging such a file is rarely meant, and a web page can
/// start a drag that carries a path: [DropZone] does not add them. [home] is the user's
/// folder.
bool isPrivatePlace(
  String path, {
  required HostSystem system,
  required String home,
}) {
  final windows = system == HostSystem.windows;
  if (_parts(path, windows).any((part) => part.startsWith('.'))) return true;
  if (windows) return _privateOnWindows(path);
  // The usual file system of a Mac does not tell `Library` from `library`.
  final macos = system == HostSystem.macos;
  final parts = _parts(path, false, fold: macos);
  final inHome = _parts(home, false, fold: macos);
  if (inHome.isNotEmpty &&
      parts.length > inHome.length &&
      [for (var i = 0; i < inHome.length; i++) parts[i] == inHome[i]]
          .every((same) => same)) {
    final below = parts[inHome.length];
    // The system's cloud drive (`Mobile Documents`), and the folders of cloud-sync
    // services (`CloudStorage`, macOS 12.3 and later), live in the Library and hold the
    // user's own files.
    final own =
        parts.length > inHome.length + 1 &&
        const {
          'mobile documents',
          'cloudstorage',
        }.contains(parts[inHome.length + 1]);
    if (macos && below == 'library' && !own) return true;
  }
  if (parts.isEmpty) return false;
  const system0 = {'etc', 'root', 'proc', 'sys', 'dev', 'boot'};
  if (system0.contains(parts.first)) return true;
  if (parts.length > 1 && parts.first == 'var') {
    return const {'lib', 'log', 'spool', 'backups', 'mail'}.contains(parts[1]);
  }
  if (macos) {
    // `/private` is where `/etc` and `/var` really are; `/var/folders` holds the
    // temporary files of applications.
    if (parts.first == 'private') {
      if (parts.length > 1 && parts[1] == 'etc') return true;
      return parts.length > 2 &&
          parts[1] == 'var' &&
          const {'db', 'log', 'root', 'spool', 'at'}.contains(parts[2]);
    }
    return parts.first == 'system' ||
        (parts.first == 'library' &&
            parts.length > 1 &&
            parts[1] == 'keychains');
  }
  if (parts.first == 'run') {
    // Runtime files (sockets, secrets, agents) but not the portal's documents the person
    // picked (`/run/user/<id>/doc`) nor mounted drives.
    if (parts.length > 1 && parts[1] == 'media') return false;
    return !(parts.length > 3 && parts[1] == 'user' && parts[3] == 'doc');
  }
  return false;
}

/// [isPrivatePlace] on Windows, where names are read without their case and a path starts
/// with a drive (`C:`), a share (`\\server\share`) or the prefix of either (`\\?\`).
bool _privateOnWindows(String path) {
  var named = path.replaceAll('/', r'\');
  var share = false;
  if (named.startsWith(r'\\?\') || named.startsWith(r'\\.\')) {
    named = named.substring(4);
    if (named.toLowerCase().startsWith(r'unc\')) {
      named = named.substring(4);
      share = true;
    }
  } else if (named.startsWith(r'\\')) {
    share = true;
  }
  final parts = _parts(named, true, fold: true);
  // What is below the drive, or below the server and share: `Windows`, `AppData`...
  final rest = share
      ? parts.skip(2).toList()
      : parts.isNotEmpty && RegExp(r'^[a-z]:$').hasMatch(parts.first)
      ? parts.skip(1).toList()
      : parts;
  // Their short (8.3) names too: `PROGRA~1` is either program folder, `APPDAT~1` AppData.
  // Where a short name stands for another folder that begins the same, it is taken as
  // private all the same: the real name is looked at too, once the links are followed.
  final short = RegExp(r'^(progra|appdat)~\d+$');
  if (rest.isNotEmpty &&
      (const {
            'windows',
            'programdata',
            'program files',
            'program files (x86)',
          }.contains(rest.first) ||
          (short.firstMatch(rest.first)?.group(1) == 'progra'))) {
    return true;
  }
  return rest.any(
    (p) =>
        p == 'appdata' ||
        p.startsWith('ntuser.dat') ||
        short.firstMatch(p)?.group(1) == 'appdat',
  );
}

/// The folder and file names of [path] with `.` and `..` worked out, as the system would
/// walk them, in lower case if asked. Windows leaves out the dots and spaces that end a
/// name (`AppData.` is `AppData`). A file system that does not tell the case apart also
/// takes the letters whose lower case is a plain one as that one (the long s `ſ` as `s`,
/// the Kelvin sign as `k`), so they are read as such too.
List<String> _parts(String path, bool windows, {bool fold = false}) {
  final parts = <String>[];
  for (var part in path.split(windows ? RegExp(r'[\\/]') : RegExp('/'))) {
    if (windows && part != '.' && part != '..') {
      part = part.replaceFirst(RegExp(r'[. ]+$'), '');
    }
    if (part.isEmpty || part == '.') continue;
    if (fold) {
      part = part
          .replaceAll('\u017F', 's')
          .replaceAll('\u212A', 'k')
          .replaceAll('\u212B', '\u00E5')
          .toLowerCase();
    }
    if (part == '..') {
      if (parts.isNotEmpty) parts.removeLast();
      continue;
    }
    parts.add(part);
  }
  return parts;
}

/// The user's folders, as the places private to them are looked for under each: `HOME`, and
/// the account's own folder where that is another. In macOS's App Sandbox `HOME` is the
/// application's container (`~/Library/Containers/<id>/Data`), and the user's Library (mail,
/// messages, cookies, browsers' profiles) is under the account's folder, not under it.
List<String> get homeFolders => homesOf(
  Platform.environment[Platform.isWindows ? 'USERPROFILE' : 'HOME'] ?? '',
  account: Platform.isWindows ? null : _accountHome(),
);

/// [home] and [account] without repeats or empty ones, and the owner of a sandbox's
/// container when [home] is one.
@visibleForTesting
List<String> homesOf(String home, {String? account}) {
  final container = RegExp(r'^(/Users/[^/]+)/Library/Containers/[^/]+/Data/?$')
      .firstMatch(home);
  return {
    for (final h in [home, ?account, ?container?.group(1)])
      if (h.isNotEmpty) h,
  }.toList();
}

/// The folder of the account this runs as, from the system's user database
/// (`getpwuid(getuid())`), whatever `HOME` says; null when it cannot be read.
String? _accountHome() {
  try {
    final process = DynamicLibrary.process();
    final getuid = process.lookupFunction<Uint32 Function(), int Function()>(
      'getuid',
    );
    final getpwuid = process
        .lookupFunction<
          Pointer<Pointer<Uint8>> Function(Uint32),
          Pointer<Pointer<Uint8>> Function(int)
        >('getpwuid');
    final entry = getpwuid(getuid());
    if (entry == nullptr) return null;
    // Where `pw_dir` is in `struct passwd`, counted in pointers: after the name, the
    // password, the user and group ids (one pointer's room together), and on macOS also
    // the time of the next change, the class and the full name; on Linux the full name.
    final dir = entry[Platform.isMacOS ? 6 : 4];
    if (dir == nullptr) return null;
    var length = 0;
    while (length < 4096 && dir[length] != 0) {
      length++;
    }
    return utf8.decode(dir.asTypedList(length), allowMalformed: true);
  } catch (_) {
    return null;
  }
}

/// Those of [paths] that are hidden or in a private place ([isPrivatePlace]), looking at
/// where each really is as well as at how it was named: a link in a plain folder can point
/// into `.ssh`. [homes] are the user's folders ([homeFolders]) unless a test gives them.
/// Read without holding up the window.
///
/// A hard link to a private file, made in a plain folder, is not found: nothing tells it
/// from the file itself. Making one takes the user's own rights over that folder, and so
/// is no web page's doing.
Future<List<String>> privatePaths(
  Iterable<String> paths, {
  List<String>? homes,
}) async {
  final folders = homes ?? homeFolders;
  final homesNow = folders.isEmpty ? const [''] : folders;
  final found = <String>[];
  for (final path in paths) {
    bool privateHere(String path) =>
        homesNow.any((home) => isPrivatePlace(path, system: host, home: home));
    var private = privateHere(path);
    if (!private) {
      try {
        private = privateHere(await File(path).resolveSymbolicLinks());
      } on Exception {
        // Gone, unreadable or no path: it will not be added either.
      }
    }
    if (private) found.add(path);
  }
  return found;
}
