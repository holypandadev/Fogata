// A problem report: the log it carries keeps the steps and loses what is personal, and
// its archive opens.

import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/report.dart';
import 'package:fogata/src/zip.dart';

import 'private_writer.dart';
import 'unzip.dart';

const _peer = '12D3KooWGRUVh7MLpAFtJ6tXc1oFmDy2X1WbSmN4QZbWq9VtXk3A';
const _device =
    'b1946ac92492d2347c6235b4d2611184e2b6a1bda1c0e2c8a1f9d5c3b4a2e1f0';
const _secret = 'kQ7vR2xN9pL4mW8sT1zY6bC3dF5gH0jK2nP7qR9sU4w=';

/// A log as the core writes it, with everything a report must not carry.
const _log =
    '''
2026-10-30T09:12:01.120Z  INFO fogata_core::updates: build 3 (0.1.0-beta.3), linux-flatpak
2026-10-30T09:12:02.400Z  INFO fogata_net::node: friend 7Q4MK2XR9PMD3VHC online peer=$_peer path=direct address=/ip4/203.0.113.7/udp/40123/quic-v1/p2p/$_peer
2026-10-30T09:12:02.500Z  INFO fogata_net::node: member 7Q4M K2XR 9PMD 3VHC connected peer=$_peer address=/ip6/2001:db8::42/udp/4001/quic-v1
2026-10-30T09:12:03.000Z  WARN fogata_net::node: dial to 192.168.1.20:4001 and [fe80::1c2b:3cff:fe4d:5e6f]:4001 failed; port 51820 closed
2026-10-30T09:12:04.000Z  INFO fogata_net::node: joined the group Marta's Friends
2026-10-30T09:12:05.000Z  INFO fogata_net::node: Bea Ruiz asks to join a group
2026-10-30T09:12:06.000Z  WARN fogata_core: cannot save group hilltop gang: disk full
2026-10-30T09:12:06.500Z  WARN fogata_core: group hilltop (2) not loaded: bad file
2026-10-30T09:12:07.000Z  WARN fogata_net::group: the group key of "Gaming" is broken: 7Q4MK2XR9PMD3VHC gave this device a key that does not open
2026-10-30T09:12:08.000Z DEBUG fogata_net::node: call step signal=Offer { sdp: "v=0 o=- 1 2 IN IP4 203.0.113.7 a=ice-pwd:abc" } text="see you at 8, Leo"
2026-10-30T09:12:09.000Z  WARN fogata_core::direct: cannot remove /home/marta/.local/share/fogata/direct/$_device/files/beach holiday.jpg: busy
2026-10-30T09:12:10.000Z  WARN fogata_core: C:\\Users\\Marta\\Downloads\\final report.pdf is in use
2026-10-30T09:12:11.000Z  INFO fogata_ffi: shared birthday-photo.png with Leo; key $_secret device $_device
2026-10-30T09:12:12.000Z  INFO fogata_net: invite fogata://join/AbCdEf123456?k=$_secret and code FGT-7Q4MK-2XR9P from marta@example.com
2026-10-30T09:12:13.000Z ERROR app: Bad state: Leo’s chat “hi, are you coming?” failed
2026-10-30T09:12:14.000Z  WARN libp2p_mdns: found bea-laptop.local with fogata:friend:AQIDBAUG and 'Sofía and Ana'
''';

