import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/coach_profile.dart';
import '../state/student_flow.dart';
import '../ui/coach_review_entry.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/lesson_menu_entry.dart';

/// コーチの紹介（コーチを探す一覧の行から開く）。表示名 → 自己紹介 → 評価の平均と件数と最近の評価 → 公開中のレッスンメニュー →
/// 「このコーチの空き枠を見る」（そのコーチの予約枠に絞ったスロット検索へ）。欄ごとに別の API を読み、読めなかった欄にだけ
/// その旨と「もう一度読み込む」を出す（ほかの欄と空き枠への移動はそのまま使える）
class CoachProfileScreen extends ConsumerWidget {
  const CoachProfileScreen({super.key});

  /// 最近の評価として出す件数（新しい順）
  static const recentReviewCount = 3;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final state = ref.watch(coachProfileProvider);
    final notifier = ref.read(coachProfileProvider.notifier);
    final textTheme = Theme.of(context).textTheme;
    final displayName =
        state.profile?.displayName ?? state.coach?.displayName ?? '';

    // 欄 [section] を読めなかったときの旨と読み直しのボタン
    Widget failed(CoachProfileSection section, String message) => Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Card(
          color: Theme.of(context).colorScheme.errorContainer,
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Text(message),
          ),
        ),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton(
            onPressed: busy ? null : () => notifier.reload(section),
            child: const Text('もう一度読み込む'),
          ),
        ),
      ],
    );

    Widget heading(String text) => Padding(
      padding: const EdgeInsets.only(top: 16, bottom: 4),
      child: Text(text, style: textTheme.titleMedium),
    );

    final profile = state.profile;
    final summary = state.reviewSummary;
    final reviews = state.reviews;
    final menus = state.lessonMenus;
    final failures = state.failures;
    return FlowScaffold(
      title: 'コーチの紹介',
      leading: IconButton(
        tooltip: 'コーチの一覧に戻る',
        icon: const Icon(Icons.arrow_back),
        onPressed: busy ? null : notifier.close,
      ),
      children: [
        Text(displayName, style: textTheme.headlineSmall),
        heading('自己紹介'),
        if (failures.contains(CoachProfileSection.profile))
          failed(CoachProfileSection.profile, '自己紹介を読み込めませんでした。')
        else if (profile == null)
          const Text('自己紹介を読み込んでいます…')
        else
          Text(profile.bio ?? 'まだ自己紹介はありません。'),
        heading('評価'),
        if (failures.contains(CoachProfileSection.reviewSummary))
          failed(CoachProfileSection.reviewSummary, '評価の平均と件数を読み込めませんでした。')
        else if (summary == null)
          const Text('評価を読み込んでいます…')
        else
          Text(coachReviewSummaryLabel(summary)),
        if (failures.contains(CoachProfileSection.reviews))
          failed(CoachProfileSection.reviews, '最近の評価を読み込めませんでした。')
        else if (reviews == null)
          const Text('最近の評価を読み込んでいます…')
        else if (reviews.isNotEmpty) ...[
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text('最近の評価', style: textTheme.titleSmall),
          ),
          // 契約どおり送った時刻の新しい順で返るので、先頭から出す
          for (final review in reviews.take(recentReviewCount))
            CoachReviewEntry(review: review),
        ],
        heading('レッスンメニュー'),
        if (failures.contains(CoachProfileSection.lessonMenus))
          failed(CoachProfileSection.lessonMenus, 'レッスンメニューを読み込めませんでした。')
        else if (menus == null)
          const Text('レッスンメニューを読み込んでいます…')
        else if (menus.isEmpty)
          const Text('まだ公開中のレッスンメニューはありません。')
        else
          // 契約どおり作った時刻の古い順で返るので、返った順のまま出す
          for (final menu in menus) LessonMenuEntry(menu: menu),
        const SizedBox(height: 24),
        SizedBox(
          width: double.infinity,
          child: FilledButton(
            onPressed: busy || state.coach == null ? null : notifier.showSlots,
            child: const Text('このコーチの空き枠を見る'),
          ),
        ),
      ],
    );
  }
}
