import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 担当コーチとのメッセージ（`listMessages` / `sendMessage`）。開いたら相手のメッセージは既読にする。
/// 自分が送ったメッセージには、コーチが読んだか（既読・未読）を出す。
class MessagesScreen extends ConsumerStatefulWidget {
  const MessagesScreen({super.key});

  @override
  ConsumerState<MessagesScreen> createState() => _MessagesScreenState();
}

class _MessagesScreenState extends ConsumerState<MessagesScreen> {
  /// 契約の `SendMessageRequest.body` の maxLength
  static const _maxLength = 2000;

  final _body = TextEditingController();

  @override
  void initState() {
    super.initState();
    _body.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _body.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final sent = await ref
        .read(studentFlowProvider.notifier)
        .sendMessage(_body.text.trim());
    if (sent && mounted) _body.clear();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final messages = state.messages ?? const <Message>[];
    return FlowScaffold(
      title: '担当コーチとのチャット',
      leading: IconButton(
        tooltip: 'スレッドの一覧へ戻る',
        icon: const Icon(Icons.arrow_back),
        onPressed: state.busy ? null : notifier.backToThreads,
      ),
      children: [
        if (messages.isEmpty) const Text('まだメッセージはありません。'),
        for (final message in messages) _MessageTile(message: message),
        const SizedBox(height: 8),
        OutlinedButton(
          onPressed: state.busy ? null : notifier.refreshMessages,
          child: const Text('新しいメッセージを確かめる'),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _body,
          decoration: const InputDecoration(labelText: 'メッセージ'),
          maxLength: _maxLength,
          maxLines: 3,
          minLines: 1,
        ),
        FilledButton(
          onPressed: state.busy || _body.text.trim().isEmpty ? null : _send,
          child: const Text('送信する'),
        ),
      ],
    );
  }
}

class _MessageTile extends StatelessWidget {
  const _MessageTile({required this.message});

  final Message message;

  @override
  Widget build(BuildContext context) {
    final mine = message.senderRole == MessageSenderRole.student;
    final read = readLabel(message);
    return Align(
      alignment: mine ? Alignment.centerRight : Alignment.centerLeft,
      child: Card(
        color: mine ? Theme.of(context).colorScheme.primaryContainer : null,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: mine
                ? CrossAxisAlignment.end
                : CrossAxisAlignment.start,
            children: [
              Text(message.body),
              const SizedBox(height: 4),
              Text(
                [
                  messageSenderLabel(message.senderRole),
                  formatDateTime(message.sentAt),
                  ?read,
                ].join(' ・ '),
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
