import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/error_message.dart';
import '../api/http_student_api.dart';
import '../api/models.dart';
import '../api/student_api.dart';
import 'chat.dart';
import 'coach_profile.dart';
import 'coach_search.dart';
import 'lesson_review.dart';
import 'notifications.dart';
import 'subscription.dart';

/// 一周シナリオ（体験予約 → 初回レッスン → 完了記録 → 評価 → 月額契約）とホーム・自分の予約・自分の動画・チャット・通知の受け取り設定・お知らせ・プロフィールの画面の段。
enum FlowStep {
  register,

  /// ホーム（登録後に最初に開く。これからの予約・今月のビデオ通話の残り・コーチとのやり取り・返ってきた添削・添削の依頼の入口）
  home,

  /// 生徒のスポーツのコーチの一覧（ホームから開く。状態は [CoachSearchState]）
  coachSearch,

  /// コーチの紹介（コーチの一覧の行から開く。状態は [CoachProfileState]）
  coachProfile,
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

  /// お知らせの一覧（登録後のどの段からも開け、閉じると開いた段に戻る。開いたらすべて既読にする。状態は [NotificationsState]）。
  /// 行からは種類に応じて自分の予約・自分の動画・チャットを開く（閉じるとここに戻る）
  notifications,

  /// 自分のプロフィール（表示名を見て変える。登録後のどの段からも開け、閉じると開いた段に戻る）
  profile,

  /// 自分の予約の一覧（登録後のどの段からも開け、閉じると開いた段に戻る）
  myBookings,

  /// 自分が送った動画の一覧（登録後のどの段からも開け、閉じると開いた段に戻る）
  myVideos,

  /// 自分の動画の一覧かホームの「返ってきた添削」から開いた、返却済みの動画の添削結果（閉じると開いた段に戻る）
  myVideoReview;

  bool get isChat => this == messageThreads || this == messages;

  /// 一周シナリオの脇に開く画面（チャット・通知の受け取り設定・お知らせ・プロフィール・自分の予約・自分の動画）。
  /// ここからはアプリバーでほかの脇の画面を開かない（お知らせの行からは開く）
  bool get isAside =>
      isChat ||
      this == notificationSettings ||
      this == notifications ||
      this == profile ||
      this == myBookings ||
      this == myVideos ||
      this == myVideoReview;
}

/// ホームの欄（API ごとに読み、どれかが失敗しても他の欄は出す）
enum HomeSection {
  /// これからの予約（`listMyBookings`）
  bookings,

  /// 今月のビデオ通話の残り（`getMyEntitlement`）
  entitlement,

  /// コーチとのやり取り（`listMessageThreads`）
  threads,

  /// 返ってきた添削（`listMyVideos`）
  videos,
}

/// 添削の依頼（チャットの [ChatNotifier.requestVideoReview] と一周シナリオの [StudentFlowNotifier.submitVideo]）の結果
enum VideoReviewRequestResult {
  /// 受け付けられた
  sent,

  /// 選んだレッスンメニューを付けて 422 で断られた（選んだ後にメニューが変わったものと読み、メニューを読み直してある）
  menuRejected,

  /// それ以外の失敗（文言は [StudentFlowState.errorMessage]）
  failed,
}

