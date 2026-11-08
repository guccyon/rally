import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../device/local_media.dart';
import '../state/prejoin_check.dart';
import '../state/student_flow.dart';
import '../ui/components/join_bar.dart';
import '../ui/components/level_meter.dart';
import '../ui/components/media_toggle.dart';
import '../ui/components/notice.dart';
import '../ui/components/rally_button.dart';
import '../ui/components/rally_camera_preview.dart';
import '../ui/components/rally_card.dart';
import '../ui/flow_scaffold.dart';
import '../ui/labels.dart';
import '../ui/theme/rally_tokens.dart';
import '../ui/ticking_now.dart';

/// 入室前チェック（specs/ux/issue-1040.yaml の studentPrejoinCheck）。開始 5 分前から開け、端末の本物のカメラの映り（左右反転）・
/// マイクの音の大きさ・スピーカーの試し音を確かめ、開始時刻から「入室する」で `startLiveLessonSession` を呼ぶ。
/// 許可を断った・端末にないときは知らせるだけで、入室は止めない。画面を離れる・アプリが裏に回るとカメラとマイクを止める。
/// API を読まない（予約は予約の確定から渡す）。映像と音は端末の外に出さない。
class PrejoinCheckScreen extends ConsumerStatefulWidget {
  const PrejoinCheckScreen({super.key});

  @override
  ConsumerState<PrejoinCheckScreen> createState() => _PrejoinCheckScreenState();
}

class _PrejoinCheckScreenState extends ConsumerState<PrejoinCheckScreen> {
  late final PrejoinCheckNotifier _notifier;
  late final AppLifecycleListener _lifecycle;

  @override
  void initState() {
    super.initState();
    _notifier = ref.read(prejoinCheckProvider.notifier);
    _lifecycle = AppLifecycleListener(
      onHide: _notifier.pause,
      onShow: _notifier.start,
    );
    // 画面を描いてから、端末にカメラとマイクの許可を求める（provider を描く途中で変えない）
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _notifier.start();
    });
  }

  @override
  void dispose() {
    _lifecycle.dispose();
    _notifier.release();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final flow = ref.watch(studentFlowProvider);
    final check = ref.watch(prejoinCheckProvider);
    final media = ref.watch(localMediaProvider);
    final booking = flow.booking!;
    final slot = flow.bookedSlot;
    final menu = booking.lessonMenu;
    return FlowScaffold(
      title: '入室前チェック',
      showNavigation: false,
      showError: false,
      leading: IconButton(
        tooltip: '予約に戻る',
        icon: const Icon(Icons.arrow_back),
        onPressed: _notifier.leave,
      ),
      bottomBar: TickingNow(
        builder: (context, now) {
          final remaining = booking.startsAt.difference(now);
          final before = !canJoinLiveLesson(booking.startsAt, now);
          return JoinBar(
            startTime: formatClock(booking.startsAt),
            remaining: before ? remaining : Duration.zero,
            loading: flow.busy,
            error: flow.errorMessage,
            reason: flow.busy
                ? '入室しています…'
                : before
                ? null
                : '開始時刻になりました。入室すると、コーチとのビデオ通話が始まります。',
            onJoin: _notifier.join,
          );
        },
      ),
      children: [
        RallyCard(
          children: [
            Text(
              slot == null
                  ? formatDateTime(booking.startsAt)
                  : formatTimeRange(slot.startsAt, slot.endsAt),
              style: RallyTypography.figureSm,
            ),
            if (slot != null)
              Text(
                [
                  sportLabel(slot.sport),
                  if (slot.coachDisplayName case final name?) 'コーチ: $name',
                ].join('・'),
                style: RallyTypography.small.copyWith(
                  color: RallyColors.textMuted,
                ),
              ),
            if (menu != null)
              Text(
                bookingLessonMenuLabel(menu),
                style: RallyTypography.small.copyWith(
                  color: RallyColors.textMuted,
                ),
              ),
          ],
        ),
        const SizedBox(height: RallySpacing.md),
        if (_deviceNotice(check) case final notice?) ...[
          Notice(message: notice, status: RallyStatus.warning),
          const SizedBox(height: RallySpacing.sm),
          if (_denied(check) && media.canOpenSettings)
            RallyButton(
              label: '端末の設定を開く',
              variant: RallyButtonVariant.secondary,
              onPressed: _notifier.openSettings,
            )
          else
            RallyButton(
              label: 'もう一度確かめる',
              variant: RallyButtonVariant.secondary,
              onPressed: _notifier.recheck,
            ),
          const SizedBox(height: RallySpacing.md),
        ],
        RallyCameraPreview(
          state: _previewState(check),
          video: check.cameraReady && check.cameraOn
              ? media.cameraView()
              : null,
        ),
        if (check.cameraReady || check.micReady) ...[
          const SizedBox(height: RallySpacing.sm),
          Row(
            spacing: RallySpacing.sm,
            children: [
              if (check.micReady)
                Expanded(
                  child: MediaToggle(
                    kind: MediaToggleKind.mic,
                    on: check.micOn,
                    onToggle: _notifier.toggleMic,
                  ),
                ),
              if (check.cameraReady)
                Expanded(
                  child: MediaToggle(
                    kind: MediaToggleKind.camera,
                    on: check.cameraOn,
                    onToggle: _notifier.toggleCamera,
                  ),
                ),
            ],
          ),
        ],
        const SizedBox(height: RallySpacing.lg),
        const _SectionTitle('マイク'),
        LevelMeter(
          level: check.micLevel,
          label: _micLabel(check),
          stopped: !(check.micReady && check.micOn),
        ),
        if (check.micReady && check.micOn)
          Text(
            '声を出すとバーが動きます。',
            style: RallyTypography.small.copyWith(color: RallyColors.textMuted),
          ),
        const SizedBox(height: RallySpacing.lg),
        const _SectionTitle('スピーカー'),
        MediaToggle(
          kind: MediaToggleKind.speakerTest,
          on: check.tonePlaying,
          onToggle: _notifier.toggleTestTone,
        ),
        const SizedBox(height: RallySpacing.sm),
        if (check.toneFailed)
          Semantics(
            liveRegion: true,
            child: Text(
              '試し音を鳴らせませんでした。端末の音量と、音の出る先を確かめてください。',
              style: RallyTypography.small.copyWith(
                color: RallyColors.danger.fg,
              ),
            ),
          )
        else
          Text(
            check.tonePlaying
                ? '試し音を鳴らしています。聞こえないときは端末の音量を上げてください。'
                : 'イヤホンをつないでいれば、イヤホンから鳴ります。',
            style: RallyTypography.small.copyWith(color: RallyColors.textMuted),
          ),
      ],
    );
  }
}

