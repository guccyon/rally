import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 担当コーチとのスレッドの一覧（`listMessageThreads`）。スレッドは予約が確定すると開く。
class MessageThreadsScreen extends ConsumerWidget {
  const MessageThreadsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final threads = state.threads;
    return FlowScaffold(
      title: 'チャット',
      leading: IconButton(
        tooltip: 'チャットを閉じる',
        icon: const Icon(Icons.close),
        onPressed: state.busy ? null : notifier.closeChat,
      ),
      children: [
        if (threads != null && threads.isEmpty)
          const Text('まだスレッドはありません。予約が確定すると、担当コーチとのスレッドが開きます。'),
        for (final thread in threads ?? const [])
          Card(
            child: ListTile(
              title: const Text('担当コーチ'),
              subtitle: Text(
                thread.lastMessage == null
                    ? 'まだメッセージはありません'
                    : '${messageSenderLabel(thread.lastMessage!.senderRole)}: '
                          '${thread.lastMessage!.body}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              trailing: thread.unreadCount > 0
                  ? Badge(label: Text('未読 ${thread.unreadCount}'))
                  : null,
              onTap: state.busy ? null : () => notifier.openThread(thread),
            ),
          ),
        const SizedBox(height: 8),
        OutlinedButton(
          onPressed: state.busy ? null : notifier.refreshThreads,
          child: const Text('一覧を更新する'),
        ),
      ],
    );
  }
}
