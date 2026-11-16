import 'dart:convert';

import 'package:desktop_drop/src/drop_item.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:universal_platform/universal_platform.dart';

import 'events.dart';
import 'web_drop_item.dart';

typedef RawDropListener = void Function(DropEvent);

class DesktopDrop {
  static const MethodChannel _channel = MethodChannel('desktop_drop');

  DesktopDrop._();

  static final instance = DesktopDrop._();

  final _listeners = <RawDropListener>{};

  /// Fogata: inside the Flatpak on Linux, how many of the files a drop's list of files
  /// names are refused, judged before the plugin asks the FileTransfer portal for the drop
  /// (which gives this application every file of it for good): the portal is asked only
  /// when it says 0. Set by the application; while it is null, no drop that offers the
  /// portal's target is taken.
  Future<int> Function(List<String> paths)? portalDropCheck;

  /// Fogata: the drop targets that take drops now, each saying whether it holds a point of
  /// the window (see [holds]).
  final _holders = <bool Function(Offset location)>{};

  /// Fogata: whether a drop target that takes drops now holds [location]: inside the
  /// Flatpak the FileTransfer portal is asked only for a drop something takes.
  bool holds(Offset location) => _holders.any((holder) => holder(location));

  void addHolder(bool Function(Offset location) holder) => _holders.add(holder);

  void removeHolder(bool Function(Offset location) holder) =>
      _holders.remove(holder);

  var _inited = false;

  Offset? _offset;

  void init() {
    if (_inited) {
      return;
    }
    _inited = true;
    _channel.setMethodCallHandler((call) async {
      try {
        return await _handleMethodChannel(call);
      } catch (e, s) {
        debugPrint('_handleMethodChannel: $e $s');
      }
    });
  }

  /// macOS: Attempt to start security-scoped access for a bookmarked URL.
  ///
  /// Pass the [DropItem.extraAppleBookmark] bytes from a dropped file that
  /// originated outside the app container. Returns `true` if access began.
  ///
  /// If [bookmark] is empty, this function returns `false` and does not
  /// invoke the platform call. Promise files written under your container do
  /// not require security-scoped access.
  Future<bool> startAccessingSecurityScopedResource(
      {required Uint8List bookmark}) async {
    if (bookmark.isEmpty) return false;
    Map<String, dynamic> resultMap = {};
    resultMap["apple-bookmark"] = bookmark;
    final bool? result = await _channel.invokeMethod(
        "startAccessingSecurityScopedResource", resultMap);
    if (result == null) return false;
    return result;
  }

  /// macOS: Stop security-scoped access previously started.
  ///
  /// If [bookmark] is empty, this function returns `true` and does not
  /// invoke the platform call, acting as a no-op.
  Future<bool> stopAccessingSecurityScopedResource(
      {required Uint8List bookmark}) async {
    if (bookmark.isEmpty) return true;
    Map<String, dynamic> resultMap = {};
    resultMap["apple-bookmark"] = bookmark;
    final bool result = await _channel.invokeMethod(
        "stopAccessingSecurityScopedResource", resultMap);
    return result;
  }

  /// Fogata: where the document portal's files with these [ids] really are, as the
  /// Documents portal says (`GetHostPaths`), each path as the bytes of its name. Linux only.
  /// An id the portal does not answer for (one this application was not given) is left
  /// out. Throws a [PlatformException] when the portal cannot be asked, gives no answer in
  /// time or is too old to say (before version 5 of its interface).
  Future<Map<String, Uint8List>> documentHostPaths(List<String> ids) async {
    final answer = await _channel.invokeMapMethod<String, Uint8List>(
      'documentHostPaths',
      ids,
    );
    return answer ?? const {};
  }

