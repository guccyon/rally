import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'coach_profile.dart';
import 'student_flow.dart';

/// お気に入りのコーチ（glossary の favorite_coach）の状態。ホームの「お気に入りのコーチ」の欄が読む。
/// 読めなかったことはホームの欄として [StudentFlowState.homeFailures] に持つ。
/// 一覧をコンストラクタで写して持つので const では作らない。
class FavoriteCoachesState {
  FavoriteCoachesState({List<FavoriteCoach>? favoriteCoaches})
    // 渡した元のリストを後で変えても状態が変わらないよう、読み取り専用の写しを持つ
    : favoriteCoaches = favoriteCoaches == null
          ? null
          : List.unmodifiable(favoriteCoaches);

  /// 生徒のお気に入りのコーチ（`listFavoriteCoaches`。お気に入りに入れた時刻の新しい順のまま）。読み込む前は null
  final List<FavoriteCoach>? favoriteCoaches;
}

final favoriteCoachesProvider =
    NotifierProvider<FavoriteCoachesNotifier, FavoriteCoachesState>(
      FavoriteCoachesNotifier.new,
    );

/// お気に入りのコーチの操作。`listFavoriteCoaches` を呼ぶのはここだけ（切り替えはコーチの紹介の
/// [CoachProfileNotifier.toggleFavorite] で、応答の一覧を [replace] で受け取る）。段の移動と busy は [StudentFlowNotifier] に任せる。
class FavoriteCoachesNotifier extends Notifier<FavoriteCoachesState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  FavoriteCoachesNotifier([FavoriteCoachesState? initial])
    : _initial = initial ?? FavoriteCoachesState();

  final FavoriteCoachesState _initial;

  @override
  FavoriteCoachesState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  /// ホームの欄のために一覧を読む（[StudentFlowNotifier] がホームの欄と並べて呼ぶ）。
  /// 読めなければ [StudentApiException] をそのまま投げ、前に読んだ一覧は残さない
  Future<void> loadForHome() async {
    try {
      final list = await _api.listFavoriteCoaches();
      if (!ref.mounted) return;
      state = FavoriteCoachesState(favoriteCoaches: list.favoriteCoaches);
    } on StudentApiException {
      if (ref.mounted) state = FavoriteCoachesState();
      rethrow;
    }
  }

  /// 切り替えの応答（入れた・外したあとの一覧）で置き換える
  void replace(FavoriteCoachList list) =>
      state = FavoriteCoachesState(favoriteCoaches: list.favoriteCoaches);

  /// ホームの行を押したとき。そのコーチの紹介を開き、戻るとホームに戻る。
  /// レッスンメニューと空き枠は、コーチが提供していれば生徒のスポーツで絞る
  Future<void> open(FavoriteCoach coach) {
    final student = ref.read(studentFlowProvider).student!;
    return ref
        .read(coachProfileProvider.notifier)
        .open(
          SelectedCoach.fromFavorite(coach, studentSport: student.sport),
          returnStep: FlowStep.home,
        );
  }
}
