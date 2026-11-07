import 'dart:async';
import 'dart:io';

import 'package:desktop_drop/desktop_drop.dart' show linuxDropPaths;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/host.dart';
import 'package:fogata/src/outgoing.dart';
import 'package:fogata/src/private_places.dart';
import 'package:fogata/src/theme.dart';

void main() {
  bool private(String path, HostSystem system, [String home = '/home/leo']) =>
      isPrivatePlace(path, system: system, home: home);

  test('hidden names and private places are told from the usual ones', () {
    for (final path in [
      '/home/leo/.ssh/id_ed25519',
      '/home/leo/.gnupg/private-keys-v1.d/key.key',
      '/home/leo/.browser/abc.default/logins.json',
      '/home/leo/.config/web-browser/Default/Cookies',
      '/home/leo/.local/share/keyrings/login.keyring',
      '/home/leo/Pictures/.hidden/photo.jpg',
      '/home/leo/.env',
      // `..` is worked out before the names are read.
      '/home/leo/Pictures/../.ssh/id_rsa',
      '/home/leo/./.ssh/id_rsa',
      '/etc/shadow',
      '/root/notes.txt',
      '/proc/self/environ',
      '/var/lib/secret',
    ]) {
      expect(private(path, HostSystem.linux), isTrue, reason: path);
    }
    for (final path in [
      '/home/leo/Pictures/blue lake.jpg',
      '/home/leo/Documents/plan.pdf',
      '/home/leo/Pictures/a..b.png',
      '/run/user/1000/doc/ab12/plan.pdf',
      '/run/media/leo/USB/trip.mp4',
      '/tmp/plan.pdf',
      '/var/tmp/plan.pdf',
      '/home/leo/Pictures/Library/x.png',
    ]) {
      expect(private(path, HostSystem.linux), isFalse, reason: path);
    }
  });

  test(
    'macOS keeps its Library and keychains private, but not its cloud drive',
    () {
      const home = '/Users/leo';
      for (final path in [
        '/Users/leo/Library/Keychains/login.keychain-db',
        '/Users/leo/Library/Application Support/Browser/Profiles/x/logins.json',
        '/Users/leo/Library/Messages/chat.db',
        '/Library/Keychains/System.keychain',
        '/private/etc/master.passwd',
        '/private/var/db/dslocal/x',
        '/Users/leo/.ssh/id_rsa',
      ]) {
        expect(private(path, HostSystem.macos, home), isTrue, reason: path);
      }
      for (final path in [
        '/Users/leo/Library/Mobile Documents/com~apple~CloudDocs/plan.pdf',
        '/Users/leo/Documents/plan.pdf',
        '/Users/leo/Desktop/Screenshot.png',
        '/private/var/folders/ab/T/plan.pdf',
        '/Volumes/USB/trip.mp4',
      ]) {
        expect(private(path, HostSystem.macos, home), isFalse, reason: path);
      }
    },
  );

  test('Windows keeps AppData and the system folders private', () {
    const home = r'C:\Users\leo';
    for (final path in [
      r'C:\Users\leo\AppData\Local\Browser\User Data\Default\Login Data',
      r'C:\Users\leo\NTUSER.DAT',
      r'C:\Windows\System32\config\SAM',
      r'C:\ProgramData\x\secret',
      r'C:\Users\leo\.ssh\id_rsa',
      'C:/Users/leo/.ssh/id_rsa',
    ]) {
      expect(private(path, HostSystem.windows, home), isTrue, reason: path);
    }
    for (final path in [
      r'C:\Users\leo\Documents\plan.pdf',
      r'D:\Photos\blue lake.jpg',
    ]) {
      expect(private(path, HostSystem.windows, home), isFalse, reason: path);
    }
  });

  test(
    'Linux keeps /run, backups and mail private, not the portal or media',
    () {
      for (final path in [
        '/var/backups/shadow.bak',
        '/var/mail/leo',
        '/run/user/1000/bus',
        '/run/user/1000/gnupg/S.gpg-agent',
        '/run/user/1000',
        '/run/secrets/token',
        '/run/user/1000/doc/../keyring/control',
      ]) {
        expect(private(path, HostSystem.linux), isTrue, reason: path);
      }
      for (final path in [
        '/run/user/1000/doc/ab12/plan.pdf',
        '/run/media/leo/USB/trip.mp4',
        '/var/tmp/plan.pdf',
        '/var/backups-of-mine/plan.pdf',
      ]) {
        expect(private(path, HostSystem.linux), isFalse, reason: path);
      }
      // Names differ by case on Linux.
      expect(private('/ETC/shadow', HostSystem.linux), isFalse);
    },
  );

  test('macOS and Windows compare names without telling the case', () {
    for (final path in [
      '/Users/leo/library/Keychains/login.keychain-db',
      '/users/LEO/LIBRARY/Messages/chat.db',
      '/system/Library/x',
      '/PRIVATE/etc/master.passwd',
      '/library/keychains/System.keychain',
    ]) {
      expect(
        private(path, HostSystem.macos, '/Users/leo'),
        isTrue,
        reason: path,
      );
    }
    for (final path in [
      r'c:\windows\system32\config\sam',
      r'C:\USERS\leo\APPDATA\Local\x',
      r'C:\PROGRAM FILES\x',
      r'C:\Users\leo\ntuser.DAT.LOG1',
    ]) {
      expect(
        private(path, HostSystem.windows, r'C:\Users\leo'),
        isTrue,
        reason: path,
      );
    }
  });

  test('macOS File Provider and cloud drive folders in the Library are not private', () {
    const home = '/Users/leo';
    for (final path in [
      '/Users/leo/Library/CloudStorage/SyncService/plan.pdf',
      '/Users/leo/Library/CloudStorage/SyncService-leo@example.com/Shared/a.png',
      '/Users/leo/Library/Mobile Documents/com~apple~CloudDocs/plan.pdf',
      '/users/leo/library/cloudstorage/OtherSync/plan.pdf',
    ]) {
      expect(private(path, HostSystem.macos, home), isFalse, reason: path);
    }
    // The rest of the Library is private.
    expect(
      private('/Users/leo/Library/Caches/x', HostSystem.macos, home),
      isTrue,
    );
    expect(
      private('/Users/leo/Library/CloudStorageX/y', HostSystem.macos, home),
      isTrue,
    );
  });

  test('Windows system folders on any drive, and UNC paths', () {
    const home = r'C:\Users\leo';
    for (final path in [
      r'D:\Windows\System32\config\SAM',
      r'E:\ProgramData\x',
      r'\\?\C:\Windows\System32\x',
      r'\\.\D:\Windows\x',
      r'\\server\share\Windows\x',
      r'\\server\C$\Windows\x',
      r'\\?\UNC\server\share\ProgramData\x',
      r'\\server\share\Users\leo\AppData\Local\x',
      '//server/share/Windows/x',
    ]) {
      expect(private(path, HostSystem.windows, home), isTrue, reason: path);
    }
    for (final path in [
      r'D:\Photos\Windows wallpapers\a.jpg',
      r'\\server\Windows\plan.pdf',
      r'\\server\share\Photos\a.jpg',
      r'\\?\D:\Photos\a.jpg',
      r'\\?\UNC\server\share\Photos\a.jpg',
    ]) {
      expect(private(path, HostSystem.windows, home), isFalse, reason: path);
    }
  });

  test(
    'in macOS\'s sandbox, the Library under the account\'s folder is private',
    () {
      const container =
          '/Users/leo/Library/Containers/dev.holypanda.fogata/Data';
      final homes = homesOf(container, account: '/Users/leo');
      expect(homes, [container, '/Users/leo']);
      // Without the account's folder, the container's owner is found from its path.
      expect(homesOf(container), [container, '/Users/leo']);
      expect(homesOf('/Users/leo', account: '/Users/leo'), ['/Users/leo']);
      bool private(String path) => homes.any(
        (home) => isPrivatePlace(path, system: HostSystem.macos, home: home),
      );
      for (final path in [
        '/Users/leo/Library/Mail/V10/x.emlx',
        '/Users/leo/Library/Messages/chat.db',
        '/Users/leo/Library/Cookies/Cookies.binarycookies',
        '/Users/leo/Library/Application Support/Browser/Default/Cookies',
      ]) {
        expect(private(path), isTrue, reason: path);
        // Under the container alone it would not be.
        expect(
          isPrivatePlace(path, system: HostSystem.macos, home: container),
          isFalse,
          reason: path,
        );
      }
      expect(private('/Users/leo/Documents/plan.pdf'), isFalse);
    },
  );

  test('names written another way are read as the system reads them', () {
    // Windows drops the dots and spaces that end a name, and knows short names.
    const home = r'C:\Users\leo';
    for (final path in [
      r'C:\Users\leo\AppData.\Local\x',
      r'C:\Users\leo\AppData \Local\x',
      r'C:\Users\leo\AppData. .\Local\x',
      r'C:\Users\leo\APPDAT~1\Local\x',
      r'C:\PROGRA~1\x',
      r'C:\PROGRA~2\x',
      r'C:\Windows.\System32\config\SAM',
      r'C:\Users\leo\.ssh.\id_rsa',
    ]) {
      expect(private(path, HostSystem.windows, home), isTrue, reason: path);
    }
    expect(
      private(r'C:\Users\leo\Docs\..\AppData\x', HostSystem.windows, home),
      isTrue,
    );
    expect(
      private(r'C:\Users\leo\Docs.\plan.pdf', HostSystem.windows, home),
      isFalse,
    );
    // APFS takes the long s and the Kelvin sign as s and k.
    for (final path in [
      '/\u017Fystem/Library/x',
      '/Library/\u212Aeychains/System.keychain',
      '/private/etc/ma\u017Fter.passwd',
      '/Users/leo/Library/Mobile Documents/../\u212Aeychains/x',
    ]) {
      expect(
        private(path, HostSystem.macos, '/Users/leo'),
        isTrue,
        reason: path,
      );
    }
  });

  test(
    'a short name on Windows is judged by the folder it stands for',
    () async {
      final dir = Directory.systemTemp.createTempSync('fogata_short');
      addTearDown(() => dir.deleteSync(recursive: true));
      final folder = Directory('${dir.path}\\AppData Local Long')..createSync();
      final file = File('${folder.path}\\x.txt')..writeAsStringSync('x');
      // `dir` itself may be a short name: what matters is that the real one is read.
      final real = await file.resolveSymbolicLinks();
      expect(real.toLowerCase(), contains('appdata local long'));
    },
    skip: !Platform.isWindows,
  );

  group('on disk', () {
    late Directory dir;
    late String secret, plain, link;
    setUp(() {
      dir = Directory.systemTemp.createTempSync('fogata_private');
      Directory('${dir.path}/.ssh').createSync();
      Directory('${dir.path}/Pictures').createSync();
      secret = '${dir.path}/.ssh/id_ed25519';
      plain = '${dir.path}/Pictures/lake.jpg';
      link = '${dir.path}/Pictures/innocent.jpg';
      File(secret).writeAsStringSync('key');
      File(plain).writeAsStringSync('photo');
      Link(link).createSync(secret);
    });
    tearDown(() => dir.deleteSync(recursive: true));

    test('a link in a plain folder is judged by where it leads', () async {
      expect(await privatePaths([plain, link, secret]), [link, secret]);
    });
  }, skip: Platform.isWindows);

  group('what browsers drop on Linux', () {
    // The payloads browsers hand over on X11 and on Wayland when a page starts
    // a drag that names files, as GTK gives them to the drop plugin: the target and its
    // bytes as text. Only a text/uri-list names files; every other target gives nothing.
    const ssh = '/home/leo/.ssh/id_ed25519';
    const plain = '/home/leo/Pictures/blue lake.jpg';

    test('a text/uri-list with CRLF, comments and percent-encoding', () {
      // Most browsers: CRLF after each line, spaces encoded.
      expect(
        linuxDropPaths(
          'text/uri-list',
          'file:///home/leo/.ssh/id_ed25519\r\n'
              'file:///home/leo/Pictures/blue%20lake.jpg\r\n',
        ),
        [ssh, plain],
      );
      // Some (on Wayland) leave the last CRLF out; comments are the list's own.
      expect(
        linuxDropPaths(
          'text/uri-list',
          '# dragged from a page\r\nfile:///home/leo/.ssh/id_ed25519',
        ),
        [ssh],
      );
      // LF alone, a NUL at the end, `localhost`, `file:` with one slash, upper case.
      expect(
        linuxDropPaths(
          'text/uri-list',
          'file://localhost/home/leo/.ssh/id_ed25519\n'
              'file:/home/leo/.ssh/id_ed25519\n'
              'FILE:///home/leo/.ssh/id_ed25519\n\x00',
        ),
        [ssh, ssh, ssh],
      );
      // Dots and names encoded: decoded before anything is judged.
      expect(
        linuxDropPaths(
          'text/uri-list',
          'file:///home/leo/%2Essh/id_ed25519\r\n'
              'file:///home/leo/Pictures/%2E%2E/.ssh/id_ed25519\r\n',
        ),
        // The address's own rules work the dots out already.
        [ssh, ssh],
      );
    });

    test('lines that name no file of this computer are left out', () {
      expect(
        linuxDropPaths(
          'text/uri-list',
          'https://example.com/a.png\r\n'
              'file://example.com/home/leo/.ssh/id_ed25519\r\n'
              'smb://server/share/a.png\r\n'
              'file:///home/leo/a.png?x=1\r\n'
              'file:///home/leo/a.png#x\r\n'
              'file:///home/leo/..%2F.ssh/id_ed25519\r\n'
              'file:///home/leo/a%00b\r\n'
              'copy\r\n'
              '/home/leo/.ssh/id_ed25519\r\n',
        ),
        isEmpty,
      );
    });

    test('other targets give nothing, whatever they hold', () {
      final payloads = {
        // One browser's own target: the address and its title, in UTF-16.
        'text/x-moz-url': 'file:///home/leo/.ssh/id_ed25519\nid_ed25519',
        // Browsers on X11: the address and its title.
        '_NETSCAPE_URL': 'file:///home/leo/.ssh/id_ed25519\nid_ed25519',
        // File managers (and a page can offer it too): what to do, then the list.
        'x-special/gnome-copied-files':
            'copy\nfile:///home/leo/.ssh/id_ed25519',
        'text/plain': 'file:///home/leo/.ssh/id_ed25519',
        'UTF8_STRING': '/home/leo/.ssh/id_ed25519',
      };
      for (final MapEntry(key: target, value: text) in payloads.entries) {
        expect(linuxDropPaths(target, text), isEmpty, reason: target);
      }
      expect(linuxDropPaths(null, 'file:///home/leo/.ssh/id_ed25519'), isEmpty);
    });

    test('every path a list can carry into .ssh is private', () {
      for (final text in [
        'file:///home/leo/.ssh/id_ed25519',
        'file:///home/leo/%2Essh/id_ed25519',
        'file:///home/leo/Pictures/%2E%2E/.ssh/id_ed25519',
        'file:///home/leo/Pictures/../.ssh/id_ed25519',
        'file://localhost/home/leo/./.ssh/id_ed25519',
        'file:///etc/shadow',
        'file:///proc/self/environ',
      ]) {
        final paths = linuxDropPaths('text/uri-list', text);
        expect(paths, hasLength(1), reason: text);
        expect(private(paths.single, HostSystem.linux), isTrue, reason: text);
      }
    });
  });

  group('dropping on the room', () {
    late Directory dir;
    late String secret, plain, link;
    setUp(() {
      dir = Directory.systemTemp.createTempSync('fogata_private');
      Directory('${dir.path}/.ssh').createSync();
      Directory('${dir.path}/Pictures').createSync();
      secret = '${dir.path}/.ssh/id_ed25519';
      plain = '${dir.path}/lake.jpg';
      link = '${dir.path}/Pictures/innocent.jpg';
      File(secret).writeAsStringSync('key');
      File(plain).writeAsStringSync('photo');
      Link(link).createSync(secret);
    });
    tearDown(() => dir.deleteSync(recursive: true));

    Future<AppState> pump(WidgetTester tester) async {
      final state = AppState('/nonexistent/fogata-private-drop-test');
      state.roomState.roomGroup = 'group';
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: Scaffold(
              body: DropZone(
                state: state,
                enabled: true,
                child: const SizedBox.expand(),
              ),
            ),
          ),
        ),
      );
      return state;
    }

    /// Drops [text] as GTK hands it over, as [target]; [meanwhile] runs once the drop is
    /// taken, while its files are still being looked at.
    Future<void> dropText(
      WidgetTester tester,
      String text, {
      String target = 'text/uri-list',
      void Function()? meanwhile,
    }) async {
      // In the real zone: the files are read for real, outside the test's own clock.
      await tester.runAsync(() async {
        final done = Completer<ByteData?>();
        await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
          'desktop_drop',
          const StandardMethodCodec().encodeMethodCall(
            MethodCall('performOperation_linux', [
              text,
              [10.0, 20.0],
              target,
            ]),
          ),
          done.complete,
        );
        await done.future;
        meanwhile?.call();
        await Future<void>.delayed(const Duration(milliseconds: 200));
      });
      await tester.pump();
    }

    Future<void> drop(WidgetTester tester, List<String> paths) => dropText(
      tester,
      [for (final p in paths) Uri.file(p).toString()].join('\r\n'),
    );

    List<String> waiting(AppState state) => [
      for (final f in state.roomState.outgoingHere) f.path,
    ];

    testWidgets(
      'a hidden file is left out, and the person told how to share it',
      (tester) async {
        final state = await pump(tester);
        await drop(tester, [secret, plain]);
        expect(waiting(state), [plain]);
        expect(
          state.toasts.list.map((t) => t.text),
          contains(contains('pick it with the paperclip')),
        );
      },
      skip: !Platform.isLinux,
    );

    testWidgets('so is a link in a plain folder that leads into .ssh', (
      tester,
    ) async {
      final state = await pump(tester);
      await drop(tester, [link]);
      expect(waiting(state), isEmpty);
    }, skip: !Platform.isLinux);

    testWidgets('and one named with .. or percent-encoding', (tester) async {
      final state = await pump(tester);
      final encoded = Uri.file(secret)
          .toString()
          .replaceFirst('.ssh', '%2Essh');
      final dotted = Uri.file('${dir.path}/Pictures').toString();
      await dropText(
        tester,
        '# a page\r\n$encoded\r\n$dotted/%2E%2E/.ssh/id_ed25519\r\n',
      );
      expect(waiting(state), isEmpty);
    }, skip: !Platform.isLinux);

    testWidgets('files in the usual places are added without a word', (
      tester,
    ) async {
      final state = await pump(tester);
      await drop(tester, [plain]);
      expect(waiting(state), [plain]);
      expect(state.toasts.list, isEmpty);
    }, skip: !Platform.isLinux);

    testWidgets('a browser\'s other targets add nothing', (tester) async {
      final state = await pump(tester);
      for (final target in [
        'text/x-moz-url',
        '_NETSCAPE_URL',
        'x-special/gnome-copied-files',
      ]) {
        await dropText(tester, Uri.file(plain).toString(), target: target);
      }
      expect(waiting(state), isEmpty);
    }, skip: !Platform.isLinux);

    testWidgets('drops are added in the order they came', (tester) async {
      final state = await pump(tester);
      final other = '${dir.path}/other.jpg';
      File(other).writeAsStringSync('photo');
      await drop(tester, [secret, plain]);
      await drop(tester, [other]);
      expect(waiting(state), [plain, other]);
    }, skip: !Platform.isLinux);

    testWidgets('a drop is dropped if the room changed meanwhile', (
      tester,
    ) async {
      final state = await pump(tester);
      await dropText(
        tester,
        Uri.file(plain).toString(),
        meanwhile: () => state.roomState.roomGroup = 'other',
      );
      expect(state.roomState.outgoing, isEmpty);
    }, skip: !Platform.isLinux);
  });
}
