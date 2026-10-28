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
/// 載っていればコーチの名前と自己紹介文（`coach_display_name` / `coach_bio`）→ 体験予約のボタン（行の幅いっぱい）
class _SlotCard extends StatelessWidget {
  const _SlotCard({required this.slot, required this.onBook});

  final Slot slot;
  final VoidCallback? onBook;

  @override
  Widget build(BuildContext context) {
    final name = slot.coachDisplayName;
    final bio = slot.coachBio;
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
            if (name != null) Text('コーチ: $name'),
            if (bio != null) ...[
              const SizedBox(height: 4),
              _CoachBio(bio: bio),
            ],
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
