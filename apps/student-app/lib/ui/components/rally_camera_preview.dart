import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 枠の状態。
enum CameraPreviewState {
  /// 映っている（default）
  live,

  /// 許可待ち（loading）
  waiting,

  /// 切った（disabled）
  off,

  /// 使えない（error。入室は止めない）
  denied,

  /// ない（error。入室は止めない）
  none,
}

/// placeholder の文（title と small）。denied・none は components.yaml の camera_preview.errors の形。
const _placeholders = <CameraPreviewState, ({String title, String detail})>{
  CameraPreviewState.waiting: (
    title: 'カメラの許可を待っています',
    detail: '端末の確認で「許可」を選ぶと、ここに映ります',
  ),
  CameraPreviewState.off: (title: 'カメラを切っています', detail: '入室したあとも切ったままです'),
  CameraPreviewState.denied: (
    title: 'カメラが使えません',
    detail: '端末の設定で許可すると確かめられます。入室は開始時刻からできます。',
  ),
  CameraPreviewState.none: (
    title: 'カメラが見つかりません',
    detail: 'カメラがなくても、開始時刻から入室できます。',
  ),
};

/// カメラの映りの枠（components.yaml の camera_preview。0.7.0）。student-app は 4:3。
/// カメラを開く・止めるのは呼ぶ側（枠を出しているあいだだけ開き、画面を離れたら止める）で、映像は [video] に渡す。
/// 映像は端末の中で映すだけ。録画・静止画にしない、外へ送らない（must。governance の lint の media-no-record）。
/// 映っているときは「自分のカメラの映り（左右反転）」の画像として読み、placeholder は文として読む（変わったら liveRegion で 1 回）。
/// 枠は inverse_surface の 1 か所に数える（同じ画面に score_panel やほかの濃い面を置かない）。
class RallyCameraPreview extends StatelessWidget {
  const RallyCameraPreview({
    super.key,
    required this.state,
    this.video,
    this.caption,
  });

  final CameraPreviewState state;

  /// 映像（カメラの preview の widget）。左右反転し、枠いっぱいに切り抜いて（cover）映す
  final Widget? video;

  /// 枠の下の small text_muted の文（任意）
  final String? caption;

  @override
  Widget build(BuildContext context) {
    final placeholder = _placeholders[state];
    // 許可待ちは濃い面のまま、切・使えない・ないは surface の面に text の文と videocam_off
    final dark =
        state == CameraPreviewState.live ||
        state == CameraPreviewState.waiting;
    final frame = AspectRatio(
      aspectRatio: 4 / 3,
      child: ClipRRect(
        borderRadius: BorderRadius.circular(RallyRadius.md),
        child: ColoredBox(
          color: dark ? RallyColors.inverseSurface : RallyColors.surface,
          child: placeholder == null
              ? Semantics(
                  image: true,
                  label: '自分のカメラの映り（左右反転）',
                  child: SizedBox.expand(
                    child: video == null
                        ? null
                        : ExcludeSemantics(
                            child: Transform.flip(
                              flipX: true,
                              child: FittedBox(
                                fit: BoxFit.cover,
                                clipBehavior: Clip.hardEdge,
                                child: video,
                              ),
                            ),
                          ),
                  ),
                )
              : Semantics(
                  liveRegion: true,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: RallySpacing.md,
                    ),
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      spacing: RallySpacing.sm,
                      children: [
                        if (state != CameraPreviewState.waiting)
                          const ExcludeSemantics(
                            child: Icon(
                              Icons.videocam_off_rounded,
                              size: 24,
                              color: RallyColors.text,
                            ),
                          ),
                        Text(
                          placeholder.title,
                          textAlign: TextAlign.center,
                          style: RallyTypography.title.copyWith(
                            color: dark
                                ? RallyColors.onInverseSurface
                                : RallyColors.text,
                          ),
                        ),
                        Text(
                          placeholder.detail,
                          textAlign: TextAlign.center,
                          style: RallyTypography.small.copyWith(
                            color: dark
                                ? RallyColors.onInverseSurfaceMuted
                                : RallyColors.textMuted,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
        ),
      ),
    );
    final caption = this.caption;
    if (caption == null) return frame;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      spacing: RallySpacing.sm,
      children: [
        frame,
        Text(
          caption,
          style: RallyTypography.small.copyWith(color: RallyColors.textMuted),
        ),
      ],
    );
  }
}