/// 生徒アプリの状態（チャットの状態は [ChatState]、完了したレッスンの評価の状態は [LessonReviewState]、
/// 月額契約と受講権の状態は [SubscriptionState]、コーチを探す画面の状態は [CoachSearchState]、コーチの紹介の状態は
/// [CoachProfileState] に分けた。画面の段と busy・失敗の文言はここで 1 つ）。
/// 一覧をコンストラクタで写して持つので const では作らない。
class StudentFlowState {
  StudentFlowState({
    this.step = FlowStep.register,
    this.busy = false,
    this.errorMessage,
    this.accessToken,
    this.student,
    this.searchSport,
    this.searchCoach,
    this.slots,
    this.coachReviewSummaries = const {},
    this.coachReviewSummaryFailures = const {},
    this.coachReviews = const {},
    this.coachReviewsFailures = const {},
    this.coachLessonMenus = const {},
    this.coachLessonMenusFailures = const {},
    this.bookedSlot,
    this.booking,
    this.session,
    this.video,
    this.review,
    this.lesson,
    this.homeFailures = const {},
    this.settingsReturnStep,
    this.notificationSettings,
    this.profileReturnStep,
    this.bookingsReturnStep,
    this.myBookings,
    this.videosReturnStep,
    List<MyVideo>? myVideos,
    this.myVideoReview,
    this.videoReviewReturnStep,
    // 渡した元のリストを後で変えても状態が変わらないよう、読み取り専用の写しを持つ
  }) : myVideos = myVideos == null ? null : List.unmodifiable(myVideos);

  final FlowStep step;

  /// API を呼んでいる間は true（ボタンを押せなくする）
  final bool busy;

  /// 直前の操作の失敗（画面の文言）。次の操作で消える
  final String? errorMessage;

  /// 契約の bearerAuth。中身は解釈しない
  final String? accessToken;

  /// 登録した生徒（`registerAccount`）。プロフィールを開いたら `getMyStudentProfile`、表示名を変えたら
  /// `updateMyStudentProfile` の応答で置き換える（ホームの挨拶などはここの表示名を出す）
  final Student? student;

  /// スロットを絞ったスポーツ（未検索なら null）
  final Sport? searchSport;

  /// スロットを絞ったコーチ（コーチの紹介の「このコーチの空き枠を見る」から来たとき。絞っていなければ null）
  final CoachListing? searchCoach;
  final List<Slot>? slots;

  /// 検索結果のコーチの評価の件数と平均（`getCoachReviewSummary`。コーチの ID ごと）。検索のたびに読み直す
  final Map<String, CoachReviewSummary> coachReviewSummaries;

  /// 評価の件数と平均を読めなかったコーチの ID（検索結果は出し、評価の欄にだけその旨を出す）
  final Set<String> coachReviewSummaryFailures;

  /// 開いたコーチの評価の一覧（`listCoachReviews`。コーチの ID ごと。送った時刻の新しい順）。検索のたびに捨てる
  final Map<String, List<CoachReview>> coachReviews;

  /// 評価の一覧を読めなかったコーチの ID
  final Set<String> coachReviewsFailures;

  /// 開いたコーチの公開中のレッスンメニューのうち、生徒のスポーツのもの（`listCoachLessonMenus`。コーチの ID ごと。
  /// 作った時刻の古い順）。検索のたびに捨てる
  final Map<String, List<LessonMenu>> coachLessonMenus;

  /// レッスンメニューを読めなかったコーチの ID
  final Set<String> coachLessonMenusFailures;

  /// 添削の依頼で選べる、コーチの公開中の非同期レッスンのメニューのうち生徒のスポーツのもの（[coachLessonMenus] を
  /// 非同期で絞る。[sport] を渡せばそのスポーツでも絞る。作った時刻の古い順）。まだ読んでいなければ null
  List<LessonMenu>? asyncLessonMenusOf(String coachId, {Sport? sport}) =>
      coachLessonMenus[coachId]
          ?.where(
            (menu) =>
                menu.lessonType == LessonType.asyncLesson &&
                (sport == null || menu.sport == sport),
          )
          .toList(growable: false);

  /// 体験予約したスロット（動画の送付でスポーツを使う）
  final Slot? bookedSlot;

  /// 体験予約
  final Booking? booking;
  final LiveLessonSession? session;
  final Video? video;
  final VideoReview? review;

  /// 完了を記録したレッスン（評価する対象。評価の状態は [LessonReviewState]）
  final Lesson? lesson;

  /// ホームで読めなかった欄（その欄にだけ旨を出す）。ホームを読むたびに読み直す
  final Set<HomeSection> homeFailures;

  /// 通知の受け取り設定を閉じたときに戻る段（設定を開いた段）
  final FlowStep? settingsReturnStep;

