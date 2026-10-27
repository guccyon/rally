import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/messages_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  StudentFlowState threadOpened() => StudentFlowState(
    step: FlowStep.messages,
    chatReturnStep: FlowStep.subscription,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
    threads: [Fixtures.thread],
    thread: Fixtures.thread,
    messages: Fixtures.messages,
  );

  testWidgets('メッセージを送った順に出し、自分が送ったものにだけ既読・未読を出す', (tester) async {
    await pumpScreen(
      tester,
      const MessagesScreen(),
      api: FakeStudentApi(),
      initial: threadOpened(),
    );

    expect(find.text('よろしくお願いします'), findsOneWidget);
    expect(find.text('体験レッスンでお待ちしています'), findsOneWidget);
    expect(
      tester.getTopLeft(find.text('よろしくお願いします')).dy,
      lessThan(tester.getTopLeft(find.text('体験レッスンでお待ちしています')).dy),
    );
    expect(find.textContaining(RegExp(r'^自分 ・ .* ・ 既読$')), findsOneWidget);
    expect(find.textContaining(RegExp(r'^コーチ ・ [^・]*$')), findsOneWidget);
    expect(find.textContaining('未読'), findsNothing);
  });

  testWidgets('送信すると sendMessage を呼び、未読のメッセージとして末尾に足して入力欄を空にする', (
    tester,
  ) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const MessagesScreen(),
      api: api,
      initial: threadOpened(),
    );

    final send = find.widgetWithText(FilledButton, '送信する');
    expect(tester.widget<FilledButton>(send).onPressed, isNull);

    await tester.enterText(find.byType(TextField), '  フォームを見てください  ');
    await tester.pump();
    await tapText(tester, '送信する');

    expect(api.calls, ['sendMessage']);
    expect((api.requests.single as SendMessageRequest).toJson(), {
      'body': 'フォームを見てください',
    });
    expect(find.text('フォームを見てください'), findsOneWidget);
    expect(find.textContaining(RegExp(r'^自分 ・ .* ・ 未読$')), findsOneWidget);
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      '',
    );
  });

  testWidgets('月額契約の前の生徒が送ると 403 chat_not_entitled の文言を出し、入力は残す', (
    tester,
  ) async {
    final api = FakeStudentApi(
      failures: {
        'sendMessage': const StudentApiException(
          statusCode: 403,
          error: ApiError(
            code: ErrorCode.chatNotEntitled,
            message: 'not entitled',
          ),
        ),
      },
    );
    await pumpScreen(
      tester,
      const MessagesScreen(),
      api: api,
      initial: threadOpened(),
    );

    await tester.enterText(find.byType(TextField), '質問です');
    await tester.pump();
    await tapText(tester, '送信する');

    expect(find.text('チャットは月額契約を始めると使えます。'), findsOneWidget);
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      '質問です',
    );
  });

  testWidgets('新しいメッセージを確かめると読み直して既読にし、戻るとスレッドの一覧を読み直す', (tester) async {
    final api = FakeStudentApi();
    final container = await pumpScreen(
      tester,
      const MessagesScreen(),
      api: api,
      initial: threadOpened(),
    );

    await tapText(tester, '新しいメッセージを確かめる');
    expect(api.calls, ['listMessages', 'markMessageThreadRead']);

    await tester.tap(find.byTooltip('スレッドの一覧へ戻る'));
    await tester.pumpAndSettle();
    expect(api.calls.last, 'listMessageThreads');
    expect(container.read(studentFlowProvider).step, FlowStep.messageThreads);
  });
}
