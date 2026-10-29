import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';

/// 動画の送付（`submitVideo`）。ファイルの選択と動画本体のアップロードは Gen1 ではモック。
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

  @override
  void dispose() {
    _note.dispose();
    super.dispose();
  }

  void _submit() {
    final note = _note.text.trim();
    ref
        .read(studentFlowProvider.notifier)
        .submitVideo(
          contentType: VideoSubmitScreen.mockContentType,
          note: note.isEmpty ? null : note,
        );
  }

  @override
  Widget build(BuildContext context) {
    final busy = ref.watch(studentFlowProvider.select((s) => s.busy));
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
        TextField(
          controller: _note,
          decoration: const InputDecoration(labelText: 'コーチへの相談内容（任意）'),
          maxLength: 1000,
          maxLines: 3,
        ),
        const SizedBox(height: 16),
        FilledButton(
          onPressed: busy || !_fileSelected ? null : _submit,
          child: const Text('送付する'),
        ),
      ],
    );
  }
}