  /// 通知の受け取り設定（`getMyNotificationSettings`。未取得なら null）。
  /// 切り替えたときは保存を待たずに変え、保存に失敗したら元に戻す
  final NotificationSettings? notificationSettings;

  /// プロフィールを閉じたときに戻る段（プロフィールを開いた段）
  final FlowStep? profileReturnStep;

  /// 自分の予約の一覧を閉じたときに戻る段（一覧を開いた段）
  final FlowStep? bookingsReturnStep;

  /// 自分の予約（`listMyBookings`。未取得なら null）。キャンセルしたらその予約を応答で置き換える
  final List<MyBooking>? myBookings;

  /// 自分の動画の一覧を閉じたときに戻る段（一覧を開いた段）
  final FlowStep? videosReturnStep;

  /// 自分が送った動画（`listMyVideos`。未取得なら null。送った時刻の新しい順）。
  /// コンストラクタでも copyWith でも、渡したリストの読み取り専用の写しを持つ（Riverpod を通さずに状態を変えさせない）
  final List<MyVideo>? myVideos;

  /// 自分の動画の一覧から開いた動画の添削（`getVideoReview`）。一周シナリオの [review] とは別に持つ
  final VideoReview? myVideoReview;

  /// [myVideoReview] の画面を閉じたときに戻る段（自分の動画の一覧かホーム）
  final FlowStep? videoReviewReturnStep;

  /// `errorMessage` は引き継がない（渡さなければ消える）。[searchCoach] は [clearSearchCoach] で外す。
  StudentFlowState copyWith({
    FlowStep? step,
    bool? busy,
    String? errorMessage,
    String? accessToken,
    Student? student,
    Sport? searchSport,
    CoachListing? searchCoach,
    bool clearSearchCoach = false,
    List<Slot>? slots,
    Map<String, CoachReviewSummary>? coachReviewSummaries,
    Set<String>? coachReviewSummaryFailures,
    Map<String, List<CoachReview>>? coachReviews,
    Set<String>? coachReviewsFailures,
    Map<String, List<LessonMenu>>? coachLessonMenus,
    Set<String>? coachLessonMenusFailures,
    Slot? bookedSlot,
    Booking? booking,
    LiveLessonSession? session,
    Video? video,
    VideoReview? review,
    Lesson? lesson,
    Set<HomeSection>? homeFailures,
    FlowStep? settingsReturnStep,
    NotificationSettings? notificationSettings,
    FlowStep? profileReturnStep,
    FlowStep? bookingsReturnStep,
    List<MyBooking>? myBookings,
    FlowStep? videosReturnStep,
    List<MyVideo>? myVideos,
    VideoReview? myVideoReview,
    FlowStep? videoReviewReturnStep,
  }) => StudentFlowState(
    step: step ?? this.step,
    busy: busy ?? this.busy,
    errorMessage: errorMessage,
    accessToken: accessToken ?? this.accessToken,
    student: student ?? this.student,
    searchSport: searchSport ?? this.searchSport,
    searchCoach: clearSearchCoach ? null : searchCoach ?? this.searchCoach,
    slots: slots ?? this.slots,
    coachReviewSummaries: coachReviewSummaries ?? this.coachReviewSummaries,
    coachReviewSummaryFailures:
        coachReviewSummaryFailures ?? this.coachReviewSummaryFailures,
    coachReviews: coachReviews ?? this.coachReviews,
    coachReviewsFailures: coachReviewsFailures ?? this.coachReviewsFailures,
    coachLessonMenus: coachLessonMenus ?? this.coachLessonMenus,
    coachLessonMenusFailures:
        coachLessonMenusFailures ?? this.coachLessonMenusFailures,
    bookedSlot: bookedSlot ?? this.bookedSlot,
    booking: booking ?? this.booking,
    session: session ?? this.session,
    video: video ?? this.video,
    review: review ?? this.review,
    lesson: lesson ?? this.lesson,
    homeFailures: homeFailures ?? this.homeFailures,
    settingsReturnStep: settingsReturnStep ?? this.settingsReturnStep,
    notificationSettings: notificationSettings ?? this.notificationSettings,
    profileReturnStep: profileReturnStep ?? this.profileReturnStep,
    bookingsReturnStep: bookingsReturnStep ?? this.bookingsReturnStep,
    // 外から書き換えて state = を通さずに状態を変えられないよう、読み取り専用で持つ（const のコンストラクタでは包めないので copyWith で包む）
    myBookings: myBookings != null
        ? List.unmodifiable(myBookings)
        : this.myBookings,
    videosReturnStep: videosReturnStep ?? this.videosReturnStep,
    myVideos: myVideos ?? this.myVideos,
    myVideoReview: myVideoReview ?? this.myVideoReview,
    videoReviewReturnStep: videoReviewReturnStep ?? this.videoReviewReturnStep,
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

/// 画面の操作。チャットと評価と契約・受講権のほかの API を呼ぶのはここだけで、画面は state を表示してこのメソッドを呼ぶ
/// （チャットの操作は [ChatNotifier]、完了したレッスンの評価の操作は [LessonReviewNotifier]、月額契約と受講権の操作は [SubscriptionNotifier]。
/// 段の移動と busy・失敗の文言はここの [moveTo] と [run] を使う）。
class StudentFlowNotifier extends Notifier<StudentFlowState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  StudentFlowNotifier([StudentFlowState? initial])
    : _initial = initial ?? StudentFlowState();

  final StudentFlowState _initial;

  @override
  StudentFlowState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  /// `registerAccount`。登録したらホームへ（ホームの欄を読んでおく）。
  Future<void> register({
    required String email,
    required String displayName,
    required Sport sport,
  }) => run(() async {
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
      step: FlowStep.home,
    );
    await _loadHome();
  });