class _SectionTitle extends StatelessWidget {
  const _SectionTitle(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: RallySpacing.sm),
    child: Semantics(
      header: true,
      child: Text(text, style: RallyTypography.title),
    ),
  );
}

CameraPreviewState _previewState(PrejoinCheckState check) =>
    switch (check.camera) {
      null => CameraPreviewState.waiting,
      MediaDeviceStatus.denied => CameraPreviewState.denied,
      MediaDeviceStatus.missing => CameraPreviewState.none,
      MediaDeviceStatus.ready =>
        check.cameraOn ? CameraPreviewState.live : CameraPreviewState.off,
    };

bool _denied(PrejoinCheckState check) =>
    check.camera == MediaDeviceStatus.denied ||
    check.mic == MediaDeviceStatus.denied;

/// 端末の知らせ（許可を断られた・ないときだけ）。断ったものとないものを分けて書き、入室は止めないことを添える
String? _deviceNotice(PrejoinCheckState check) {
  bool denied(MediaDeviceStatus? s) => s == MediaDeviceStatus.denied;
  bool missing(MediaDeviceStatus? s) => s == MediaDeviceStatus.missing;
  final lines = <String>[
    if (denied(check.camera) && denied(check.mic))
      'カメラとマイクが使えません。端末の設定で許可すると確かめられます。'
    else if (denied(check.camera))
      'カメラが使えません。端末の設定で許可すると確かめられます。'
    else if (denied(check.mic))
      'マイクが使えません。端末の設定で許可すると確かめられます。',
    if (missing(check.camera) && missing(check.mic))
      'この端末にカメラとマイクが見つかりません。スピーカーは確かめられます。'
    else if (missing(check.camera))
      'この端末にカメラが見つかりません。マイクとスピーカーは確かめられます。'
    else if (missing(check.mic))
      'この端末にマイクが見つかりません。カメラとスピーカーは確かめられます。',
  ];
  if (lines.isEmpty) return null;
  // 届かないものは断ったものに合わせて言う（片方だけ断った人に、事実と違う案内を出さない）
  final notReached = denied(check.camera) && denied(check.mic)
      ? '映像と声'
      : denied(check.camera)
      ? '映像'
      : '声';
  return [
    ...lines,
    if (_denied(check))
      '許可しなくても、開始時刻から入室できます。そのときコーチには、あなたの$notReachedが届きません。'
    else
      'なくても、開始時刻から入室できます。',
  ].join('\n');
}

String _micLabel(PrejoinCheckState check) => switch (check.mic) {
  null => 'マイクの許可を待っています',
  MediaDeviceStatus.denied => 'マイクが使えないため、音の大きさを出せません。端末の設定で許可すると確かめられます。',
  MediaDeviceStatus.missing => 'マイクが見つからないため、音の大きさを出せません。',
  MediaDeviceStatus.ready =>
    !check.micOn
        ? 'マイクを切っています'
        : check.micHeard
        ? '音が届いています'
        : 'まだ音が届いていません',
};
