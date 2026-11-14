import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../api/student_api.dart';
import 'student_flow.dart';

/// 生徒の次の操作（`getMyNextActions`。ADR 0010）の状態。ホームの「次にやること」と、添削結果の「完了の記録へ進む」を出すかに使う。
/// 端末には残さず、開くたびにサーバーから読み直す（アプリの段の状態から次の操作を出さない）。
class NextActionsState {
  NextActionsState({List<NextAction>? actions, this.failed = false})
    : actions = actions == null ? null : List.unmodifiable(actions);

  /// サーバーが返した並びのまま（並べ替え・数え直しをしない。ADR 0006）。まだ読んでいなければ null
  final List<NextAction>? actions;

  /// 直前に読めなかった（欄にその旨を出す。失敗の文言は出さない）
  final bool failed;

  /// [videoId] の動画の完了の記録が、サーバーの記録でまだない（`record_lesson_completion` がある）か。読めていなければ false
  bool completesVideo(String videoId) =>
      actions?.any(
        (action) =>
            action.kind == NextActionKind.recordLessonCompletion &&
            action.completion is AsyncLessonCompletion &&
            action.videoId == videoId,
      ) ??
      false;
}

final nextActionsProvider =
    NotifierProvider<NextActionsNotifier, NextActionsState>(
      NextActionsNotifier.new,
    );

/// 次の操作を読み、選んだ操作の続きの画面を開く。続きの画面は [NextAction] に載る ID でサーバーから読み直して開く
/// （段の移動と読み直しは [StudentFlowNotifier] のメソッド）
class NextActionsNotifier extends Notifier<NextActionsState> {
  /// [initial] はテストで途中の状態から始めるときに渡す。
  NextActionsNotifier([NextActionsState? initial]) : _initial = initial;

  final NextActionsState? _initial;

  @override
  NextActionsState build() => _initial ?? NextActionsState();

  /// `getMyNextActions` を読む。読み直すあいだは前に読んだ並びを残し、読めたら差し替える。
  /// 読めなければ [NextActionsState.failed] にするだけで投げない（ほかの欄の表示を止めない）
  Future<void> load() async {
    try {
      final list = await ref.read(studentApiProvider).getMyNextActions();
      if (!ref.mounted) return;
      state = NextActionsState(actions: list.nextActions);
    } on StudentApiException {
      if (!ref.mounted) return;
      state = NextActionsState(failed: true);
    }
  }

  /// 次の操作の主なボタン（ホームのカードのボタンと 2 件目からの行）を押した。行き先は種類ごとに 1 つ
  Future<void> open(NextAction action) async {
    final flow = ref.read(studentFlowProvider.notifier);
    switch (action.kind) {
      case NextActionKind.confirmBooking:
        if (action.bookingId case final id?) await flow.openBooking(id);
      case NextActionKind.joinLiveLesson:
        if (action.bookingId case final id?) {
          await flow.openPrejoinCheckFor(id);
        }
      case NextActionKind.videoReviewInProgress:
        if (action.videoId case final id?) await flow.openMyVideoReview(id);
      case NextActionKind.recordLessonCompletion:
        if (action.completion case final completion?) {
          flow.goToCompletion(completion);
        }
      case NextActionKind.startSubscription:
        if (!ref.read(studentFlowProvider).busy) flow.goToSubscription();
    }
  }
}
