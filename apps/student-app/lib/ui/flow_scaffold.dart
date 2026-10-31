import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';

/// 各画面の共通の枠。直前の操作の失敗と、API を呼んでいる間の表示を出す。
/// 登録後はアプリバーにホーム（ホームの画面では出さない）・自分の予約・自分の動画・チャット・通知の受け取り設定の入口を出す（それらの画面では [leading] で戻り先を出す）。
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
              icon: const Icon(Icons.chat_bubble_outline),
              onPressed: state.busy ? null : notifier.openChat,
            ),
            IconButton(
              tooltip: '通知の設定',
              icon: const Icon(Icons.notifications_outlined),
              onPressed: state.busy ? null : notifier.openNotificationSettings,
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
