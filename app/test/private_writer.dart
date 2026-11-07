import 'dart:io';

import 'package:fogata/src/private_file.dart';

/// Tests run without the core's library: files the window writes privately are written as
/// the core does, through a temporary file renamed into place, by Dart.
void writePrivatelyInTests() {
  privateWriter = (path, bytes) async {
    final temporary = File('$path.test.tmp');
    await temporary.writeAsBytes(bytes, flush: true);
    await temporary.rename(path);
  };
}
