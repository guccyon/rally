import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/screens/messages_screen.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';
import '../support/pump.dart';

void main() {
  const otherVideoId = '67676767-6767-4767-8767-676767676767';

  Message message(
    String id, {
    required MessageSenderRole role,
    MessageKind kind = MessageKind.text,
    required String body,
    String? videoId,
    String? reviewId,
    required int minutes,
  }) => Message(
    messageId: id,
    threadId: Fixtures.threadId,
    senderId: role == MessageSenderRole.student
        ? Fixtures.studentId
        : Fixtures.coachId,
    senderRole: role,
    kind: kind,
    body: body,
    videoId: videoId,
    reviewId: reviewId,
    sentAt: Fixtures.now.add(Duration(minutes: minutes)),
  );

  /// 本文・1 回目の依頼・その返却・2 回目の依頼・知らない種類 の順のスレッド
  final reviewThread = [
    message(
      'm1',
      role: MessageSenderRole.student,
      body: 'よろしくお願いします',
      minutes: 0,
    ),
    message(
      'm2',
      role: MessageSenderRole.student,
      kind: MessageKind.videoReviewRequest,
      body: '動画の添削を依頼しました',
      videoId: Fixtures.videoId,
      minutes: 1,
    ),
    message(
      'm3',
      role: MessageSenderRole.coach,
      kind: MessageKind.videoReviewReturned,
      body: '動画の添削を返しました',
      videoId: Fixtures.videoId,
      reviewId: Fixtures.reviewReturned.reviewId,
      minutes: 2,
    ),
    message(
      'm4',
      role: MessageSenderRole.student,
      kind: MessageKind.videoReviewRequest,
      body: '動画の添削を依頼しました',
      videoId: otherVideoId,
      minutes: 3,
    ),
    message(
      'm5',
      role: MessageSenderRole.coach,
      kind: MessageKind.other,
      body: 'スタンプを送りました',
      minutes: 4,
    ),
  ];

  double topOf(WidgetTester tester, String text) =>
      tester.getTopLeft(find.text(text)).dy;

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
    expect(api.calls, ['markMessageThreadRead', 'listMessages']);

    await tester.tap(find.byTooltip('スレッドの一覧へ戻る'));
    await tester.pumpAndSettle();
    expect(api.calls.last, 'listMessageThreads');
    expect(container.read(studentFlowProvider).step, FlowStep.messageThreads);
  });

  testWidgets(
    '添削の依頼と返却を本文と見分けられるカードで時間の順に出し、返却にはコーチの添削と AI の一次フィードバックを別の枠で出す',
    (tester) async {
      // カードをすべて描いた状態で並びを比べる
      await tester.binding.setSurfaceSize(const Size(800, 2000));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final api = FakeStudentApi(
        messages: reviewThread,
        videoReviews: {Fixtures.videoId: Fixtures.reviewReturnedWithAi},
      );
      await pumpScreen(
        tester,
        const MessagesScreen(),
        api: api,
        initial: threadOpened(),
      );
      await tapText(tester, '新しいメッセージを確かめる');

      // 返却が流れた動画の添削だけを読む
      expect(api.calls, [
        'markMessageThreadRead',
        'listMessages',
        'getVideoReview',
      ]);
      expect(api.requests.last, Fixtures.videoId);

      expect(find.text('1 回目の添削の依頼'), findsOneWidget);
      expect(find.text('コーチの添削（1 回目の依頼）'), findsOneWidget);
      expect(find.text('2 回目の添削の依頼'), findsOneWidget);
      expect(find.text('動画を送って、添削を依頼しました。'), findsNWidgets(2));
      expect(find.text('テイクバックをもう少し早く'), findsOneWidget);
      expect(find.text('AI の一次フィードバック'), findsOneWidget);
      expect(find.text('フォームの全体の流れは安定しています'), findsOneWidget);
      // 種類を知らない版向けの短い説明は、知っている種類では出さない
      expect(find.text('動画の添削を依頼しました'), findsNothing);
      expect(find.text('動画の添削を返しました'), findsNothing);
      // 知らない種類は隠さず、本文として出す
      expect(find.text('スタンプを送りました'), findsOneWidget);

      expect(
        topOf(tester, 'よろしくお願いします'),
        lessThan(topOf(tester, '1 回目の添削の依頼')),
      );
      expect(
        topOf(tester, '1 回目の添削の依頼'),
        lessThan(topOf(tester, 'コーチの添削（1 回目の依頼）')),
      );
      expect(
        topOf(tester, 'テイクバックをもう少し早く'),
        lessThan(topOf(tester, 'フォームの全体の流れは安定しています')),
      );
      expect(
        topOf(tester, 'コーチの添削（1 回目の依頼）'),
        lessThan(topOf(tester, '2 回目の添削の依頼')),
      );
      expect(
        topOf(tester, '2 回目の添削の依頼'),
        lessThan(topOf(tester, 'スタンプを送りました')),
      );

      // 添削の依頼・返却はアイコンと見出しの付いたカード、本文はアイコンのない吹き出しで見分ける
      Finder cardOf(String text) =>
          find.ancestor(of: find.text(text), matching: find.byType(Card)).first;
      expect(
        find.descendant(
          of: cardOf('1 回目の添削の依頼'),
          matching: find.byIcon(Icons.videocam_outlined),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: cardOf('コーチの添削（1 回目の依頼）'),
          matching: find.byIcon(Icons.rate_review_outlined),
        ),
        findsOneWidget,
      );
      for (final text in ['よろしくお願いします', 'スタンプを送りました']) {
        expect(
          find.descendant(of: cardOf(text), matching: find.byType(Icon)),
          findsNothing,
        );
      }

      // 読めた添削は読み直さない
      await tapText(tester, '新しいメッセージを確かめる');
      expect(api.calls.where((c) => c == 'getVideoReview'), hasLength(1));
    },
  );

  testWidgets('添削を読めなければ返却のカードにだけその旨を出し、確かめ直すと読み直す', (tester) async {
    final api = FakeStudentApi(
      messages: reviewThread,
      videoReviews: {Fixtures.videoId: Fixtures.reviewReturned},
    )..videoReviewFailures.add(Fixtures.videoId);
    await pumpScreen(
      tester,
      const MessagesScreen(),
      api: api,
      initial: threadOpened(),
    );
    await tapText(tester, '新しいメッセージを確かめる');

    expect(
      find.text('添削の内容を読み込めませんでした。「新しいメッセージを確かめる」で読み直せます。'),
      findsOneWidget,
    );
    expect(find.text('よろしくお願いします'), findsOneWidget);
    expect(find.text('通信に失敗しました。時間をおいてもう一度お試しください。'), findsNothing);

    api.videoReviewFailures.clear();
    await tapText(tester, '新しいメッセージを確かめる');

    expect(find.textContaining('読み込めませんでした'), findsNothing);
    expect(find.text('テイクバックをもう少し早く'), findsOneWidget);
    expect(find.text('AI の一次フィードバック'), findsNothing);
  });

  testWidgets('「添削を依頼する」で動画を選んで送ると、担当コーチに submitVideo し、スレッドに添削の依頼が並ぶ', (
    tester,
  ) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const MessagesScreen(),
      api: api,
      initial: threadOpened(),
    );

    await tapText(tester, '添削を依頼する');
    final submit = find.widgetWithText(FilledButton, '依頼する');
    expect(tester.widget<FilledButton>(submit).onPressed, isNull);

    await tapText(tester, '動画を選ぶ（モック）');
    expect(find.text('選んだ動画: swing.mp4'), findsOneWidget);
    await tester.enterText(
      find.descendant(
        of: find.byType(AlertDialog),
        matching: find.byType(TextField),
      ),
      '  バックハンドを見てください  ',
    );
    await tapText(tester, '依頼する');

    expect(find.byType(AlertDialog), findsNothing);
    expect(api.calls, ['submitVideo', 'markMessageThreadRead', 'listMessages']);
    expect((api.requests.first as SubmitVideoRequest).toJson(), {
      'coach_id': Fixtures.coachId,
      'sport': 'tennis',
      'content_type': 'video/mp4',
      'note': 'バックハンドを見てください',
    });
    expect(find.text('1 回目の添削の依頼'), findsOneWidget);
    expect(find.textContaining(RegExp(r'^自分 ・ .* ・ 未読$')), findsOneWidget);
  });

  testWidgets('添削の依頼をやめると何も送らない', (tester) async {
    final api = FakeStudentApi();
    await pumpScreen(
      tester,
      const MessagesScreen(),
      api: api,
      initial: threadOpened(),
    );

    await tapText(tester, '添削を依頼する');
    await tapText(tester, '動画を選ぶ（モック）');
    await tapText(tester, 'やめる');

    expect(find.byType(AlertDialog), findsNothing);
    expect(api.calls, isEmpty);
  });
}
