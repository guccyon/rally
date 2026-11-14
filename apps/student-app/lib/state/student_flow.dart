import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/error_message.dart';
import '../api/http_student_api.dart';
import '../api/models.dart';
import '../api/server_clock.dart';
import '../api/student_api.dart';
import '../session/session_store.dart';
import 'chat.dart';
import 'coach_profile.dart';
import 'coach_search.dart';
import 'favorite_coaches.dart';
import 'lesson_completions.dart';
import 'lesson_review.dart';
import 'next_actions.dart';
import 'notifications.dart';
import 'prejoin_check.dart';
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

  /// 入室前チェック（予約の確定から開始 5 分前を過ぎて開く。端末のカメラ・マイク・スピーカーを確かめ、開始時刻から入室する。
  /// 状態は [PrejoinCheckState]）
  prejoinCheck,
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

  /// 自分の動画の一覧かホームの「返ってきた添削」か受けたレッスンの動画添削の行から開いた、動画の添削結果（閉じると開いた段に戻る）
  myVideoReview,

  /// 受けたレッスン（完了したレッスンの一覧。ホームから開く。状態は [LessonCompletionsState]）。
  /// 行からはまだ評価していないレッスンの評価と、動画添削の添削結果を開く
  lessonCompletions,

  /// 受けたレッスンのまだ評価していない行から開いた、そのレッスンの評価（評価するレッスンは [LessonCompletionsState.reviewing]）
  lessonCompletionReview;

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

  /// お気に入りのコーチ（`listFavoriteCoaches`）
  favoriteCoaches,
}

/// 予約の確定待ちの画面で、予約をサーバーの記録（`listMyBookings`）から読み直した結果（ADR 0010 の決定 3）
enum BookingLookup {
  /// [StudentFlowState.booking] がサーバーの記録（作った・確定した・キャンセルした応答か、読み直したもの）
  ready,

  /// 予約の ID で開き、読んでいるところ
  loading,

  /// 読めなかった（通信・5xx）
  failed,

  /// 自分の予約に見つからない（他人の予約・消えた）
  notFound,
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
    this.slotsFailed = false,
    this.coachReviewSummaries = const {},
    this.coachReviewSummaryFailures = const {},
    this.coachReviews = const {},
    this.coachReviewsFailures = const {},
    this.coachLessonMenus = const {},
    this.coachLessonMenusFailures = const {},
    this.bookedSlot,
    this.booking,
    this.openBookingId,
    this.bookingLookup = BookingLookup.ready,
    this.session,
    this.video,
    this.review,
    this.lesson,
    this.completing,
    this.homeFailures = const {},
    this.settingsReturnStep,
    this.subscriptionFromChat = false,
    this.notificationSettings,
    this.profileReturnStep,
    this.bookingsReturnStep,
    this.myBookings,
    this.trialAvailable,
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
  final SelectedCoach? searchCoach;

  /// 検索結果（`searchSlots`）。未検索・検索中・読めなかったときは null
  final List<Slot>? slots;

  /// 直前の検索（`searchSlots`）を読めなかった（検索の欄にその旨と読み直しのボタンを出す。失敗の文言は出さない）
  final bool slotsFailed;

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

  /// 予約したスロット（動画の送付でスポーツを使う）。予約の ID で開いたときは `listMyBookings` の該当の予約のもの
  final Slot? bookedSlot;

  /// 予約の確定待ちの画面の予約（体験予約の直後は作った予約、自分の予約から開いたときはサーバーから読み直したもの）
  final Booking? booking;

  /// 予約の確定待ちの画面で開いている予約の ID（予約の ID で開いたとき。読めるまで [booking] は null）。
  /// null なら [booking] の ID
  final String? openBookingId;

  /// [booking] をサーバーの記録から読み直した結果
  final BookingLookup bookingLookup;
  final LiveLessonSession? session;
  final Video? video;
  final VideoReview? review;

  /// 完了を記録したレッスン（評価する対象。評価の状態は [LessonReviewState]）
  final Lesson? lesson;

  /// レッスンの完了の画面で記録する対象（通話セッションか動画の ID）。通話の終了・添削結果・ホームの「次にやること」から渡す
  /// （アプリの段に持っている通話や動画からは決めない。ADR 0010）
  final RecordLessonCompletionRequest? completing;

  /// ホームで読めなかった欄（その欄にだけ旨を出す）。ホームを読むたびに読み直す
  final Set<HomeSection> homeFailures;

