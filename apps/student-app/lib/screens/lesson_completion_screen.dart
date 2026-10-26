import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// レッスンの完了記録（`recordLessonCompletion`）
class LessonCompletionScreen extends ConsumerWidget {
  const LessonCompletionScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final lesson = state.lesson;
    return FlowScaffold(
      title: 'レッスンの完了',
      children: [
        if (lesson == null) ...[
          Text(state.session != null ? '通話が終わりました。' : '添削が返されました。'),
          const Text('レッスンを受け終えたら、完了を記録してください。'),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: state.busy ? null : notifier.recordCompletion,
            child: const Text('完了を記録する'),
          ),
        ] else ...[
          Text(lesson.isFirstLesson ? '初回レッスンを完了しました' : 'レッスンを完了しました'),
          Text(lessonTypeLabel(lesson.lessonType)),
          Text('記録: ${formatDateTime(lesson.completedAt)}'),
          const SizedBox(height: 16),
          const Text('月額契約で、動画とチャットは送り放題、ビデオ通話は月 2 回まで受けられます。'),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: state.busy ? null : notifier.goToSubscription,
            child: const Text('月額契約へ進む'),
          ),
        ],
      ],
    );
  }
}
