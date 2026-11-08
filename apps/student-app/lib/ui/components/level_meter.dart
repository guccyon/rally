import 'package:flutter/material.dart';

import '../theme/rally_tokens.dart';

/// 音の大きさの表示（components.yaml の level_meter。0.7.0）。
/// 10 段のバー（各 高さ 8px・角丸 pill・あいだ xs 4px、全体の高さ 24px）。届いた段は primary、残りは primary_container、
/// 止めているとき（[stopped]。許可待ち・切・使えない・ない）は全部 surface。補間の動きを付けない（reduced_motion でも同じ）。
/// [level] は届いた段の数（0〜10。外れた値は丸める）。段は読み上げず、[label]（「音が届いています」など）を liveRegion で読む。
/// 音は端末の中で計るだけ（camera_preview の must）。書き換えは 1 秒に 10 回まで、label は 2 秒続いたら替える（どちらも計る側で間引く）。
class LevelMeter extends StatelessWidget {
  const LevelMeter({
    super.key,
    required this.level,
    required this.label,
    this.stopped = false,
  });

  static const segments = 10;

  final int level;
  final String label;
  final bool stopped;

  @override
  Widget build(BuildContext context) {
    final reached = stopped ? 0 : level.clamp(0, segments);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      spacing: RallySpacing.xs,
      children: [
        Semantics(
          liveRegion: true,
          child: Text(
            label,
            style: RallyTypography.small.copyWith(color: RallyColors.text),
          ),
        ),
        ExcludeSemantics(
          child: SizedBox(
            height: 24,
            child: Row(
              spacing: RallySpacing.xs,
              children: [
                for (var i = 0; i < segments; i++)
                  Expanded(
                    child: Container(
                      key: ValueKey('level-segment-$i'),
                      height: 8,
                      decoration: BoxDecoration(
                        color: stopped
                            ? RallyColors.surface
                            : i < reached
                            ? RallyColors.primary
                            : RallyColors.primaryContainer,
                        borderRadius: BorderRadius.circular(RallyRadius.pill),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}
