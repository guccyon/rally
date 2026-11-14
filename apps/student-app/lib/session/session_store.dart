import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
export 'session_store_stub.dart'
    if (dart.library.js_interop) 'session_store_web.dart'
    show openSessionStore;

/// 端末に残す登録の結果（`registerAccount` の応答のトークンと生徒）。開き直したときに同じ生徒で始めるためだけに持つ。
/// 段・予約・通話セッションは持たない（次の操作はサーバーの状態から出す。ADR 0010）。
class SavedSession {
  const SavedSession({required this.accessToken, required this.student});

  /// 契約の bearerAuth。中身は解釈しない
  final String accessToken;
  final Student student;

  /// 保存の形（JSON の文字列）。生徒は契約の `Student` と同じ形で持つ
  String encode() =>
      jsonEncode({'access_token': accessToken, 'student': student.toJson()});

  /// [encode] の逆。形が合わない（古い版の保存・壊れた値）なら null
  static SavedSession? decode(String raw) {
    try {
      final json = jsonDecode(raw) as Map<String, dynamic>;
      return SavedSession(
        accessToken: json['access_token'] as String,
        student: Student.fromJson(json['student'] as Map<String, dynamic>),
      );
    } on Object {
      return null;
    }
  }
}

/// 登録の結果を端末に残す所（ポート）。本物は [openSessionStore]（Web 版はブラウザーの localStorage、
/// iOS / Android はアプリのファイル）、テストは [MemorySessionStore] を渡して開き直しを確かめる（[sessionStoreProvider]）。
abstract class SessionStore {
  /// 残した登録の結果。ない・読めなければ null
  SavedSession? read();

  /// [session] で置き換えて残す。残せなくても例外は投げない（開いている間はそのまま使える）
  void save(SavedSession session);

  /// 残した登録の結果を消す（トークンが無効だったとき）
  void clear();
}

/// 開いている間だけ持つ [SessionStore]（テストと、端末の保管の置き場所が分からないとき）。同じものを渡し直せば開き直しを再現できる
class MemorySessionStore implements SessionStore {
  MemorySessionStore([this._raw]);

  String? _raw;

  /// 残っている保存の形（[SavedSession.encode]）。なければ null
  String? get raw => _raw;

  @override
  SavedSession? read() {
    final raw = _raw;
    return raw == null ? null : SavedSession.decode(raw);
  }

  @override
  void save(SavedSession session) => _raw = session.encode();

  @override
  void clear() => _raw = null;
}

/// 登録の結果を残す所。`main` が [openSessionStore] で開いたもので差し替える。
/// 差し替えないとき（テスト・画面のキャプチャ）は開いている間だけのメモリ
final sessionStoreProvider = Provider<SessionStore>(
  (ref) => MemorySessionStore(),
);
