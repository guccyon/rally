import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'student_flow.dart';

/// 受けたレッスン（生徒の完了したレッスン。glossary の lesson_completion）の画面の状態。画面の段（[FlowStep]）と、
/// API を呼んでいる間の印・直前の操作の失敗の文言はアプリ全体で 1 つなので、[StudentFlowState] の `step` / `busy` /
/// `errorMessage` を使う。一覧をコンストラクタで写して持つので const では作らない。
class LessonCompletionsState {
  LessonCompletionsState({
    List<MyLessonCompletion>? completions,
    this.failed = false,
    this.reviewing,
    // 渡した元のリストを後で変えても状態が変わらないよう、読み取り専用の写しを持つ
  }) : completions = completions == null
           ? null
           : List.unmodifiable(completions);

  /// 受けたレッスン（`listMyLessonCompletions`。サーバーの並び（完了した時刻の新しい順）のまま）。
  /// 読み込む前・読み込み中・読めなかったときは null
  final List<MyLessonCompletion>? completions;

  /// 一覧を読めなかったら true（画面にその旨と読み直しのボタンを出す）
  final bool failed;

  /// 一覧の行から評価の画面（[FlowStep.lessonCompletionReview]）に進んだ、評価する完了したレッスン
  final MyLessonCompletion? reviewing;
}

final lessonCompletionsProvider =
    NotifierProvider<LessonCompletionsNotifier, LessonCompletionsState>(
      LessonCompletionsNotifier.new,
    );

/// 受けたレッスンの画面の操作。`listMyLessonCompletions` を呼ぶのはここだけ。段の移動と busy は [StudentFlowNotifier] に任せる
/// （[StudentFlowNotifier.moveTo] と [StudentFlowNotifier.run]）。評価を送る操作は [LessonReviewNotifier]。
class LessonCompletionsNotifier extends Notifier<LessonCompletionsState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  LessonCompletionsNotifier([LessonCompletionsState? initial])
    : _initial = initial ?? LessonCompletionsState();

  final LessonCompletionsState _initial;

  @override
  LessonCompletionsState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  StudentFlowNotifier get _flow => ref.read(studentFlowProvider.notifier);

  /// 受けたレッスンの画面を開き、一覧を読む（開くたびに読み直す）
  Future<void> open() {
    _flow.moveTo(FlowStep.lessonCompletions);
    return refresh();
  }

  /// 一覧を読み直す。読めなければ [LessonCompletionsState.failed] を立て、画面の一覧の欄にだけその旨を出す
  /// （失敗の文言は出さない）
  Future<void> refresh() => _flow.run(() async {
    state = LessonCompletionsState();
    try {
      final list = await _api.listMyLessonCompletions();
      if (!ref.mounted) return;
      state = LessonCompletionsState(completions: list.lessonCompletions);
    } on StudentApiException {
      if (!ref.mounted) return;
      state = LessonCompletionsState(failed: true);
    }
  });

  /// まだ評価していない行から、そのレッスンを評価する画面へ進む（評価の操作は [LessonReviewNotifier]）
  void review(MyLessonCompletion completion) {
    state = LessonCompletionsState(
      completions: state.completions,
      failed: state.failed,
      reviewing: completion,
    );
    _flow.moveTo(FlowStep.lessonCompletionReview);
  }

  /// 評価の画面から一覧に戻る（評価を付けたかを読み直す）
  Future<void> backFromReview() {
    _flow.moveTo(FlowStep.lessonCompletions);
    return refresh();
  }

  /// 動画添削の行から、その動画の添削結果を開く（読むだけ。閉じるとこの画面に戻る）。
  /// 読めなければこの画面に留まって失敗の文言を出す
  Future<void> openVideoReview(MyLessonCompletion completion) =>
      _flow.openMyVideoReview(completion.videoId!);
}
