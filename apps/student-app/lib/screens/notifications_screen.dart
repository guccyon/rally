import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/notifications.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// お知らせ（`listMyNotifications`。作った時刻の新しい順）。開いたらすべて既読にし（`markMyNotificationsRead`）、
/// 開いたときに未読だったものは「新着」と添える。行を押すと、種類に応じて自分の予約・自分の動画・チャットを開く
/// （このアプリの版が知らない種類の行は押せない）
class NotificationsScreen extends ConsumerWidget {
  const NotificationsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
    final state = ref.watch(notificationsProvider);
    final notifier = ref.read(notificationsProvider.notifier);
    final notifications = state.notifications;
    return FlowScaffold(
      title: 'お知らせ',
      leading: IconButton(
        tooltip: 'お知らせを閉じる',
        icon: const Icon(Icons.close),
        onPressed: busy ? null : notifier.close,
      ),
      children: [
        if (state.failed) ...[
          Card(
            color: Theme.of(context).colorScheme.errorContainer,
            child: const Padding(
              padding: EdgeInsets.all(12),
              child: Text('お知らせを読み込めませんでした。'),
            ),
          ),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton(
              onPressed: busy ? null : notifier.refresh,
              child: const Text('もう一度読み込む'),
            ),
          ),
        ] else if (notifications == null)
          const Text('お知らせを読み込んでいます…')
        else if (notifications.isEmpty)
          const Text('まだお知らせはありません。予約の確定・添削の返却・コーチからのメッセージがここに届きます。')
        else
          for (final notification in notifications)
            Card(
              child: ListTile(
                leading: Icon(_iconOf(notification.kind)),
                title: Text(notification.body),
                subtitle: Text(
                  [
                    if (state.newIds.contains(notification.notificationId))
                      '新着',
                    formatDateTime(notification.createdAt),
                  ].join('・'),
                ),
                trailing: notifier.canOpen(notification)
                    ? const Icon(Icons.chevron_right)
                    : null,
                onTap: busy || !notifier.canOpen(notification)
                    ? null
                    : () => notifier.select(notification),
              ),
            ),
      ],
    );
  }

  static IconData _iconOf(NotificationKind? kind) => switch (kind) {
    NotificationKind.bookingConfirmed => Icons.event_available_outlined,
    NotificationKind.videoReviewReturned => Icons.rate_review_outlined,
    NotificationKind.messageReceived => Icons.chat_bubble_outline,
    null => Icons.notifications_outlined,
  };
}
