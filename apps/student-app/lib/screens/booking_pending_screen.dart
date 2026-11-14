import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:flutter/semantics.dart';

import '../api/models.dart';
import '../state/prejoin_check.dart';
import '../state/student_flow.dart';
import '../ui/components/confirm_dialog.dart';
import '../ui/components/notice.dart';
import '../ui/components/section_loading.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/theme/rally_tokens.dart';
import '../ui/ticking_now.dart';

/// 予約の確定待ち（`confirmBooking`）。確定したら初回レッスンの受け方を選ぶ。
/// 「ビデオ通話で受ける」は開始 5 分前から押せ、押すと入室前チェックを開く（それより早いときは開ける時刻を出す）。
/// 予約した時点のレッスンメニューの写し（`Booking.lesson_menu`）があれば、その名前と 1 回の時間も出す。
/// 出すのはサーバーの記録の予約（作った・確定した・キャンセルした応答か、予約の ID で `listMyBookings` から読み直したもの。ADR 0010）。
/// キャンセル済み・見つからない予約は info の知らせと「ホームに戻る」だけで、確定・入室・動画の送付の道は出さない。
class BookingPendingScreen extends ConsumerWidget {
  const BookingPendingScreen({super.key});

  /// キャンセル済み・見つからない予約の知らせ
  static const _goneNotice = 'この予約はもう確定待ちではありません。ホームで次にやることを確かめてください。';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final booking = state.booking;
    final lookup = state.bookingLookup;
    final backHome = OutlinedButton(
      onPressed: state.busy ? null : notifier.goHome,
      child: const Text('ホームに戻る'),
    );
    if (lookup == BookingLookup.failed) {
      return FlowScaffold(
        title: '予約',
        children: [
          const Notice(
            message: '予約を読み込めませんでした。「ホームに戻る」から、もう一度開けます。',
            status: RallyStatus.danger,
          ),
          const SizedBox(height: 8),
          backHome,
        ],
      );
    }
    if (lookup == BookingLookup.notFound) {
      return FlowScaffold(
        title: '予約',
        children: [
          const Notice(message: _goneNotice, status: RallyStatus.info),
          const SizedBox(height: 8),
          backHome,
        ],
      );
    }
    if (lookup == BookingLookup.loading || booking == null) {
      return const FlowScaffold(
        title: '予約',
        children: [SectionLoading(message: '予約を読み込んでいます…')],
      );
    }
    final cancelled = booking.status == BookingStatus.cancelled;
    final confirmed = booking.status == BookingStatus.confirmed;
    final menu = booking.lessonMenu;
    return FlowScaffold(
      title: cancelled
          ? '予約'
          : confirmed
          ? '予約が確定しました'
          : '予約の確定待ち',
      children: [
        Text(booking.isTrial ? '体験予約' : '予約'),
        Text('開始: ${formatDateTime(booking.startsAt)}'),
        if (menu != null) Text(bookingLessonMenuLabel(menu)),
        Text('状態: ${bookingStatusLabel(booking.status)}'),
        const SizedBox(height: 16),
        if (cancelled) ...[
          const Notice(message: _goneNotice, status: RallyStatus.info),
          const SizedBox(height: 8),
          backHome,
        ] else if (!confirmed) ...[
          const Text('予約はまだ確定していません。内容を確かめて確定してください。'),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: state.busy ? null : notifier.confirmBooking,
            child: const Text('予約を確定する'),
          ),
          TextButton(
            onPressed: state.busy ? null : () => _confirmCancel(context, ref),
            child: const Text('この予約をキャンセルする'),
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

  /// 確定待ちの予約をキャンセルしてよいかを確かめ、よければ `cancelBooking`。キャンセルできたらホームへ戻り、toast で知らせる。
  /// 確定待ちはキャンセル規定で回数を使わないので、そのことは書かない
  Future<void> _confirmCancel(BuildContext context, WidgetRef ref) async {
    final state = ref.read(studentFlowProvider);
    final booking = state.booking!;
    final sport = state.bookedSlot?.sport;
    final range = state.bookedSlot == null
        ? formatDateTime(booking.startsAt)
        : formatTimeRange(state.bookedSlot!.startsAt, state.bookedSlot!.endsAt);
    final kind = booking.isTrial ? '体験予約' : '予約';
    final what = sport == null ? kind : '${sportLabel(sport)}の$kind';
    final messenger = ScaffoldMessenger.of(context);
    final confirmed = await showConfirmDialog(
      context,
      title: '予約をキャンセルしますか？',
      body: '$range の$whatをキャンセルします。押さえていたスロットはほかの生徒も予約できるようになります。',
      confirmLabel: 'キャンセルする',
      danger: true,
    );
    if (!confirmed) return;
    final cancelled = await ref
        .read(studentFlowProvider.notifier)
        .cancelOpenBooking();
    if (cancelled) {
      messenger.showSnackBar(const SnackBar(content: Text('予約をキャンセルしました')));
    }
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
