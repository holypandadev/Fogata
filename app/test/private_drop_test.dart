import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:desktop_drop/desktop_drop.dart'
    show DesktopDrop, linuxDropPaths, portalListPaths;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/host.dart';
import 'package:fogata/src/l10n.dart';
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
    'Linux keeps /run, the portal folder, backups and mail private, not media',
    () {
      for (final path in [
        '/var/backups/shadow.bak',
        '/var/mail/leo',
        '/run/user/1000/bus',
        '/run/user/1000/gnupg/S.gpg-agent',
        '/run/user/1000',
        '/run/secrets/token',
        '/run/user/1000/doc/../keyring/control',
        // The document portal's folder: outside a sandbox it holds every document ever
        // given to any sandboxed application (inside the Flatpak a document is judged by
        // its real place instead).
        '/run/user/1000/doc/ab12/plan.pdf',
        '/run/flatpak/doc/ab12/plan.pdf',
      ]) {
        expect(private(path, HostSystem.linux), isTrue, reason: path);
      }
      for (final path in [
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

  group('inside the Flatpak, the document portal\'s files', () {
    // The real places the fake portal knows, by id, as this application was given them.
    late Map<String, String> places;
    late List<List<String>> asked;
    const home = ['/home/leo'];
    setUp(() {
      inFlatpak = true;
      asked = [];
      places = {
        'a1b2c3d4': '/home/leo/Documents/plan.pdf',
        'ssh00001': '/home/leo/.ssh/id_ed25519',
        'profile1': '/home/leo/.mozilla/firefox/x.default/logins.json',
        'dotenv01': '/home/leo/Projects/.env',
        'etc00001': '/etc/shadow',
        'other001': '/home/leo/Documents/other.pdf',
        'relative': 'Documents/plan.pdf',
        'portal01': '/run/user/1000/doc/a1b2c3d4/plan.pdf',
        'dotdot01': '/home/leo/Documents/../.ssh/plan.pdf',
      };
      documentHostPaths = (ids) async {
        asked.add(ids);
        return {
          for (final id in ids)
            if (places[id] case final place?)
              id: Uint8List.fromList(utf8.encode(place)),
        };
      };
    });
    tearDown(() {
      inFlatpak = Platform.isLinux && File('/.flatpak-info').existsSync();
      documentHostPaths = DesktopDrop.instance.documentHostPaths;
      resolveLinks = (path) => File(path).resolveSymbolicLinks();
    });

    Future<List<String>> privateOf(List<String> paths) =>
        privatePaths(paths, homes: home);

    test('the portal is asked once for a whole drop', () async {
      // Each spelling of the folder, a link that leads to one, and files of the folder
      // that are not documents: one question, with each id once.
      resolveLinks = (path) async => path == '/home/leo/Desktop/plan.pdf'
          ? '/run/flatpak/doc/a1b2c3d4/plan.pdf'
          : path.replaceFirst('/run/user/1000/doc', '/run/flatpak/doc');
      final judged = await judgeDropped([
        '/run/user/1000/doc/a1b2c3d4/plan.pdf',
        '/run/flatpak/doc/a1b2c3d4/plan.pdf',
        '/home/leo/Desktop/plan.pdf',
        '/run/user/1000/doc/ssh00001/id_ed25519',
        '/run/user/1000/doc/a1b2c3d4/Photos/x.jpg',
      ], homes: home);
      expect(asked, [
        ['a1b2c3d4', 'ssh00001'],
      ]);
      expect(judged.private, [
        '/run/user/1000/doc/ssh00001/id_ed25519',
        '/run/user/1000/doc/a1b2c3d4/Photos/x.jpg',
      ]);
      expect(judged.real, {
        '/run/user/1000/doc/a1b2c3d4/plan.pdf': '/home/leo/Documents/plan.pdf',
        '/run/flatpak/doc/a1b2c3d4/plan.pdf': '/home/leo/Documents/plan.pdf',
        '/home/leo/Desktop/plan.pdf': '/home/leo/Documents/plan.pdf',
      });
      // A portal that does not answer costs one wait for the drop, not one a file.
      asked.clear();
      documentHostPaths = (ids) async {
        asked.add(ids);
        throw PlatformException(code: 'portal', message: 'Timeout was reached');
      };
      final unanswered = await judgeDropped([
        '/run/user/1000/doc/a1b2c3d4/plan.pdf',
        '/run/user/1000/doc/ssh00001/id_ed25519',
      ], homes: home);
      expect(asked, hasLength(1));
      expect(unanswered.unknown, hasLength(2));
    });

    test('a link is judged where it leads, by the portal when it leads there', () async {
      resolveLinks = (path) async => switch (path) {
        // Flatpak's own link of the folder, and /var/run, which leads to /run.
        '/run/user/1000/doc/ssh00001/id_ed25519' =>
          '/run/flatpak/doc/ssh00001/id_ed25519',
        '/var/run/user/1000/doc/ssh00001/id_ed25519' =>
          '/run/flatpak/doc/ssh00001/id_ed25519',
        '/var/run/user/1000/doc/a1b2c3d4/plan.pdf' =>
          '/run/flatpak/doc/a1b2c3d4/plan.pdf',
        // A link in a plain folder into a document the portal does not vouch for.
        '/home/leo/Desktop/notes.txt' => '/run/flatpak/doc/unknown1/notes.txt',
        _ => path,
      };
      expect(
        await privateOf([
          '/run/user/1000/doc/ssh00001/id_ed25519',
          '/var/run/user/1000/doc/ssh00001/id_ed25519',
          '/var/run/user/1000/doc/a1b2c3d4/plan.pdf',
          '/home/leo/Desktop/notes.txt',
        ]),
        [
          '/run/user/1000/doc/ssh00001/id_ed25519',
          '/var/run/user/1000/doc/ssh00001/id_ed25519',
          '/home/leo/Desktop/notes.txt',
        ],
      );
      // Outside the Flatpak the portal's folder is private like the rest of /run, by
      // whatever path it is reached.
      inFlatpak = false;
      expect(
        await privateOf(['/var/run/user/1000/doc/a1b2c3d4/plan.pdf']),
        hasLength(1),
      );
    });

    test(
      'before the portal is asked, the list is judged by its names',
      () async {
        expect(
          await refusedByName([
            '/home/leo/Documents/plan.pdf',
            '/home/leo/.ssh/id_ed25519',
            '/home/leo/Documents/.env',
            '/home/leo/Pictures/../.gnupg/key',
            '/run/user/1000/doc/a1b2c3d4/plan.pdf',
          ], homes: home),
          4,
        );
        expect(
          await refusedByName([
            '/home/leo/Documents/plan.pdf',
            '/home/leo/Downloads/report.pdf',
          ], homes: home),
          0,
        );
        expect(asked, isEmpty);
      },
    );

    test(
      'the folder shown is the real one, with no further question',
      () async {
        final dir = Directory.systemTemp.createTempSync('fogata_portal');
        addTearDown(() => dir.deleteSync(recursive: true));
        final file = File('${dir.path}/plan.pdf')..writeAsStringSync('plan');
        // As the sandbox sees a document: its path leads to Flatpak's mount of the folder.
        resolveLinks = (path) async =>
            path == file.path ? '/run/flatpak/doc/a1b2c3d4/plan.pdf' : path;
        var (files, _) = await readOutgoing([file.path]);
        expect(files.single.folder, '/home/leo/Documents');
        expect(asked, hasLength(1));
        // A drop's judgement hands its places on: nothing more is asked.
        asked.clear();
        (files, _) = await readOutgoing(
          [file.path],
          real: {file.path: '/home/leo/Documents/plan.pdf'},
        );
        expect(files.single.folder, '/home/leo/Documents');
        expect(asked, isEmpty);
        // With no answer, the folder is the sandbox's own.
        documentHostPaths = (_) async =>
            throw PlatformException(code: 'portal');
        (files, _) = await readOutgoing([file.path]);
        expect(files.single.folder, '/run/flatpak/doc/a1b2c3d4');
      },
      skip: Platform.isWindows,
    );

    test(
      'with no plugin or no portal at all, a document is never added',
      () async {
        // The plugin's real channel, which nothing answers here: no mocked answer.
        TestWidgetsFlutterBinding.ensureInitialized();
        documentHostPaths = DesktopDrop.instance.documentHostPaths;
        const plan = '/run/user/1000/doc/a1b2c3d4/plan.pdf';
        final judged = await judgeDropped([plan], homes: home);
        expect(judged.unknown, [plan]);
        expect(judged.real, isEmpty);
        expect(await privateOf([plan]), [plan]);
        expect(await realPlace(plan), isNull);
      },
    );

    test('are judged by where the portal says they are', () async {
      const plan = '/run/user/1000/doc/a1b2c3d4/plan.pdf';
      const key = '/run/user/1000/doc/ssh00001/id_ed25519';
      const logins = '/run/user/1000/doc/profile1/logins.json';
      const env = '/run/user/1000/doc/dotenv01/.env';
      const shadow = '/run/user/1000/doc/etc00001/shadow';
      expect(await privateOf([plan, key, logins, env, shadow]), [
        key,
        logins,
        env,
        shadow,
      ]);
      expect(await realPlace(plan), '/home/leo/Documents/plan.pdf');
      // Only the ids are asked, once for the drop, never a place: the drag's data names
      // none.
      expect(asked.first, [
        'a1b2c3d4',
        'ssh00001',
        'profile1',
        'dotenv01',
        'etc00001',
      ]);
      // Where Flatpak mounts the portal's folder, to which /run/user/<uid>/doc leads: the
      // place a document is found at once its links are followed.
      expect(
        await realPlace('/run/flatpak/doc/a1b2c3d4/plan.pdf'),
        '/home/leo/Documents/plan.pdf',
      );
      expect(
        await privateOf([
          '/run/flatpak/doc/a1b2c3d4/plan.pdf',
          '/run/flatpak/doc/ssh00001/id_ed25519',
        ]),
        ['/run/flatpak/doc/ssh00001/id_ed25519'],
      );
    });

    test('one whose real place cannot be learnt is never added', () async {
      // Not given to this application, or no longer there: the portal answers, without
      // it. That is no question of the system's, and it counts as private.
      // [private, unknown] of a drop.
      Future<List<List<String>>> judged(List<String> paths) async {
        final j = await judgeDropped(paths, homes: home);
        return [j.private, j.unknown];
      }

      const unknown = '/run/user/1000/doc/unknown1/plan.pdf';
      expect(await privateOf([unknown]), [unknown]);
      expect(await judged([unknown]), [
        [unknown],
        isEmpty,
      ]);
      // A portal older than GetHostPaths, one that gives no answer in time, no plugin:
      // the plugin's error. Nothing is known of the file but its name, so it is left out
      // as one whose place this system cannot tell, unless its name is hidden.
      const plan = '/run/user/1000/doc/a1b2c3d4/plan.pdf';
      const env = '/run/user/1000/doc/dotenv01/.env';
      for (final failure in <Exception>[
        MissingPluginException('no documentHostPaths'),
        PlatformException(
          code: 'portal',
          message: 'No such method “GetHostPaths”',
        ),
        PlatformException(code: 'portal', message: 'Timeout was reached'),
      ]) {
        documentHostPaths = (_) async => throw failure;
        expect(await privateOf([plan, env]), [plan, env]);
        expect(await judged([plan, env]), [
          [env],
          [plan],
        ]);
        expect(await realPlace(plan), isNull);
      }
      // An answer that breaks in any other way is no answer either.
      documentHostPaths = (_) async =>
          throw StateError('an answer of another shape');
      expect(await judged([plan, env]), [
        [env],
        [plan],
      ]);
      // Any other file is judged as before, without the portal.
      expect(await judged(['/home/leo/Downloads/report.pdf']), [
        isEmpty,
        isEmpty,
      ]);
    });

    test('an answer that does not fit the file is not believed', () async {
      for (final path in [
        // The portal's file is another name than the real one.
        '/run/user/1000/doc/other001/plan.pdf',
        // Not an absolute path, or one in the portal's folder again.
        '/run/user/1000/doc/relative/plan.pdf',
        '/run/user/1000/doc/portal01/plan.pdf',
      ]) {
        expect(await realPlace(path), isNull, reason: path);
        expect(await privateOf([path]), [path], reason: path);
      }
      // `..` in the answer is worked out before it is judged: it leads into `.ssh`.
      expect(
        await privateOf(['/run/user/1000/doc/dotdot01/plan.pdf']),
        hasLength(1),
      );
    });

    test(
      'a path in the portal\'s folder that is not exactly one of its files is '
      'never added, and its id never asked',
      () async {
        for (final path in [
          // Deeper, in an exported folder: the portal knows only the folder's place.
          '/run/user/1000/doc/a1b2c3d4/Photos/lake.jpg',
          // Dots on the way, which could lead to another document than the id named.
          '/run/user/1000/doc/ssh00001/../a1b2c3d4/plan.pdf',
          '/run/user/1000/doc/./a1b2c3d4/plan.pdf',
          '/run/user/1000/doc/a1b2c3d4/./plan.pdf',
          '/home/leo/Downloads/../../../run/user/1000/doc/a1b2c3d4/plan.pdf',
          '/run/user/1000/other/../doc/a1b2c3d4/plan.pdf',
          // The listing by application, which a sandbox does not see.
          '/run/user/1000/doc/by-app/a1b2c3d4/plan.pdf',
          '/run/user/1000/doc/by-app/dev.holypanda.fogata',
          // The folder itself, and a document's own folder.
          '/run/user/1000/doc',
          '/run/user/1000/doc/a1b2c3d4',
          '/run/user/me/doc/a1b2c3d4/plan.pdf',
          '/run/flatpak/doc/a1b2c3d4/Photos/lake.jpg',
          '/run/flatpak/doc/by-app/dev.holypanda.fogata',
          '/run/flatpak/doc/../doc/a1b2c3d4/plan.pdf',
          '/run/flatpak/doc',
        ]) {
          expect(inDocumentPortal(path), isTrue, reason: path);
          expect(await realPlace(path), isNull, reason: path);
          expect(await privateOf([path]), [path], reason: path);
        }
        expect(asked, isEmpty);
      },
    );

    test('the portal\'s files are told exactly', () {
      expect(portalDocument('/run/user/1000/doc/a1b2c3d4/plan.pdf'), (
        id: 'a1b2c3d4',
        name: 'plan.pdf',
      ));
      // Doubled slashes name the same file.
      expect(portalDocument('//run/user/1000//doc/a1b2c3d4/plan.pdf'), (
        id: 'a1b2c3d4',
        name: 'plan.pdf',
      ));
      expect(portalDocument('/run/flatpak/doc/a1b2c3d4/plan.pdf'), (
        id: 'a1b2c3d4',
        name: 'plan.pdf',
      ));
      for (final path in [
        'run/user/1000/doc/a1b2c3d4/plan.pdf',
        '/run/flatpak/doc/a1b2c3d4/plan.pdf/x',
        '/run/flatpak/plan.pdf',
        '/run/user/1000/doc/a1b2c3d4/plan.pdf/x',
        '/run/user/1000/doc/a1b2c3d4/..',
        '/run/user/1000/doc/a1b2c3d4/plan\x00.pdf',
        '/home/leo/run/user/1000/doc/a1b2c3d4/plan.pdf',
      ]) {
        expect(portalDocument(path), isNull, reason: path);
      }
      for (final path in [
        '/run/flatpak/docs/a1b2c3d4/plan.pdf',
        '/run/flatpak/app/plan.pdf',
        '/home/leo/Documents/plan.pdf',
        '/run/user/1000/plan.pdf',
        '/run/user',
      ]) {
        expect(inDocumentPortal(path), isFalse, reason: path);
      }
    });

    test(
      'any other file is judged as before, without asking the portal',
      () async {
        // The downloads folder, which the Flatpak sees itself: the portal hands its files
        // over by their own path.
        expect(
          await realPlace('/home/leo/Downloads/report.pdf'),
          '/home/leo/Downloads/report.pdf',
        );
        expect(
          await privateOf([
            '/home/leo/Downloads/report.pdf',
            '/home/leo/Downloads/.hidden.pdf',
            '/run/flatpak/app/plan.pdf',
          ]),
          ['/home/leo/Downloads/.hidden.pdf', '/run/flatpak/app/plan.pdf'],
        );
        expect(asked, isEmpty);
      },
    );

    test('a name that is not UTF-8 is still judged by its dots', () async {
      documentHostPaths = (ids) async => {
        ids.single: Uint8List.fromList([
          ...utf8.encode('/home/leo/'),
          0xff,
          ...utf8.encode('/.ssh/plan.pdf'),
        ]),
      };
      expect(
        await privateOf(['/run/user/1000/doc/a1b2c3d4/plan.pdf']),
        hasLength(1),
      );
    });

    test(
      'outside the Flatpak nothing is asked, and the folder is private',
      () async {
        inFlatpak = false;
        const plan = '/run/user/1000/doc/ssh00001/id_ed25519';
        expect(await realPlace(plan), plan);
        expect(await privateOf([plan]), [plan]);
        expect(asked, isEmpty);
      },
    );
  });

  group('a list judged before the portal is asked', () {
    const key = '/home/leo/.ssh/id_ed25519';
    test('every entry must be a plain file of this computer, read alike', () {
      expect(
        portalListPaths(
          '# a comment\r\nfile:///home/leo/a.pdf\r\n\r\nfile:///home/leo/.ssh/id_ed25519\r\n',
          ['/home/leo/a.pdf', key],
        ),
        [
          ['/home/leo/a.pdf', '/home/leo/a.pdf'],
          [key, key],
        ],
      );
      // What the source's GIO reads as the key, though this side's reading leaves it
      // out: a query, a fragment, a host, another scheme a gvfs mount turns into a path.
      for (final (line, read) in [
        ('file:///home/leo/.ssh/id_ed25519#x', key),
        ('file:///home/leo/.ssh/id_ed25519?x', key),
        ('file://evil/home/leo/.ssh/id_ed25519', key),
        ('file://localhost.localdomain/home/leo/.ssh/id_ed25519', key),
        (
          'sftp://server/home/leo/.ssh/id_ed25519',
          '/run/user/1000/gvfs/sftp:host=server/x',
        ),
        ('smb://server/share/id_ed25519', ''),
        ('https://example.com/a.png', ''),
        ('/home/leo/.ssh/id_ed25519', ''),
      ]) {
        expect(
          portalListPaths('file:///home/leo/a.pdf\r\n$line\r\n', [
            '/home/leo/a.pdf',
            read,
          ]),
          isNull,
          reason: line,
        );
      }
      // An entry that is no plain file refuses the list by itself, even where GIO's
      // reading has no entry for it to differ by.
      expect(
        portalListPaths(
          'file:///home/leo/a.pdf\r\nhttps://example.com/a.png\r\n',
          ['/home/leo/a.pdf'],
        ),
        isNull,
      );
      // GIO splits the list at LF alone and ends an entry at a CR, and a comment is a line
      // that starts with `#`: a list read otherwise here is not judged. These are what the
      // plugin hands over for them (g_uri_list_extract_uris, then GIO for `file:` only).
      expect(
        portalListPaths(
          'file:///home/leo/a.pdf\rfile:///home/leo/.ssh/id_ed25519\r\n',
          ['/home/leo/a.pdf'],
        ),
        isNull,
      );
      expect(
        portalListPaths(' #x\r\nfile:///home/leo/a.pdf\r\n', [
          '',
          '/home/leo/a.pdf',
        ]),
        isNull,
      );
      // No file at all, or only lines that are none.
      for (final text in [
        '',
        '\r\n',
        '# only a comment\r\n',
        'https://example.com/a\r\n',
      ]) {
        expect(portalListPaths(text, const []), isNull, reason: text);
      }
      // GIO's reading must have one absolute path for each entry.
      for (final read in [
        <String>[],
        ['/home/leo/a.pdf', '/home/leo/b.pdf'],
        [''],
        ['home/leo/a.pdf'],
        ['/home/leo/a\x00.pdf'],
      ]) {
        expect(
          portalListPaths('file:///home/leo/a.pdf\r\n', read),
          isNull,
          reason: '$read',
        );
      }
    });
  });

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

    testWidgets('inside the Flatpak, a file of the document portal that is really in .ssh is '
        'left out, whatever the drop named', (tester) async {
      inFlatpak = true;
      addTearDown(() {
        inFlatpak = Platform.isLinux && File('/.flatpak-info').existsSync();
        documentHostPaths = DesktopDrop.instance.documentHostPaths;
      });
      documentHostPaths = (ids) async => {
        if (ids.contains('ssh00001'))
          'ssh00001': Uint8List.fromList(
            utf8.encode('/home/leo/.ssh/id_ed25519'),
          ),
      };
      final state = await pump(tester);
      // As a page could name a file the portal gave this application before, or as the
      // portal hands it over: either way it is judged by where the portal says it is.
      await dropText(
        tester,
        'file:///run/user/1000/doc/ssh00001/id_ed25519\r\n'
        'file:///run/user/1000/doc/unknown1/notes.txt\r\n',
      );
      expect(waiting(state), isEmpty);
      expect(
        state.toasts.list.map((t) => t.text),
        contains(contains('2 hidden or private files')),
      );
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

    group('inside the Flatpak', () {
      setUp(() => inFlatpak = true);
      tearDown(() {
        inFlatpak = Platform.isLinux && File('/.flatpak-info').existsSync();
        documentHostPaths = DesktopDrop.instance.documentHostPaths;
      });

      List<String> told(AppState state) => [
        for (final t in state.toasts.list) t.text,
      ];

      testWidgets(
        'a file whose place the portal cannot say is not called hidden or private',
        (tester) async {
          documentHostPaths = (_) async => throw PlatformException(
            code: 'portal',
            message: 'No such method “GetHostPaths”',
          );
          final state = await pump(tester);
          await dropText(
            tester,
            'file:///run/user/1000/doc/a1b2c3d4/plan.pdf\r\n',
          );
          expect(waiting(state), isEmpty);
          expect(told(state), [l10n.outgoingUnknownPlaceLeftOut(1)]);
          expect(told(state).single, contains('cannot tell where'));
        },
        skip: !Platform.isLinux,
      );

      testWidgets('files a plain list names that the sandbox cannot reach are said so, the same '
          'whether they are anywhere or not', (tester) async {
        final state = await pump(tester);
        // Neither is there for the copy: one in a folder that is (as the Flatpak sees
        // the person's home but not their documents), one where nothing is. Both say
        // the same, and the file that is there is added.
        await drop(tester, [
          '${dir.path}/Pictures/not-there.jpg',
          '/home/nobody-${dir.path.hashCode}/Documents/plan.pdf',
          plain,
        ]);
        expect(waiting(state), [plain]);
        expect(told(state), [l10n.outgoingUnreachableLeftOut(2)]);
        expect(told(state).single, contains('could not be reached'));
      }, skip: !Platform.isLinux);

      testWidgets(
        'outside the Flatpak such a file is left out without a word',
        (tester) async {
          inFlatpak = false;
          final state = await pump(tester);
          await drop(tester, ['${dir.path}/Pictures/not-there.jpg']);
          expect(waiting(state), isEmpty);
          expect(told(state), isEmpty);
        },
        skip: !Platform.isLinux,
      );

      /// The plugin's word that it did not take a drop from the portal, as it sends it.
      Future<void> refused(WidgetTester tester, String why, int n) async {
        await tester.runAsync(() async {
          final done = Completer<ByteData?>();
          await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
            'desktop_drop',
            const StandardMethodCodec().encodeMethodCall(
              MethodCall('performOperation_linux', [
                '',
                [10.0, 20.0],
                'text/uri-list',
                why,
                n,
              ]),
            ),
            done.complete,
          );
          await done.future;
          await Future<void>.delayed(const Duration(milliseconds: 100));
        });
        await tester.pump();
      }

      testWidgets(
        'a drop the plugin did not take from the portal is said, and adds nothing',
        (tester) async {
          final state = await pump(tester);
          await refused(tester, 'declined', 2);
          expect(waiting(state), isEmpty);
          expect(told(state), [l10n.outgoingDropDeclined(2)]);
          expect(told(state).single, contains('Drag the others again'));
          state.toasts.list.clear();
          await refused(tester, 'unreached', 3);
          expect(waiting(state), isEmpty);
          expect(told(state), [l10n.outgoingUnreachableLeftOut(3)]);
          // A drop that named no file it refuses is never called hidden or private.
          state.toasts.list.clear();
          await refused(tester, 'declined', 0);
          await refused(tester, 'unreached', 0);
          expect(told(state), [
            l10n.outgoingUnreachableLeftOut(1),
            l10n.outgoingUnreachableLeftOut(1),
          ]);
        },
        skip: !Platform.isLinux,
      );

      testWidgets('the plugin learns how many files of a drop\'s list are refused before it asks the '
          'portal', (tester) async {
        await pump(tester);
        // The plugin's question: where the drop is, its list (null: the source offers
        // none) and the paths GIO reads in it.
        Future<Object?> check(
          String? list, {
          List<String>? read,
          List<double> at = const [10.0, 20.0],
        }) async {
          final reply = await tester.runAsync(() async {
            final done = Completer<ByteData?>();
            await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
              'desktop_drop',
              const StandardMethodCodec().encodeMethodCall(
                MethodCall('checkPortalDrop', [
                  at,
                  list,
                  read ??
                      (list == null
                          ? <String>[]
                          : linuxDropPaths('text/uri-list', list)),
                ]),
              ),
              done.complete,
            );
            return done.future;
          });
          return const StandardMethodCodec().decodeEnvelope(reply!);
        }

        expect(
          await check(
            '${Uri.file(secret)}\r\n${Uri.file(plain)}\r\n'
            'file:///home/leo/Documents/.env\r\n',
          ),
          2,
        );
        expect(await check('${Uri.file(plain)}\r\n'), 0);
        // A line that is no file of this computer: not judged, the portal not asked.
        expect(
          await check(
            '${Uri.file(plain)}\r\nhttps://example.com/a.png\r\n',
            read: [plain, ''],
          ),
          -1,
        );
        // What GIO reads is judged too: a list that reads one way here and another there.
        expect(
          await check(
            '${Uri.file(plain)}\r\n',
            read: ['/home/leo/.ssh/id_ed25519'],
          ),
          1,
        );
        // A file listed twice is one file refused.
        expect(
          await check(
            '${Uri.file(secret)}\r\n${Uri.file(secret)}\r\n${Uri.file(plain)}\r\n',
          ),
          1,
        );
        // A source that offers no list: nothing judged what its transfer holds, so the
        // portal is never asked.
        expect(await check(null), -1);
        // Where no drop target takes it, nothing is asked of the portal.
        expect(await check('${Uri.file(plain)}\r\n', at: [-50.0, -50.0]), -2);
        expect(await check(null, at: [-50.0, -50.0]), -2);
        // No judge, no portal: the plugin is told to leave it.
        DesktopDrop.instance.portalDropCheck = null;
        expect(await check('${Uri.file(plain)}\r\n'), -1);
      }, skip: Platform.isWindows);

      testWidgets(
        'a private file listed twice is one file left out, as before',
        (tester) async {
          inFlatpak = false;
          final state = await pump(tester);
          await drop(tester, [secret, secret, plain]);
          expect(waiting(state), [plain]);
          expect(told(state), [l10n.outgoingPrivateLeftOut(1)]);
        },
        skip: !Platform.isLinux,
      );
    });
  });
}
