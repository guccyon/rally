import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/coach_review_entry.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/lesson_menu_entry.dart';
import '../ui/sport_selector.dart';

/// スロットの検索（`searchSlots`。スポーツで絞り、コーチの紹介から来たときはそのコーチでも絞って、見出しにコーチの名前と
/// 「絞り込みを外す」を出す）と体験予約（`createBooking`）。検索の読み込み中・0 件・読めなかったとき（「もう一度読み込む」）を出し分ける
class SlotSearchScreen extends ConsumerWidget {
  const SlotSearchScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final slots = state.slots;
    final coach = state.searchCoach;
    final List<Widget> results;
    if (state.slotsFailed) {
      results = [
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Card(
              color: Theme.of(context).colorScheme.errorContainer,
              child: const Padding(
                padding: EdgeInsets.all(12),
                child: Text('予約できるスロットを読み込めませんでした。'),
              ),
            ),
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton(
                onPressed: state.busy ? null : notifier.retrySlotSearch,
                child: const Text('もう一度読み込む'),
              ),
            ),
          ],
        ),
      ];
    } else if (slots == null) {
      results = [
        Text(state.busy ? '予約できるスロットを読み込んでいます…' : 'スポーツを選ぶと、予約できるスロットを表示します。'),
      ];
    } else if (slots.isEmpty) {
      results = [
        Text(
          coach == null ? '予約できるスロットはありません。' : 'このコーチの、このスポーツの予約できる空き枠はありません。',
        ),
      ];
    } else {
      results = [
        for (final slot in slots)
          _SlotCard(
            slot: slot,
            onBook: state.busy ? null : () => notifier.bookTrial(slot),
          ),
      ];
    }
    return FlowScaffold(
      title: 'スロットを探す',
      children: [
        // コーチの紹介の「このコーチの空き枠を見る」から来たときの見出し（外すとすべてのコーチの枠を探し直す）
        if (coach != null) ...[
          Row(
            children: [
              Expanded(
                child: Text(
                  coachSlotsHeadingLabel(coach.displayName),
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
              TextButton(
                onPressed: state.busy ? null : notifier.clearSearchCoach,
                child: const Text('絞り込みを外す'),
              ),
            ],
          ),
          const SizedBox(height: 8),
        ],
        const Text('スポーツで絞る'),
        SportSelector(
          selected: state.searchSport,
          onSelected: state.busy ? (_) {} : notifier.searchSlots,
        ),
        const SizedBox(height: 16),
        ...results,
      ],
    );
  }
}

/// スロットの行。自己紹介文を行の幅で読めるよう縦に並べる: 日時 → スポーツ →
/// グループレッスンの枠なら定員と残りの席 → 結びついていればレッスンメニューの名前と 1 回の時間（`lesson_menu`）→ 載っていればコーチの名前と自己紹介文（`coach_display_name` / `coach_bio`）→
/// コーチの評価の件数と平均（押すと評価の一覧）→ 体験予約のボタン（行の幅いっぱい）
class _SlotCard extends StatelessWidget {
  const _SlotCard({required this.slot, required this.onBook});

  final Slot slot;
  final VoidCallback? onBook;

