import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 添削結果の表示（`getVideoReview`）。コーチが返却するまでは確かめ直す。
class VideoReviewScreen extends ConsumerWidget {
  const VideoReviewScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final review = state.review;
    final returned = review?.status == VideoReviewStatus.returned;
    return FlowScaffold(
      title: '添削結果',
      children: [
        const Text('動画を送りました。'),
        const SizedBox(height: 8),
        if (returned) ...[
          if (review!.returnedAt case final at?)
            Text('返却: ${formatDateTime(at)}'),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Text(review.comment ?? ''),
            ),
          ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: state.busy ? null : notifier.goToCompletion,
            child: const Text('完了の記録へ進む'),
          ),
        ] else ...[
          const Text('コーチが添削しています。返却されるまでお待ちください。'),
          const SizedBox(height: 8),
          OutlinedButton(
            onPressed: state.busy ? null : notifier.refreshReview,
            child: const Text('添削結果を確かめる'),
          ),
        ],
      ],
    );
  }
}
