import 'dart:io';

import 'session_store.dart';

/// 端末のファイル 1 つに残す [SessionStore]（iOS / Android）。アプリを止めて起動し直しても同じ生徒で始められる。
/// 読めない・残せない・消せないときも例外は投げない（読めなければ登録の画面から、残せなければ開いている間だけ使える）
class FileSessionStore implements SessionStore {
  FileSessionStore(this.file);

  /// 残す先。親のディレクトリがなければ残すときに作る
  final File file;

  @override
  SavedSession? read() {
    try {
      if (!file.existsSync()) return null;
      return SavedSession.decode(file.readAsStringSync());
    } on Object {
      return null;
    }
  }

  @override
  void save(SavedSession session) {
    // 書きかけで止まっても前の保存が壊れないよう、別のファイルに書いてから置き換える
    final temp = File('${file.path}.tmp');
    try {
      file.parent.createSync(recursive: true);
      temp.writeAsStringSync(session.encode(), flush: true);
      temp.renameSync(file.path);
    } on Object {
      // 残せなくても開いている間はそのまま使える（起動し直すと登録から）
    }
  }

  @override
  void clear() {
    try {
      if (file.existsSync()) file.deleteSync();
    } on Object {
      // 消せなくても次に読んだときのトークンがまた 401 になり、そこで登録の画面になる
    }
  }
}
