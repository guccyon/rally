import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/ai_feedback_card.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/lesson_menu_picker.dart';
import 'video_submit_screen.dart';

/// 担当コーチとのメッセージ（`listMessages` / `sendMessage`）。開いたら相手のメッセージは既読にする。
/// 自分が送ったメッセージには、コーチが読んだか（既読・未読）を出す。
/// 動画の添削の依頼（`submitVideo`）もここからでき、添削の依頼と返却は本文のメッセージと見分けられるカードで時間の順に並べる。
/// 入力欄のそばの「スタンプ」から 8 種類のスタンプを選んで送れ、スタンプは本文より大きな絵文字で吹き出しなしに出す。
/// 知らない種類のメッセージ（と、このアプリの版が知らないスタンプ）は隠さず、`body`（短い説明）を本文のメッセージとして出す。
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

  /// スタンプを選んで送る。選ばずに閉じたら何もしない
  Future<void> _sendStamp() async {
    final stamp = await showModalBottomSheet<StampId>(
      context: context,
      builder: (_) => const _StampPicker(),
    );
    if (stamp == null || !mounted) return;
    await ref.read(studentFlowProvider.notifier).sendStamp(stamp);
  }

  /// 動画を選んで添削を依頼する（動画の選択はモック）。担当コーチの非同期のメニューを読んで選ばせる。やめたら何もしない。
  /// メニューを付けて断られたら、選んだ動画と相談内容はそのままで、読み直したメニューを選び直させる
  Future<void> _requestVideoReview() async {
    final notifier = ref.read(studentFlowProvider.notifier);
    final coachId = ref.read(studentFlowProvider).thread!.coachId;
    String? note;
    var fileSelected = false;
    var menuRejected = false;
    while (true) {
      if (!mounted) return;
      if (!menuRejected) unawaited(notifier.loadCoachLessonMenus(coachId));
      final request = await showDialog<_VideoReviewRequest>(
        context: context,
        builder: (_) => _VideoReviewRequestDialog(
          coachId: coachId,
          initialNote: note,
          fileSelected: fileSelected,
          menuRejected: menuRejected,
        ),
      );
      if (request == null || !mounted) return;
      final result = await notifier.requestVideoReview(
        contentType: VideoSubmitScreen.mockContentType,
        note: request.note,
        lessonMenuId: request.lessonMenuId,
      );
      if (result != VideoReviewRequestResult.menuRejected) return;
      note = request.note;
      fileSelected = true;
      menuRejected = true;
    }
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
            // 知らない ID のスタンプは、知らない種類と同じく body の説明を出す
            MessageKind.stamp when message.stampId != null => _StampTile(
              message: message,
              stamp: message.stampId!,
            ),
            MessageKind.text ||
            MessageKind.stamp ||
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
          onPressed: state.busy ? null : _sendStamp,
          icon: const Icon(Icons.emoji_emotions_outlined),
          label: const Text('スタンプ'),
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

/// スタンプの選択（8 種類を絵文字と短い言葉で並べる）。押したスタンプを返す
class _StampPicker extends StatelessWidget {
  const _StampPicker();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('送るスタンプを選んでください', style: theme.textTheme.titleMedium),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final stamp in StampId.values)
                  SizedBox(
                    width: 96,
                    child: OutlinedButton(
                      onPressed: () => Navigator.of(context).pop(stamp),
                      style: OutlinedButton.styleFrom(
                        padding: const EdgeInsets.symmetric(vertical: 8),
                      ),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            stampEmoji(stamp),
                            style: const TextStyle(fontSize: 28),
                          ),
                          Text(stampLabel(stamp), textAlign: TextAlign.center),
                        ],
                      ),
                    ),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// 添削の依頼の中身（相談内容は空なら null、メニューは「指定しない」なら null）
typedef _VideoReviewRequest = ({String? note, String? lessonMenuId});

/// 添削の依頼に添える動画の選択（モック）と、担当コーチの非同期のメニューの選択と、相談内容。依頼するならその中身を返す。
/// メニューは「指定しない」（既定）と、コーチの公開中の非同期のメニューのうち生徒のスポーツのもの（名前と説明）から選ぶ。
/// メニューがなければ選択の欄を出さない
class _VideoReviewRequestDialog extends ConsumerStatefulWidget {
  const _VideoReviewRequestDialog({
    required this.coachId,
    required this.initialNote,
    required this.fileSelected,
    required this.menuRejected,
  });

  final String coachId;

  /// 断られたあとに開き直したときの相談内容と、動画を選んだか
  final String? initialNote;
  final bool fileSelected;

  /// 直前の依頼が、選んだメニューを付けて断られた
  final bool menuRejected;

  @override
  ConsumerState<_VideoReviewRequestDialog> createState() =>
      _VideoReviewRequestDialogState();
}

class _VideoReviewRequestDialogState
    extends ConsumerState<_VideoReviewRequestDialog> {
  late final _note = TextEditingController(text: widget.initialNote);
  late bool _fileSelected = widget.fileSelected;

  /// 選んだメニュー（null は「指定しない」）
  String? _lessonMenuId;

  @override
  void dispose() {
    _note.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(studentFlowProvider);
    final sport = state.student!.sport;
    final lessonMenuId = LessonMenuPicker.selectionOf(
      state,
      coachId: widget.coachId,
      sport: sport,
      selected: _lessonMenuId,
    );
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
            if (widget.menuRejected) ...[
              const SizedBox(height: 8),
              const LessonMenuRejectedNotice(),
            ],
            LessonMenuPicker(
              coachId: widget.coachId,
              sport: sport,
              selected: lessonMenuId,
              onChanged: (id) => setState(() => _lessonMenuId = id),
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
                  Navigator.of(context).pop((
                    note: note.isEmpty ? null : note,
                    lessonMenuId: lessonMenuId,
                  ));
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

/// 添削の返却（依頼で選んだメニューがあればその名前と、コーチの添削の本文と、あれば AI の一次フィードバックを別の枠で）
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
        if (review?.lessonMenu case final menu?) ...[
          Text(videoLessonMenuLabel(menu)),
          const SizedBox(height: 4),
        ],
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

/// スタンプ。本文のメッセージより大きな絵文字に短い言葉を添え、吹き出し（カード）なしで送り手の側に寄せる
class _StampTile extends StatelessWidget {
  const _StampTile({required this.message, required this.stamp});

  final Message message;
  final StampId stamp;

  /// 絵文字の大きさ（本文の文字よりはっきり大きく）
  static const emojiSize = 56.0;

  @override
  Widget build(BuildContext context) {
    final mine = message.senderRole == MessageSenderRole.student;
    final read = readLabel(message);
    return Align(
      alignment: mine ? Alignment.centerRight : Alignment.centerLeft,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 8),
        child: Column(
          crossAxisAlignment: mine
              ? CrossAxisAlignment.end
              : CrossAxisAlignment.start,
          children: [
            Text(
              stampEmoji(stamp),
              style: const TextStyle(fontSize: emojiSize),
            ),
            Text(stampLabel(stamp)),
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
    );
  }
}
