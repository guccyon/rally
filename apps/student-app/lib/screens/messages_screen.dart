import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/ai_feedback_card.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import 'video_submit_screen.dart';

/// 担当コーチとのメッセージ（`listMessages` / `sendMessage`）。開いたら相手のメッセージは既読にする。
/// 自分が送ったメッセージには、コーチが読んだか（既読・未読）を出す。
/// 動画の添削の依頼（`submitVideo`）もここからでき、添削の依頼と返却は本文のメッセージと見分けられるカードで時間の順に並べる。
/// 知らない種類のメッセージは隠さず、`body`（短い説明）を本文のメッセージとして出す。
class MessagesScreen extends ConsumerStatefulWidget {
  const MessagesScreen({super.key});

  @override
  ConsumerState<MessagesScreen> createState() => _MessagesScreenState();
}

class _MessagesScreenState extends ConsumerState<MessagesScreen> {
  final _body = TextEditingController();

  @override
  void initState() {
    super.initState();
    _body.addListener(() => setState(() {}));
    // ホームの「添削を依頼する」から開いたら、すぐに添削の依頼（動画の選択）を出す
    if (ref.read(studentFlowProvider).promptVideoReviewRequest) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        ref.read(studentFlowProvider.notifier).clearVideoReviewRequestPrompt();
        _requestVideoReview();
      });
    }
  }

  @override
  void dispose() {
    _body.dispose();
    super.dispose();
  }

  /// 契約の `SendMessageRequest` に収まる本文か（入力欄の maxLength は書記素で数えるので、コードポイントでも確かめる）
  bool get _sendable => SendMessageRequest.isValidBody(_body.text.trim());

  Future<void> _send() async {
    final sent = await ref
        .read(studentFlowProvider.notifier)
        .sendMessage(_body.text.trim());
    if (sent && mounted) _body.clear();
  }

  /// 動画を選んで添削を依頼する（動画の選択はモック）。やめたら何もしない
  Future<void> _requestVideoReview() async {
    final request = await showDialog<({String? note})>(
      context: context,
      builder: (_) => const _VideoReviewRequestDialog(),
    );
    if (request == null || !mounted) return;
    await ref
        .read(studentFlowProvider.notifier)
        .requestVideoReview(
          contentType: VideoSubmitScreen.mockContentType,
          note: request.note,
        );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final messages = state.messages ?? const <Message>[];
    // 添削の依頼が何回目か（スレッドの中で送った順。返却のカードもこの番号でどの依頼への返却かを示す）
    final requestNumbers = <String, int>{};
    for (final message in messages) {
      if (message.kind == MessageKind.videoReviewRequest &&
          message.videoId != null) {
        requestNumbers.putIfAbsent(
          message.videoId!,
          () => requestNumbers.length + 1,
        );
      }
    }
    return FlowScaffold(
      title: '担当コーチとのチャット',
      leading: IconButton(
        tooltip: 'スレッドの一覧へ戻る',
        icon: const Icon(Icons.arrow_back),
        onPressed: state.busy ? null : notifier.backToThreads,
      ),
      children: [
        if (messages.isEmpty) const Text('まだメッセージはありません。'),
        for (final message in messages)
          switch (message.kind) {
            MessageKind.videoReviewRequest => _ReviewRequestCard(
              message: message,
              number: requestNumbers[message.videoId],
            ),
            MessageKind.videoReviewReturned => _ReviewReturnedCard(
              message: message,
              number: requestNumbers[message.videoId],
              review: state.chatReviews[message.videoId],
              failed: state.chatReviewFailures.contains(message.videoId),
            ),
            MessageKind.text ||
            MessageKind.other => _MessageTile(message: message),
          },
        const SizedBox(height: 8),
        OutlinedButton(
          onPressed: state.busy ? null : notifier.refreshMessages,
          child: const Text('新しいメッセージを確かめる'),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _body,
          decoration: const InputDecoration(labelText: 'メッセージ'),
          maxLength: SendMessageRequest.maxBodyLength,
          maxLines: 3,
          minLines: 1,
        ),
        FilledButton(
          onPressed: state.busy || !_sendable ? null : _send,
          child: const Text('送信する'),
        ),
        const SizedBox(height: 8),
        OutlinedButton.icon(
          onPressed: state.busy ? null : _requestVideoReview,
          icon: const Icon(Icons.videocam_outlined),
          label: const Text('添削を依頼する'),
        ),
      ],
    );
  }
}

