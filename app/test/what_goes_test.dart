import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/screens/room.dart';

void main() {
  test('what closing a room takes away, in words', () {
    expect(whatGoes(0, 0), 'The room closes for everyone');
    expect(whatGoes(1, 0), '1 message goes');
    expect(whatGoes(3, 0), '3 messages go');
    expect(whatGoes(0, 1), '1 file you shared goes');
    expect(whatGoes(0, 2), '2 files you shared go');
    expect(whatGoes(1, 1), '1 message and 1 file go');
  });
}
