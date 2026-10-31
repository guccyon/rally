import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/lesson_menu_picker.dart';

/// 動画の送付（`submitVideo`）。ファイルの選択と動画本体のアップロードは Gen1 ではモック。
/// チャットの添削の依頼と同じく、体験予約のコーチの公開中の非同期のメニューと「指定しない」（既定）から選べる。
class VideoSubmitScreen extends ConsumerStatefulWidget {
  const VideoSubmitScreen({super.key});

  /// モックで選んだことにするファイル（チャットの添削の依頼でも使う）
  static const mockFileName = 'swing.mp4';
  static const mockContentType = VideoContentType.mp4;

  @override
  ConsumerState<VideoSubmitScreen> createState() => _VideoSubmitScreenState();
}

class _VideoSubmitScreenState extends ConsumerState<VideoSubmitScreen> {
  final _note = TextEditingController();
  bool _fileSelected = false;

  /// 選んだメニュー（null は「指定しない」）
  String? _lessonMenuId;

  /// 直前の送付が、選んだメニューを付けて断られた
  bool _menuRejected = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      ref
          .read(studentFlowProvider.notifier)
          .loadCoachLessonMenus(ref.read(studentFlowProvider).booking!.coachId);
    });
  }

  @override
  void dispose() {
    _note.dispose();
    super.dispose();
  }

  /// 送付する。メニューを付けて断られたら、選んだ動画と相談内容はそのままで、読み直したメニューを選び直させる
  Future<void> _submit(String? lessonMenuId) async {
    final note = _note.text.trim();
    final result = await ref
        .read(studentFlowProvider.notifier)
        .submitVideo(
          contentType: VideoSubmitScreen.mockContentType,
          note: note.isEmpty ? null : note,
          lessonMenuId: lessonMenuId,
        );
    if (!mounted) return;
    setState(
      () => _menuRejected = result == VideoReviewRequestResult.menuRejected,
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(studentFlowProvider);
    final coachId = state.booking!.coachId;
    final sport = state.bookedSlot!.sport;
    final lessonMenuId = LessonMenuPicker.selectionOf(
      state,
      coachId: coachId,
      sport: sport,
      selected: _lessonMenuId,
    );
    return FlowScaffold(
      title: '動画を送る',
      children: [
        OutlinedButton(
          onPressed: () => setState(() => _fileSelected = true),
          child: const Text('動画を選ぶ（モック）'),
        ),
        Text(
          _fileSelected
              ? '選んだ動画: ${VideoSubmitScreen.mockFileName}'
              : '動画が選ばれていません',
        ),
        if (_menuRejected) ...[
          const SizedBox(height: 8),
          const LessonMenuRejectedNotice(),
        ],
        LessonMenuPicker(
          coachId: coachId,
          sport: sport,
          selected: lessonMenuId,
          onChanged: (id) => setState(() => _lessonMenuId = id),
        ),
        TextField(
          controller: _note,
          decoration: const InputDecoration(labelText: 'コーチへの相談内容（任意）'),
          maxLength: 1000,
          maxLines: 3,
        ),
        const SizedBox(height: 16),
        FilledButton(
          onPressed: state.busy || !_fileSelected
              ? null
              : () => _submit(lessonMenuId),
          child: const Text('送付する'),
        ),
      ],
    );
  }
}
