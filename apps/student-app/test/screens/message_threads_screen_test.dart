import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/message_threads_screen.dart';
import 'package:rally_student_app/screens/student_flow_screen.dart';
import 'package:rally_student_app/state/chat.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  final threadsOpened = StudentFlowState(
    step: FlowStep.messageThreads,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  ChatState chatOf({required List<MessageThread> threads}) =>
      ChatState(returnStep: FlowStep.subscription, threads: threads);

  testWidgets('スレッドに最後のメッセージと未読の数を出し、開くと読んで既読にしてメッセージの画面へ進む', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const MessageThreadsScreen(),
      api: api,
      initial: threadsOpened,
      chat: chatOf(threads: [Fixtures.thread]),
    );

    expect(find.text('担当コーチ'), findsOneWidget);
    expect(find.text('コーチ: 体験レッスンでお待ちしています'), findsOneWidget);
    expect(find.text('未読 1'), findsOneWidget);

    await tapText(tester, '担当コーチ');

    expect(api.calls, ['markMessageThreadRead', 'listMessages']);
    expect(api.requests, [Fixtures.threadId, Fixtures.threadId]);
    expect(container.read(studentFlowProvider).step, FlowStep.messages);
    final chat = container.read(chatProvider);
    expect(chat.thread!.unreadCount, 0);
    expect(chat.messages, hasLength(2));
  });

  testWidgets('開いて既読にすると、一覧に戻ったときに未読の数が消える', (tester) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: threadsOpened,
      chat: chatOf(threads: [Fixtures.thread]),
    );
    expect(find.text('未読 1'), findsOneWidget);

    await tapText(tester, '担当コーチ');
    expect(find.text('担当コーチとのチャット'), findsOneWidget);
    await tester.tap(find.byTooltip('スレッドの一覧へ戻る'));
    await tester.pumpAndSettle();

    expect(api.calls, [
      'markMessageThreadRead',
      'listMessages',
      'listMessageThreads',
    ]);
    expect(find.text('担当コーチ'), findsOneWidget);
    expect(find.text('未読 1'), findsNothing);
  });

  testWidgets('既読にできなくてもメッセージの画面を開いたままにし、一覧に戻ると未読の数を残す', (tester) async {
    final api = FakeStudentApi(
      failures: {'markMessageThreadRead': Fixtures.unreachable},
    );
    final container = await pumpScreen(
      tester,
      const StudentFlowScreen(),
      api: api,
      initial: threadsOpened,
      chat: chatOf(threads: [Fixtures.thread]),
    );

    await tapText(tester, '担当コーチ');

    expect(find.text('担当コーチとのチャット'), findsOneWidget);
    expect(find.text('体験レッスンでお待ちしています'), findsOneWidget);
    // 失敗の文言は出さない（読めたメッセージはそのまま読める）
    expect(container.read(studentFlowProvider).errorMessage, isNull);

    await tester.tap(find.byTooltip('スレッドの一覧へ戻る'));
    await tester.pumpAndSettle();
    expect(find.text('未読 1'), findsOneWidget);
  });

  testWidgets('スレッドがなければその旨を出し、閉じるとチャットを開いた段に戻る', (tester) async {
    final container = await pumpScreen(
      tester,
      const MessageThreadsScreen(),
      api: FakeStudentApi(threads: []),
      initial: threadsOpened,
      chat: chatOf(threads: []),
    );

    expect(
      find.text('まだスレッドはありません。予約が確定すると、担当コーチとのスレッドが開きます。'),
      findsOneWidget,
    );

    await tester.tap(find.byTooltip('チャットを閉じる'));
    await tester.pumpAndSettle();
    expect(container.read(studentFlowProvider).step, FlowStep.subscription);
  });

  testWidgets('最後のメッセージが知らない種類でも隠さず、本文（短い説明）を出す', (tester) async {
    final thread = MessageThread(
      threadId: Fixtures.threadId,
      studentId: Fixtures.studentId,
      coachId: Fixtures.coachId,
      openedAt: Fixtures.now,
      lastMessage: Message(
        messageId: 'message-sticker',
        threadId: Fixtures.threadId,
        senderId: Fixtures.coachId,
        senderRole: MessageSenderRole.coach,
        kind: MessageKind.other,
        body: 'スタンプを送りました',
        sentAt: Fixtures.now,
      ),
      unreadCount: 1,
    );
    await pumpScreen(
      tester,
      const MessageThreadsScreen(),
      api: FakeStudentApi(threads: [thread]),
      initial: threadsOpened,
      chat: chatOf(threads: [thread]),
    );

    expect(find.text('コーチ: スタンプを送りました'), findsOneWidget);
  });
}
