import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/prejoin_check.dart';
import '../state/student_flow.dart';
import '../ui/components/confirm_dialog.dart';
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
            inProgress
                ? 'コーチと通話中です\n映像が表示されなくても、故障ではありません'
                : '「通話を開始する」を押すと、コーチと通話できます',
            textAlign: TextAlign.center,
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

/// ビデオ通話の最中にブラウザーや端末の「戻る」を押したときの確認（router/student_route_delegate.dart が出す）。
/// 黙って通話の画面を離れず、「通話を終了する」なら通話を終えて完了の記録へ進み、「通話を続ける」ならそのままにする
Future<void> confirmEndingCall(
  BuildContext context,
  StudentFlowNotifier notifier,
) async {
  final confirmed = await showConfirmDialog(
    context,
    title: '通話を終了しますか？',
    body: 'コーチとのビデオ通話を終了し、レッスンの完了の記録へ進みます。終了した通話には戻れません。',
    confirmLabel: '通話を終了する',
    cancelLabel: '通話を続ける',
    danger: true,
  );
  if (confirmed) await notifier.endCall();
}