  Future<Object?> _handleMethodChannel(MethodCall call) async {
    switch (call.method) {
      case "entered":
        final position = (call.arguments as List).cast<double>();
        _offset = Offset(position[0], position[1]);
        _notifyEvent(DropEnterEvent(location: _offset!));
        break;
      case "updated":
        if (_offset == null && UniversalPlatform.isLinux) {
          final position = (call.arguments as List).cast<double>();
          _offset = Offset(position[0], position[1]);
          _notifyEvent(DropEnterEvent(location: _offset!));
          return null;
        }
        final position = (call.arguments as List).cast<double>();
        _offset = Offset(position[0], position[1]);
        _notifyEvent(DropUpdateEvent(location: _offset!));
        break;
      case "exited":
        _notifyEvent(DropExitEvent(location: _offset ?? Offset.zero));
        _offset = null;
        break;
      case "performOperation":
        final paths = (call.arguments as List).cast<String>();
        _notifyEvent(
          DropDoneEvent(
            location: _offset ?? Offset.zero,
            files: paths.map((e) => DropItemFile(e)).toList(),
          ),
        );
        _offset = null;
        break;
      case "performOperation_macos":
        final items = (call.arguments as List).cast<Map>();
        _notifyEvent(
          DropDoneEvent(
            location: _offset ?? Offset.zero,
            files: items.map((raw) {
              final path = raw["path"] as String;
              final bookmark = raw["apple-bookmark"] as Uint8List?;
              final isDir = (raw["isDirectory"] as bool?) ?? false;
              final fromPromise = (raw["fromPromise"] as bool?) ?? false;
              if (isDir) {
                return DropItemDirectory(
                  path,
                  const [],
                  extraAppleBookmark: bookmark,
                  fromPromise: fromPromise,
                );
              }
              return DropItemFile(
                path,
                extraAppleBookmark: bookmark,
                fromPromise: fromPromise,
              );
            }).toList(),
          ),
        );
        _offset = null;
        break;

      case "performOperation_linux":
        // gtk notify 'exit' before 'performOperation'.
        final args = call.arguments as List<dynamic>;
        final text = args[0] as String;
        final offset = (args[1] as List<dynamic>).cast<double>();
        final target = args.length > 2 ? args[2] as String? : null;
        // Fogata: inside the Flatpak, why the portal's files were not taken, if they were
        // not (see the plugin's send_drop).
        final refusal = args.length > 4
            ? switch (args[3]) {
                'declined' => LinuxPortalRefusal.declined,
                _ => LinuxPortalRefusal.unreached,
              }
            : null;
        // Fogata: only the files of this computer the list names (linuxDropPaths).
        _notifyEvent(
          DropDoneEvent(
            location: Offset(offset[0], offset[1]),
            files: refusal != null
                ? const []
                : linuxDropPaths(
                    target,
                    text,
                  ).map((e) => DropItemFile(e)).toList(),
            rawText: text,
            portalRefusal: refusal,
            portalCount: refusal != null ? args[4] as int : 0,
          ),
        );
        break;
      case "checkPortalDrop":
        // Fogata: inside the Flatpak, before the plugin asks the FileTransfer portal for a
        // drop's files (which gives this application each of them for good): -2 when no
        // drop target takes a drop where it is, -1 when its list cannot be judged
        // (portalListPaths) or there is no judge or it fails, else how many files the
        // application refuses (portalDropCheck), each counted once; the portal is asked
        // only for 0. A source that offers no list (null) is never asked: nothing judged
        // what its transfer holds.
        final args = call.arguments as List;
        final point = (args[0] as List).cast<double>();
        if (!holds(Offset(point[0], point[1]))) return -2;
        final check = portalDropCheck;
        if (check == null) return -1;
        final text = args[1] as String?;
        if (text == null) return -1;
        final entries = portalListPaths(text, (args[2] as List).cast<String>());
        if (entries == null) return -1;
        try {
          final refused = <String>{};
          for (final entry in entries) {
            if (await check(entry) > 0) refused.add(entry.first);
          }
          return refused.length;
        } catch (error) {
          debugPrint('desktop_drop: no judgement of the drop: $error');
          return -1;
        }
      case "performOperation_web":
        final results = (call.arguments as List)
            .cast<Map>()
            .map((e) => WebDropItem.fromJson(e.cast<String, dynamic>()))
            .map((e) => e.toDropItem())
            .toList();
        _notifyEvent(
          DropDoneEvent(location: _offset ?? Offset.zero, files: results),
        );
        _offset = null;
        break;
      default:
        throw UnimplementedError('${call.method} not implement.');
    }
    return null;
  }

