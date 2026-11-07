import 'package:fogata/src/screens/donate.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('the donation button goes to Fogata\'s Ko-fi page, over https', () {
    final uri = Uri.parse(kofiUrl);
    expect(uri.scheme, 'https');
    expect(uri.host, 'ko-fi.com');
    expect(uri.path, '/holypandadev');
  });
}
