import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:flutter/semantics.dart';

import '../api/models.dart';
import '../state/prejoin_check.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/theme/rally_tokens.dart';
import '../ui/ticking_now.dart';

/// 予約の確定待ち（`confirmBooking`）。確定したら初回レッスンの受け方を選ぶ。
/// 「ビデオ通話で受ける」は開始 5 分前から押せ、押すと入室前チェックを開く（それより早いときは開ける時刻を出す）。
/// 予約した時点のレッスンメニューの写し（`Booking.lesson_menu`）があれば、その名前と 1 回の時間も出す。
class BookingPendingScreen extends ConsumerWidget {
  const BookingPendingScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final booking = state.booking!;
    final confirmed = booking.status == BookingStatus.confirmed;
    final menu = booking.lessonMenu;
    return FlowScaffold(
      title: confirmed ? '予約が確定しました' : '予約の確定待ち',
      children: [
        Text(booking.isTrial ? '体験予約' : '予約'),
        Text('開始: ${formatDateTime(booking.startsAt)}'),
        if (menu != null) Text(bookingLessonMenuLabel(menu)),
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
          _LiveLessonEntry(startsAt: booking.startsAt, busy: state.busy),
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

/// 「ビデオ通話で受ける」と、開始 5 分前より早いときの開ける時刻の案内。開いたまま 5 分前になったら押せるようにし、読み上げで 1 回伝える
class _LiveLessonEntry extends ConsumerStatefulWidget {
  const _LiveLessonEntry({required this.startsAt, required this.busy});

  final DateTime startsAt;
  final bool busy;

  @override
  ConsumerState<_LiveLessonEntry> createState() => _LiveLessonEntryState();
}

class _LiveLessonEntryState extends ConsumerState<_LiveLessonEntry> {
  bool? _wasOpen;

  @override
  Widget build(BuildContext context) {
    return TickingNow(
      builder: (context, now) {
        final open = canOpenPrejoinCheck(widget.startsAt, now);
        if (_wasOpen == false && open) {
          final view = View.of(context);
          final direction = Directionality.of(context);
          WidgetsBinding.instance.addPostFrameCallback(
            (_) => SemanticsService.sendAnnouncement(
              view,
              '入室前チェックができるようになりました',
              direction,
            ),
          );
        }
        _wasOpen = open;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          spacing: RallySpacing.sm,
          children: [
            FilledButton(
              onPressed: widget.busy || !open
                  ? null
                  : ref.read(prejoinCheckProvider.notifier).open,
              child: const Text('ビデオ通話で受ける'),
            ),
            if (!open)
              Text(
                '${prejoinOpensAtLabel(prejoinCheckOpensAt(widget.startsAt), now)} '
                'から入室前チェックができます（開始 ${prejoinCheckLeadTime.inMinutes} 分前から）。'
                'カメラとマイクの映りと音を確かめてから、開始時刻に入室します。',
                style: RallyTypography.small.copyWith(
                  color: RallyColors.textMuted,
                ),
              ),
          ],
        );
      },
    );
  }
}
