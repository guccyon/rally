import 'package:flutter/material.dart';

import '../api/models.dart';
import 'labels.dart';

/// コーチが受けた評価 1 件（点 → 評価したレッスンのメニューがあれば「メニュー: 〇〇」→ 一言 → 送った時刻）。
/// スロットを探す画面のコーチの詳細と、コーチの紹介で使う
class CoachReviewEntry extends StatelessWidget {
  const CoachReviewEntry({super.key, required this.review});

  final CoachReview review;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(coachReviewRatingLabel(review.rating)),
          if (review.lessonMenu case final menu?)
            Text(reviewLessonMenuLabel(menu)),
          Text(review.comment),
          Text(
            formatDateTime(review.submittedAt),
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}
