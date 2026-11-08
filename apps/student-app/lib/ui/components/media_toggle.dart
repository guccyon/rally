import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';

import 'rally_button.dart';

/// 切り替えの種類（components.yaml の media_toggle.variants）。
enum MediaToggleKind {
  mic,
  camera,

  /// 試し音の「スピーカーを試す」⇄「試し音を止める」
  speakerTest,
}

/// 種類 × 今の状態ごとの文言・アイコン・読み上げる結果。文言は content.yaml の actions.verbs。
typedef _Face = ({String label, IconData icon, String result});

const _faces = <MediaToggleKind, ({_Face on, _Face off})>{
  MediaToggleKind.mic: (
    on: (label: 'マイクを切る', icon: Icons.mic_rounded, result: 'マイクを入れました'),
    off: (label: 'マイクを入れる', icon: Icons.mic_off_rounded, result: 'マイクを切りました'),
  ),
  MediaToggleKind.camera: (
    on: (label: 'カメラを切る', icon: Icons.videocam_rounded, result: 'カメラを入れました'),
    off: (
      label: 'カメラを入れる',
      icon: Icons.videocam_off_rounded,
      result: 'カメラを切りました',
    ),
  ),
  MediaToggleKind.speakerTest: (
    on: (
      label: '試し音を止める',
      icon: Icons.stop_circle_rounded,
      result: '試し音を鳴らしています',
    ),
    off: (
      label: 'スピーカーを試す',
      icon: Icons.volume_up_rounded,
      result: '試し音を止めました',
    ),
  ),
};

/// 種類と今の状態に合うボタンの文言（画面のテストとキャプチャの一覧で使う）。
String mediaToggleLabel(MediaToggleKind kind, {required bool on}) =>
    (on ? _faces[kind]!.on : _faces[kind]!.off).label;

/// 入・切の切り替えボタン（components.yaml の media_toggle。0.7.0）。button の secondary に leading_icon。
/// 面の色は入でも切でも変えず、入・切は文言とアイコンを入れ替えて示す（toggled の印は付けない。文言が操作を言う）。
/// [on] はマイク・カメラなら入、試し音なら鳴らしているとき。押したら確認せずに [onToggle] を呼び（入・切を変えるのは呼ぶ側）、
/// [on] が変わったら結果を読み上げで 1 回伝える（「マイクを切りました」）。
/// [loading] は許可を求めているあいだ（spinner を前に出し押せない）、[disabled] は端末にない・許可がないとき（理由は近くの文で）。
/// 切り替えに失敗したら、呼ぶ側が [on] を元に戻して notice の inline（danger）を出す。
/// 2 つ並べるときは Row（spacing は sm）の中で Expanded にして幅を等分する。
class MediaToggle extends StatefulWidget {
  const MediaToggle({
    super.key,
    required this.kind,
    required this.on,
    required this.onToggle,
    this.loading = false,
    this.disabled = false,
  });

  final MediaToggleKind kind;
  final bool on;
  final VoidCallback onToggle;
  final bool loading;
  final bool disabled;

  @override
  State<MediaToggle> createState() => _MediaToggleState();
}

class _MediaToggleState extends State<MediaToggle> {
  /// 押したあとだけ結果を読む（画面を開いたときの状態は読まない）
  var _pressed = false;

  _Face get _face =>
      widget.on ? _faces[widget.kind]!.on : _faces[widget.kind]!.off;

  @override
  void didUpdateWidget(MediaToggle oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_pressed && oldWidget.on != widget.on) {
      SemanticsService.sendAnnouncement(
        View.of(context),
        _face.result,
        Directionality.of(context),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: double.infinity,
      child: RallyButton(
        label: _face.label,
        icon: _face.icon,
        variant: RallyButtonVariant.secondary,
        loading: widget.loading,
        onPressed: widget.disabled
            ? null
            : () {
                _pressed = true;
                widget.onToggle();
              },
      ),
    );
  }
}
