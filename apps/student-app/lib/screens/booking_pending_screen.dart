import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:flutter/semantics.dart';

import '../api/models.dart';
import '../state/prejoin_check.dart';
import '../state/student_flow.dart';
import '../ui/components/notice.dart';
import '../ui/components/section_loading.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/theme/rally_tokens.dart';
import '../ui/ticking_now.dart';

/// 予約の確定待ち（`confirmBooking`）。確定したら初回レッスンの受け方を選ぶ。
/// 「ビデオ通話で受ける」は開始 5 分前から押せ、押すと入室前チェックを開く（それより早いときは開ける時刻を出す）。
/// 予約した時点のレッスンメニューの写し（`Booking.lesson_menu`）があれば、その名前と 1 回の時間も出す。
/// 開くたび（自分の予約などから戻ったときも）に予約を ID で読み直し（ADR 0010）、読み込み中・失敗・見つからない・
/// キャンセル済み・確定待ち・確定済みを出し分ける。受け方のボタンは確定済みのときだけ出す。
class BookingPendingScreen extends ConsumerStatefulWidget {
  const BookingPendingScreen({super.key});

  @override
  ConsumerState<BookingPendingScreen> createState() =>
      _BookingPendingScreenState();
}

class _BookingPendingScreenState extends ConsumerState<BookingPendingScreen> {
  /// 開いてから読み直しを始めたか（始める前の 1 枚目も読み込み中にし、前に持っていた予約の状態を出さない）
  bool _reloading = false;

  @override
  void initState() {
    super.initState();
    // 画面を描いてから読み直す（provider を描く途中で変えない）
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      setState(() => _reloading = true);
      ref.read(studentFlowProvider.notifier).reloadBooking();
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final booking = state.booking!;
    final goHome = state.busy ? null : notifier.goHome;
    switch (_reloading ? state.bookingLoad : BookingLoad.loading) {
      case BookingLoad.loading:
        return const FlowScaffold(
          title: '予約',
          children: [
            LinearProgressIndicator(),
            SizedBox(height: 8),
            SectionLoading(message: '予約を読み込んでいます…'),
          ],
        );
      case BookingLoad.failed:
        return FlowScaffold(
          title: '予約',
          children: [
            const Notice(
              message: '予約を読み込めませんでした。通信の状態を確かめて、ホームからもう一度開いてください。',
              status: RallyStatus.danger,
            ),
            const SizedBox(height: 16),
            OutlinedButton(onPressed: goHome, child: const Text('ホームに戻る')),
          ],
        );
      case BookingLoad.notFound:
        return FlowScaffold(
          title: '予約',
          children: [
            const Notice(
              message: 'この予約は見つかりませんでした。ホームや自分の予約から、今の予約を確かめてください。',
              status: RallyStatus.info,
            ),
            const SizedBox(height: 16),
            OutlinedButton(onPressed: goHome, child: const Text('ホームに戻る')),
          ],
        );
      case BookingLoad.loaded:
        break;
    }
    final menu = booking.lessonMenu;
    final details = [
      Text(booking.isTrial ? '体験予約' : '予約'),
      Text('開始: ${formatDateTime(booking.startsAt)}'),
      if (menu != null) Text(bookingLessonMenuLabel(menu)),
      Text('状態: ${bookingStatusLabel(booking.status)}'),
      const SizedBox(height: 16),
    ];
    return switch (booking.status) {
      BookingStatus.cancelled => FlowScaffold(
        title: '予約はキャンセル済みです',
        children: [
          ...details,
          const Notice(
            message: 'この予約はキャンセルしました。このレッスンは受けられないので、ほかの枠やコーチを探して予約してください。',
            status: RallyStatus.info,
          ),
          const SizedBox(height: 16),
          OutlinedButton(onPressed: goHome, child: const Text('ホームに戻る')),
        ],
      ),
      BookingStatus.pending => FlowScaffold(
        title: '予約の確定待ち',
        children: [
          ...details,
          const Text('予約はまだ確定していません。内容を確かめて確定してください。'),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: state.busy ? null : notifier.confirmBooking,
            child: const Text('予約を確定する'),
          ),
        ],
      ),
      BookingStatus.confirmed => FlowScaffold(
        title: '予約が確定しました',
        children: [
          ...details,
          const Text('初回レッスンの受け方を選んでください。'),
          const SizedBox(height: 8),
          _LiveLessonEntry(startsAt: booking.startsAt, busy: state.busy),
          const SizedBox(height: 8),
          OutlinedButton(
            onPressed: state.busy ? null : notifier.chooseVideo,
            child: const Text('動画を送って添削を受ける'),
          ),
        ],
      ),
    };
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
