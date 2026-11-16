import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:desktop_drop/desktop_drop.dart' show DesktopDrop;
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
/// into `.ssh`, and inside the Flatpak a file of the document portal is judged by the place
/// the portal says it is in ([realPlace]), one whose place cannot be learnt as private.
/// [homes] are the user's folders ([homeFolders]) unless a test gives them. Read without
/// holding up the window.
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
    Future<bool> privateThere(String path) async {
      final real = await realPlace(path);
      // A document is judged by its real place alone, which ends in its own name: Flatpak
      // shows the portal's folder at `/run/flatpak/doc`, which is no private place.
      return real == null || privateHere(real);
    }

    var private = await privateThere(path);
    if (!private) {
      try {
        final linked = await File(path).resolveSymbolicLinks();
        if (linked != path) private = await privateThere(linked);
      } on Exception {
        // Gone, unreadable or no path: it will not be added either.
      }
    }
    if (private) found.add(path);
  }
  return found;
}

/// Whether this runs inside a Flatpak, where files can come through the document portal;
/// the tests set it.
@visibleForTesting
bool inFlatpak = Platform.isLinux && File('/.flatpak-info').existsSync();

/// Asks the document portal where its files with these ids really are (the drop plugin's
/// `documentHostPaths`); the tests put their own in its place.
@visibleForTesting
Future<Map<String, Uint8List>> Function(List<String> ids) documentHostPaths =
    DesktopDrop.instance.documentHostPaths;

/// How many of [parts] name the document portal's folder as a sandboxed application sees
/// it: `/run/user/<uid>/doc`, or `/run/flatpak/doc`, where Flatpak mounts it and to which
/// the first leads; 0 when they do not begin with it.
int _portalFolder(List<String> parts) {
  if (parts.length >= 4 &&
      parts[0] == 'run' &&
      parts[1] == 'user' &&
      parts[3] == 'doc') {
    return 4;
  }
  if (parts.length >= 3 &&
      parts[0] == 'run' &&
      parts[1] == 'flatpak' &&
      parts[2] == 'doc') {
    return 3;
  }
  return 0;
}

/// Whether [path], with `.` and `..` worked out, is in the document portal's folder as a
/// sandboxed application sees it (`/run/user/<id>/doc` or `/run/flatpak/doc`), or is that
/// folder.
bool inDocumentPortal(String path) => _portalFolder(_parts(path, false)) > 0;

/// The id and name of the document portal's file at [path], exactly as the portal names
/// one to a sandboxed application: `/run/user/<uid>/doc/<id>/<name>`, or
/// `/run/flatpak/doc/<id>/<name>` where that leads, with no `.` or `..` on the way. Null for
/// any other path, one in the portal's folder among them (deeper in an exported folder, its
/// listing by application, a name climbing out of it).
({String id, String name})? portalDocument(String path) {
  if (!path.startsWith('/') || path.contains('\x00')) return null;
  final parts = [
    for (final part in path.split('/'))
      if (part.isNotEmpty) part,
  ];
  final folder = _portalFolder(parts);
  if (folder == 0 ||
      parts.length != folder + 2 ||
      parts.any((part) => part == '.' || part == '..') ||
      (folder == 4 && !RegExp(r'^[0-9]+$').hasMatch(parts[2])) ||
      parts[folder] == 'by-app') {
    return null;
  }
  return (id: parts[folder], name: parts[folder + 1]);
}

/// Where the file at [path] really is, as far as this application can know it: inside the
/// Flatpak, for a path in the document portal's folder ([inDocumentPortal]), the path the
/// Documents portal gives for its id ([documentHostPaths]), never one a drag's data named;
/// any other path is its own place. Null when that cannot be learnt, and the file must then
/// be taken as private: a path in the portal's folder that is not exactly one of its files
/// ([portalDocument]), an id the portal does not answer for (not given to this application,
/// or a portal too old to say), or an answer that is not an absolute path ending in the
/// file's own name, or is in the portal's folder itself.
Future<String?> realPlace(String path) async {
  if (!inFlatpak || !inDocumentPortal(path)) return path;
  final document = portalDocument(path);
  if (document == null) return null;
  final Map<String, Uint8List> places;
  try {
    places = await documentHostPaths([document.id]);
  } on Exception catch (error) {
    debugPrint('the document portal says no real place: $error');
    return null;
  }
  final bytes = places[document.id];
  if (bytes == null) return null;
  final real = utf8.decode(bytes, allowMalformed: true);
  if (!real.startsWith('/') ||
      real.contains('\x00') ||
      inDocumentPortal(real) ||
      _parts(real, false).lastOrNull != document.name) {
    return null;
  }
  return real;
}