  /// ホームへ戻る（欄を読み直す）
  Future<void> goHome() {
    state = state.copyWith(step: FlowStep.home);
    return run(_loadHome);
  }

  /// ホームの欄を読み直す
  Future<void> refreshHome() => run(_loadHome);

  /// ホームの欄（自分の予約・受講権（[SubscriptionNotifier.loadEntitlement]）・スレッド（[ChatNotifier.loadThreads]）・自分の動画）と、
  /// アプリバーのお知らせのしるし（[NotificationsNotifier.loadForHome]。読めなければしるしを出さないだけ）を
  /// 並べて読む。どれかが失敗しても他の欄は出す（失敗した欄を [StudentFlowState.homeFailures] に入れるだけで、失敗の文言は出さない）。
  /// 受講権の 404 は失敗ではなく、月額契約の前（[SubscriptionState.noEntitlement]）
  Future<void> _loadHome() async {
    final failures = <HomeSection>{};
    Future<void> load(HomeSection section, Future<void> Function() read) async {
      try {
        await read();
      } on StudentApiException {
        failures.add(section);
      }
    }

    await Future.wait([
      load(HomeSection.bookings, _loadMyBookings),
      load(
        HomeSection.entitlement,
        ref.read(subscriptionProvider.notifier).loadEntitlement,
      ),
      load(HomeSection.threads, ref.read(chatProvider.notifier).loadThreads),
      load(HomeSection.videos, _loadMyVideos),
      ref.read(notificationsProvider.notifier).loadForHome(),
    ]);
    if (!ref.mounted) return;
    state = state.copyWith(homeFailures: Set.unmodifiable(failures));
  }

  /// これからの予約（キャンセル済みでなく、まだ始まっていないもの）を開始の近い順に [limit] 件まで
  List<MyBooking> upcomingBookings({int limit = 3}) {
    final upcoming = [
      for (final item in state.myBookings ?? const <MyBooking>[])
        if (canCancel(item.booking)) item,
    ]..sort((a, b) => a.booking.startsAt.compareTo(b.booking.startsAt));
    return upcoming.take(limit).toList();
  }

