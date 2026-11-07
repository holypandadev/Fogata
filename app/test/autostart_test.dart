import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/autostart.dart';

/// The Linux entry is made of POSIX paths (XDG folders, the AppImage's mount): on Windows
/// the test's own folders are not such paths, and the Linux class rightly refuses them.
final String? posixOnly = Platform.isWindows
    ? 'Linux autostart: POSIX paths only'
    : null;

void main() {
  late Directory temp;

  setUp(() => temp = Directory.systemTemp.createTempSync('fogata-autostart-'));
  tearDown(() => temp.deleteSync(recursive: true));

  test(
    'the entry goes into XDG_CONFIG_HOME/autostart and away again',
    () async {
      final config = '${temp.path}/config';
      final autostart = LinuxAutostart(
        environment: {'XDG_CONFIG_HOME': config, 'HOME': '${temp.path}/home'},
        executable: '/opt/fogata/fogata',
      );
      final file = File('$config/autostart/${LinuxAutostart.fileName}');
      expect(await autostart.read(), AutostartState.off);
      expect(await autostart.write(true), AutostartState.on);
      expect(file.existsSync(), isTrue);
      final text = file.readAsStringSync();
      expect(text, startsWith('[Desktop Entry]\n'));
      expect(text, contains('\nType=Application\n'));
      expect(text, contains('\nExec="/opt/fogata/fogata"\n'));
      expect(await autostart.read(), AutostartState.on);
      // Nothing is left beside it, and the home's folder was never used.
      expect(Directory('$config/autostart').listSync(), hasLength(1));
      expect(Directory('${temp.path}/home').existsSync(), isFalse);
      expect(await autostart.write(false), AutostartState.off);
      expect(file.existsSync(), isFalse);
      expect(await autostart.read(), AutostartState.off);
      // Off twice is still off.
      expect(await autostart.write(false), AutostartState.off);
    },
    skip: posixOnly,
  );

  test(
    'without XDG_CONFIG_HOME, or with a relative one, ~/.config is used',
    () async {
      for (final environment in [
        {'HOME': temp.path},
        {'HOME': temp.path, 'XDG_CONFIG_HOME': 'relative/config'},
        {'HOME': temp.path, 'XDG_CONFIG_HOME': ''},
      ]) {
        final autostart = LinuxAutostart(
          environment: environment,
          executable: '/opt/fogata/fogata',
        );
        expect(autostart.folder, '${temp.path}/.config/autostart');
        expect(await autostart.write(true), AutostartState.on);
        expect(
          File('${temp.path}/.config/autostart/${LinuxAutostart.fileName}')
              .existsSync(),
          isTrue,
        );
        expect(await autostart.write(false), AutostartState.off);
      }
    },
  );

  test('an AppImage starts the AppImage, not its mounted copy', () {
    // This test runs from no AppImage: one named by the environment is another's.
    final inherited = LinuxAutostart(
      environment: {
        'HOME': temp.path,
        'APPIMAGE': '/home/leo/Apps/Launcher.AppImage',
        'APPDIR': '/tmp/.mount_LauncherQ2',
      },
    );
    expect(inherited.executable, Platform.resolvedExecutable);
    // Run from inside the mounted AppImage: the AppImage starts.
    final mounted = File(Platform.resolvedExecutable).parent.path;
    final inside = LinuxAutostart(
      environment: {
        'HOME': temp.path,
        'APPIMAGE': '/home/leo/Apps/Fogata.AppImage',
        'APPDIR': mounted,
      },
    );
    expect(inside.executable, '/home/leo/Apps/Fogata.AppImage');
    expect(
      ownAppImage({
        'APPIMAGE': '/home/leo/Apps/Fogata.AppImage',
        'APPDIR': '/tmp/.mount_Fogata',
      }, '/tmp/.mount_FogataEvil/fogata'),
      isNull,
    );
  }, skip: posixOnly);

  test('an entry the desktop turned off reads as off', () async {
    final autostart = LinuxAutostart(
      environment: {'XDG_CONFIG_HOME': temp.path},
      executable: '/opt/fogata/fogata',
    );
    await autostart.write(true);
    final file = File('${autostart.folder}/${LinuxAutostart.fileName}');
    file.writeAsStringSync('${file.readAsStringSync()}Hidden=true\n');
    expect(await autostart.read(), AutostartState.off);
  }, skip: posixOnly);

  test('no home cannot do it', () async {
    expect(
      await LinuxAutostart(environment: const {}).read(),
      AutostartState.unsupported,
    );
  });

  test('in a Flatpak the Background portal does it', () async {
    final asked = <(bool, List<String>)>[];
    bool? answer;
    final sandboxed = LinuxAutostart(
      environment: {'HOME': temp.path, 'FLATPAK_ID': 'dev.holypanda.fogata'},
      executable: '/app/fogata/fogata',
      background: (on, commandline) async {
        asked.add((on, commandline));
        return answer;
      },
    );
    // No portal to ask: it cannot, and nothing is written.
    expect(await sandboxed.write(true), AutostartState.unsupported);
    expect(Directory('${temp.path}/.config').existsSync(), isFalse);
    expect(await sandboxed.read(), AutostartState.off);
    // The portal will.
    answer = true;
    expect(await sandboxed.write(true), AutostartState.on);
    expect(await sandboxed.read(), AutostartState.on);
    // Refused (the person said no in the portal's dialog).
    answer = false;
    expect(await sandboxed.write(true), AutostartState.off);
    expect(await sandboxed.read(), AutostartState.off);
    // Turned off.
    answer = true;
    await sandboxed.write(true);
    answer = false;
    expect(await sandboxed.write(false), AutostartState.off);
    expect(await sandboxed.read(), AutostartState.off);
    expect(asked.map((a) => a.$1), [true, true, true, true, false]);
    expect(asked.first.$2, ['/app/fogata/fogata']);
  });

  test('a path with reserved characters stays one argument', () {
    expect(
      LinuxAutostart.execArgument(r'/home/leo/My Apps/fo"g$a`ta\%1/fogata'),
      r'"/home/leo/My Apps/fo\\"g\\$a\\`ta\\\\%%1/fogata"',
    );
    final entry = LinuxAutostart.entry('/opt/fogata/fogata');
    // No bundle there, so no icon.
    expect(entry, isNot(contains('Icon=')));
  });

  test('the entry names the icon once it is installed', () {
    final data = '${temp.path}/data';
    final autostart = LinuxAutostart(
      environment: {
        'XDG_DATA_HOME': data,
        'XDG_DATA_DIRS': '${temp.path}/system',
        'HOME': '${temp.path}/home',
      },
      executable: '/opt/fogata/fogata',
    );
    expect(autostart.iconInstalled, isFalse);
    final icon = File(
      '$data/icons/hicolor/256x256/apps/${LinuxAutostart.iconName}.png',
    );
    icon.createSync(recursive: true);
    expect(autostart.iconInstalled, isTrue);
    final entry = LinuxAutostart.entry('/opt/fogata/fogata', namedIcon: true);
    expect(entry, contains('\nIcon=dev.holypanda.fogata\n'));
    // Under the system's folders too.
    icon.deleteSync();
    File(
      '${temp.path}/system/icons/hicolor/scalable/apps/${LinuxAutostart.iconName}.svg',
    ).createSync(recursive: true);
    expect(autostart.iconInstalled, isTrue);
  }, skip: posixOnly);

  testWidgets('macOS and Windows: the runner\'s answers', (tester) async {
    final calls = <MethodCall>[];
    var answer = 'off';
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('fogata/autostart'),
      (call) async {
        calls.add(call);
        if (call.method == 'set') {
          answer = call.arguments == true ? 'on' : 'off';
        }
        return answer;
      },
    );
    const runner = RunnerAutostart();
    expect(await runner.read(), AutostartState.off);
    expect(await runner.write(true), AutostartState.on);
    expect(calls.last.arguments, isTrue);
    answer = 'approval';
    expect(await runner.read(), AutostartState.needsApproval);
    answer = 'unsupported';
    expect(await runner.read(), AutostartState.unsupported);
    expect(await runner.write(false), AutostartState.off);
    expect(calls.last.arguments, isFalse);
  });
}
