import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/sport_selector.dart';

/// スロットの検索（`searchSlots`。スポーツで絞る）と体験予約（`createBooking`）
class SlotSearchScreen extends ConsumerWidget {
  const SlotSearchScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final slots = state.slots;
    return FlowScaffold(
      title: 'スロットを探す',
      children: [
        const Text('スポーツで絞る'),
        SportSelector(
          selected: state.searchSport,
          onSelected: state.busy ? (_) {} : notifier.searchSlots,
        ),
        const SizedBox(height: 16),
        if (slots == null)
          const Text('スポーツを選ぶと、予約できるスロットを表示します。')
        else if (slots.isEmpty)
          const Text('予約できるスロットはありません。')
        else
          for (final slot in slots)
            _SlotCard(
              slot: slot,
              onBook: state.busy ? null : () => notifier.bookTrial(slot),
            ),
      ],
    );
  }
}

/// スロットの行。自己紹介文を行の幅で読めるよう縦に並べる: 日時 → スポーツ →
/// グループレッスンの枠なら定員と残りの席 → 載っていればコーチの名前と自己紹介文（`coach_display_name` / `coach_bio`）→
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

/// コーチの評価の件数と平均。評価があれば押すとそのコーチの評価の一覧（新しい順。点・一言・送った時刻）を出す（もう一度押すと戻す）。
/// 評価を読めなかったときは、この欄にだけその旨を出す（検索結果と体験予約はそのまま使える）
class _CoachReviews extends ConsumerStatefulWidget {
  const _CoachReviews({required this.coachId});

  final String coachId;

  @override
  ConsumerState<_CoachReviews> createState() => _CoachReviewsState();
}

class _CoachReviewsState extends ConsumerState<_CoachReviews> {
  bool _expanded = false;

  void _toggle() {
    setState(() => _expanded = !_expanded);
    final state = ref.read(studentFlowProvider);
    if (_expanded && !state.coachReviews.containsKey(widget.coachId)) {
      _load();
    }
  }

  void _load() =>
      ref.read(studentFlowProvider.notifier).loadCoachReviews(widget.coachId);

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
    if (summaryFailed) return const Text('評価を読み込めませんでした。');
    if (summary == null) return const SizedBox.shrink();
    final label = coachReviewSummaryLabel(summary);
    if (summary.reviewCount == 0) return Text(label);
    final link = TextStyle(color: Theme.of(context).colorScheme.primary);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        InkWell(
          onTap: _toggle,
          child: Row(
            children: [
              Text(label),
              const SizedBox(width: 8),
              Text(_expanded ? '評価を閉じる' : '評価を見る', style: link),
            ],
          ),
        ),
        if (_expanded) _list(),
      ],
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
        for (final review in reviews)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(coachReviewRatingLabel(review.rating)),
                Text(review.comment),
                Text(
                  formatDateTime(review.submittedAt),
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ),
          ),
      ],
    );
  }
}
