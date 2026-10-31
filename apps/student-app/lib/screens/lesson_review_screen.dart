import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/lesson_completions.dart';
import '../state/lesson_review.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 完了したレッスンの評価（`submitLessonReview`）。評価はレッスン 1 件につき 1 回なので、
/// この端末で送ったか、送って 409 `lesson_already_reviewed` が返ったら入力させない。
/// 評価の状態と操作は [lessonReviewProvider]、評価する完了したレッスンと busy・段の移動は [studentFlowProvider] のもの。
/// 受けたレッスンの行から開いたとき（[FlowStep.lessonCompletionReview]）は、評価するレッスンは
/// [LessonCompletionsState.reviewing] で、コーチとメニューも出し、月額契約へは進まずにアプリバーの戻るで受けたレッスンに戻る。
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

  void _submit(String lessonId) => ref
      .read(lessonReviewProvider.notifier)
      .submitReview(
        rating: _rating!,
        comment: _comment.text.trim(),
        lessonId: lessonId,
      );

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(lessonReviewProvider);
    final fromCompletions = ref.watch(
      studentFlowProvider.select(
        (s) => s.step == FlowStep.lessonCompletionReview,
      ),
    );
    final completion = fromCompletions
        ? ref.watch(lessonCompletionsProvider.select((s) => s.reviewing))
        : null;
    // 受けたレッスンに戻るとき、この画面が入れ替わる前に一覧の読み直しが評価するレッスンを外す
    if (fromCompletions && completion == null) return const SizedBox.shrink();
    final lesson = fromCompletions
        ? null
        : ref.watch(studentFlowProvider.select((s) => s.lesson!));
    final lessonId = completion?.lessonId ?? lesson!.lessonId;
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final flowNotifier = ref.read(studentFlowProvider.notifier);
    final sent = state.lessonReview;
    final reviewed =
        state.isReviewed(lessonId) || (completion?.isReviewed ?? false);
    return FlowScaffold(
      title: 'レッスンの評価',
      leading: fromCompletions
          ? IconButton(
              tooltip: '受けたレッスンに戻る',
              icon: const Icon(Icons.arrow_back),
              onPressed: busy
                  ? null
                  : ref.read(lessonCompletionsProvider.notifier).backFromReview,
            )
          : null,
      children: [
        Text(lessonTypeLabel(completion?.lessonType ?? lesson!.lessonType)),
        Text(
          '完了: ${formatDateTime(completion?.completedAt ?? lesson!.completedAt)}',
        ),
        if (completion != null) ...[
          Text(coachNameLabel(completion.coachDisplayName)),
          if (completion.lessonMenu case final menu?)
            Text(lessonCompletionMenuLabel(menu)),
        ],
        const SizedBox(height: 16),
        if (!reviewed) ...[
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
                    enabled: !busy,
                    title: Text(ratingChoiceLabel(rating)),
                    contentPadding: EdgeInsets.zero,
                  ),
              ],
            ),
          ),
          TextField(
            controller: _comment,
            enabled: !busy,
            decoration: const InputDecoration(labelText: '一言'),
            maxLength: _maxLength,
            maxLines: 3,
            minLines: 1,
          ),
          FilledButton(
            onPressed: busy || _rating == null || _comment.text.trim().isEmpty
                ? null
                : () => _submit(lessonId),
            child: const Text('評価を送る'),
          ),
        ] else if (sent != null && sent.lessonId == lessonId) ...[
          const Text('評価を送りました'),
          Text('評価: ${ratingWithLabel(sent.rating)}'),
          Text('一言: ${sent.comment}'),
        ] else
          const Text('このレッスンは評価済みのため、入力できません。'),
        if (!fromCompletions) ...[
          const SizedBox(height: 16),
          OutlinedButton(
            onPressed: busy ? null : flowNotifier.goToSubscription,
            child: const Text('月額契約へ進む'),
          ),
        ],
      ],
    );
  }
}