void main() {
  setUpAll(writePrivatelyInTests);
  final scrubber = Scrubber(['Leo', 'Marta', 'Gaming', "Marta's Friends", 'X']);
  final out = scrubber.scrub(_log);

  test('names of files with spaces, short keys, 0x hex and bare move codes go', () {
    final out = Scrubber(const []).scrub(
      'shared beach holiday.jpg and Picnic 2.png; token q83vEjRWeJCrze8SNFZ4mA== and '
      'q83vEjRWeJCrze8SNFZ4mA; secret 0x9f86d081884c7d659a2feaa0c55ad015; code '
      '7Q4MK2XR9P and 7q4mk2xr9p; took 1234567890 ms at 0x10 in Ed25519PublicKey',
    );
    for (final gone in [
      'beach',
      'Picnic',
      'q83vEjRW',
      '9f86d081884c',
      '7Q4MK2XR9P',
      '7q4mk2xr9p',
    ]) {
      expect(out, isNot(contains(gone)), reason: out);
    }
    for (final kept in [
      'shared ‹file› and ‹file›',
      '‹id 1›',
      '1234567890 ms',
      '0x10',
      'Ed25519PublicKey',
    ]) {
      expect(out, contains(kept), reason: out);
    }
  });

  test('nothing personal is left in a report\'s log', () {
    for (final gone in [
      // Message text and other text in quotes.
      'see you at 8', 'hi, are', 'coming', 'ice-pwd', 'IN IP4',
      // Names of people and groups, known here or said by the core.
      'Leo', 'Marta', 'Gaming', 'Friends', 'Bea', 'Ruiz', 'hilltop', 'gang',
      // Addresses and ports.
      '203.0.113.7',
      '192.168.1.20',
      '2001:db8',
      'fe80',
      '40123',
      '4001',
      '51820',
      // Full ids and keys.
      _peer, _device, _secret, '7Q4MK2XR9PMD3VHC', 'K2XR 9PMD',
      // Files and where they are.
      'beach',
      'holiday',
      '/home',
      '.local',
      'Users',
      'report',
      'birthday-photo',
      // Links, codes and e-mail addresses.
      'fogata://', 'AbCdEf123456', 'FGT-7Q4MK', 'example.com',
      'bea-laptop', 'AQIDBAUG', 'Sofía',
    ]) {
      expect(out, isNot(contains(gone)), reason: '"$gone" is still in:\n$out');
    }
  });

  test('the steps stay readable', () {
    for (final kept in [
      '2026-10-30T09:12:01.120Z  INFO fogata_core::updates: build 3 (0.1.0-beta.3), linux-flatpak',
      'friend ‹fingerprint 1› online peer=‹peer 1›',
      'path=direct address=/ip4/‹ip›/udp/‹port›/quic-v1/p2p/‹peer 1›',
      'member ‹fingerprint 1› connected peer=‹peer 1›',
      '/ip6/‹ip›/udp/‹port›/quic-v1',
      'dial to ‹ip› and ‹ip› failed; port ‹port› closed',
      'joined the group ‹name›',
      '‹name› asks to join a group',
      'cannot save group ‹name›: disk full',
      'not loaded: bad file',
      'device ‹id 1›',
      'is broken: ‹fingerprint 1› gave this device a key that does not open',
      'ERROR app: Bad state',
      "found ‹host› with ‹link› and '‹text›'",
    ]) {
      expect(out, contains(kept), reason: '"$kept" is missing from:\n$out');
    }
    expect(
      const LineSplitter().convert(out).length,
      const LineSplitter().convert(_log).length,
    );
  });

  test('a time of day is no address', () {
    expect(
      scrubber.line('2026-10-30T23:59:58.000Z  INFO x: took 12:30:45 of 10:00'),
      '2026-10-30T23:59:58.000Z  INFO x: took 12:30:45 of 10:00',
    );
  });

  test('only the lines of the last two days are kept', () {
    final since = DateTime.utc(2026, 10, 28, 12);
    final kept = recentLines(
      '2026-10-28T11:59:59.000Z  INFO a: old\n'
      '  a line of the old one\n'
      '2026-10-28T12:00:00.000Z  INFO a: new\n'
      '  a line of the new one\n',
      since,
    );
    expect(
      kept,
      '2026-10-28T12:00:00.000Z  INFO a: new\n  a line of the new one\n',
    );
  });

  test('the archive opens, with each file as written', () {
    final files = {
      'report.txt': utf8.encode('What happened:\nThe microphone stopped.\n'),
      'log/fogata-2026-10-30.log': utf8.encode(_log * 50),
      'empty.txt': <int>[],
    };
    final zip = ZipWriter();
    files.forEach((name, data) => zip.add(name, data));
    expect(unzip(zip.bytes()), files);
  });

  test(
    'a report stays under its size, its newest lines kept',
    timeout: const Timeout(Duration(minutes: 3)),
    () async {
      final dir = await Directory.systemTemp.createTemp('fogata-report-');
      addTearDown(() => dir.delete(recursive: true));
      final data = Directory('${dir.path}/data/$logFolder')
        ..createSync(recursive: true);
      final now = DateTime.utc(2026, 10, 30, 12);
      // Lines that hardly compress.
      final random = Random(7);
      final big = StringBuffer();
      for (var i = 0; i < 300000; i++) {
        final start = now.subtract(Duration(milliseconds: (300000 - i) * 500));
        final noise = String.fromCharCodes([
          for (var k = 0; k < 40; k++) 97 + random.nextInt(26),
        ]);
        big.writeln('${start.toIso8601String()}  INFO a: step $i $noise');
      }
      File('${data.path}/fogata-2026-10-30.log').writeAsStringSync('$big');
      File('${data.path}/fogata-2026-10-20.log').writeAsStringSync('$big');
      final report = await prepareReport(
        dataDir: '${dir.path}/data',
        what: 'It froze.',
        withLog: true,
        facts: ReportFacts(build: 3, language: 'es', at: now),
      );
      final path = await writeReport(folder: '${dir.path}/out', report: report);
      expect(path, endsWith('fogata-report-2026-10-30.zip'));
      final bytes = File(path).readAsBytesSync();
      expect(bytes.length, lessThanOrEqualTo(reportMaxBytes));
      final files = unzip(bytes);
      expect(files.keys, ['report.txt', 'log/fogata-2026-10-30.log']);
      // What the page shows is what the archive holds, byte for byte.
      expect({
        for (final MapEntry(:key, :value) in files.entries)
          key: utf8.decode(value),
      }, report.files);
      final text = utf8.decode(files['report.txt']!);
      expect(text, contains('Version: '));
      expect(text, contains('(build 3)'));
      expect(text, contains('It froze.'));
      final log = utf8.decode(files['log/fogata-2026-10-30.log']!);
      expect(log, contains('step 299999 '));
      expect(log, isNot(contains('step 0 ')));
      // A second report the same day takes a name of its own.
      final again = await writeReport(
        folder: '${dir.path}/out',
        report: await prepareReport(
          dataDir: '${dir.path}/data',
          what: 'Again.',
          withLog: false,
          facts: ReportFacts(build: null, language: 'en', at: now),
        ),
      );
      expect(again, endsWith('fogata-report-2026-10-30-2.zip'));
      expect(unzip(File(again).readAsBytesSync()).keys, ['report.txt']);
    },
  );

  test('a report is never written through a link where it goes', () async {
    final dir = Directory.systemTemp.createTempSync('fogata-report-link-');
    addTearDown(() => dir.deleteSync(recursive: true));
    final out = Directory('${dir.path}/out')..createSync();
    final now = DateTime.utc(2026, 10, 30, 12);
    final theirs = File('${dir.path}/theirs')..writeAsStringSync('keep');
    Link('${out.path}/fogata-report-2026-10-30.zip').createSync(theirs.path);
    // A link to nothing yet: writing through it would make a file elsewhere.
    Link('${out.path}/fogata-report-2026-10-30-2.zip')
        .createSync('${dir.path}/elsewhere.zip');
    final path = await writeReport(
      folder: out.path,
      report: await prepareReport(
        dataDir: '${dir.path}/data',
        what: 'It froze.',
        withLog: false,
        facts: ReportFacts(build: 3, language: 'en', at: now),
      ),
    );
    expect(path, endsWith('fogata-report-2026-10-30-3.zip'));
    expect(theirs.readAsStringSync(), 'keep');
    expect(File('${dir.path}/elsewhere.zip').existsSync(), isFalse);
    expect(
      FileSystemEntity.typeSync(path, followLinks: false),
      FileSystemEntityType.file,
    );
  });

  test('a window\'s error is written with nothing personal in it', () async {
    final dir = Directory.systemTemp.createTempSync('fogata-app-log-');
    addTearDown(() => dir.deleteSync(recursive: true));
    noteAppError(
      dir.path,
      'FileSystemException: Cannot open file, path = '
      '\'/home/marta/Pictures/beach.jpg\' (dial 203.0.113.7:4001, "hello")',
      StackTrace.fromString('#0 open (package:fogata/src/files.dart:120:7)'),
    );
    await flushAppErrors();
    final written = Directory('${dir.path}/$logFolder')
        .listSync()
        .whereType<File>()
        .single
        .readAsStringSync();
    for (final gone in ['marta', 'beach', '203.0.113.7', '4001', 'hello']) {
      expect(written, isNot(contains(gone)), reason: written);
    }
    expect(written, contains('ERROR app: FileSystemException: Cannot open'));
    expect(written, contains('files.dart:120:7'));
  });

  test(
    'a window\'s error loses the names known, also those forgotten since',
    () async {
      final dir = Directory.systemTemp.createTempSync('fogata-app-log-');
      addTearDown(() => dir.deleteSync(recursive: true));
      // A group seen before, left since: no longer among the names known now.
      rememberNames(['The Hillside']);
      appErrorNames = () => rememberNames(['Marta Ruiz']);
      addTearDown(() => appErrorNames = () => const []);
      noteAppError(dir.path, 'Bad state: Marta Ruiz left The Hillside', null);
      await flushAppErrors();
      final written = Directory('${dir.path}/$logFolder')
          .listSync()
          .whereType<File>()
          .single
          .readAsStringSync();
      for (final gone in ['Marta', 'Ruiz', 'Hillside']) {
        expect(written, isNot(contains(gone)), reason: written);
      }
      expect(written, contains('ERROR app: Bad state: ‹name› left ‹name›'));
    },
  );

  test('an error that repeats every frame is written once a second', () async {
    final dir = Directory.systemTemp.createTempSync('fogata-app-log-');
    addTearDown(() => dir.deleteSync(recursive: true));
    final log = Directory('${dir.path}/$logFolder');
    String written() => log.existsSync()
        ? log.listSync().whereType<File>().single.readAsStringSync()
        : '';
    final stack = StackTrace.fromString(
      '#0 layout (package:fogata/a.dart:1:1)',
    );
    // Noting is not waiting for the disk: the first goes out in the background, the
    // others wait to be written together.
    for (var frame = 0; frame < 500; frame++) {
      noteAppError(dir.path, 'RenderFlex overflowed', stack);
    }
    await flushAppErrors();
    final text = written();
    expect(
      RegExp('RenderFlex overflowed').allMatches(text).length,
      lessThanOrEqualTo(2),
      reason: text,
    );
    expect(text, matches(RegExp(r'this came \d+ times')));
    expect(text.length, lessThan(1000));
  });

  test('only a page or an e-mail opens outside Fogata', () async {
    final asked = <List<String>>[];
    Future<ProcessResult> run(String program, List<String> args) async {
      asked.add([program, ...args]);
      return ProcessResult(0, 0, '', '');
    }

    for (final uri in [
      'file:///etc/passwd',
      'FILE:///C:/Windows/System32/calc.exe',
      'smb://server/share/run.exe',
      'javascript:alert(1)',
      'custom-scheme://run/1',
      'http://example.com',
    ]) {
      expect(
        await openExternal(Uri.parse(uri), run: run),
        isFalse,
        reason: uri,
      );
    }
    expect(asked, isEmpty);
    expect(
      await openExternal(Uri.parse('https://ko-fi.com/holypandadev'), run: run),
      isTrue,
    );
    expect(asked.single.last, 'https://ko-fi.com/holypandadev');
  });

  test('the e-mail is addressed to the team and names the file', () {
    final uri = reportMail('fogata-report.zip');
    expect(uri.scheme, 'mailto');
    expect(uri.path, reportAddress);
    final text = uri.toString();
    expect(text, contains('subject=Fogata%20'));
    expect(text, contains('fogata-report.zip'));
    expect(text, isNot(contains('+')));
  });
}