  /// ホームに出す返ってきた添削: 返却済みの動画を、返却の新しい順に [limit] 件まで
  List<MyVideo> returnedVideos({int limit = 3}) {
    DateTime returnedAt(MyVideo video) => video.returnedAt ?? video.submittedAt;
    final returned = [
      for (final video in state.myVideos ?? const <MyVideo>[])
        if (video.reviewStatus == VideoReviewStatus.returned) video,
    ]..sort((a, b) => returnedAt(b).compareTo(returnedAt(a)));
    return returned.take(limit).toList();
  }

  /// スロット検索へ（生徒のスポーツで絞って検索しておく）。[coach] を渡せばそのコーチの枠に絞り、
  /// スポーツはコーチを探したスポーツ（[CoachListing.sport]）にする。渡さなければコーチの絞り込みを外す
  Future<void> goToSlotSearch({CoachListing? coach}) {
    state = state.copyWith(
      step: FlowStep.slotSearch,
      searchCoach: coach,
      clearSearchCoach: coach == null,
    );
    return run(
      () => _search(coach?.sport ?? state.searchSport ?? state.student!.sport),
    );
  }

  /// `searchSlots`。スポーツで絞る（コーチで絞っていればそのまま絞る）。
  Future<void> searchSlots(Sport sport) => run(() => _search(sport));

  /// コーチの絞り込みを外して、今のスポーツで探し直す
  Future<void> clearSearchCoach() {
    state = state.copyWith(clearSearchCoach: true);
    return run(() => _search(state.searchSport ?? state.student!.sport));
  }

  /// スロットを探し、出てきたコーチの評価の件数と平均（`getCoachReviewSummary`）を読む。
  /// 評価を読めなくても検索結果は出す（そのコーチを [StudentFlowState.coachReviewSummaryFailures] に入れるだけで、失敗の文言は出さない）
  Future<void> _search(Sport sport) async {
    final page = await _api.searchSlots(
      sport: sport,
      coachId: state.searchCoach?.coachId,
    );
    state = state.copyWith(
      searchSport: sport,
      slots: page.items,
      coachReviewSummaries: const {},
      coachReviewSummaryFailures: const {},
      coachReviews: const {},
      coachReviewsFailures: const {},
      coachLessonMenus: const {},
      coachLessonMenusFailures: const {},
    );
    final coachIds = {for (final slot in page.items) slot.coachId};
    final summaries = <String, CoachReviewSummary>{};
    final failures = <String>{};
    Future<void> load(String coachId) async {
      try {
        summaries[coachId] = await _api.getCoachReviewSummary(coachId);
      } on StudentApiException {
        failures.add(coachId);
      }
    }

    await Future.wait(coachIds.map(load));
    if (!ref.mounted) return;
    state = state.copyWith(
      coachReviewSummaries: Map.unmodifiable(summaries),
      coachReviewSummaryFailures: Set.unmodifiable(failures),
    );
  }

  /// コーチの評価の一覧（`listCoachReviews`）を読む。検索結果の操作を止めないよう、busy にせず失敗の文言も出さない
  /// （読めなければそのコーチを [StudentFlowState.coachReviewsFailures] に入れ、評価の欄にだけその旨を出す）。
  Future<void> loadCoachReviews(String coachId) async {
    state = state.copyWith(
      errorMessage: state.errorMessage,
      coachReviewsFailures: Set.unmodifiable(
        state.coachReviewsFailures.difference({coachId}),
      ),
    );
    try {
      final list = await _api.listCoachReviews(coachId);
      if (!ref.mounted) return;
      state = state.copyWith(
        errorMessage: state.errorMessage,
        coachReviews: Map.unmodifiable({
          ...state.coachReviews,
          coachId: list.reviews,
        }),
      );
    } on StudentApiException {
      if (!ref.mounted) return;
      state = state.copyWith(
        errorMessage: state.errorMessage,
        coachReviewsFailures: Set.unmodifiable({
          ...state.coachReviewsFailures,
          coachId,
        }),
      );
    }
  }

