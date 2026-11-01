import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'coach_search.dart';
import 'favorite_coaches.dart';
import 'student_flow.dart';

/// コーチの紹介で開いたコーチ（コーチを探す一覧の行か、ホームのお気に入りのコーチの行で選んだもの）。
/// 表示名は紹介を読む前と読めなかったときにも出し、[sport] はレッスンメニューと空き枠を絞るスポーツ
class SelectedCoach {
  const SelectedCoach({
    required this.coachId,
    required this.displayName,
    required this.sport,
  });

  /// コーチを探す一覧の行（スポーツはコーチを探したスポーツ）
  SelectedCoach.fromListing(CoachListing coach)
    : this(
        coachId: coach.coachId,
        displayName: coach.displayName,
        sport: coach.sport,
      );

  /// ホームのお気に入りのコーチの行。スポーツは、コーチが提供していれば生徒のスポーツ [studentSport]、
  /// していなければコーチの提供するスポーツの先頭
  SelectedCoach.fromFavorite(FavoriteCoach coach, {required Sport studentSport})
    : this(
        coachId: coach.coachId,
        displayName: coach.displayName,
        sport: coach.sports.contains(studentSport)
            ? studentSport
            : coach.sports.first,
      );

  final String coachId;

  /// コーチの表示名（選んだ一覧に載っていたもの）
  final String displayName;

  /// レッスンメニューと空き枠を絞るスポーツ
  final Sport sport;
}

/// コーチの紹介の欄（API ごとに読み、どれかが失敗しても他の欄は出す）
enum CoachProfileSection {
  /// 表示名と自己紹介とお気に入りか（`getCoachProfile`）
  profile,

  /// 評価の平均と件数（`getCoachReviewSummary`）
  reviewSummary,

  /// 最近の評価（`listCoachReviews`）
  reviews,

  /// 公開中のレッスンメニュー（`listCoachLessonMenus`）
  lessonMenus,
}

/// コーチの紹介の画面の状態。画面の段（[FlowStep]）と、API を呼んでいる間の印・直前の操作の失敗の文言は
/// アプリ全体で 1 つなので、[StudentFlowState] の `step` / `busy` / `errorMessage` を使う。
/// 一覧をコンストラクタで写して持つので const では作らない。
class CoachProfileState {
  CoachProfileState({
    this.coach,
    this.returnStep = FlowStep.coachSearch,
    this.profile,
    this.reviewSummary,
    List<CoachReview>? reviews,
    List<LessonMenu>? lessonMenus,
    Set<CoachProfileSection> failures = const {},
    // 渡した元のリストを後で変えても状態が変わらないよう、読み取り専用の写しを持つ
  }) : reviews = reviews == null ? null : List.unmodifiable(reviews),
       lessonMenus = lessonMenus == null ? null : List.unmodifiable(lessonMenus),
       failures = Set.unmodifiable(failures);

  /// コーチを探す一覧かホームのお気に入りで選んだコーチ（表示名は紹介を読む前と読めなかったときにも出す）。開く前は null
  final SelectedCoach? coach;

  /// アプリバーの戻るで戻る段（コーチを探す一覧から開けば [FlowStep.coachSearch]、ホームから開けば [FlowStep.home]）
  final FlowStep returnStep;

  /// コーチの紹介（`getCoachProfile`）。読む前・読めなかったときは null
  final CoachProfile? profile;

  /// 評価の件数と平均（`getCoachReviewSummary`）。読む前・読めなかったときは null
  final CoachReviewSummary? reviewSummary;

  /// 評価の一覧（`listCoachReviews`。送った時刻の新しい順）。読む前・読めなかったときは null
  final List<CoachReview>? reviews;

  /// 公開中のレッスンメニューのうち、コーチを探したスポーツのもの（`listCoachLessonMenus`。作った時刻の古い順）。
  /// 読む前・読めなかったときは null
  final List<LessonMenu>? lessonMenus;

  /// 読めなかった欄（その欄にだけ旨と読み直しのボタンを出す）
  final Set<CoachProfileSection> failures;
}

final coachProfileProvider =
    NotifierProvider<CoachProfileNotifier, CoachProfileState>(
      CoachProfileNotifier.new,
    );

