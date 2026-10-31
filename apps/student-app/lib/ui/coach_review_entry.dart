import 'package:flutter/material.dart';

import '../api/models.dart';
import 'labels.dart';

/// コーチが受けた評価 1 件（点 → 評価したレッスンのメニューがあれば「メニュー: 〇〇」→ 一言 → 送った時刻）。
/// コーチの返信があれば、その下に字下げして「コーチからの返信」→ 返信の文 → 返信を付けた時刻を出す（なければ何も出さない）。
/// スロットを探す画面のコーチの詳細と、コーチの紹介で使う
class CoachReviewEntry extends StatelessWidget {
  const CoachReviewEntry({super.key, required this.review});

  final CoachReview review;

  @override
  Widget build(BuildContext context) {
    final small = Theme.of(context).textTheme.bodySmall;
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(coachReviewRatingLabel(review.rating)),
          if (review.lessonMenu case final menu?)
            Text(reviewLessonMenuLabel(menu)),
          Text(review.comment),
          Text(formatDateTime(review.submittedAt), style: small),
          if (review.reply case final reply?) _ReplyEntry(reply: reply),
        ],
      ),
    );
  }
}

/// 評価へのコーチの返信（評価の下に、左の線で字下げして出す）
class _ReplyEntry extends StatelessWidget {
  const _ReplyEntry({required this.reply});

  final LessonReviewReply reply;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      margin: const EdgeInsets.only(top: 4, left: 12),
      padding: const EdgeInsets.only(left: 8),
      decoration: BoxDecoration(
        border: Border(
          left: BorderSide(color: theme.colorScheme.outlineVariant, width: 2),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(coachReviewReplyHeading, style: theme.textTheme.labelMedium),
          Text(reply.comment),
          Text(
            formatDateTime(reply.repliedAt),
            style: theme.textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}
