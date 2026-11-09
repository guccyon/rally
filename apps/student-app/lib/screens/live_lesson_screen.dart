import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/prejoin_check.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/theme/rally_tokens.dart';

/// 通話への参加（`startLiveLessonSession` / `endLiveLessonSession`）。
/// 通話基盤は Gen1 ではモックなので、映像の代わりに枠だけを出す。
/// 入室前チェックから入ったときは、そこで決めたカメラ・マイクの入・切を枠の下に 1 行出す（端末のカメラ・マイクは入室で止めている）。
class LiveLessonScreen extends ConsumerWidget {
  const LiveLessonScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final session = state.session;
    final inProgress = session?.status == LiveLessonSessionStatus.inProgress;
    final carried = ref.watch(prejoinCheckProvider.select((s) => s.carried));
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
        if (carried != null) ...[
          const SizedBox(height: RallySpacing.sm),
          Semantics(
            liveRegion: true,
            child: Text(
              'カメラ: ${carried.camera ? '入' : '切'}・'
              'マイク: ${carried.mic ? '入' : '切'}（入室前チェックのまま）',
              style: RallyTypography.small.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
        ],
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