/// コーチの紹介の画面の操作。`getCoachProfile` を呼ぶのはここだけ。段の移動と busy は [StudentFlowNotifier] に任せる
/// （[StudentFlowNotifier.moveTo] と [StudentFlowNotifier.run]）。
class CoachProfileNotifier extends Notifier<CoachProfileState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  CoachProfileNotifier([CoachProfileState? initial])
    : _initial = initial ?? CoachProfileState();

  final CoachProfileState _initial;

  @override
  CoachProfileState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  StudentFlowNotifier get _flow => ref.read(studentFlowProvider.notifier);

  /// 当てた欄の 1 つだけを [CoachProfileState] で置き換える（ほかの欄はそのまま）
  CoachProfileState _with({
    CoachProfile? profile,
    CoachReviewSummary? reviewSummary,
    List<CoachReview>? reviews,
    List<LessonMenu>? lessonMenus,
    required Set<CoachProfileSection> failures,
  }) => CoachProfileState(
    coach: state.coach,
    returnStep: state.returnStep,
    profile: profile ?? state.profile,
    reviewSummary: reviewSummary ?? state.reviewSummary,
    reviews: reviews ?? state.reviews,
    lessonMenus: lessonMenus ?? state.lessonMenus,
    failures: failures,
  );

  /// [coach] の紹介を開き、すべての欄を読む（開くたびに読み直す）。戻るで [returnStep] に戻る
  Future<void> open(
    SelectedCoach coach, {
    FlowStep returnStep = FlowStep.coachSearch,
  }) {
    state = CoachProfileState(coach: coach, returnStep: returnStep);
    _flow.moveTo(FlowStep.coachProfile);
    return _flow.run(() async {
      await Future.wait(CoachProfileSection.values.map(_load));
    });
  }

  /// 読めなかった欄 [section] だけを読み直す
  Future<void> reload(CoachProfileSection section) => _flow.run(() {
    state = _with(failures: state.failures.difference({section}));
    return _load(section);
  });

  /// 欄を 1 つ読む。読めなければその欄を [CoachProfileState.failures] に入れるだけで、失敗の文言は出さない
  Future<void> _load(CoachProfileSection section) async {
    final coach = state.coach!;
    try {
      switch (section) {
        case CoachProfileSection.profile:
          final profile = await _api.getCoachProfile(coach.coachId);
          if (!ref.mounted) return;
          state = _with(profile: profile, failures: state.failures);
        case CoachProfileSection.reviewSummary:
          final summary = await _api.getCoachReviewSummary(coach.coachId);
          if (!ref.mounted) return;
          state = _with(reviewSummary: summary, failures: state.failures);
        case CoachProfileSection.reviews:
          final reviews = await _api.listCoachReviews(coach.coachId);
          if (!ref.mounted) return;
          state = _with(reviews: reviews.reviews, failures: state.failures);
        case CoachProfileSection.lessonMenus:
          final menus = await _api.listCoachLessonMenus(
            coach.coachId,
            sport: coach.sport,
          );
          if (!ref.mounted) return;
          state = _with(
            lessonMenus: menus.lessonMenus,
            failures: state.failures,
          );
      }
    } on StudentApiException {
      if (!ref.mounted) return;
      state = _with(failures: {...state.failures, section});
    }
  }

  /// 開いた段に戻る。コーチの一覧は読み直さず、ホームは開くたびに欄を読み直す
  Future<void> close() async {
    if (state.returnStep == FlowStep.home) return _flow.goHome();
    _flow.moveTo(state.returnStep);
  }

  /// お気に入りを切り替える（入っていれば `removeFavoriteCoach`、いなければ `addFavoriteCoach`）。
  /// 紹介を読めるまでは切り替えない。切り替えたら紹介とコーチを探す一覧のしるしとホームのお気に入りの一覧を合わせる
  /// （読み直さない）。失敗したら状態を変えずに失敗の文言を出す
  Future<void> toggleFavorite() => _flow.run(() async {
    final profile = state.profile;
    if (profile == null) return;
    final favorite = !profile.isFavorite;
    final list = favorite
        ? await _api.addFavoriteCoach(profile.coachId)
        : await _api.removeFavoriteCoach(profile.coachId);
    if (!ref.mounted) return;
    state = _with(
      profile: profile.withFavorite(favorite),
      failures: state.failures,
    );
    ref.read(favoriteCoachesProvider.notifier).replace(list);
    ref
        .read(coachSearchProvider.notifier)
        .markFavorite(profile.coachId, favorite);
  });

  /// 「このコーチの空き枠を見る」。コーチを探したスポーツで、このコーチの予約枠に絞って探す
  Future<void> showSlots() => _flow.goToSlotSearch(coach: state.coach);
}
