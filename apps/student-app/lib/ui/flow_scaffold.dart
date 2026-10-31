import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/chat.dart';
import '../state/notifications.dart';
import '../state/student_flow.dart';

/// 各画面の共通の枠。直前の操作の失敗と、API を呼んでいる間の表示を出す。
/// 登録後はアプリバーにホーム（ホームの画面では出さない）・自分の予約・自分の動画・チャット・お知らせ・通知の受け取り設定・プロフィールの入口を出す（それらの画面では [leading] で戻り先を出す）。
/// ホームの画面では、読んだスレッドに未読があればチャットの入口に未読の数の合計のしるしを、読んだお知らせに未読があれば
/// お知らせの入口に点のしるしを付ける（ホームは開くたびにスレッドとお知らせを読み直す）。
class FlowScaffold extends ConsumerWidget {
  const FlowScaffold({
    super.key,
    required this.title,
    required this.children,
    this.leading,
  });

  final String title;
  final List<Widget> children;

  /// アプリバーの左端（戻るボタンなど）
  final Widget? leading;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final threadsUnread = ref.watch(chatProvider.select((s) => s.unreadCount));
    final unreadCount = state.step == FlowStep.home ? threadsUnread : 0;
    final notificationsUnread = ref.watch(
      notificationsProvider.select((s) => s.hasUnread),
    );
    final hasUnreadNotifications =
        state.step == FlowStep.home && notificationsUnread;
    return Scaffold(
      appBar: AppBar(
        leading: leading,
        title: Text(title),
        actions: [
          if (state.accessToken != null && !state.step.isAside) ...[
            if (state.step != FlowStep.home)
              IconButton(
                tooltip: 'ホーム',
                icon: const Icon(Icons.home_outlined),
                onPressed: state.busy ? null : notifier.goHome,
              ),
            IconButton(
              tooltip: '自分の予約',
              icon: const Icon(Icons.event_note_outlined),
              onPressed: state.busy ? null : notifier.openMyBookings,
            ),
            IconButton(
              tooltip: '自分の動画',
              icon: const Icon(Icons.video_library_outlined),
              onPressed: state.busy ? null : notifier.openMyVideos,
            ),
            IconButton(
              tooltip: 'チャット',
              icon: Badge.count(
                count: unreadCount,
                isLabelVisible: unreadCount > 0,
                child: const Icon(Icons.chat_bubble_outline),
              ),
              onPressed: state.busy
                  ? null
                  : ref.read(chatProvider.notifier).openChat,
            ),
            IconButton(
              tooltip: 'お知らせ',
              icon: Badge(
                isLabelVisible: hasUnreadNotifications,
                child: const Icon(Icons.notifications_outlined),
              ),
              onPressed: state.busy
                  ? null
                  : ref.read(notificationsProvider.notifier).open,
            ),
            IconButton(
              tooltip: '通知の設定',
              icon: const Icon(Icons.edit_notifications_outlined),
              onPressed: state.busy ? null : notifier.openNotificationSettings,
            ),
            IconButton(
              tooltip: 'プロフィール',
              icon: const Icon(Icons.person_outline),
              onPressed: state.busy ? null : notifier.openProfile,
            ),
          ],
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (state.busy) const LinearProgressIndicator(),
          if (state.errorMessage case final message?)
            Card(
              color: Theme.of(context).colorScheme.errorContainer,
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: Text(message),
              ),
            ),
          ...children,
        ],
      ),
    );
  }
}
