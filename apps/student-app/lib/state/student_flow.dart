import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/error_message.dart';
import '../api/http_student_api.dart';
import '../api/models.dart';
import '../api/student_api.dart';

/// 一周シナリオ（体験予約 → 初回レッスン → 完了記録 → 月額契約）とチャットの画面の段。
enum FlowStep {
  register,
  slotSearch,
  bookingPending,
  liveLesson,
  videoSubmit,
  videoReview,
  lessonCompletion,
  subscription,

  /// 担当コーチとのスレッドの一覧（登録後のどの段からも開け、閉じると開いた段に戻る）
  messageThreads,

  /// 1 つのスレッドのメッセージ
  messages;

  bool get isChat => this == messageThreads || this == messages;
}

/// 生徒アプリの状態（アプリの状態はこれ 1 つ）。
class StudentFlowState {
  const StudentFlowState({
    this.step = FlowStep.register,
    this.busy = false,
    this.errorMessage,
    this.accessToken,
    this.student,
    this.searchSport,
    this.slots,
    this.bookedSlot,
    this.booking,
    this.session,
    this.video,
    this.review,
    this.lesson,
    this.subscription,
    this.entitlement,
    this.chatReturnStep,
    this.threads,
    this.thread,
    this.messages,
  });

  final FlowStep step;

  /// API を呼んでいる間は true（ボタンを押せなくする）
  final bool busy;

  /// 直前の操作の失敗（画面の文言）。次の操作で消える
  final String? errorMessage;

  /// 契約の bearerAuth。中身は解釈しない
  final String? accessToken;
  final Student? student;

  /// スロットを絞ったスポーツ（未検索なら null）
  final Sport? searchSport;
  final List<Slot>? slots;

  /// 体験予約したスロット（動画の送付でスポーツを使う）
  final Slot? bookedSlot;

  /// 体験予約
  final Booking? booking;
  final LiveLessonSession? session;
  final Video? video;
  final VideoReview? review;

  /// 完了を記録したレッスン
  final Lesson? lesson;
  final Subscription? subscription;
  final Entitlement? entitlement;

  /// チャットを閉じたときに戻る段（チャットを開いた段）
  final FlowStep? chatReturnStep;

  /// 担当コーチとのスレッド（`listMessageThreads`。未取得なら null）
  final List<MessageThread>? threads;

  /// 開いているスレッド
  final MessageThread? thread;

  /// 開いているスレッドのメッセージ（送った順）
  final List<Message>? messages;

  /// `errorMessage` は引き継がない（渡さなければ消える）。
  StudentFlowState copyWith({
    FlowStep? step,
    bool? busy,
    String? errorMessage,
    String? accessToken,
    Student? student,
    Sport? searchSport,
    List<Slot>? slots,
    Slot? bookedSlot,
    Booking? booking,
    LiveLessonSession? session,
    Video? video,
    VideoReview? review,
    Lesson? lesson,
    Subscription? subscription,
    Entitlement? entitlement,
    FlowStep? chatReturnStep,
    List<MessageThread>? threads,
    MessageThread? thread,
    List<Message>? messages,
  }) => StudentFlowState(
    step: step ?? this.step,
    busy: busy ?? this.busy,
    errorMessage: errorMessage,
    accessToken: accessToken ?? this.accessToken,
    student: student ?? this.student,
    searchSport: searchSport ?? this.searchSport,
    slots: slots ?? this.slots,
    bookedSlot: bookedSlot ?? this.bookedSlot,
    booking: booking ?? this.booking,
    session: session ?? this.session,
    video: video ?? this.video,
    review: review ?? this.review,
    lesson: lesson ?? this.lesson,
    subscription: subscription ?? this.subscription,
    entitlement: entitlement ?? this.entitlement,
    chatReturnStep: chatReturnStep ?? this.chatReturnStep,
    threads: threads ?? this.threads,
    thread: thread ?? this.thread,
    messages: messages ?? this.messages,
  );
}

