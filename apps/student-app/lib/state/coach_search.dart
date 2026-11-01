import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'coach_profile.dart';
import 'student_flow.dart';

/// コーチを探す画面の状態。画面の段（[FlowStep]）と、API を呼んでいる間の印・直前の操作の失敗の文言は
/// アプリ全体で 1 つなので、[StudentFlowState] の `step` / `busy` / `errorMessage` を使う。
/// 一覧をコンストラクタで写して持つので const では作らない。
class CoachSearchState {
  CoachSearchState({
    List<CoachListing>? coaches,
    this.failed = false,
    // 渡した元のリストを後で変えても状態が変わらないよう、読み取り専用の写しを持つ
  }) : coaches = coaches == null ? null : List.unmodifiable(coaches);

  /// 生徒のスポーツのコーチ（`listCoachesForStudent`。サーバーの並びのまま）。読み込む前・読み込み中は null
  final List<CoachListing>? coaches;

  /// 一覧を読めなかったら true（画面にその旨と読み直しのボタンを出す）
  final bool failed;
}

final coachSearchProvider =
    NotifierProvider<CoachSearchNotifier, CoachSearchState>(
      CoachSearchNotifier.new,
    );

/// コーチを探す画面の操作。`listCoachesForStudent` を呼ぶのはここだけ。段の移動と busy は [StudentFlowNotifier] に任せる
/// （[StudentFlowNotifier.moveTo] と [StudentFlowNotifier.run]）。
class CoachSearchNotifier extends Notifier<CoachSearchState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  CoachSearchNotifier([CoachSearchState? initial])
    : _initial = initial ?? CoachSearchState();

  final CoachSearchState _initial;

  @override
  CoachSearchState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  StudentFlowNotifier get _flow => ref.read(studentFlowProvider.notifier);

  /// コーチを探す画面を開き、生徒のスポーツのコーチを読む（開くたびに読み直す）
  Future<void> open() {
    _flow.moveTo(FlowStep.coachSearch);
    return refresh();
  }

  /// 一覧を読み直す。スポーツは省き、サーバーが生徒自身のスポーツで探す。
  /// 読めなければ [CoachSearchState.failed] を立て、画面の一覧の欄にだけその旨を出す（失敗の文言は出さない）
  Future<void> refresh() => _flow.run(() async {
    state = CoachSearchState();
    try {
      final list = await _api.listCoachesForStudent();
      if (!ref.mounted) return;
      state = CoachSearchState(coaches: list.coaches);
    } on StudentApiException {
      if (!ref.mounted) return;
      state = CoachSearchState(failed: true);
    }
  });

  /// 一覧の行を押したとき。そのコーチの紹介を開く
  Future<void> select(CoachListing coach) => ref
      .read(coachProfileProvider.notifier)
      .open(SelectedCoach.fromListing(coach));

  /// コーチの紹介でお気に入りを切り替えたあと、一覧のそのコーチのしるしを [isFavorite] に合わせる（読み直さない）
  void markFavorite(String coachId, bool isFavorite) {
    final coaches = state.coaches;
    if (coaches == null) return;
    state = CoachSearchState(
      coaches: [
        for (final coach in coaches)
          coach.coachId == coachId ? coach.withFavorite(isFavorite) : coach,
      ],
    );
  }
}
