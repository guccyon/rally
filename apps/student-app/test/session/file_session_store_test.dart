import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/session/file_session_store.dart';
import 'package:rally_student_app/session/session_store.dart';

import '../support/fake_student_api.dart';

/// iOS / Android の [FileSessionStore]（#1597）。起動し直しは、同じファイルを指す新しい [FileSessionStore] で再現する
void main() {
  late Directory directory;
  late File file;

  setUp(() {
    directory = Directory.systemTemp.createTempSync('rally_session_test');
    file = File('${directory.path}/support/rally_student_session.json');
  });
  tearDown(() => directory.deleteSync(recursive: true));

  SavedSession session(String token) =>
      SavedSession(accessToken: token, student: Fixtures.student);

  test('残したものは、起動し直して作り直した FileSessionStore から同じ生徒で読める', () {
    expect(FileSessionStore(file).read(), isNull);

    // 親のディレクトリがまだなくても残せる
    FileSessionStore(file).save(session('token'));

    final read = FileSessionStore(file).read()!;
    expect(read.accessToken, 'token');
    expect(read.student.studentId, Fixtures.student.studentId);
    expect(read.student.displayName, Fixtures.student.displayName);
  });

  test('残し直すと置き換わり、消すと起動し直しても残っていない', () {
    final store = FileSessionStore(file)..save(session('first'));
    store.save(session('second'));
    expect(FileSessionStore(file).read()!.accessToken, 'second');
    expect(File('${file.path}.tmp').existsSync(), isFalse);

    store.clear();
    expect(file.existsSync(), isFalse);
    expect(FileSessionStore(file).read(), isNull);
    // 残していないときに消しても例外を投げない
    store.clear();
  });

  test('壊れた保存は残していないものとして扱う', () {
    file.parent.createSync(recursive: true);
    file.writeAsStringSync('{"access_token": ');
    expect(FileSessionStore(file).read(), isNull);

    file.writeAsBytesSync([0xff, 0xfe, 0x00]);
    expect(FileSessionStore(file).read(), isNull);
  });

  test('読めない・残せない置き場所でも例外を投げない', () {
    // 残す先がディレクトリになっている（読めない・置き換えられない）
    Directory(file.path).createSync(recursive: true);
    final store = FileSessionStore(file);
    expect(store.read(), isNull);
    store.save(session('token'));
    expect(store.read(), isNull);
    store.clear();
  });
}
