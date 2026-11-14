import 'package:web/web.dart' as web;

import 'session_store.dart';

/// Web 版の [SessionStore]。ブラウザーの localStorage に、このオリジンで 1 件だけ残す（再読み込み・「進む」で戻っても読める）。
Future<SessionStore> openSessionStore() async => _LocalStorageSessionStore();

/// localStorage のキー
const _key = 'rally.student.session';

class _LocalStorageSessionStore implements SessionStore {
  @override
  SavedSession? read() {
    try {
      final raw = web.window.localStorage.getItem(_key);
      return raw == null ? null : SavedSession.decode(raw);
    } on Object {
      // localStorage を使えないブラウザー（プライベートの窓の制限など）は、残していないものとして扱う
      return null;
    }
  }

  @override
  void save(SavedSession session) {
    try {
      web.window.localStorage.setItem(_key, session.encode());
    } on Object {
      // 残せなくても開いている間はそのまま使える（開き直すと登録から）
    }
  }

  @override
  void clear() {
    try {
      web.window.localStorage.removeItem(_key);
    } on Object {
      // 消せなくても次に読んだときのトークンがまた 401 になり、そこで登録の画面になる
    }
  }
}