/// 接続先。`flutter run --dart-define=RALLY_API_URL=...` で変える（既定はバックエンドの既定の待ち受け）。
const _apiBaseUrl = String.fromEnvironment(
  'RALLY_API_URL',
  defaultValue: 'http://127.0.0.1:8080',
);

/// 画面が使う API。テストではモックに差し替える。
final studentApiProvider = Provider<StudentApi>(
  (ref) => HttpStudentApi(
    baseUrl: Uri.parse(_apiBaseUrl),
    accessToken: () => ref.read(studentFlowProvider).accessToken,
  ),
);

final studentFlowProvider =
    NotifierProvider<StudentFlowNotifier, StudentFlowState>(
      StudentFlowNotifier.new,
    );

/// 画面の操作。API を呼ぶのはここだけで、画面は state を表示してこのメソッドを呼ぶ。
class StudentFlowNotifier extends Notifier<StudentFlowState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  StudentFlowNotifier([this._initial = const StudentFlowState()]);

  final StudentFlowState _initial;

  @override
  StudentFlowState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  /// `registerAccount`。登録したらスロット検索へ（生徒のスポーツで絞って検索しておく）。
  Future<void> register({
    required String email,
    required String displayName,
    required Sport sport,
  }) => _run(() async {
    final response = await _api.registerAccount(
      RegisterAccountRequest(
        email: email,
        displayName: displayName,
        sport: sport,
      ),
    );
    state = state.copyWith(
      accessToken: response.accessToken,
      student: response.student,
      step: FlowStep.slotSearch,
    );
    final page = await _api.searchSlots(sport: sport);
    state = state.copyWith(searchSport: sport, slots: page.items);
  });

  /// `searchSlots`。スポーツで絞る。
  Future<void> searchSlots(Sport sport) => _run(() async {
    final page = await _api.searchSlots(sport: sport);
    state = state.copyWith(searchSport: sport, slots: page.items);
  });

  /// `createBooking`（体験予約）。作成直後は pending なので確定待ちへ。
  Future<void> bookTrial(Slot slot) => _run(() async {
    final booking = await _api.createBooking(
      CreateBookingRequest(slotId: slot.slotId, isTrial: true),
    );
    state = state.copyWith(
      bookedSlot: slot,
      booking: booking,
      step: FlowStep.bookingPending,
    );
  });

  /// `confirmBooking`
  Future<void> confirmBooking() => _run(() async {
    final booking = await _api.confirmBooking(state.booking!.bookingId);
    state = state.copyWith(booking: booking);
  });

  /// 初回レッスンを通話で受ける
  void chooseLiveLesson() => state = state.copyWith(step: FlowStep.liveLesson);

  /// 初回レッスンを動画の送付で受ける
  void chooseVideo() => state = state.copyWith(step: FlowStep.videoSubmit);

  /// `startLiveLessonSession`（映像はモックなので接続はしない）
  Future<void> startCall() => _run(() async {
    final session = await _api.startLiveLessonSession(
      StartLiveLessonSessionRequest(bookingId: state.booking!.bookingId),
    );
    state = state.copyWith(session: session);
  });

  /// `endLiveLessonSession`。終了したら完了記録へ。
  Future<void> endCall() => _run(() async {
    final session = await _api.endLiveLessonSession(state.session!.sessionId);
    state = state.copyWith(session: session, step: FlowStep.lessonCompletion);
  });

  /// `submitVideo`。送り先は体験予約のコーチで、スポーツはそのスロットのもの。動画本体のアップロードは Gen1 ではモックなので行わない。
  Future<void> submitVideo({
    required VideoContentType contentType,
    String? note,
  }) => _run(() async {
    final booking = state.booking!;
    final video = await _api.submitVideo(
      SubmitVideoRequest(
        coachId: booking.coachId,
        sport: state.bookedSlot!.sport,
        contentType: contentType,
        note: note,
      ),
    );
    state = state.copyWith(video: video, step: FlowStep.videoReview);
    final review = await _api.getVideoReview(video.videoId);
    state = state.copyWith(review: review);
  });

  /// `getVideoReview`（コーチが返却したかを確かめ直す）
  Future<void> refreshReview() => _run(() async {
    final review = await _api.getVideoReview(state.video!.videoId);
    state = state.copyWith(review: review);
  });

  /// 添削が返されたら完了記録へ
  void goToCompletion() =>
      state = state.copyWith(step: FlowStep.lessonCompletion);

  /// `recordLessonCompletion`。通話を受けたなら live_lesson、動画を送ったなら async_lesson。
  Future<void> recordCompletion() => _run(() async {
    final session = state.session;
    final lesson = await _api.recordLessonCompletion(
      session != null
          ? LiveLessonCompletion(sessionId: session.sessionId)
          : AsyncLessonCompletion(videoId: state.video!.videoId),
    );
    state = state.copyWith(lesson: lesson);
  });

  /// 月額契約の画面へ
  void goToSubscription() =>
      state = state.copyWith(step: FlowStep.subscription);

  /// `startSubscription`。決済は Gen1 ではモックなので、決済手段のトークンも固定のモック。
  Future<void> startSubscription() => _run(() async {
    final response = await _api.startSubscription(
      StartSubscriptionRequest(
        sport: state.student!.sport,
        paymentMethodToken: mockPaymentMethodToken,
      ),
    );
    state = state.copyWith(
      subscription: response.subscription,
      entitlement: response.entitlement,
    );
  });

  /// チャットを開く（`listMessageThreads`）。閉じたら今の段に戻る。
  Future<void> openChat() {
    if (!state.step.isChat) {
      state = state.copyWith(
        chatReturnStep: state.step,
        step: FlowStep.messageThreads,
      );
    }
    return _run(_loadThreads);
  }

  /// チャットを閉じて、開いた段に戻る
  void closeChat() =>
      state = state.copyWith(step: state.chatReturnStep ?? FlowStep.slotSearch);

  /// スレッドを開く。`listMessages` で読み、相手のメッセージを `markMessageThreadRead` で既読にする
  /// （一覧を読むだけでは既読にならない契約なので、画面に出したら既読にする）。
  Future<void> openThread(MessageThread thread) => _run(() async {
    final list = await _api.listMessages(thread.threadId);
    final read = await _api.markMessageThreadRead(thread.threadId);
    state = state.copyWith(
      thread: read,
      messages: list.messages,
      step: FlowStep.messages,
    );
  });

  /// 開いているスレッドの新しいメッセージを確かめる（読み直して既読にする）
  Future<void> refreshMessages() => _run(() async {
    final threadId = state.thread!.threadId;
    final list = await _api.listMessages(threadId);
    final read = await _api.markMessageThreadRead(threadId);
    state = state.copyWith(thread: read, messages: list.messages);
  });

  /// `sendMessage`。送れたら true（月額契約の前は 403 `chat_not_entitled` で false）。
  Future<bool> sendMessage(String body) async {
    var sent = false;
    await _run(() async {
      final message = await _api.sendMessage(
        state.thread!.threadId,
        SendMessageRequest(body: body),
      );
      state = state.copyWith(messages: [...?state.messages, message]);
      sent = true;
    });
    return sent;
  }

  /// スレッドの一覧を読み直す
  Future<void> refreshThreads() => _run(_loadThreads);

  /// スレッドの一覧に戻る（未読の数と最後のメッセージを読み直す）
  Future<void> backToThreads() {
    state = state.copyWith(step: FlowStep.messageThreads);
    return _run(_loadThreads);
  }

  Future<void> _loadThreads() async {
    final list = await _api.listMessageThreads();
    state = state.copyWith(threads: list.threads);
  }

  Future<void> _run(Future<void> Function() action) async {
    if (state.busy) return;
    state = state.copyWith(busy: true);
    try {
      await action();
      if (ref.mounted) state = state.copyWith(busy: false);
    } on StudentApiException catch (e) {
      if (ref.mounted) {
        state = state.copyWith(busy: false, errorMessage: errorMessageOf(e));
      }
    }
  }
}

/// 決済（Stripe 相当）のモックのトークン。サーバーは受け取るだけで使わない。
const mockPaymentMethodToken = 'mock-payment-method';