  /// 通知の受け取り設定を閉じたときに戻る段（設定を開いた段）
  final FlowStep? settingsReturnStep;

  /// 月額契約の画面をチャット（契約の前に送れなかったときの「月額契約へ進む」）から開いたか。
  /// true なら、契約を始めたあとに開いていたスレッドへ戻る入口を出す（[ChatNotifier.returnFromSubscription]）
  final bool subscriptionFromChat;

  /// 通知の受け取り設定（`getMyNotificationSettings`。未取得なら null）。
  /// 切り替えたときは保存を待たずに変え、保存に失敗したら元に戻す
  final NotificationSettings? notificationSettings;

  /// プロフィールを閉じたときに戻る段（プロフィールを開いた段）
  final FlowStep? profileReturnStep;

  /// 自分の予約の一覧を閉じたときに戻る段（一覧を開いた段）
  final FlowStep? bookingsReturnStep;

  /// 自分の予約（`listMyBookings`。未取得なら null）。キャンセルしたらその予約を応答で置き換える
  final List<MyBooking>? myBookings;

  /// 生徒が今体験予約できるか（`listMyBookings` の `trial_available`。判定はサーバーで、`createBooking` の
  /// 409 `trial_already_used` と同じ条件の否定）。応答に載っていなければ false（契約どおり体験予約を出さない）。
  /// 自分の予約をまだ読めていなければ null
  final bool? trialAvailable;

  /// スロットを体験予約として予約するか。サーバーの判定 [trialAvailable] だけで決め、予約や完了したレッスンから数え直さない。
  /// まだ読めていない（null）ときは、登録したばかりの生徒の体験予約を止めないよう true
  bool get canBookTrial => trialAvailable ?? true;

  /// 自分の動画の一覧を閉じたときに戻る段（一覧を開いた段）
  final FlowStep? videosReturnStep;

  /// 自分が送った動画（`listMyVideos`。未取得なら null。送った時刻の新しい順）。
  /// コンストラクタでも copyWith でも、渡したリストの読み取り専用の写しを持つ（Riverpod を通さずに状態を変えさせない）
  final List<MyVideo>? myVideos;

  /// 自分の動画の一覧から開いた動画の添削（`getVideoReview`）。一周シナリオの [review] とは別に持つ
  final VideoReview? myVideoReview;

  /// [myVideoReview] の画面を閉じたときに戻る段（自分の動画の一覧・ホーム・受けたレッスン）
  final FlowStep? videoReviewReturnStep;

