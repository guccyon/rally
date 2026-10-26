import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

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
                subtitle: Text(sportLabel(slot.sport)),
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