  @override
  Widget build(BuildContext context) {
    final name = slot.coachDisplayName;
    final bio = slot.coachBio;
    final seats = slotSeatsLabel(slot);
    final menu = slot.lessonMenu;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              formatTimeRange(slot.startsAt, slot.endsAt),
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 4),
            Text(sportLabel(slot.sport)),
            if (seats != null) Text(seats),
            if (menu != null) Text(slotLessonMenuLabel(menu)),
            if (name != null) Text('コーチ: $name'),
            if (bio != null) ...[
              const SizedBox(height: 4),
              _CoachBio(bio: bio),
            ],
            const SizedBox(height: 4),
            _CoachReviews(coachId: slot.coachId),
            const SizedBox(height: 12),
            SizedBox(
              width: double.infinity,
              child: FilledButton(
                onPressed: onBook,
                child: const Text('体験予約する'),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// コーチの自己紹介文。2 行までに収め、収まらなければ押すと全文を出す（もう一度押すと戻す）
class _CoachBio extends StatefulWidget {
  const _CoachBio({required this.bio});

  static const collapsedLines = 2;

  final String bio;

  @override
  State<_CoachBio> createState() => _CoachBioState();
}

class _CoachBioState extends State<_CoachBio> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final style = DefaultTextStyle.of(context).style;
    return LayoutBuilder(
      builder: (context, constraints) {
        final painter = TextPainter(
          text: TextSpan(text: widget.bio, style: style),
          maxLines: _CoachBio.collapsedLines,
          textDirection: Directionality.of(context),
          textScaler: MediaQuery.textScalerOf(context),
        )..layout(maxWidth: constraints.maxWidth);
        final overflows = painter.didExceedMaxLines;
        painter.dispose();
        if (!overflows) return Text(widget.bio);
        return InkWell(
          onTap: () => setState(() => _expanded = !_expanded),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                widget.bio,
                maxLines: _expanded ? null : _CoachBio.collapsedLines,
                overflow: _expanded ? null : TextOverflow.ellipsis,
              ),
              Text(
                _expanded ? '閉じる' : '続きを読む',
                style: TextStyle(color: Theme.of(context).colorScheme.primary),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// コーチの評価の件数と平均と、押すと開くコーチの詳細。詳細には、そのコーチの公開中のレッスンメニューのうち生徒のスポーツのもの
/// （作った時刻の古い順）と、評価があればそのコーチの評価の一覧（新しい順。点・一言・送った時刻）を出す（もう一度押すと戻す）。
/// 評価が 0 件か件数と平均を読めなかったときは、押すとメニューだけを出す。
/// 評価・メニューを読めなかったときは、その欄にだけその旨を出す（ほかの欄と検索結果と体験予約はそのまま使える）
class _CoachReviews extends ConsumerStatefulWidget {
  const _CoachReviews({required this.coachId});

  final String coachId;

  @override
  ConsumerState<_CoachReviews> createState() => _CoachReviewsState();
}

class _CoachReviewsState extends ConsumerState<_CoachReviews> {
  bool _expanded = false;

  void _toggle({required bool withReviews}) {
    setState(() => _expanded = !_expanded);
    if (!_expanded) return;
    final state = ref.read(studentFlowProvider);
    if (!state.coachLessonMenus.containsKey(widget.coachId)) _loadMenus();
    if (withReviews && !state.coachReviews.containsKey(widget.coachId)) {
      _load();
    }
  }

  void _load() =>
      ref.read(studentFlowProvider.notifier).loadCoachReviews(widget.coachId);

  void _loadMenus() => ref
      .read(studentFlowProvider.notifier)
      .loadCoachLessonMenus(widget.coachId);

  @override
  Widget build(BuildContext context) {
    final coachId = widget.coachId;
    final summary = ref.watch(
      studentFlowProvider.select((s) => s.coachReviewSummaries[coachId]),
    );
    final summaryFailed = ref.watch(
      studentFlowProvider.select(
        (s) => s.coachReviewSummaryFailures.contains(coachId),
      ),
    );
    if (summary == null && !summaryFailed) return const SizedBox.shrink();
    final withReviews = summary != null && summary.reviewCount > 0;
    final label = summaryFailed
        ? '評価を読み込めませんでした。'
        : coachReviewSummaryLabel(summary!);
    final toggle = withReviews
        ? (_expanded ? '評価を閉じる' : '評価を見る')
        : (_expanded ? 'メニューを閉じる' : 'メニューを見る');
    final link = TextStyle(color: Theme.of(context).colorScheme.primary);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        InkWell(
          onTap: () => _toggle(withReviews: withReviews),
          child: Row(
            children: [
              Flexible(child: Text(label)),
              const SizedBox(width: 8),
              Text(toggle, style: link),
            ],
          ),
        ),
        if (_expanded) ...[
          _menus(),
          if (withReviews) ...[
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                '受けた評価',
                style: Theme.of(context).textTheme.titleSmall,
              ),
            ),
            _list(),
          ],
        ],
      ],
    );
  }

  /// レッスンメニューの欄（スポーツ・同期 / 非同期・名前・説明・同期なら 1 回の時間）
  Widget _menus() {
    final coachId = widget.coachId;
    final menus = ref.watch(
      studentFlowProvider.select((s) => s.coachLessonMenus[coachId]),
    );
    final failed = ref.watch(
      studentFlowProvider.select(
        (s) => s.coachLessonMenusFailures.contains(coachId),
      ),
    );
    final textTheme = Theme.of(context).textTheme;
    final Widget body;
    if (failed) {
      body = Row(
        children: [
          const Flexible(child: Text('レッスンメニューを読み込めませんでした。')),
          TextButton(onPressed: _loadMenus, child: const Text('メニューを読み直す')),
        ],
      );
    } else if (menus == null) {
      body = const Text('レッスンメニューを読み込んでいます…');
    } else if (menus.isEmpty) {
      body = const Text('まだメニューがありません');
    } else {
      // 契約どおり作った時刻の古い順で返るので、返った順のまま出す
      body = Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          for (final menu in menus) LessonMenuEntry(menu: menu),
        ],
      );
    }
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('レッスンメニュー', style: textTheme.titleSmall),
          body,
        ],
      ),
    );
  }

  Widget _list() {
    final coachId = widget.coachId;
    final reviews = ref.watch(
      studentFlowProvider.select((s) => s.coachReviews[coachId]),
    );
    final failed = ref.watch(
      studentFlowProvider.select(
        (s) => s.coachReviewsFailures.contains(coachId),
      ),
    );
    if (failed) {
      return Row(
        children: [
          const Flexible(child: Text('評価の一覧を読み込めませんでした。')),
          TextButton(onPressed: _load, child: const Text('もう一度読み込む')),
        ],
      );
    }
    if (reviews == null) return const Text('評価を読み込んでいます…');
    // 契約どおり送った時刻の新しい順で返るので、返った順のまま出す
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (final review in reviews) CoachReviewEntry(review: review),
      ],
    );
  }
}
