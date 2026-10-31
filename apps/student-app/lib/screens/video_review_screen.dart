import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/ai_feedback_card.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 添削結果の表示（`getVideoReview`）。コーチが返却するまでは確かめ直す。
/// AI の一次フィードバック（glossary の ai_feedback）があれば、コーチの添削とは別の枠に出す。
/// 動画を送ったときにレッスンメニューを選んでいれば、その名前（送った時点の写し）を出す。
/// 自分の動画の一覧かホームの「返ってきた添削」から開いたとき（[FlowStep.myVideoReview]）は、その動画の添削を読むだけの画面にする
/// （一周シナリオの完了記録には進まず、閉じると開いた画面に戻る）。
class VideoReviewScreen extends ConsumerWidget {
  const VideoReviewScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final fromMyVideos = state.step == FlowStep.myVideoReview;
    final review = fromMyVideos ? state.myVideoReview : state.review;
    final returned = review?.status == VideoReviewStatus.returned;
    final aiComment = review?.aiFeedback?.comment.trim() ?? '';
    return FlowScaffold(
      title: '添削結果',
      leading: fromMyVideos
          ? IconButton(
              tooltip: '添削結果を閉じる',
              icon: const Icon(Icons.close),
              onPressed: state.busy ? null : notifier.closeMyVideoReview,
            )
          : null,
      children: [
        if (!fromMyVideos) const Text('動画を送りました。'),
        if (review?.lessonMenu case final menu?)
          Text(videoLessonMenuLabel(menu)),
        const SizedBox(height: 8),
        if (returned) ...[
          Text('コーチの添削', style: Theme.of(context).textTheme.titleMedium),
          if (review!.returnedAt case final at?)
            Text('返却: ${formatDateTime(at)}'),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Text(review.comment ?? ''),
            ),
          ),
          if (aiComment.isNotEmpty) ...[
            const SizedBox(height: 16),
            AiFeedbackCard(comment: aiComment),
          ],
          if (!fromMyVideos) ...[
            const SizedBox(height: 16),
            FilledButton(
              onPressed: state.busy ? null : notifier.goToCompletion,
              child: const Text('完了の記録へ進む'),
            ),
          ],
        ] else ...[
          const Text('コーチが添削しています。返却されるまでお待ちください。'),
          if (!fromMyVideos) ...[
            const SizedBox(height: 8),
            OutlinedButton(
              onPressed: state.busy ? null : notifier.refreshReview,
              child: const Text('添削結果を確かめる'),
            ),
          ],
          if (aiComment.isNotEmpty) ...[
            const SizedBox(height: 16),
            AiFeedbackCard(comment: aiComment),
          ],
        ],
      ],
    );
  }
}
