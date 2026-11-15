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

  /// Fogata: the text of a Linux drop as the plugin received it, a `text/uri-list` (one
  /// URI per line), whatever lines of it name no file of this computer: [files] has only
  /// those that do. This copy does not take the FileTransfer portal's target, so it is
  /// never a portal's transfer key (see FOGATA.md).
  ///
  /// Is `null` on platforms where raw text isn't exposed (Windows,
  /// macOS, or when the platform channel doesn't provide it).
  final String? rawText;

  DropDoneEvent({
    required Offset location,
    required this.files,
    this.rawText,
  }) : super(location);

  @override
  String toString() {
    return '$runtimeType($location, $files, rawText: ${rawText != null ? 'present' : 'null'})';
  }
}
