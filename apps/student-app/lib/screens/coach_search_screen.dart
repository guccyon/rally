import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/coach_profile.dart';
import '../state/coach_search.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// コーチを探す（`listCoachesForStudent`。生徒のスポーツのコーチをサーバーの並びのまま出す）。
/// 各行は表示名と、評価の平均と件数（0 件なら「評価なし」）・レッスンメニューの数を「・」でつなぐ。お気に入りのコーチには
/// ハートのしるしを付ける（コーチの紹介で切り替えたら、一覧を読み直さずに合わせる）。
/// 行を押すと、そのコーチの紹介（[CoachProfileState]）を開く
class CoachSearchScreen extends ConsumerWidget {
  const CoachSearchScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final student = ref.watch(studentFlowProvider.select((s) => s.student));
    final state = ref.watch(coachSearchProvider);
    final notifier = ref.read(coachSearchProvider.notifier);
    final coaches = state.coaches;
    return FlowScaffold(
      title: 'コーチを探す',
      children: [
        if (student != null) Text('${sportLabel(student.sport)}のコーチ'),
        const SizedBox(height: 16),
        if (state.failed) ...[
          Card(
            color: Theme.of(context).colorScheme.errorContainer,
            child: const Padding(
              padding: EdgeInsets.all(12),
              child: Text('コーチの一覧を読み込めませんでした。'),
            ),
          ),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton(
              onPressed: busy ? null : notifier.refresh,
              child: const Text('もう一度読み込む'),
            ),
          ),
        ] else if (coaches == null)
          const Text('コーチを読み込んでいます…')
        else if (coaches.isEmpty)
          const Text('まだこのスポーツのコーチはいません。')
        else
          for (final coach in coaches)
            Card(
              child: ListTile(
                title: Text(coach.displayName),
                subtitle: Text(
                  [
                    coachListingRatingLabel(coach),
                    coachListingLessonMenuCountLabel(coach),
                  ].join('・'),
                ),
                trailing: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (coach.isFavorite)
                      Icon(
                        Icons.favorite,
                        semanticLabel: 'お気に入り',
                        color: Theme.of(context).colorScheme.primary,
                      ),
                    const Icon(Icons.chevron_right),
                  ],
                ),
                onTap: busy ? null : () => notifier.select(coach),
              ),
            ),
      ],
    );
  }
}