/// 添削の依頼に添える動画の選択（モック）と相談内容。依頼するなら相談内容（空なら null）を返す
class _VideoReviewRequestDialog extends StatefulWidget {
  const _VideoReviewRequestDialog();

  @override
  State<_VideoReviewRequestDialog> createState() =>
      _VideoReviewRequestDialogState();
}

class _VideoReviewRequestDialogState extends State<_VideoReviewRequestDialog> {
  final _note = TextEditingController();
  bool _fileSelected = false;

  @override
  void dispose() {
    _note.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('動画の添削を依頼する'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('動画を送ると、担当コーチが添削して、このチャットに返します。'),
            const SizedBox(height: 8),
            OutlinedButton(
              onPressed: () => setState(() => _fileSelected = true),
              child: const Text('動画を選ぶ（モック）'),
            ),
            Text(
              _fileSelected
                  ? '選んだ動画: ${VideoSubmitScreen.mockFileName}'
                  : '動画が選ばれていません',
            ),
            TextField(
              controller: _note,
              decoration: const InputDecoration(labelText: 'コーチへの相談内容（任意）'),
              maxLength: 1000,
              maxLines: 3,
              minLines: 1,
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('やめる'),
        ),
        FilledButton(
          onPressed: !_fileSelected
              ? null
              : () {
                  final note = _note.text.trim();
                  Navigator.of(context).pop((note: note.isEmpty ? null : note));
                },
          child: const Text('依頼する'),
        ),
      ],
    );
  }
}

/// 添削の依頼・返却のカードの枠。本文のメッセージ（吹き出し）と見分けられるよう、幅いっぱいに枠線とアイコンと見出しを付ける
class _ReviewEventCard extends StatelessWidget {
  const _ReviewEventCard({
    required this.message,
    required this.icon,
    required this.title,
    required this.children,
  });

  final Message message;
  final IconData icon;
  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final read = readLabel(message);
    return Card.outlined(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(icon, color: theme.colorScheme.primary),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(title, style: theme.textTheme.titleMedium),
                ),
              ],
            ),
            const SizedBox(height: 8),
            ...children,
            const SizedBox(height: 4),
            Text(
              [
                messageSenderLabel(message.senderRole),
                formatDateTime(message.sentAt),
                ?read,
              ].join(' ・ '),
              style: theme.textTheme.bodySmall,
            ),
          ],
        ),
      ),
    );
  }
}

/// 添削の依頼（いつ・どの動画）。どの動画かは、このスレッドで何回目の依頼かで示す
class _ReviewRequestCard extends StatelessWidget {
  const _ReviewRequestCard({required this.message, required this.number});

  final Message message;

  /// 何回目の依頼か
  final int? number;

  @override
  Widget build(BuildContext context) => _ReviewEventCard(
    message: message,
    icon: Icons.videocam_outlined,
    title: number == null ? '添削の依頼' : '$number 回目の添削の依頼',
    children: [const Text('動画を送って、添削を依頼しました。')],
  );
}

/// 添削の返却（コーチの添削の本文と、あれば AI の一次フィードバックを別の枠で）
class _ReviewReturnedCard extends StatelessWidget {
  const _ReviewReturnedCard({
    required this.message,
    required this.number,
    required this.review,
    required this.failed,
  });

  final Message message;

  /// どの依頼への返却か（何回目の依頼か）
  final int? number;

  /// 返却された添削（まだ読めていなければ null）
  final VideoReview? review;

  /// 添削を読めなかった
  final bool failed;

  @override
  Widget build(BuildContext context) {
    final comment = review?.comment ?? '';
    final aiComment = review?.aiFeedback?.comment.trim() ?? '';
    return _ReviewEventCard(
      message: message,
      icon: Icons.rate_review_outlined,
      title: number == null ? 'コーチの添削' : 'コーチの添削（$number 回目の依頼）',
      children: [
        if (review != null)
          Text(comment)
        else if (failed)
          const Text('添削の内容を読み込めませんでした。「新しいメッセージを確かめる」で読み直せます。')
        else
          const Text('添削の内容を読み込んでいます。'),
        if (aiComment.isNotEmpty) ...[
          const SizedBox(height: 8),
          AiFeedbackCard(comment: aiComment),
        ],
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
