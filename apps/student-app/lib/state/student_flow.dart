import 'dart:collection';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/error_message.dart';
import '../api/http_student_api.dart';
import '../api/models.dart';
import '../api/student_api.dart';

/// 一周シナリオ（体験予約 → 初回レッスン → 完了記録 → 評価 → 月額契約）と自分の予約・チャット・通知の受け取り設定の画面の段。
enum FlowStep {
  register,
  slotSearch,
  bookingPending,
  liveLesson,
  videoSubmit,
  videoReview,
  lessonCompletion,

  /// 完了したレッスンの評価（1〜5 の点と一言）
  lessonReview,
  subscription,

  /// 担当コーチとのスレッドの一覧（登録後のどの段からも開け、閉じると開いた段に戻る）
  messageThreads,

  /// 1 つのスレッドのメッセージ
  messages,

  /// 通知の受け取り設定（登録後のどの段からも開け、閉じると開いた段に戻る）
  notificationSettings,

  /// 自分の予約の一覧（登録後のどの段からも開け、閉じると開いた段に戻る）
  myBookings;

  bool get isChat => this == messageThreads || this == messages;

  /// 一周シナリオの脇に開く画面（チャット・通知の受け取り設定・自分の予約）。ここからはほかの脇の画面を開かない
  bool get isAside =>
      isChat || this == notificationSettings || this == myBookings;
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
    this.lessonReview,
    Set<String> reviewedLessonIds = const {},
    this.subscription,
    this.entitlement,
    this.chatReturnStep,
    this.threads,
    this.thread,
    this.messages,
    this.settingsReturnStep,
    this.notificationSettings,
    this.bookingsReturnStep,
    this.myBookings,
    // 名前付き引数は _ で始められないので、初期化子で private の欄に入れる
    // ignore: prefer_initializing_formals
  }) : _reviewedLessonIds = reviewedLessonIds;

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

  /// この端末で送った評価（`submitLessonReview` の応答）
  final LessonReview? lessonReview;

  /// 評価済みと分かっている完了したレッスンの ID。評価済みを返す読み取りの API はないので、
  /// この端末で送れたときと、送って 409 `lesson_already_reviewed` が返ったときに足す。
  /// 読み出しは変更できないビューで返す（どの経路で作っても、`add` などで Riverpod を通さずに状態を変えさせない）
  Set<String> get reviewedLessonIds => UnmodifiableSetView(_reviewedLessonIds);
  final Set<String> _reviewedLessonIds;

  /// 完了を記録したレッスンを評価済みと分かっているか
  bool get lessonReviewed => reviewedLessonIds.contains(lesson?.lessonId);

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

  /// 通知の受け取り設定を閉じたときに戻る段（設定を開いた段）
  final FlowStep? settingsReturnStep;

  /// 通知の受け取り設定（`getMyNotificationSettings`。未取得なら null）。
  /// 切り替えたときは保存を待たずに変え、保存に失敗したら元に戻す
  final NotificationSettings? notificationSettings;

  /// 自分の予約の一覧を閉じたときに戻る段（一覧を開いた段）
  final FlowStep? bookingsReturnStep;

  /// 自分の予約（`listMyBookings`。未取得なら null）。キャンセルしたらその予約を応答で置き換える
  final List<MyBooking>? myBookings;

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
    LessonReview? lessonReview,
    Set<String>? reviewedLessonIds,
    Subscription? subscription,
    Entitlement? entitlement,
    FlowStep? chatReturnStep,
    List<MessageThread>? threads,
    MessageThread? thread,
    List<Message>? messages,
    FlowStep? settingsReturnStep,
    NotificationSettings? notificationSettings,
    FlowStep? bookingsReturnStep,
    List<MyBooking>? myBookings,
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
    lessonReview: lessonReview ?? this.lessonReview,
    reviewedLessonIds: reviewedLessonIds ?? _reviewedLessonIds,
    subscription: subscription ?? this.subscription,
    entitlement: entitlement ?? this.entitlement,
    chatReturnStep: chatReturnStep ?? this.chatReturnStep,
    threads: threads ?? this.threads,
    thread: thread ?? this.thread,
    messages: messages ?? this.messages,
    settingsReturnStep: settingsReturnStep ?? this.settingsReturnStep,
    notificationSettings: notificationSettings ?? this.notificationSettings,
    bookingsReturnStep: bookingsReturnStep ?? this.bookingsReturnStep,
    // 外から書き換えて state = を通さずに状態を変えられないよう、読み取り専用で持つ（const のコンストラクタでは包めないので copyWith で包む）
    myBookings: myBookings != null
        ? List.unmodifiable(myBookings)
        : this.myBookings,
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

