import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';

void main() {
  group('SendMessageRequest', () {
    test('2000 文字までの本文は作れる', () {
      final body = 'あ' * SendMessageRequest.maxBodyLength;
      expect(SendMessageRequest(body: body).toJson(), {'body': body});
    });

    test('空・空白だけの本文は ArgumentError', () {
      expect(() => SendMessageRequest(body: ''), throwsArgumentError);
      expect(() => SendMessageRequest(body: ' 　\n'), throwsArgumentError);
    });

    test('2000 文字を超える本文は ArgumentError（コードポイントで数える）', () {
      expect(() => SendMessageRequest(body: 'あ' * 2001), throwsArgumentError);
      // サロゲートペアの文字は 1 文字と数える
      expect(SendMessageRequest(body: '😀' * 2000).body, '😀' * 2000);
      expect(() => SendMessageRequest(body: '😀' * 2001), throwsArgumentError);
    });
  });

  final message = {
    'message_id': 'm1',
    'thread_id': 't1',
    'sender_id': 'c1',
    'sender_role': 'coach',
    'body': 'こんにちは',
    'sent_at': '2026-10-26T03:00:00Z',
    'read_at': null,
  };

  test('MessageThreadList.fromJson のスレッドは書き換えられない', () {
    final list = MessageThreadList.fromJson({
      'threads': [
        {
          'thread_id': 't1',
          'student_id': 's1',
          'coach_id': 'c1',
          'opened_at': '2026-10-26T02:00:00Z',
          'last_message': message,
          'unread_count': 1,
        },
      ],
    });

    expect(list.threads, hasLength(1));
    expect(() => list.threads.clear(), throwsUnsupportedError);
    expect(() => list.threads[0] = list.threads[0], throwsUnsupportedError);
  });

  test('MessageList.fromJson のメッセージは書き換えられない', () {
    final list = MessageList.fromJson({
      'thread_id': 't1',
      'messages': [message],
    });

    expect(list.messages, hasLength(1));
    expect(() => list.messages.add(list.messages[0]), throwsUnsupportedError);
    expect(() => list.messages[0] = list.messages[0], throwsUnsupportedError);
  });
}