  /// `errorMessage` は引き継がない（渡さなければ消える）。[searchCoach] は [clearSearchCoach]、[slots] は [clearSlots]、
  /// [lesson] は [clearLesson]、[booking] は [clearBooking] で外す。
  StudentFlowState copyWith({
    FlowStep? step,
    bool? busy,
    String? errorMessage,
    String? accessToken,
    Student? student,
    Sport? searchSport,
    SelectedCoach? searchCoach,
    bool clearSearchCoach = false,
    List<Slot>? slots,
    bool clearSlots = false,
    bool? slotsFailed,
    Map<String, CoachReviewSummary>? coachReviewSummaries,
    Set<String>? coachReviewSummaryFailures,
    Map<String, List<CoachReview>>? coachReviews,
    Set<String>? coachReviewsFailures,
    Map<String, List<LessonMenu>>? coachLessonMenus,
    Set<String>? coachLessonMenusFailures,
    Slot? bookedSlot,
    Booking? booking,
    bool clearBooking = false,
    String? openBookingId,
    BookingLookup? bookingLookup,
    LiveLessonSession? session,
    Video? video,
    VideoReview? review,
    Lesson? lesson,
    bool clearLesson = false,
    RecordLessonCompletionRequest? completing,
    Set<HomeSection>? homeFailures,
    FlowStep? settingsReturnStep,
    bool? subscriptionFromChat,
    NotificationSettings? notificationSettings,
    FlowStep? profileReturnStep,
    FlowStep? bookingsReturnStep,
    List<MyBooking>? myBookings,
    bool? trialAvailable,
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
    slots: clearSlots ? null : slots ?? this.slots,
    slotsFailed: slotsFailed ?? this.slotsFailed,
    coachReviewSummaries: coachReviewSummaries ?? this.coachReviewSummaries,
    coachReviewSummaryFailures:
        coachReviewSummaryFailures ?? this.coachReviewSummaryFailures,
    coachReviews: coachReviews ?? this.coachReviews,
    coachReviewsFailures: coachReviewsFailures ?? this.coachReviewsFailures,
    coachLessonMenus: coachLessonMenus ?? this.coachLessonMenus,
    coachLessonMenusFailures:
        coachLessonMenusFailures ?? this.coachLessonMenusFailures,
    bookedSlot: bookedSlot ?? this.bookedSlot,
    booking: clearBooking ? null : booking ?? this.booking,
    openBookingId: openBookingId ?? this.openBookingId,
    bookingLookup: bookingLookup ?? this.bookingLookup,
    session: session ?? this.session,
    video: video ?? this.video,
    review: review ?? this.review,
    lesson: clearLesson ? null : lesson ?? this.lesson,
    completing: completing ?? this.completing,
    homeFailures: homeFailures ?? this.homeFailures,
    settingsReturnStep: settingsReturnStep ?? this.settingsReturnStep,
    subscriptionFromChat: subscriptionFromChat ?? this.subscriptionFromChat,
    notificationSettings: notificationSettings ?? this.notificationSettings,
    profileReturnStep: profileReturnStep ?? this.profileReturnStep,
    bookingsReturnStep: bookingsReturnStep ?? this.bookingsReturnStep,
    // 外から書き換えて state = を通さずに状態を変えられないよう、読み取り専用で持つ（const のコンストラクタでは包めないので copyWith で包む）
    myBookings: myBookings != null
        ? List.unmodifiable(myBookings)
        : this.myBookings,
    trialAvailable: trialAvailable ?? this.trialAvailable,
    videosReturnStep: videosReturnStep ?? this.videosReturnStep,
    myVideos: myVideos ?? this.myVideos,
    myVideoReview: myVideoReview ?? this.myVideoReview,
    videoReviewReturnStep: videoReviewReturnStep ?? this.videoReviewReturnStep,
  );
}

/// 接続先。`flutter run --dart-define=RALLY_API_URL=...` で変える（既定はバックエンドの既定の待ち受け）。
/// `/api` のような相対の値は、開いたページのオリジンからの相対として解く（Web 版の出荷用イメージは
/// 同じオリジンの `/api/*` をバックエンドへ中継する。`Dockerfile`）。
const _apiBaseUrl = String.fromEnvironment(
  'RALLY_API_URL',
  defaultValue: 'http://127.0.0.1:8080',
);

/// 画面が使う API。テストではモックに差し替える。
final studentApiProvider = Provider<StudentApi>(
  (ref) => HttpStudentApi(
    baseUrl: Uri.base.resolve(_apiBaseUrl),
    accessToken: () => ref.read(studentFlowProvider).accessToken,
  ),
);

/// サーバーに合わせた時計（端末の時計と、応答の Date ヘッダーから出したずれ）。ずれはここ 1 か所で持ち、
/// 自分の予約の一覧（`listMyBookings`）を読んだときと、入室（`startLiveLessonSession`）が 409 `slot_not_started` だったときに合わせ直す。
/// テストでは端末の時計を差し替えた [ServerClock] を渡す。
final serverClockProvider = NotifierProvider<ServerClockNotifier, ServerClock>(
  ServerClockNotifier.new,
);

class ServerClockNotifier extends Notifier<ServerClock> {
  ServerClockNotifier([this._initial]);

  final ServerClock? _initial;

  @override
  ServerClock build() => _initial ?? const ServerClock(DateTime.now);

