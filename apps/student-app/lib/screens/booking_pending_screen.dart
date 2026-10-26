import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 予約の確定待ち（`confirmBooking`）。確定したら初回レッスンの受け方を選ぶ。
class BookingPendingScreen extends ConsumerWidget {
  const BookingPendingScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final booking = state.booking!;
    final confirmed = booking.status == BookingStatus.confirmed;
    return FlowScaffold(
      title: confirmed ? '予約が確定しました' : '予約の確定待ち',
      children: [
        Text(booking.isTrial ? '体験予約' : '予約'),
        Text('開始: ${formatDateTime(booking.startsAt)}'),
        Text('状態: ${bookingStatusLabel(booking.status)}'),
        const SizedBox(height: 16),
        if (!confirmed) ...[
          const Text('予約はまだ確定していません。内容を確かめて確定してください。'),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: state.busy ? null : notifier.confirmBooking,
            child: const Text('予約を確定する'),
          ),
        ] else ...[
          const Text('初回レッスンの受け方を選んでください。'),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: state.busy ? null : notifier.chooseLiveLesson,
            child: const Text('ビデオ通話で受ける'),
          ),
          const SizedBox(height: 8),
          OutlinedButton(
            onPressed: state.busy ? null : notifier.chooseVideo,
            child: const Text('動画を送って添削を受ける'),
          ),
        ],
      ],
    );
  }
}
