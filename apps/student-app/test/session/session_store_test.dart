import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/session/session_store.dart';

import '../support/fake_student_api.dart';

void main() {
  test('保存の形はトークンと契約の Student の形だけで、読み直すと同じ生徒になる', () {
    final session = SavedSession(
      accessToken: Fixtures.registered.accessToken,
      student: Fixtures.student,
    );

    final json = jsonDecode(session.encode()) as Map<String, dynamic>;
    expect(json.keys, unorderedEquals(['access_token', 'student']));
    expect(
      (json['student'] as Map<String, dynamic>).keys,
      unorderedEquals(['student_id', 'display_name', 'sport', 'created_at']),
    );

    final read = SavedSession.decode(session.encode())!;
    expect(read.accessToken, session.accessToken);
    expect(read.student.studentId, Fixtures.student.studentId);
    expect(read.student.displayName, Fixtures.student.displayName);
    expect(read.student.sport, Fixtures.student.sport);
    expect(read.student.createdAt, Fixtures.student.createdAt);
  });

  test('形が合わない保存は残していないものとして扱う', () {
    expect(SavedSession.decode('not json'), isNull);
    expect(SavedSession.decode('{"access_token": "t"}'), isNull);
    expect(MemorySessionStore('[]').read(), isNull);
  });

  test('MemorySessionStore は残して読み直し、消せる', () {
    final store = MemorySessionStore();
    expect(store.read(), isNull);

    store.save(SavedSession(accessToken: 'token', student: Fixtures.student));
    expect(store.read()!.accessToken, 'token');

    store.clear();
    expect(store.read(), isNull);
    expect(store.raw, isNull);
  });
}
