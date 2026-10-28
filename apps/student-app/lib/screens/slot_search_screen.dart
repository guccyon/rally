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
            Card(
              child: ListTile(
                title: Text(
                  '${formatDateTime(slot.startsAt)} 〜 ${formatDateTime(slot.endsAt)}',
                ),
                subtitle: _SlotSubtitle(slot: slot),
                trailing: FilledButton(
                  onPressed: state.busy ? null : () => notifier.bookTrial(slot),
                  child: const Text('体験予約する'),
                ),
              ),
            ),
      ],
    );
  }
}

/// スロットの行の補足。スポーツと、載っていればコーチの名前と自己紹介文（`coach_display_name` / `coach_bio`）
class _SlotSubtitle extends StatelessWidget {
  const _SlotSubtitle({required this.slot});

  final Slot slot;

  @override
  Widget build(BuildContext context) {
    final name = slot.coachDisplayName;
    final bio = slot.coachBio;
    if (name == null && bio == null) return Text(sportLabel(slot.sport));
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(sportLabel(slot.sport)),
        if (name != null) Text('コーチ: $name'),
        if (bio != null) _CoachBio(bio: bio),
      ],
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
