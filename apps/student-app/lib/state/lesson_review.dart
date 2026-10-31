import 'dart:collection';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'student_flow.dart';

/// 完了したレッスンの評価（コーチの評価）の状態。評価する完了したレッスン（[StudentFlowState.lesson]）は一周シナリオの
/// 完了記録が持ち、画面の段（[FlowStep]）と、API を呼んでいる間の印・直前の操作の失敗の文言はアプリ全体で 1 つなので、
/// [StudentFlowState] の `lesson` / `step` / `busy` / `errorMessage` を使う。
class LessonReviewState {
  LessonReviewState({
    this.lessonReview,
    Set<String> reviewedLessonIds = const {},
  })
    // 名前付き引数は _ で始められないので、初期化子で private の欄に入れる
    // ignore: prefer_initializing_formals
    : _reviewedLessonIds = reviewedLessonIds;

  /// この端末で送った評価（`submitLessonReview` の応答）
  final LessonReview? lessonReview;

  /// 評価済みと分かっている完了したレッスンの ID。評価済みを返す読み取りの API はないので、
  /// この端末で送れたときと、送って 409 `lesson_already_reviewed` が返ったときに足す。
  /// 読み出しは変更できないビューで返す（どの経路で作っても、`add` などで Riverpod を通さずに状態を変えさせない）
  Set<String> get reviewedLessonIds => UnmodifiableSetView(_reviewedLessonIds);
  final Set<String> _reviewedLessonIds;

  /// [lessonId] の完了したレッスンを評価済みと分かっているか
  bool isReviewed(String? lessonId) => _reviewedLessonIds.contains(lessonId);

  LessonReviewState copyWith({
    LessonReview? lessonReview,
    Set<String>? reviewedLessonIds,
  }) => LessonReviewState(
    lessonReview: lessonReview ?? this.lessonReview,
    reviewedLessonIds: reviewedLessonIds ?? _reviewedLessonIds,
  );
}

final lessonReviewProvider =
    NotifierProvider<LessonReviewNotifier, LessonReviewState>(
      LessonReviewNotifier.new,
    );

/// 完了したレッスンの評価の操作。`submitLessonReview` を呼ぶのはここだけ。busy・失敗の文言は [StudentFlowNotifier] に任せる
/// （[StudentFlowNotifier.run] で 1 つずつ走らせる）。
class LessonReviewNotifier extends Notifier<LessonReviewState> {
  /// [initial] はテストで途中の画面から始めるときに渡す。
  LessonReviewNotifier([LessonReviewState? initial])
    : _initial = initial ?? LessonReviewState();

  final LessonReviewState _initial;

  @override
  LessonReviewState build() => _initial;

  StudentApi get _api => ref.read(studentApiProvider);

  /// `submitLessonReview`。評価するのは完了を記録したレッスン（[StudentFlowState.lesson]）。評価は完了したレッスン 1 件につき
  /// 1 回なので、送れたときも 409 `lesson_already_reviewed` が返ったときも評価済みにする（以後は入力させない）。
  Future<void> submitReview({required int rating, required String comment}) =>
      ref.read(studentFlowProvider.notifier).run(() async {
        final lessonId = ref.read(studentFlowProvider).lesson!.lessonId;
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
}
