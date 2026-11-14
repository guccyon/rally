import 'session_store.dart';

/// Web 版のほか（iOS / Android のビルドはまだない）の [SessionStore]。開いている間だけメモリに持つ。
SessionStore createSessionStore() => MemorySessionStore();
