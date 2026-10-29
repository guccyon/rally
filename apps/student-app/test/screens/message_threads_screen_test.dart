import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/screens/message_threads_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  StudentFlowState threadsOpened({required List<MessageThread> threads}) =>
      StudentFlowState(
        step: FlowStep.messageThreads,
        chatReturnStep: FlowStep.subscription,
        accessToken: Fixtures.registered.accessToken,
        student: Fixtures.student,
        threads: threads,
      );

  testWidgets('スレッドに最後のメッセージと未読の数を出し、開くと読んで既読にしてメッセージの画面へ進む', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const MessageThreadsScreen(),
      api: api,
      initial: threadsOpened(threads: [Fixtures.thread]),
    );

    expect(find.text('担当コーチ'), findsOneWidget);
    expect(find.text('コーチ: 体験レッスンでお待ちしています'), findsOneWidget);
    expect(find.text('未読 1'), findsOneWidget);

    await tapText(tester, '担当コーチ');

    expect(api.calls, ['markMessageThreadRead', 'listMessages']);
    expect(api.requests, [Fixtures.threadId, Fixtures.threadId]);
    final state = container.read(studentFlowProvider);
    expect(state.step, FlowStep.messages);
    expect(state.thread!.unreadCount, 0);
    expect(state.messages, hasLength(2));
  });

  testWidgets('スレッドがなければその旨を出し、閉じるとチャットを開いた段に戻る', (tester) async {
    final container = await pumpScreen(
      tester,
      const MessageThreadsScreen(),
      api: FakeStudentApi(threads: []),
      initial: threadsOpened(threads: []),
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
      initial: threadsOpened(threads: [thread]),
    );

    expect(find.text('コーチ: スタンプを送りました'), findsOneWidget);
  });
}
