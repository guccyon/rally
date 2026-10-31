import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rally_student_app/api/models.dart';
import 'package:rally_student_app/api/student_api.dart';
import 'package:rally_student_app/state/chat.dart';
import 'package:rally_student_app/state/student_flow.dart';

import '../support/fake_student_api.dart';

void main() {
  final home = StudentFlowState(
    step: FlowStep.home,
    accessToken: Fixtures.registered.accessToken,
    student: Fixtures.student,
  );

  /// API をモックにした ProviderContainer（画面は描かない）
  ProviderContainer containerOf(
    FakeStudentApi api, {
    StudentFlowState? flow,
    ChatState? chat,
  }) {
    final container = ProviderContainer(
      overrides: [
        studentApiProvider.overrideWithValue(api),
        studentFlowProvider.overrideWith(
          () => StudentFlowNotifier(flow ?? home),
        ),
        chatProvider.overrideWith(() => ChatNotifier(chat)),
        clockProvider.overrideWithValue(() => Fixtures.now),
      ],
    );
    addTearDown(container.dispose);
    return container;
  }

  Message returned(String videoId, {required int minutes}) => Message(
    messageId: 'returned-$videoId',
    threadId: Fixtures.threadId,
    senderId: Fixtures.coachId,
    senderRole: MessageSenderRole.coach,
    kind: MessageKind.videoReviewReturned,
    body: '動画の添削を返しました',
    videoId: videoId,
    sentAt: Fixtures.now.add(Duration(minutes: minutes)),
  );

  test('チャットを開くと開いた段を覚えてスレッドの一覧へ移り、閉じるとその段に戻る', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      flow: home.copyWith(step: FlowStep.subscription),
    );
    final chat = container.read(chatProvider.notifier);

    await chat.openChat();

    expect(api.calls, ['listMessageThreads']);
    expect(container.read(studentFlowProvider).step, FlowStep.messageThreads);
    expect(container.read(studentFlowProvider).busy, isFalse);
    expect(container.read(chatProvider).returnStep, FlowStep.subscription);
    expect(container.read(chatProvider).threads, [Fixtures.thread]);

    // チャットの中で開き直しても戻り先は変えない
    await chat.openChat();
    expect(container.read(chatProvider).returnStep, FlowStep.subscription);

    chat.closeChat();
    expect(container.read(studentFlowProvider).step, FlowStep.subscription);
  });

  test('スレッドを開くと既読にしてからメッセージを読み、メッセージの段へ移る', () async {
    final api = FakeStudentApi();
    final container = containerOf(api);

    await container.read(chatProvider.notifier).openThread(Fixtures.thread);

    expect(api.calls, ['markMessageThreadRead', 'listMessages']);
    final state = container.read(chatProvider);
    expect(state.returnStep, FlowStep.home);
    expect(state.thread!.unreadCount, 0);
    expect(state.messages, hasLength(Fixtures.messages.length));
    expect(container.read(studentFlowProvider).step, FlowStep.messages);
  });

  test('既読にすると、読んだスレッドの一覧のそのスレッドも未読 0 にする（一覧を読み直さなくても数が消える）', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      chat: ChatState(threads: [Fixtures.thread]),
    );
    expect(container.read(chatProvider).unreadCount, 1);

    await container.read(chatProvider.notifier).openThread(Fixtures.thread);

    final state = container.read(chatProvider);
    expect(state.threads!.single.unreadCount, 0);
    expect(state.unreadCount, 0);
  });

  test('既読にできなくても、メッセージを読めればメッセージの段へ移り、未読の数は残して失敗の文言は出さない', () async {
    final api = FakeStudentApi(
      failures: {'markMessageThreadRead': Fixtures.unreachable},
    );
    final container = containerOf(
      api,
      chat: ChatState(threads: [Fixtures.thread]),
    );
    final chat = container.read(chatProvider.notifier);

    await chat.openThread(Fixtures.thread);

    expect(api.calls, ['markMessageThreadRead', 'listMessages']);
    final flow = container.read(studentFlowProvider);
    expect(flow.step, FlowStep.messages);
    expect(flow.busy, isFalse);
    expect(flow.errorMessage, isNull);
    final state = container.read(chatProvider);
    expect(state.thread, Fixtures.thread);
    expect(state.messages, hasLength(Fixtures.messages.length));
    expect(state.unreadCount, 1);

    // 確かめ直したときにもう一度既読にする
    api.failures.remove('markMessageThreadRead');
    await chat.refreshMessages();
    expect(container.read(chatProvider).thread!.unreadCount, 0);
    expect(container.read(chatProvider).unreadCount, 0);
  });

  test('メッセージを読めなければ開いた段に留まり、失敗の文言は全体の状態に出す', () async {
    final api = FakeStudentApi(
      failures: {'listMessages': Fixtures.unreachable},
    );
    final container = containerOf(api);

    await container.read(chatProvider.notifier).openThread(Fixtures.thread);

    final flow = container.read(studentFlowProvider);
    expect(flow.step, FlowStep.home);
    expect(flow.busy, isFalse);
    expect(flow.errorMessage, isNotNull);
    expect(container.read(chatProvider).messages, isNull);
  });

  test('ほかの操作で busy の間は、チャットの操作も何もしない（busy はアプリで 1 つ）', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      flow: home.copyWith(step: FlowStep.messageThreads, busy: true),
    );

    await container.read(chatProvider.notifier).refreshThreads();

    expect(api.calls, isEmpty);
    expect(container.read(chatProvider).threads, isNull);
  });

  test('送れたらメッセージを末尾に足して true を返し、断られたら文言を出して false を返す', () async {
    final api = FakeStudentApi(failures: {});
    final container = containerOf(
      api,
      flow: home.copyWith(step: FlowStep.messages),
      chat: ChatState(thread: Fixtures.thread, messages: Fixtures.messages),
    );
    final chat = container.read(chatProvider.notifier);

    expect(await chat.sendMessage('質問です'), isTrue);
    expect(await chat.sendStamp(StampId.letsGo), isTrue);
    final messages = container.read(chatProvider).messages!;
    expect(messages, hasLength(Fixtures.messages.length + 2));
    expect(messages[messages.length - 2].body, '質問です');
    expect(messages.last.stampId, StampId.letsGo);

    api.failures['sendMessage'] = const StudentApiException(
      statusCode: 403,
      error: ApiError(code: ErrorCode.chatNotEntitled, message: 'not entitled'),
    );
    expect(await chat.sendMessage('もう一度'), isFalse);
    expect(
      container.read(studentFlowProvider).errorMessage,
      'チャットは月額契約を始めると使えます。',
    );
    expect(container.read(chatProvider).messages, hasLength(messages.length));
  });

  test('返却が流れた添削は読めたものを読み直さず、読めなかったものだけ確かめ直したときに読み直す', () async {
    const failingVideoId = 'video-failing';
    final api = FakeStudentApi(
      messages: [
        returned(Fixtures.videoId, minutes: 1),
        returned(failingVideoId, minutes: 2),
      ],
      videoReviews: {
        Fixtures.videoId: Fixtures.reviewReturned,
        failingVideoId: Fixtures.reviewReturnedWithAi,
      },
    )..videoReviewFailures.add(failingVideoId);
    final container = containerOf(
      api,
      flow: home.copyWith(step: FlowStep.messages),
      chat: ChatState(thread: Fixtures.thread),
    );
    final chat = container.read(chatProvider.notifier);

    await chat.refreshMessages();

    var state = container.read(chatProvider);
    expect(state.reviews.keys, [Fixtures.videoId]);
    expect(state.reviewFailures, {failingVideoId});
    expect(container.read(studentFlowProvider).errorMessage, isNull);

    api.videoReviewFailures.clear();
    api.calls.clear();
    await chat.refreshMessages();

    expect(api.calls.where((c) => c == 'getVideoReview'), hasLength(1));
    expect(api.requests.last, failingVideoId);
    state = container.read(chatProvider);
    expect(
      state.reviews.keys,
      unorderedEquals([Fixtures.videoId, failingVideoId]),
    );
    expect(state.reviewFailures, isEmpty);
  });

  test('添削の依頼はスレッドの担当コーチと生徒のスポーツで送り、送れたらスレッドを読み直す', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      flow: home.copyWith(step: FlowStep.messages),
      chat: ChatState(thread: Fixtures.thread, messages: Fixtures.messages),
    );

    final result = await container
        .read(chatProvider.notifier)
        .requestVideoReview(contentType: VideoContentType.mp4, note: 'フォーム');

    expect(result, VideoReviewRequestResult.sent);
    expect(api.calls, ['submitVideo', 'markMessageThreadRead', 'listMessages']);
    final request = api.requests.whereType<SubmitVideoRequest>().single;
    expect(request.coachId, Fixtures.thread.coachId);
    expect(request.sport, Fixtures.student.sport);
    expect(
      container.read(chatProvider).messages!.last.kind,
      MessageKind.videoReviewRequest,
    );
  });

  test('メニューを付けた依頼が 422 で断られたら、文言を出さずにメニューを読み直して menuRejected を返す', () async {
    final api = FakeStudentApi(
      failures: {
        'submitVideo': const StudentApiException(
          statusCode: 422,
          error: ApiError(code: ErrorCode.validationFailed, message: 'invalid'),
        ),
      },
    );
    final container = containerOf(
      api,
      flow: home.copyWith(step: FlowStep.messages),
      chat: ChatState(thread: Fixtures.thread, messages: Fixtures.messages),
    );

    final result = await container
        .read(chatProvider.notifier)
        .requestVideoReview(
          contentType: VideoContentType.mp4,
          lessonMenuId: 'menu-gone',
        );

    expect(result, VideoReviewRequestResult.menuRejected);
    expect(api.calls, ['submitVideo', 'listCoachLessonMenus']);
    final flow = container.read(studentFlowProvider);
    expect(flow.errorMessage, isNull);
    expect(flow.busy, isFalse);
    expect(flow.step, FlowStep.messages);
  });

  test('ホームの「添削を依頼する」はスレッドが 1 つならそのスレッドを開いて依頼を出す印を立てる', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      chat: ChatState(threads: [Fixtures.thread]),
    );
    final chat = container.read(chatProvider.notifier);

    await chat.startVideoReviewRequest();

    expect(container.read(studentFlowProvider).step, FlowStep.messages);
    expect(container.read(chatProvider).promptVideoReviewRequest, isTrue);
    expect(container.read(chatProvider).returnStep, FlowStep.home);

    chat.clearVideoReviewRequestPrompt();
    expect(container.read(chatProvider).promptVideoReviewRequest, isFalse);
  });

  test('「添削を依頼する」でスレッドを開けなければ印を下ろし、ホームに留まって文言を出す', () async {
    final api = FakeStudentApi(
      failures: {'listMessages': Fixtures.unreachable},
    );
    final container = containerOf(
      api,
      chat: ChatState(threads: [Fixtures.thread]),
    );

    await container.read(chatProvider.notifier).startVideoReviewRequest();

    final flow = container.read(studentFlowProvider);
    expect(flow.step, FlowStep.home);
    expect(flow.errorMessage, isNotNull);
    expect(container.read(chatProvider).promptVideoReviewRequest, isFalse);
  });

  test('ホームを読むと、スレッドの一覧はチャットの状態に入る（読めなければホームの欄の失敗にする）', () async {
    final api = FakeStudentApi(failures: {});
    final container = containerOf(api);

    await container.read(studentFlowProvider.notifier).refreshHome();
    expect(container.read(chatProvider).threads, [Fixtures.thread]);
    expect(container.read(studentFlowProvider).homeFailures, isEmpty);

    api.failures['listMessageThreads'] = Fixtures.unreachable;
    await container.read(studentFlowProvider.notifier).refreshHome();
    expect(container.read(studentFlowProvider).homeFailures, {
      HomeSection.threads,
    });
  });

  test('スレッドの一覧とメッセージは、コンストラクタで直接渡しても copyWith で渡しても写して持ち、外から書き換えられない', () {
    // コンストラクタで直接渡した元のリストを後で変えても、状態は変わらない
    final threads = [Fixtures.thread];
    final messages = [...Fixtures.messages];
    final constructed = ChatState(threads: threads, messages: messages);
    threads.clear();
    messages.clear();
    expect(constructed.threads, [Fixtures.thread]);
    expect(constructed.messages, Fixtures.messages);
    expect(() => constructed.threads!.clear(), throwsUnsupportedError);
    expect(
      () => constructed.messages!.add(Fixtures.messages.first),
      throwsUnsupportedError,
    );

    // copyWith で渡した元のリストを後で変えても、状態は変わらない
    final threadsSource = [Fixtures.thread];
    final messagesSource = [...Fixtures.messages];
    final copied = ChatState().copyWith(
      threads: threadsSource,
      messages: messagesSource,
    );
    threadsSource.clear();
    messagesSource.clear();
    expect(copied.threads, [Fixtures.thread]);
    expect(copied.messages, Fixtures.messages);
    expect(() => copied.threads!.add(Fixtures.thread), throwsUnsupportedError);
    expect(() => copied.messages!.clear(), throwsUnsupportedError);
    // 渡さなければ未取得のまま
    expect(ChatState().threads, isNull);
    expect(ChatState().messages, isNull);
  });

  test('添削と読めなかった動画は、コンストラクタで直接渡しても copyWith で渡しても写して持ち、外から書き換えられない', () {
    // コンストラクタで直接渡した元の一覧を後で変えても、状態は変わらない
    final reviews = {Fixtures.videoId: Fixtures.reviewReturned};
    final failures = {'video-failed'};
    final constructed = ChatState(reviews: reviews, reviewFailures: failures);
    reviews.clear();
    failures.clear();
    expect(constructed.reviews, {Fixtures.videoId: Fixtures.reviewReturned});
    expect(constructed.reviewFailures, {'video-failed'});
    expect(() => constructed.reviews.clear(), throwsUnsupportedError);
    expect(
      () => constructed.reviewFailures.add('video-other'),
      throwsUnsupportedError,
    );

    // copyWith で渡した元の一覧を後で変えても、状態は変わらない
    final reviewsSource = {Fixtures.videoId: Fixtures.reviewReturned};
    final failuresSource = {'video-failed'};
    final copied = ChatState().copyWith(
      reviews: reviewsSource,
      reviewFailures: failuresSource,
    );
    reviewsSource.clear();
    failuresSource.clear();
    expect(copied.reviews, {Fixtures.videoId: Fixtures.reviewReturned});
    expect(copied.reviewFailures, {'video-failed'});
    expect(() => copied.reviews.clear(), throwsUnsupportedError);
    expect(() => copied.reviewFailures.clear(), throwsUnsupportedError);
    // 渡さなければ空
    expect(ChatState().reviews, isEmpty);
    expect(ChatState().reviewFailures, isEmpty);
  });

  test('メッセージを送ると、送ったメッセージを足した一覧も書き換えられない', () async {
    final api = FakeStudentApi();
    final container = containerOf(
      api,
      chat: ChatState(thread: Fixtures.thread, messages: Fixtures.messages),
    );

    final sent = await container
        .read(chatProvider.notifier)
        .sendMessage('こんにちは');

    expect(sent, isTrue);

    final messages = container.read(chatProvider).messages!;
    expect(messages, hasLength(Fixtures.messages.length + 1));
    expect(() => messages.clear(), throwsUnsupportedError);
  });
}
