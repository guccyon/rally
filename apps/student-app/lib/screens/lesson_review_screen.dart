import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 完了したレッスンの評価（`submitLessonReview`）。評価はレッスン 1 件につき 1 回なので、
/// この端末で送ったか、送って 409 `lesson_already_reviewed` が返ったら入力させない。
class LessonReviewScreen extends ConsumerStatefulWidget {
  const LessonReviewScreen({super.key});

  @override
  ConsumerState<LessonReviewScreen> createState() => _LessonReviewScreenState();
}

class _LessonReviewScreenState extends ConsumerState<LessonReviewScreen> {
  /// 契約の `SubmitLessonReviewRequest.comment` の maxLength
  static const _maxLength = 200;

  final _comment = TextEditingController();
  int? _rating;

  @override
  void initState() {
    super.initState();
    _comment.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _comment.dispose();
    super.dispose();
  }

  void _submit() => ref
      .read(studentFlowProvider.notifier)
      .submitReview(rating: _rating!, comment: _comment.text.trim());

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final lesson = state.lesson!;
    final sent = state.lessonReview;
    return FlowScaffold(
      title: 'レッスンの評価',
      children: [
        Text(lessonTypeLabel(lesson.lessonType)),
        Text('完了: ${formatDateTime(lesson.completedAt)}'),
        const SizedBox(height: 16),
        if (!state.lessonReviewed) ...[
          Text(
            'コーチの指導を ${ratingWithLabel(1)}〜${ratingWithLabel(5)} で評価し、一言を添えてください。評価は 1 回だけで、送ったあとは変えられません。',
          ),
          const SizedBox(height: 8),
          // 狭い幅でも言葉が折り返さないよう、点と言葉を 1 行にした選択肢を 5 → 1 の順に縦に並べる
          RadioGroup<int>(
            groupValue: _rating,
            onChanged: (rating) => setState(() => _rating = rating),
            child: Column(
              children: [
                for (var rating = 5; rating >= 1; rating--)
                  RadioListTile<int>(
                    value: rating,
                    enabled: !state.busy,
                    title: Text(ratingChoiceLabel(rating)),
                    contentPadding: EdgeInsets.zero,
                  ),
              ],
            ),
          ),
          TextField(
            controller: _comment,
            enabled: !state.busy,
            decoration: const InputDecoration(labelText: '一言'),
            maxLength: _maxLength,
            maxLines: 3,
            minLines: 1,
          ),
          FilledButton(
            onPressed:
                state.busy || _rating == null || _comment.text.trim().isEmpty
                ? null
                : _submit,
            child: const Text('評価を送る'),
          ),
        ] else if (sent != null && sent.lessonId == lesson.lessonId) ...[
          const Text('評価を送りました'),
          Text('評価: ${ratingWithLabel(sent.rating)}'),
          Text('一言: ${sent.comment}'),
        ] else
          const Text('このレッスンは評価済みのため、入力できません。'),
        const SizedBox(height: 16),
        OutlinedButton(
          onPressed: state.busy ? null : notifier.goToSubscription,
          child: const Text('月額契約へ進む'),
        ),
      ],
    );
  }
}