  void _notifyEvent(DropEvent event) {
    for (final listener in _listeners) {
      listener(event);
    }
  }

  void addRawDropEventListener(RawDropListener listener) {
    assert(!_listeners.contains(listener));
    _listeners.add(listener);
  }

  void removeRawDropEventListener(RawDropListener listener) {
    assert(_listeners.contains(listener));
    _listeners.remove(listener);
  }
}

/// Fogata: the files of this computer that a Linux drop of [text], given as [target],
/// names. Only a `text/uri-list` (RFC 2483) names files: any other target (a browser's
/// `text/x-moz-url` or `_NETSCAPE_URL`, a file manager's `x-special/gnome-copied-files`,
/// plain text) or none gives nothing. Of the list, only its `file:` entries of this
/// computer are taken (no host, or `localhost`; no query or fragment), their
/// percent-encoding read; comments, web addresses, shares (`smb:`) and any other line are
/// left out instead of being handed on as a path. Lines end in CRLF, as the list's
/// standard says, or in LF alone, as some applications send them; a NUL that ends the text
/// is not part of it, and a line with a NUL elsewhere names no file.
List<String> linuxDropPaths(String? target, String text) {
  if (target != 'text/uri-list') return const [];
  final paths = <String>[];
  for (final raw in const LineSplitter().convert(text)) {
    final line = raw.replaceFirst(RegExp(r'\x00+$'), '').trim();
    if (line.isEmpty || line.startsWith('#') || line.contains('\x00')) {
      continue;
    }
    try {
      final uri = Uri.tryParse(line);
      if (uri == null ||
          uri.scheme != 'file' ||
          uri.hasQuery ||
          uri.hasFragment ||
          (uri.host.isNotEmpty && uri.host != 'localhost')) {
        continue;
      }
      // The list comes from a Linux desktop: its paths are POSIX ones wherever this runs.
      final path = Uri(
        scheme: 'file',
        path: uri.path,
      ).toFilePath(windows: false);
      if (path.isNotEmpty && !path.contains('\x00')) paths.add(path);
    } catch (error) {
      // An encoded slash or the like: no path of this computer.
      debugPrint('desktop_drop: not a file: $error');
    }
  }
  return paths;
}

/// Fogata: each entry of a drop's list of files ([text], a `text/uri-list`) as both readings
/// name it ([linuxDropPaths]'s path, then GIO's), to be judged
/// before the FileTransfer portal is asked for the drop, or null when it cannot be judged
/// and the portal must not be asked. The source hands the portal the files its own GIO
/// reads in that list, which takes as a file more than [linuxDropPaths] does (a `file:`
/// entry with a query, a fragment or any host; another scheme that gvfs mounts). So every
/// entry (each line but empty ones and comments) must be a file of this computer by
/// [linuxDropPaths], there must be one at least, and [read], the paths GIO reads there
/// (by the plugin, one per entry, "" where GIO reads none), must be one each and
/// absolute: an entry is refused when either reading is.
List<List<String>>? portalListPaths(String text, List<String> read) {
  final named = <String>[];
  for (final raw in const LineSplitter().convert(text)) {
    final line = raw.replaceFirst(RegExp(r'\x00+$'), '').trim();
    if (line.isEmpty || line.startsWith('#')) continue;
    final paths = linuxDropPaths('text/uri-list', line);
    if (paths.length != 1) return null;
    named.add(paths.single);
  }
  if (named.isEmpty ||
      read.length != named.length ||
      read.any((path) => !path.startsWith('/') || path.contains('\x00'))) {
    return null;
  }
  return [
    for (var i = 0; i < named.length; i++) [named[i], read[i]],
  ];
}
