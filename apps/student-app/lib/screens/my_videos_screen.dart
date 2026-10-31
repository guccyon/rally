import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 自分が送った動画の一覧（`listMyVideos`）。添削待ちも返却済みも、送った時刻の新しい順に出す。
/// 返却済みの動画には「添削結果を見る」を出し、押すとその動画の添削結果（`getVideoReview`）を開く。
class MyVideosScreen extends ConsumerWidget {
  const MyVideosScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final videos = state.myVideos;
    return FlowScaffold(
      title: '自分の動画',
      leading: IconButton(
        tooltip: '自分の動画を閉じる',
        icon: const Icon(Icons.close),
        onPressed: state.busy ? null : notifier.closeMyVideos,
      ),
      children: [
        if (videos != null && videos.isEmpty) const Text('まだ動画を送っていません'),
        for (final item in videos ?? const <MyVideo>[])
          Card(
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '${formatDateTime(item.submittedAt)} に送信',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  Text(sportLabel(item.sport)),
                  if (item.lessonMenu case final menu?)
                    Text(videoLessonMenuLabel(menu)),
                  if (item.coachDisplayName case final name?)
                    Text('コーチ: $name'),
                  Text('状態: ${videoReviewStatusLabel(item.reviewStatus)}'),
                  if (item.reviewStatus == VideoReviewStatus.returned) ...[
                    const SizedBox(height: 8),
                    OutlinedButton(
                      onPressed: state.busy
                          ? null
                          : () => notifier.openMyVideoReview(item),
                      child: const Text('添削結果を見る'),
                    ),
                  ],
                ],
              ),
            ),
          ),
        const SizedBox(height: 8),
        OutlinedButton(
          onPressed: state.busy ? null : notifier.refreshMyVideos,
          child: const Text('一覧を更新する'),
        ),
      ],
    );
  }
}
