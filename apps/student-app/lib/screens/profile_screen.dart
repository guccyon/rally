import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 自分のプロフィール（`getMyStudentProfile` / `updateMyStudentProfile`）。表示名を見て変え、「保存する」で保存する。
/// 表示名の規則はサーバーが判定し、断られたら（422）サーバーの文言を出す。スポーツは見るだけ（変えられない）。
class ProfileScreen extends ConsumerStatefulWidget {
  const ProfileScreen({super.key});

  @override
  ConsumerState<ProfileScreen> createState() => _ProfileScreenState();
}

class _ProfileScreenState extends ConsumerState<ProfileScreen> {
  late final TextEditingController _displayName;

  /// 直前の「保存する」で保存できたか（入力を変えたら消す）
  bool _saved = false;

  /// 「保存する」を押してから応答が返るまで true
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _displayName = TextEditingController(
      text: ref.read(studentFlowProvider).student?.displayName ?? '',
    );
  }

  @override
  void dispose() {
    _displayName.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    setState(() {
      _saved = false;
      _saving = true;
    });
    final saved = await ref
        .read(studentFlowProvider.notifier)
        .saveDisplayName(_displayName.text);
    if (!mounted) return;
    setState(() {
      _saved = saved;
      _saving = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    // 読み直した表示名は、入力を変えていなければ入力欄にも写す
    ref.listen(studentFlowProvider.select((s) => s.student?.displayName), (
      previous,
      next,
    ) {
      if (next != null && _displayName.text == (previous ?? '')) {
        _displayName.text = next;
      }
    });
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final student = state.student;
    final unchanged = _displayName.text == (student?.displayName ?? '');
    return FlowScaffold(
      title: 'プロフィール',
      leading: IconButton(
        tooltip: 'プロフィールを閉じる',
        icon: const Icon(Icons.close),
        onPressed: state.busy ? null : notifier.closeProfile,
      ),
      children: [
        const Text('表示名は担当コーチにも表示されます。'),
        const SizedBox(height: 8),
        TextField(
          controller: _displayName,
          decoration: const InputDecoration(labelText: '表示名'),
          enabled: student != null,
          onChanged: (_) => setState(() => _saved = false),
        ),
        if (student != null) ...[
          const SizedBox(height: 8),
          Text('スポーツ: ${sportLabel(student.sport)}'),
        ],
        const SizedBox(height: 16),
        FilledButton(
          onPressed: state.busy || student == null || unchanged ? null : _save,
          child: Text(_saving ? '保存しています…' : '保存する'),
        ),
        if (_saved) ...[const SizedBox(height: 8), const Text('表示名を保存しました。')],
      ],
    );
  }
}
