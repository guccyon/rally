import 'dart:io';

import 'package:path_provider/path_provider.dart';

import 'file_session_store.dart';
import 'session_store.dart';

/// 残す先のファイル名（アプリの Application Support の下）
const _fileName = 'rally_student_session.json';

/// Web 版のほか（iOS / Android）の [SessionStore]。アプリの Application Support のファイルに 1 件だけ残す。
/// 置き場所が分からないときは開いている間だけメモリに持つ（アプリは落とさず、起動し直すと登録から）
Future<SessionStore> openSessionStore() async {
  try {
    final directory = await getApplicationSupportDirectory();
    return FileSessionStore(File('${directory.path}/$_fileName'));
  } on Object {
    return MemorySessionStore();
  }
}
