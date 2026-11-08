import { Button } from "./Button";
import { countdownAnnouncement, formatCountdown } from "./countdown";
import { Notice } from "./Notice";

// 入室の帯（specs/design/components.yaml の join_bar。0.7.0）。画面の下に固定する操作の帯（z_order の sticky_bar）。
// 面は background・上に border の 1px・上下 md 16px・影なし。中身は 768px（content_width_px の reading）の幅にそろえる。

/**
 * 開始時刻の前（remainingSeconds > 0）は「入室する」を無効にし、理由と残り時間（1 時間未満だけ「あと 2:48」）を出す（押してから断らない）。
 * 開始時刻になったら開き直さずに押せるようにし、「開始時刻になりました。入室できます」を role=status で 1 回読む。
 * remainingSeconds はサーバーの時刻に合わせた時計で数える（呼ぶ側）。
 * 入室に失敗したら error にサーバーの文言を渡す（ボタンの上に notice の inline（danger）。ボタンは押せるまま）。
 * 帯は本文の下に重なるので、画面は本文の下に帯の高さの余白をとる。
 */
export function JoinBar({
  startTime,
  remainingSeconds,
  meta,
  reason,
  loading = false,
  error,
  onJoin,
  secondaryAction,
}: {
  /** 開始時刻（「12:00」） */
  startTime: string;
  remainingSeconds: number;
  /** 時刻の行に添える文（「田中 美咲・フォアハンドの基礎（30 分）」） */
  meta?: string;
  /** 理由の文。省くと開始の前は「開始時刻になると入室できます」、あとは引き継ぎの文 */
  reason?: string;
  loading?: boolean;
  error?: string;
  onJoin: () => void;
  /** 左に置く secondary のボタン（1 つまで） */
  secondaryAction?: { label: string; onClick: () => void };
}) {
  const before = remainingSeconds > 0;
  const countdown = before ? formatCountdown(remainingSeconds) : null;
  const reasonText =
    reason ?? (before ? "開始時刻になると入室できます" : "カメラとマイクの入・切は、そのまま通話に引き継ぎます");
  return (
    <div role="region" aria-label="入室" className="sticky bottom-0 z-10 border-t border-border bg-background py-4">
      <div className="mx-auto flex max-w-3xl flex-col gap-2 px-6">
        <div className="flex flex-wrap items-baseline gap-2">
          <span className="text-sm font-bold">開始</span>
          <span className="font-numeric text-figure-sm font-bold tabular-nums">{startTime}</span>
          {meta && <span className="text-sm text-text-muted">{meta}</span>}
          {countdown && (
            <span aria-hidden="true" className="ml-auto">
              <span className="text-sm font-bold">あと</span>{" "}
              <span className="font-numeric text-figure-sm font-bold tabular-nums">{countdown}</span>
            </span>
          )}
        </div>
        <p className="text-sm text-text-muted">{reasonText}</p>
        <span role="status" className="sr-only">
          {countdownAnnouncement(remainingSeconds)}
        </span>
        {error && <Notice message={error} status="danger" />}
        <div className="flex flex-wrap justify-end gap-2">
          {secondaryAction && (
            <Button variant="secondary" onClick={secondaryAction.onClick}>
              {secondaryAction.label}
            </Button>
          )}
          <Button disabled={before} loading={loading} onClick={onJoin}>
            入室する
          </Button>
        </div>
      </div>
    </div>
  );
}
