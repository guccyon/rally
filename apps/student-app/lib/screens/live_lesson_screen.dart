import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';

/// 通話への参加（`startLiveLessonSession` / `endLiveLessonSession`）。
/// 通話基盤は Gen1 ではモックなので、映像の代わりに枠だけを出す。
class LiveLessonScreen extends ConsumerWidget {
  const LiveLessonScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final session = state.session;
    final inProgress = session?.status == LiveLessonSessionStatus.inProgress;
    return FlowScaffold(
      title: 'ビデオ通話',
      children: [
        Container(
          height: 180,
          color: Colors.black87,
          alignment: Alignment.center,
          child: Text(
            inProgress ? '通話中（映像はモック）' : '映像はモックです',
            style: const TextStyle(color: Colors.white),
          ),
        ),
        const SizedBox(height: 16),
        if (session == null)
          FilledButton(
            onPressed: state.busy ? null : notifier.startCall,
            child: const Text('通話を開始する'),
          )
        else if (inProgress)
          FilledButton(
            onPressed: state.busy ? null : notifier.endCall,
            child: const Text('通話を終了する'),
          ),
      ],
    );
  }
}
