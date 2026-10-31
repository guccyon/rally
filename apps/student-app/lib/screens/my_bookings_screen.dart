import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 自分の予約の一覧（`listMyBookings`）とキャンセル（`cancelBooking`）。
/// まだ始まっていない予約にだけキャンセルのボタンを出し、押したら確認してからキャンセルする。
class MyBookingsScreen extends ConsumerWidget {
  const MyBookingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final bookings = state.myBookings;
    return FlowScaffold(
      title: '自分の予約',
      leading: IconButton(
        tooltip: '自分の予約を閉じる',
        icon: const Icon(Icons.close),
        onPressed: state.busy ? null : notifier.closeMyBookings,
      ),
      children: [
        if (bookings != null && bookings.isEmpty)
          const Text('まだ予約はありません。スロットを探して予約すると、ここに出ます。'),
        for (final item in bookings ?? const <MyBooking>[])
          Card(
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    formatTimeRange(item.slot.startsAt, item.slot.endsAt),
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  Text(sportLabel(item.slot.sport)),
                  if (item.booking.lessonMenu case final menu?)
                    Text(bookingLessonMenuLabel(menu)),
                  if (item.slot.coachDisplayName case final name?)
                    Text('コーチ: $name'),
                  Text('状態: ${bookingStatusLabel(item.booking.status)}'),
                  if (notifier.canCancel(item.booking)) ...[
                    const SizedBox(height: 8),
                    OutlinedButton(
                      onPressed: state.busy
                          ? null
                          : () => _confirmCancel(context, ref, item),
                      child: const Text('この予約をキャンセルする'),
                    ),
                  ],
                ],
              ),
            ),
          ),
        const SizedBox(height: 8),
        OutlinedButton(
          onPressed: state.busy ? null : notifier.refreshMyBookings,
          child: const Text('一覧を更新する'),
        ),
      ],
    );
  }

  /// キャンセルしてよいかを確かめ、よければ `cancelBooking`。
  /// 今キャンセルすると今月のビデオ通話の回数を使うなら、そのことを添える
  Future<void> _confirmCancel(
    BuildContext context,
    WidgetRef ref,
    MyBooking item,
  ) async {
    final notifier = ref.read(studentFlowProvider.notifier);
    final usesQuota = notifier.cancellationUsesQuota(item.booking);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('予約をキャンセルしますか？'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              '${formatTimeRange(item.slot.startsAt, item.slot.endsAt)} の'
              '${sportLabel(item.slot.sport)}の予約をキャンセルします。',
            ),
            if (usesQuota) ...[
              const SizedBox(height: 8),
              const Text('今キャンセルすると、今月のビデオ通話の回数を 1 回使います。'),
            ],
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('やめる'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('キャンセルする'),
          ),
        ],
      ),
    );
    if (confirmed ?? false) await notifier.cancelBooking(item.booking);
  }
}
