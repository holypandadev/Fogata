import 'package:desktop_drop/src/drop_item.dart';
import 'package:flutter/painting.dart';

abstract class DropEvent {
  Offset location;

  DropEvent(this.location);

  @override
  String toString() {
    return '$runtimeType($location)';
  }
}

class DropEnterEvent extends DropEvent {
  DropEnterEvent({required Offset location}) : super(location);
}

class DropExitEvent extends DropEvent {
  DropExitEvent({required Offset location}) : super(location);
}

class DropUpdateEvent extends DropEvent {
  DropUpdateEvent({required Offset location}) : super(location);
}

class DropDoneEvent extends DropEvent {
  final List<DropItem> files;

  /// Fogata: the text of a Linux drop as the plugin handed it on, a `text/uri-list` (one
  /// URI per line), whatever lines of it name no file of this computer: [files] has only
  /// those that do. It is never a portal's transfer key: inside the Flatpak the plugin
  /// hands on the files the FileTransfer portal gave as such a list (see FOGATA.md).
  ///
  /// Is `null` on platforms where raw text isn't exposed (Windows,
  /// macOS, or when the platform channel doesn't provide it).
  final String? rawText;

  /// Fogata: inside the Flatpak, when the files a source offered through the FileTransfer
  /// portal were not taken, why: [LinuxPortalRefusal.declined], the drop's list of files
  /// named [portalCount] files the application refuses (`DesktopDrop.portalDropCheck`), so
  /// the portal was not asked and nothing of the drop was exported to the sandbox; or
  /// [LinuxPortalRefusal.unreached], the portal gave none of its [portalCount] files in
  /// time. [files] is empty then. Null for any other drop.
  final LinuxPortalRefusal? portalRefusal;

  /// Fogata: how many files [portalRefusal] is about.
  final int portalCount;

  DropDoneEvent({
    required Offset location,
    required this.files,
    this.rawText,
    this.portalRefusal,
    this.portalCount = 0,
  }) : super(location);

  @override
  String toString() {
    return '$runtimeType($location, $files, rawText: ${rawText != null ? 'present' : 'null'})';
  }
}

/// Fogata: why the files a source offered through the FileTransfer portal were not taken
/// (see [DropDoneEvent.portalRefusal]).
enum LinuxPortalRefusal { declined, unreached }
