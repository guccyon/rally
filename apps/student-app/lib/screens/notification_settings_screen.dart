import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/models.dart';
import '../state/student_flow.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';

/// 通知の受け取り設定（`getMyNotificationSettings` / `updateMyNotificationSettings`）。
/// 種類ごとのスイッチで、切り替えたらすぐ保存し、保存に失敗したら元に戻して知らせる。
class NotificationSettingsScreen extends ConsumerWidget {
  const NotificationSettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(studentFlowProvider);
    final notifier = ref.read(studentFlowProvider.notifier);
    final settings = state.notificationSettings;
    return FlowScaffold(
      title: '通知の設定',
      leading: IconButton(
        tooltip: '通知の設定を閉じる',
        icon: const Icon(Icons.close),
        onPressed: state.busy ? null : notifier.closeNotificationSettings,
      ),
      children: [
        const Text('受け取るプッシュ通知を選べます。切り替えるとすぐに保存します。'),
        const Text('オフにしている間に届くはずだった通知は、あとでオンに戻しても届きません。'),
        const SizedBox(height: 8),
        if (settings != null)
          for (final kind in NotificationKind.values)
            SwitchListTile(
              title: Text(notificationKindLabel(kind)),
              value: settings.receives(kind),
              onChanged: state.busy
                  ? null
                  : (receives) => notifier.setNotification(kind, receives),
              contentPadding: EdgeInsets.zero,
            )
        else if (!state.busy)
          OutlinedButton(
            onPressed: notifier.refreshNotificationSettings,
            child: const Text('設定を読み込み直す'),
          ),
      ],
    );
  }
}
