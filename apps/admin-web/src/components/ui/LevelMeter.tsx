// 音の大きさの表示（specs/design/components.yaml の level_meter。0.7.0）。
// 10 段のバー（各 高さ 8px・角丸 pill・あいだ xs 4px、全体の高さ 24px）。届いた段は primary、残りは primary_container、止めているときは全部 surface。
// 補間の動きを付けない（transition なし。reduced_motion でも同じ）。

export const LEVEL_METER_SEGMENTS = 10;

/**
 * level は届いた段の数（0〜10。外れた値は丸める）。stopped は許可待ち・切・使えない・ないのとき（全部 surface）。
 * label は近くの文（「音が届いています」「マイクを切っています」など）で role=status。段は読み上げない。
 * 音は端末の中で計るだけ（camera_preview の must）。書き換えは 1 秒に 10 回まで、label は 2 秒続いたら替える（どちらも計る側で間引く）。
 */
export function LevelMeter({ level, label, stopped = false }: { level: number; label: string; stopped?: boolean }) {
  const reached = stopped ? 0 : Math.min(LEVEL_METER_SEGMENTS, Math.max(0, Math.round(level)));
  return (
    <div className="flex flex-col gap-1">
      <p role="status" className="text-sm text-text">
        {label}
      </p>
      <div aria-hidden="true" data-testid="level-meter" className="flex h-6 items-center gap-1">
        {Array.from({ length: LEVEL_METER_SEGMENTS }, (_, i) => (
          <span
            key={i}
            data-reached={i < reached || undefined}
            className={`h-2 flex-1 rounded-pill ${
              stopped ? "bg-surface" : i < reached ? "bg-primary" : "bg-primary-container"
            }`}
          />
        ))}
      </div>
    </div>
  );
}