/// 今の時刻。予約が始まったか・キャンセルで回数を使うかの表示に使う。テストでは固定の時刻に差し替える。
final clockProvider = Provider<DateTime Function()>((ref) => DateTime.now);

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

  /// 完了したレッスンを評価する画面へ
  void goToReview() => state = state.copyWith(step: FlowStep.lessonReview);

  /// `submitLessonReview`。評価は完了したレッスン 1 件につき 1 回なので、送れたときも
  /// 409 `lesson_already_reviewed` が返ったときも評価済みにする（以後は入力させない）。
  Future<void> submitReview({required int rating, required String comment}) =>
      _run(() async {
        final lessonId = state.lesson!.lessonId;
        try {
          final review = await _api.submitLessonReview(
            SubmitLessonReviewRequest(
              lessonId: lessonId,
              rating: rating,
              comment: comment,
            ),
          );
          state = state.copyWith(
            lessonReview: review,
            reviewedLessonIds: {...state.reviewedLessonIds, lessonId},
          );
        } on StudentApiException catch (e) {
          if (e.error?.code == ErrorCode.lessonAlreadyReviewed) {
            state = state.copyWith(
              reviewedLessonIds: {...state.reviewedLessonIds, lessonId},
            );
          }
          rethrow;
        }
      });

  /// 月額契約の画面へ
  void goToSubscription() =>
      state = state.copyWith(step: FlowStep.subscription);

  /// `startSubscription`。[plan] は選んだ契約プラン（既定は standard）。
  /// 決済は Gen1 ではモックなので、決済手段のトークンも固定のモック。
  Future<void> startSubscription({
    SubscriptionPlan plan = SubscriptionPlan.standard,
  }) => _run(() async {
    final response = await _api.startSubscription(
      StartSubscriptionRequest(
        sport: state.student!.sport,
        plan: plan,
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

  /// スレッドを開く。相手のメッセージを `markMessageThreadRead` で既読にしてから `listMessages` で読む
  /// （一覧を読むだけでは既読にならない契約なので、画面に出したら既読にする）。
  /// 既読が先なので、既読にしたメッセージは必ず一覧に入る（間に届いたものは未読のまま出て、次に確かめたときに既読になる）。
  Future<void> openThread(MessageThread thread) => _run(() async {
    final read = await _api.markMessageThreadRead(thread.threadId);
    final list = await _api.listMessages(thread.threadId);
    state = state.copyWith(
      thread: read,
      messages: list.messages,
      step: FlowStep.messages,
    );
  });

  /// 開いているスレッドの新しいメッセージを確かめる（既読にしてから読み直す。順は [openThread] と同じ）
  Future<void> refreshMessages() => _run(() async {
    final threadId = state.thread!.threadId;
    final read = await _api.markMessageThreadRead(threadId);
    final list = await _api.listMessages(threadId);
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

  /// 通知の受け取り設定を開く（`getMyNotificationSettings`）。閉じたら今の段に戻る。
  Future<void> openNotificationSettings() {
    if (state.step != FlowStep.notificationSettings) {
      state = state.copyWith(
        settingsReturnStep: state.step,
        step: FlowStep.notificationSettings,
      );
    }
    return _run(_loadNotificationSettings);
  }

  /// 通知の受け取り設定を読み直す（読み込みに失敗したとき）
  Future<void> refreshNotificationSettings() => _run(_loadNotificationSettings);

  /// 通知の受け取り設定を閉じて、開いた段に戻る
  void closeNotificationSettings() => state = state.copyWith(
    step: state.settingsReturnStep ?? FlowStep.slotSearch,
  );

  /// `updateMyNotificationSettings`。[kind] の種類だけを変えて保存する。画面はすぐに切り替え、
  /// 保存に失敗したら切り替える前に戻して失敗を知らせる（成功したらサーバーが返した設定にそろえる）。
  Future<void> setNotification(NotificationKind kind, bool receives) =>
      _run(() async {
        final before = state.notificationSettings!;
        state = state.copyWith(
          notificationSettings: before.withKind(kind, receives),
        );
        try {
          final saved = await _api.updateMyNotificationSettings(
            UpdateNotificationSettingsRequest.only(kind, receives),
          );
          state = state.copyWith(notificationSettings: saved);
        } on StudentApiException {
          if (ref.mounted) state = state.copyWith(notificationSettings: before);
          rethrow;
        }
      });

  Future<void> _loadNotificationSettings() async {
    final settings = await _api.getMyNotificationSettings();
    state = state.copyWith(notificationSettings: settings);
  }

  /// 自分の予約の一覧を開く（`listMyBookings`）。閉じたら今の段に戻る。
  Future<void> openMyBookings() {
    if (state.step != FlowStep.myBookings) {
      state = state.copyWith(
        bookingsReturnStep: state.step,
        step: FlowStep.myBookings,
      );
    }
    return _run(_loadMyBookings);
  }

  /// 自分の予約の一覧を読み直す
  Future<void> refreshMyBookings() => _run(_loadMyBookings);

  /// 自分の予約の一覧を閉じて、開いた段に戻る
  void closeMyBookings() => state = state.copyWith(
    step: state.bookingsReturnStep ?? FlowStep.slotSearch,
  );

  /// キャンセルのボタンを出すか。キャンセル済みでなく、まだ始まっていない予約だけ
  bool canCancel(Booking booking) =>
      booking.status != BookingStatus.cancelled &&
      booking.startsAt.isAfter(_now());

  /// 今キャンセルすると今月のビデオ通話の回数を使うか（確認の文言を添えるかの目安）。
  /// 回数を使うかを決めるのはサーバー（scheduling のキャンセル規定。開始 24 時間前ちょうどからを含む）で、
  /// ここは確認に添える文言のためだけに同じ境界で見る。回数の制約があるのは月額契約の生徒だけ（docs/adr/0005 の 6）
  bool cancellationUsesQuota(Booking booking) =>
      state.entitlement?.contractType == ContractType.subscription &&
      !_now().add(_quotaConsumingNotice).isBefore(booking.startsAt);

  static const _quotaConsumingNotice = Duration(hours: 24);

  /// `cancelBooking`。キャンセルできたら一覧のその予約を応答で置き換える。
  Future<void> cancelBooking(Booking booking) => _run(() async {
    final cancelled = await _api.cancelBooking(booking.bookingId);
    state = state.copyWith(
      myBookings: [
        for (final item in state.myBookings ?? const <MyBooking>[])
          item.booking.bookingId == cancelled.bookingId
              ? MyBooking(booking: cancelled, slot: item.slot)
              : item,
      ],
    );
  });

  Future<void> _loadMyBookings() async {
    final list = await _api.listMyBookings();
    state = state.copyWith(myBookings: list.bookings);
  }

  DateTime _now() => ref.read(clockProvider)();

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