  /// コーチの公開中のレッスンメニュー（`listCoachLessonMenus`）を、生徒のスポーツで絞って読む。[loadCoachReviews] と同じく
  /// busy にせず失敗の文言も出さない（読めなければそのコーチを [StudentFlowState.coachLessonMenusFailures] に入れ、
  /// メニューの欄にだけその旨を出す）。
  Future<void> loadCoachLessonMenus(String coachId) async {
    state = state.copyWith(
      errorMessage: state.errorMessage,
      coachLessonMenusFailures: Set.unmodifiable(
        state.coachLessonMenusFailures.difference({coachId}),
      ),
    );
    try {
      final list = await _api.listCoachLessonMenus(
        coachId,
        sport: state.student!.sport,
      );
      if (!ref.mounted) return;
      state = state.copyWith(
        errorMessage: state.errorMessage,
        coachLessonMenus: Map.unmodifiable({
          ...state.coachLessonMenus,
          coachId: list.lessonMenus,
        }),
      );
    } on StudentApiException {
      if (!ref.mounted) return;
      state = state.copyWith(
        errorMessage: state.errorMessage,
        coachLessonMenusFailures: Set.unmodifiable({
          ...state.coachLessonMenusFailures,
          coachId,
        }),
      );
    }
  }

  /// `createBooking`（体験予約）。作成直後は pending なので確定待ちへ。
  Future<void> bookTrial(Slot slot) => run(() async {
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
  Future<void> confirmBooking() => run(() async {
    final booking = await _api.confirmBooking(state.booking!.bookingId);
    state = state.copyWith(booking: booking);
  });

  /// 初回レッスンを通話で受ける
  void chooseLiveLesson() => state = state.copyWith(step: FlowStep.liveLesson);

  /// 初回レッスンを動画の送付で受ける
  void chooseVideo() => state = state.copyWith(step: FlowStep.videoSubmit);

  /// `startLiveLessonSession`（映像はモックなので接続はしない）
  Future<void> startCall() => run(() async {
    final session = await _api.startLiveLessonSession(
      StartLiveLessonSessionRequest(bookingId: state.booking!.bookingId),
    );
    state = state.copyWith(session: session);
  });

  /// `endLiveLessonSession`。終了したら完了記録へ。
  Future<void> endCall() => run(() async {
    final session = await _api.endLiveLessonSession(state.session!.sessionId);
    state = state.copyWith(session: session, step: FlowStep.lessonCompletion);
  });

  /// `submitVideo`。送り先は体験予約のコーチで、スポーツはそのスロットのもの。動画本体のアップロードは Gen1 ではモックなので行わない。
  /// [lessonMenuId] は体験予約のコーチの非同期のメニューで、省けばメニューなし。メニューを付けて 422 が返ったときの扱いは
  /// [ChatNotifier.requestVideoReview] と同じ（失敗の文言は出さずにメニューを読み直し、この段に留まる）。
  Future<VideoReviewRequestResult> submitVideo({
    required VideoContentType contentType,
    String? note,
    String? lessonMenuId,
  }) async {
    var result = VideoReviewRequestResult.failed;
    await run(() async {
      final booking = state.booking!;
      final video = await submitVideoForReview(
        SubmitVideoRequest(
          coachId: booking.coachId,
          sport: state.bookedSlot!.sport,
          contentType: contentType,
          note: note,
          lessonMenuId: lessonMenuId,
        ),
      );
      if (video == null) {
        result = VideoReviewRequestResult.menuRejected;
        return;
      }
      result = VideoReviewRequestResult.sent;
      state = state.copyWith(video: video, step: FlowStep.videoReview);
      final review = await _api.getVideoReview(video.videoId);
      state = state.copyWith(review: review);
    });
    return result;
  }

  /// 添削の依頼の `submitVideo`（一周シナリオの [submitVideo] とチャットの [ChatNotifier.requestVideoReview]）。
  /// 選択肢はコーチ・公開中・非同期・動画のスポーツで絞ってあるので、メニューを付けて 422 が
  /// 返ったら読んだ後にメニューが変わった（非公開にした・直した）ものと読み、送り先のコーチのメニューを読み直して null を返す。
  /// busy にはしないので、呼ぶ側の [run] の中で呼ぶ。
  Future<Video?> submitVideoForReview(SubmitVideoRequest request) async {
    try {
      return await _api.submitVideo(request);
    } on StudentApiException catch (e) {
      if (request.lessonMenuId == null ||
          e.error?.code != ErrorCode.validationFailed) {
        rethrow;
      }
      await loadCoachLessonMenus(request.coachId);
      return null;
    }
  }

  /// `getVideoReview`（コーチが返却したかを確かめ直す）
  Future<void> refreshReview() => run(() async {
    final review = await _api.getVideoReview(state.video!.videoId);
    state = state.copyWith(review: review);
  });

  /// 添削が返されたら完了記録へ
  void goToCompletion() =>
      state = state.copyWith(step: FlowStep.lessonCompletion);

  /// `recordLessonCompletion`。通話を受けたなら live_lesson、動画を送ったなら async_lesson。
  Future<void> recordCompletion() => run(() async {
    final session = state.session;
    final lesson = await _api.recordLessonCompletion(
      session != null
          ? LiveLessonCompletion(sessionId: session.sessionId)
          : AsyncLessonCompletion(videoId: state.video!.videoId),
    );
    state = state.copyWith(lesson: lesson);
  });

  /// 完了したレッスンを評価する画面へ（評価の操作は [LessonReviewNotifier]）
  void goToReview() => state = state.copyWith(step: FlowStep.lessonReview);

  /// 月額契約の画面へ（契約と受講権の操作は [SubscriptionNotifier]）
  void goToSubscription() =>
      state = state.copyWith(step: FlowStep.subscription);

  /// 通知の受け取り設定を開く（`getMyNotificationSettings`）。閉じたら今の段に戻る。
  Future<void> openNotificationSettings() {
    if (state.step != FlowStep.notificationSettings) {
      state = state.copyWith(
        settingsReturnStep: state.step,
        step: FlowStep.notificationSettings,
      );
    }
    return run(_loadNotificationSettings);
  }

  /// 通知の受け取り設定を読み直す（読み込みに失敗したとき）
  Future<void> refreshNotificationSettings() => run(_loadNotificationSettings);

  /// 通知の受け取り設定を閉じて、開いた段に戻る
  void closeNotificationSettings() => state = state.copyWith(
    step: state.settingsReturnStep ?? FlowStep.slotSearch,
  );

  /// `updateMyNotificationSettings`。[kind] の種類だけを変えて保存する。画面はすぐに切り替え、
  /// 保存に失敗したら切り替える前に戻して失敗を知らせる（成功したらサーバーが返した設定にそろえる）。
  Future<void> setNotification(NotificationKind kind, bool receives) =>
      run(() async {
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

  /// 自分のプロフィールを開く（`getMyStudentProfile`）。閉じたら今の段に戻る。
  /// 読めなくても登録で受け取った表示名はそのまま出す
  Future<void> openProfile() {
    if (state.step != FlowStep.profile) {
      state = state.copyWith(
        profileReturnStep: state.step,
        step: FlowStep.profile,
      );
    }
    return run(_loadProfile);
  }

  /// プロフィールを閉じて、開いた段に戻る
  void closeProfile() =>
      state = state.copyWith(step: state.profileReturnStep ?? FlowStep.home);

  /// `updateMyStudentProfile`。表示名を [displayName] に変え、応答の生徒で [StudentFlowState.student] を置き換える
  /// （ホームなどの表示もこれで変わる）。保存できたら true。422 はサーバーの文言を出す（[profileErrorMessageOf]）
  Future<bool> saveDisplayName(String displayName) async {
    var saved = false;
    await run(() async {
      final student = await _api.updateMyStudentProfile(
        UpdateStudentProfileRequest(displayName: displayName),
      );
      state = state.copyWith(student: student);
      saved = true;
    }, messageOf: profileErrorMessageOf);
    return saved;
  }

  Future<void> _loadProfile() async {
    final student = await _api.getMyStudentProfile();
    state = state.copyWith(student: student);
  }

  /// 自分の予約の一覧を開く（`listMyBookings`）。閉じたら今の段に戻る。
  Future<void> openMyBookings() {
    if (state.step != FlowStep.myBookings) {
      state = state.copyWith(
        bookingsReturnStep: state.step,
        step: FlowStep.myBookings,
      );
    }
    return run(_loadMyBookings);
  }

  /// 自分の予約の一覧を読み直す
  Future<void> refreshMyBookings() => run(_loadMyBookings);

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
      ref.read(subscriptionProvider).entitlement?.contractType ==
          ContractType.subscription &&
      !_now().add(_quotaConsumingNotice).isBefore(booking.startsAt);

  static const _quotaConsumingNotice = Duration(hours: 24);

  /// `cancelBooking`。キャンセルできたら一覧のその予約を応答で置き換える。
  Future<void> cancelBooking(Booking booking) => run(() async {
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

  /// 自分の動画の一覧を開く（`listMyVideos`）。閉じたら今の段に戻る。
  Future<void> openMyVideos() {
    if (state.step != FlowStep.myVideos) {
      state = state.copyWith(
        videosReturnStep: state.step,
        step: FlowStep.myVideos,
      );
    }
    return run(_loadMyVideos);
  }

  /// 自分の動画の一覧を読み直す
  Future<void> refreshMyVideos() => run(_loadMyVideos);

  /// 自分の動画の一覧を閉じて、開いた段に戻る
  void closeMyVideos() =>
      state = state.copyWith(step: state.videosReturnStep ?? FlowStep.home);

  /// 自分の動画の一覧かホームの「返ってきた添削」から、返却済みの動画の添削結果を開く（`getVideoReview`）。
  /// 読めたら添削結果の画面へ移り、読めなければ開いた画面に留まって失敗の文言を出す
  Future<void> openMyVideoReview(MyVideo video) => run(() async {
    final review = await _api.getVideoReview(video.videoId);
    state = state.copyWith(
      myVideoReview: review,
      videoReviewReturnStep: state.step,
      step: FlowStep.myVideoReview,
    );
  });

  /// 開いた添削結果を閉じて、開いた画面（自分の動画の一覧かホーム）に戻る
  void closeMyVideoReview() => state = state.copyWith(
    step: state.videoReviewReturnStep ?? FlowStep.myVideos,
  );

  Future<void> _loadMyVideos() async {
    final list = await _api.listMyVideos();
    state = state.copyWith(myVideos: list.videos);
  }

  DateTime _now() => ref.read(clockProvider)();

  /// 段を [step] に移す（直前の失敗の文言は消える）。チャット（[ChatNotifier]）が段を移すのに使う
  void moveTo(FlowStep step) => state = state.copyWith(step: step);

  /// [action] を busy にして走らせ、API の失敗を [StudentFlowState.errorMessage] にする。busy の間は何もしない。
  /// busy と失敗の文言はアプリで 1 つなので、チャット（[ChatNotifier]）と評価（[LessonReviewNotifier]）と
  /// 契約（[SubscriptionNotifier]）の操作もここを通す。失敗の文言は [messageOf]（既定は [errorMessageOf]）で作る
  Future<void> run(
    Future<void> Function() action, {
    String Function(Object error) messageOf = errorMessageOf,
  }) async {
    if (state.busy) return;
    state = state.copyWith(busy: true);
    try {
      await action();
      if (ref.mounted) state = state.copyWith(busy: false);
    } on StudentApiException catch (e) {
      if (ref.mounted) {
        state = state.copyWith(busy: false, errorMessage: messageOf(e));
      }
    }
  }
}