  /// ずれを応答の Date から出した [offset] に合わせ直す。null（Date がない・読めない）なら今のまま
  void adjust(Duration? offset) => state = state.adjustedTo(offset);
}

/// 今の時刻（サーバーに合わせた時計 [serverClockProvider]）。予約が始まったか・入室前チェックを開けるか・入室できるか・
/// キャンセルで回数を使うかの表示に使う。テストでは固定の時刻に差し替える。
final clockProvider = Provider<DateTime Function()>(
  (ref) => ref.watch(serverClockProvider).now,
);

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
    _rememberSession();
    await _loadHome();
  });

  /// 起動したときに、端末に残した登録の結果（[sessionStoreProvider]）があれば同じ生徒でホームから始める。
  /// 残していなければ登録の画面のまま。段・予約・通話セッションは残していないので、ホームの欄（と次にやること）は
  /// サーバーから読む（ADR 0010）。最初に `getMyStudentProfile` でトークンを確かめて生徒を読み直し、401 なら残したものを消して
  /// 登録の画面にし、「ログインの有効期限が切れました。登録からやり直してください。」を出す。通信の失敗などそれ以外は、残した生徒のままホームの欄を読む（読めない欄はホームが出す）
  Future<void> resumeSavedSession() async {
    final saved = ref.read(sessionStoreProvider).read();
    if (saved == null) return;
    state = state.copyWith(
      accessToken: saved.accessToken,
      student: saved.student,
      step: FlowStep.home,
    );
    await run(() async {
      try {
        final student = await _api.getMyStudentProfile();
        if (!ref.mounted) return;
        state = state.copyWith(student: student);
        _rememberSession();
      } on StudentApiException catch (e) {
        if (e.statusCode == 401) {
          ref.read(sessionStoreProvider).clear();
          // 投げ直して、run が busy を外し、登録の画面に理由の文言（[errorMessageOf] の unauthorized）を出す。
          // 文言がないと、登録が消えたと思って新しいアカウントを作り直すおそれがある
          state = StudentFlowState(busy: true);
          rethrow;
        }
      }
      await _loadHome();
    });
  }

  /// 今のトークンと生徒を端末に残す（開き直したときに [resumeSavedSession] が読む）。段などほかの状態は残さない
  void _rememberSession() {
    final accessToken = state.accessToken;
    final student = state.student;
    if (accessToken == null || student == null) return;
    ref
        .read(sessionStoreProvider)
        .save(SavedSession(accessToken: accessToken, student: student));
  }

  /// ホームへ戻る（欄を読み直す）
  Future<void> goHome() {
    state = state.copyWith(step: FlowStep.home);
    return run(_loadHome);
  }

  /// ホームの欄を読み直す
  Future<void> refreshHome() => run(_loadHome);

  /// ホームの欄（自分の予約・受講権（[SubscriptionNotifier.loadEntitlement]）・スレッド（[ChatNotifier.loadThreads]）・自分の動画・
  /// お気に入りのコーチ（[FavoriteCoachesNotifier.loadForHome]））と、
  /// アプリバーのお知らせのしるし（[NotificationsNotifier.loadForHome]。読めなければしるしを出さないだけ）と、
  /// 「次にやること」（[NextActionsNotifier.load]。読めなければその欄にだけ旨を出す）と、
  /// 「次のレッスンを予約する」の入口のための受けたレッスン（[LessonCompletionsNotifier.loadForHome]。読めなければ入口を出さないだけ）を
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
      load(
        HomeSection.favoriteCoaches,
        ref.read(favoriteCoachesProvider.notifier).loadForHome,
      ),
      ref.read(notificationsProvider.notifier).loadForHome(),
      ref.read(lessonCompletionsProvider.notifier).loadForHome(),
      ref.read(nextActionsProvider.notifier).load(),
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
  /// スポーツはコーチの紹介を開いたスポーツ（[SelectedCoach.sport]）にする。渡さなければコーチの絞り込みを外す
  Future<void> goToSlotSearch({SelectedCoach? coach}) {
    state = state.copyWith(
      step: FlowStep.slotSearch,
      searchCoach: coach,
      clearSearchCoach: coach == null,
    );
    return run(
      () => Future.wait([
        _search(coach?.sport ?? state.searchSport ?? state.student!.sport),
        _refreshMyBookingsQuietly(),
      ]),
    );
  }

  /// 予約のボタンを体験予約にするか（[StudentFlowState.canBookTrial]）を今のサーバーの記録で決めるため、自分の予約を読み直す。
  /// 読めなければ前に読んだ予約のまま（失敗の文言は出さない）
  Future<void> _refreshMyBookingsQuietly() async {
    try {
      await _loadMyBookings();
    } on StudentApiException {
      return;
    }
  }

  /// `searchSlots`。スポーツで絞る（コーチで絞っていればそのまま絞る）。
  Future<void> searchSlots(Sport sport) => run(() => _search(sport));

  /// コーチの絞り込みを外して、今のスポーツで探し直す
  Future<void> clearSearchCoach() {
    state = state.copyWith(clearSearchCoach: true);
    return run(() => _search(state.searchSport ?? state.student!.sport));
  }

  /// 読めなかった検索を、同じスポーツとコーチの絞り込みで探し直す
  Future<void> retrySlotSearch() =>
      run(() => _search(state.searchSport ?? state.student!.sport));

  /// スロットを探し、出てきたコーチの評価の件数と平均（`getCoachReviewSummary`）を読む。
  /// 探している間は前の検索結果を外す（別のスポーツ・コーチの枠を出したままにしない）。検索を読めなければ
  /// [StudentFlowState.slotsFailed] にするだけで、失敗の文言は出さない。
  /// 評価を読めなくても検索結果は出す（そのコーチを [StudentFlowState.coachReviewSummaryFailures] に入れるだけで、失敗の文言は出さない）
  Future<void> _search(Sport sport) async {
    state = state.copyWith(
      searchSport: sport,
      clearSlots: true,
      slotsFailed: false,
    );
    final SlotPage page;
    try {
      page = await _api.searchSlots(
        sport: sport,
        coachId: state.searchCoach?.coachId,
      );
    } on StudentApiException {
      if (ref.mounted) state = state.copyWith(slotsFailed: true);
      return;
    }
    if (!ref.mounted) return;
    state = state.copyWith(
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

  /// `createBooking`。体験予約をまだ使っていなければ体験予約（[StudentFlowState.canBookTrial]）、使っていれば通常の予約。
  /// 作成直後は pending なので確定待ちへ。409 `trial_already_used`（読んだ予約が古かった）なら、失敗の文言を出したうえで
  /// 自分の予約を読み直し、ボタンを通常の予約に切り替える
  Future<void> bookSlot(Slot slot) => run(() async {
    final Booking booking;
    try {
      booking = await _api.createBooking(
        CreateBookingRequest(slotId: slot.slotId, isTrial: state.canBookTrial),
      );
    } on StudentApiException catch (e) {
      if (e.error?.code == ErrorCode.trialAlreadyUsed) {
        await _refreshMyBookingsQuietly();
      }
      rethrow;
    }
    state = state.copyWith(
      bookedSlot: slot,
      booking: booking,
      openBookingId: booking.bookingId,
      bookingLookup: BookingLookup.ready,
      step: FlowStep.bookingPending,
    );
  });

  /// 予約の確定待ちの画面を予約の ID で開き、`listMyBookings` からその予約を読み直す（自分の予約の「内容を確かめて確定する」。
  /// ADR 0010 の決定 3）。確定は呼ばない（中身を見せた画面で押す）。読めるまで別の予約の中身は出さない
  Future<void> openBooking(String bookingId) {
    if (state.busy) return Future.value();
    state = state.copyWith(
      step: FlowStep.bookingPending,
      clearBooking: state.booking?.bookingId != bookingId,
      openBookingId: bookingId,
      bookingLookup: BookingLookup.loading,
    );
    return run(() => _reloadBooking(bookingId));
  }

  /// 予約の確定待ちの画面の予約を、サーバーの記録から読み直す（脇の画面でキャンセル・確定していても合うように）
  Future<void> refreshBooking() {
    final bookingId = state.openBookingId ?? state.booking?.bookingId;
    if (bookingId == null) return Future.value();
    return run(() => _reloadBooking(bookingId));
  }

  /// `listMyBookings` を読み、[bookingId] の予約で [StudentFlowState.booking] と予約したスロットを置き換える。
  /// 読めなければ [BookingLookup.failed]、見つからなければ [BookingLookup.notFound] にする（失敗の文言は出さず、画面にその旨を出す）
  Future<void> _reloadBooking(String bookingId) async {
    try {
      await _loadMyBookings();
    } on StudentApiException {
      if (ref.mounted) {
        state = state.copyWith(bookingLookup: BookingLookup.failed);
      }
      return;
    }
    if (!ref.mounted) return;
    final item = (state.myBookings ?? const <MyBooking>[])
        .where((item) => item.booking.bookingId == bookingId)
        .firstOrNull;
    state = item == null
        ? state.copyWith(bookingLookup: BookingLookup.notFound)
        : state.copyWith(
            booking: item.booking,
            bookedSlot: item.slot,
            bookingLookup: BookingLookup.ready,
          );
  }

  /// `confirmBooking`。409 `invalid_booking_state`（ほかの画面や端末でキャンセル・確定していた）なら、失敗の文言は出さずに
  /// 予約を読み直し、画面をサーバーの今の状態にする
  Future<void> confirmBooking() => run(() async {
    final bookingId = state.booking!.bookingId;
    final Booking booking;
    try {
      booking = await _api.confirmBooking(bookingId);
    } on StudentApiException catch (e) {
      if (e.error?.code != ErrorCode.invalidBookingState) rethrow;
      await _reloadBooking(bookingId);
      return;
    }
    state = state.copyWith(booking: booking);
  });

  /// 予約の確定待ちの画面の「この予約をキャンセルする」（`cancelBooking`）。キャンセルできたらホームへ戻って true を返す。
  /// 409 `invalid_booking_state` なら、サーバーの文言を出したうえで予約を読み直す
  Future<bool> cancelOpenBooking() async {
    var cancelled = false;
    await run(() async {
      final bookingId = state.booking!.bookingId;
      final Booking booking;
      try {
        booking = await _api.cancelBooking(bookingId);
      } on StudentApiException catch (e) {
        if (e.error?.code == ErrorCode.invalidBookingState) {
          await _reloadBooking(bookingId);
        }
        rethrow;
      }
      cancelled = true;
      state = state.copyWith(booking: booking, step: FlowStep.home);
      await _loadHome();
    });
    return cancelled;
  }

  /// 予約の ID で予約を読み直してから、確定済みならビデオ通話の入室前チェックを開く（ホームの「次にやること」の「ビデオ通話で受ける」）。
  /// 開始 5 分前より前・もう確定済みでないときは、予約の画面にその予約の今の状態を出す
  Future<void> openPrejoinCheckFor(String bookingId) async {
    if (await _resumeConfirmedBooking(bookingId)) {
      ref.read(prejoinCheckProvider.notifier).open();
    }
  }

  /// 予約の ID で予約を読み直してから、確定済みなら初回レッスンを動画の送付で受ける（ホームの「次にやること」の「動画を送って添削を受ける」）
  Future<void> chooseVideoFor(String bookingId) async {
    if (await _resumeConfirmedBooking(bookingId)) chooseVideo();
  }

  /// 予約の画面を開いて予約を読み直し、確定済みなら true
  Future<bool> _resumeConfirmedBooking(String bookingId) async {
    if (state.busy) return false;
    await openBooking(bookingId);
    if (!ref.mounted) return false;
    return state.step == FlowStep.bookingPending &&
        state.bookingLookup == BookingLookup.ready &&
        state.booking?.bookingId == bookingId &&
        state.booking?.status == BookingStatus.confirmed;
  }

  /// 確定待ちの予約を確定できるか（自分の予約の「内容を確かめて確定する」を出すか）。確定待ちで、まだ始まっていない予約だけ
  bool canConfirm(Booking booking) =>
      booking.status == BookingStatus.pending && canCancel(booking);

  /// 受け方を選べるか（サーバーから読み直した予約が確定済みのときだけ。キャンセル済みの予約のレッスンには進めない。#1364）
  bool get canChooseLesson =>
      state.bookingLookup == BookingLookup.ready &&
      state.booking?.status == BookingStatus.confirmed;

  /// 初回レッスンを動画の送付で受ける（確定済みの予約だけ。キャンセルした予約からは送らない。#1364）
  void chooseVideo() {
    if (!canChooseLesson) return;
    state = state.copyWith(step: FlowStep.videoSubmit);
  }

  /// `startLiveLessonSession`（映像はモックなので接続はしない）
  Future<void> startCall() => run(() async {
    final session = await _api.startLiveLessonSession(
      StartLiveLessonSessionRequest(bookingId: state.booking!.bookingId),
    );
    state = state.copyWith(session: session);
  });

  /// 入室前チェックの「入室する」（`startLiveLessonSession`）。通ったらビデオ通話へ移って true を返す。
  /// 応答の前に入室前チェックから戻っていたら応答を捨てる（段を移さない）。失敗の文言は [StudentFlowState.errorMessage]
  /// 409 `slot_not_started`（サーバーの時刻ではまだ開始前）なら、その応答の Date で時計のずれを合わせ直す（入室は繰り返し送らない）。
  Future<bool> joinLiveLesson() async {
    var joined = false;
    await run(() async {
      final LiveLessonSession session;
      try {
        session = await _api.startLiveLessonSession(
          StartLiveLessonSessionRequest(bookingId: state.booking!.bookingId),
        );
      } on StudentApiException catch (e) {
        if (e.error?.code == ErrorCode.slotNotStarted) {
          ref.read(serverClockProvider.notifier).adjust(e.clockOffset);
        }
        rethrow;
      }
      if (!ref.mounted || state.step != FlowStep.prejoinCheck) return;
      state = state.copyWith(session: session, step: FlowStep.liveLesson);
      joined = true;
    });
    return joined;
  }

  /// `endLiveLessonSession`。終了したらその通話セッションの完了記録へ。
  Future<void> endCall() => run(() async {
    final session = await _api.endLiveLessonSession(state.session!.sessionId);
    state = state.copyWith(
      session: session,
      completing: LiveLessonCompletion(sessionId: session.sessionId),
      clearLesson: true,
      step: FlowStep.lessonCompletion,
    );
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
    if (!canChooseLesson) {
      // 送付の画面を開いたあとに自分の予約でキャンセルした（送らずに確定待ちの画面へ戻し、キャンセル済みを出す）
      state = state.copyWith(step: FlowStep.bookingPending);
      return result;
    }
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

  /// `getVideoReview`（コーチが返却したかを確かめ直す）。返却されていれば、完了の記録へ進めるかを次の操作（`getMyNextActions`）で確かめ直す
  Future<void> refreshReview() => run(() async {
    final review = await _api.getVideoReview(state.video!.videoId);
    state = state.copyWith(review: review);
    if (review.status == VideoReviewStatus.returned) {
      await ref.read(nextActionsProvider.notifier).load();
    }
  });

  /// [target]（通話セッションか動画の ID）の完了記録へ（添削結果とホームの「次にやること」の「完了の記録へ進む」）
  void goToCompletion(RecordLessonCompletionRequest target) {
    if (state.busy) return;
    state = state.copyWith(
      completing: target,
      clearLesson: true,
      step: FlowStep.lessonCompletion,
    );
  }

  /// `recordLessonCompletion`。記録するのは渡された [StudentFlowState.completing]（通話なら live_lesson、動画なら async_lesson）
  Future<void> recordCompletion() => run(() async {
    final lesson = await _api.recordLessonCompletion(state.completing!);
    state = state.copyWith(lesson: lesson);
  });

  /// 完了したレッスンを評価する画面へ（評価の操作は [LessonReviewNotifier]）
  void goToReview() => state = state.copyWith(step: FlowStep.lessonReview);

  /// 月額契約の画面へ（契約と受講権の操作は [SubscriptionNotifier]）。チャットから開くときは [fromChat] を true にする
  /// （契約を始めたあとにチャットへ戻る入口を出す。ほかの入口から開いたら出さない）
  void goToSubscription({bool fromChat = false}) => state = state.copyWith(
    step: FlowStep.subscription,
    subscriptionFromChat: fromChat,
  );

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
  void closeNotificationSettings() =>
      returnTo(state.settingsReturnStep ?? FlowStep.slotSearch);

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

  /// 自分のプロフィールを開く（`getMyStudentProfile` と、月額契約の欄の `getMyEntitlement`（[SubscriptionNotifier.loadForProfile]）を並べて読む）。
  /// 閉じたら今の段に戻る。読めなくても登録で受け取った表示名はそのまま出す（受講権を読めなくても失敗の文言は出さず、月額契約の欄にだけ旨を出す）
  Future<void> openProfile() {
    if (state.step != FlowStep.profile) {
      state = state.copyWith(
        profileReturnStep: state.step,
        step: FlowStep.profile,
      );
    }
    return run(
      () => Future.wait([
        _loadProfile(),
        ref.read(subscriptionProvider.notifier).loadForProfile(),
      ]),
    );
  }

  /// プロフィールを閉じて、開いた段に戻る
  void closeProfile() => returnTo(state.profileReturnStep ?? FlowStep.home);

  /// ログアウト（プロフィールの「ログアウトする」）。端末に残した登録の結果（[sessionStoreProvider]）を消し、
  /// 前の生徒の状態（トークン・生徒・予約・チャット・契約・お知らせなど）を捨てて登録の画面にする。
  /// サーバーには何も送らない（トークンを失効させる API はない）。次の生徒のホームはサーバーから読み直す（ADR 0010）
  void logout() {
    ref.read(sessionStoreProvider).clear();
    ref
      ..invalidate(chatProvider)
      ..invalidate(subscriptionProvider)
      ..invalidate(favoriteCoachesProvider)
      ..invalidate(nextActionsProvider)
      ..invalidate(notificationsProvider)
      ..invalidate(lessonCompletionsProvider)
      ..invalidate(coachSearchProvider)
      ..invalidate(coachProfileProvider)
      ..invalidate(lessonReviewProvider)
      ..invalidate(prejoinCheckProvider);
    state = StudentFlowState();
  }

  /// `updateMyStudentProfile`。表示名を [displayName] に変え、応答の生徒で [StudentFlowState.student] を置き換える
  /// （ホームなどの表示もこれで変わる）。保存できたら true。422 はサーバーの文言を出す（[profileErrorMessageOf]）
  Future<bool> saveDisplayName(String displayName) async {
    var saved = false;
    await run(() async {
      final student = await _api.updateMyStudentProfile(
        UpdateStudentProfileRequest(displayName: displayName),
      );
      state = state.copyWith(student: student);
      _rememberSession();
      saved = true;
    }, messageOf: profileErrorMessageOf);
    return saved;
  }

  Future<void> _loadProfile() async {
    final student = await _api.getMyStudentProfile();
    state = state.copyWith(student: student);
    _rememberSession();
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

  /// 自分の予約の一覧を閉じて、開いた段に戻る。動画の送付から開いてその予約をキャンセルしていたら、
  /// 予約の確定待ちの画面に戻す（読み直してキャンセル済みを出す。キャンセルした予約のレッスンには進めない）
  void closeMyBookings() {
    final back = state.bookingsReturnStep ?? FlowStep.slotSearch;
    final leftLesson =
        back == FlowStep.videoSubmit &&
        state.booking?.status == BookingStatus.cancelled;
    returnTo(leftLesson ? FlowStep.bookingPending : back);
  }

  /// キャンセルのボタンを出すか。キャンセル済みでなく、まだ始まっていない予約だけ
  bool canCancel(Booking booking) =>
      booking.status != BookingStatus.cancelled &&
      booking.startsAt.isAfter(_now());

  /// 今キャンセルすると今月のビデオ通話の枠を使うか（行の案内と確認の文言を添えるかの目安）。
  /// 決めるのはサーバー（scheduling のキャンセル規定）で、ここは `listMyBookings` の見込みを読むだけ。
  /// 境目 `free_cancel_until` があれば今の時刻と比べ（一覧を読んだあとで境目を過ぎても合うように。境目ちょうどからを含む）、
  /// なければ `cancel_consumes_quota` のまま（null は使わないとみなす）
  bool cancellationUsesQuota(MyBooking item) => switch (item.freeCancelUntil) {
    final until? => !_now().isBefore(until),
    null => item.cancelConsumesQuota ?? false,
  };

  /// `cancelBooking`。キャンセルできたら一覧のその予約を応答で置き換える（予約の確定待ちの画面の予約なら、それも置き換える）。
  Future<void> cancelBooking(Booking booking) => run(() async {
    final cancelled = await _api.cancelBooking(booking.bookingId);
    state = state.copyWith(
      booking: state.booking?.bookingId == cancelled.bookingId
          ? cancelled
          : null,
      myBookings: [
        for (final item in state.myBookings ?? const <MyBooking>[])
          item.booking.bookingId == cancelled.bookingId
              ? MyBooking(booking: cancelled, slot: item.slot)
              : item,
      ],
    );
  });

  /// 読んだら、応答の Date で時計のずれを合わせ直す（予約の確定と入室前チェックの時刻の判断に使う）
  Future<void> _loadMyBookings() async {
    final list = await _api.listMyBookings();
    ref.read(serverClockProvider.notifier).adjust(list.clockOffset);
    state = state.copyWith(
      myBookings: list.bookings,
      trialAvailable: list.trialAvailable ?? false,
    );
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
  void closeMyVideos() => returnTo(state.videosReturnStep ?? FlowStep.home);

  /// 自分の動画の一覧かホームの「返ってきた添削」・「次にやること」か受けたレッスンの動画添削の行から、[videoId] の動画の添削結果を開く
  /// （`getVideoReview`）。読めたら添削結果の画面へ移り、読めなければ開いた画面に留まって失敗の文言を出す。
  /// 返却されていれば、完了の記録へ進めるか（[NextActionsState.completesVideo]）を次の操作（`getMyNextActions`）で確かめ直す
  /// （アプリの段の状態では決めない。#1423・ADR 0010）
  Future<void> openMyVideoReview(String videoId) => run(() async {
    final review = await _api.getVideoReview(videoId);
    state = state.copyWith(
      myVideoReview: review,
      videoReviewReturnStep: state.step,
      step: FlowStep.myVideoReview,
    );
    if (review.status == VideoReviewStatus.returned) {
      await ref.read(nextActionsProvider.notifier).load();
    }
  });

  /// 開いた添削結果を閉じて、開いた画面（自分の動画の一覧・ホーム・受けたレッスン）に戻る
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

  /// 脇の画面（自分の予約・自分の動画・チャット・お知らせ・通知の受け取り設定・プロフィール）を閉じて、開いた段 [step] に戻る。
  /// 予約の確定待ちの画面に戻るときは、脇の画面でキャンセル・確定していても合うよう予約を読み直す（読み直すあいだはボタンを押せない）
  void returnTo(FlowStep step) {
    state = state.copyWith(step: step);
    if (step == FlowStep.bookingPending) refreshBooking();
  }

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
