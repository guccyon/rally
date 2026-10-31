import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/lesson_completions.dart';
import '../state/lesson_review.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 受けたレッスン（`listMyLessonCompletions`。完了した時刻の新しい順のまま）。各行は完了した日時 → 通話か動画添削か →
/// コーチ → レッスンメニュー（あれば）→ 評価したか。まだ評価していない行の「評価する」でそのレッスンの評価を開き、
/// 動画添削の行の「添削結果を見る」でその動画の添削結果を開く
class LessonCompletionsScreen extends ConsumerWidget {
  const LessonCompletionsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final state = ref.watch(lessonCompletionsProvider);
    final reviewState = ref.watch(lessonReviewProvider);
    final notifier = ref.read(lessonCompletionsProvider.notifier);
    final completions = state.completions;
    return FlowScaffold(
      title: '受けたレッスン',
      children: [
        if (state.failed) ...[
          Card(
            color: Theme.of(context).colorScheme.errorContainer,
            child: const Padding(
              padding: EdgeInsets.all(12),
              child: Text('受けたレッスンを読み込めませんでした。'),
            ),
          ),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton(
              onPressed: busy ? null : notifier.refresh,
              child: const Text('もう一度読み込む'),
            ),
          ),
        ] else if (completions == null)
          const Text('受けたレッスンを読み込んでいます…')
        else if (completions.isEmpty)
          const Text('まだ受けたレッスンはありません。レッスンの完了を記録すると、ここで振り返れます。')
        else
          for (final item in completions)
            Card(
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      formatDateTime(item.completedAt),
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    Text(lessonTypeLabel(item.lessonType)),
                    Text(coachNameLabel(item.coachDisplayName)),
                    if (item.lessonMenu case final menu?)
                      Text(lessonCompletionMenuLabel(menu)),
                    // 一覧を読んだあとにこの端末で評価したものも評価済みにする
                    if (item.isReviewed ||
                        reviewState.isReviewed(item.lessonId))
                      const Text('評価済み')
                    else
                      const Text('まだ評価していません'),
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 8,
                      children: [
                        if (!item.isReviewed &&
                            !reviewState.isReviewed(item.lessonId))
                          FilledButton(
                            onPressed: busy
                                ? null
                                : () => notifier.review(item),
                            child: const Text('評価する'),
                          ),
                        if (item.lessonType == LessonType.asyncLesson &&
                            item.videoId != null)
                          OutlinedButton(
                            onPressed: busy
                                ? null
                                : () => notifier.openVideoReview(item),
                            child: const Text('添削結果を見る'),
                          ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
      ],
    );
  }
}
