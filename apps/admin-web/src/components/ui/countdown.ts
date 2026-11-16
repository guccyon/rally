import { useState } from "react";

// 開始までの残り時間の表記（specs/design/content.yaml の notation.datetime.countdown。0.7.0）。
// 残りの秒はサーバーの時刻に合わせた時計で数える（端末の時計を使わない）。数えるのは呼ぶ側。

/** 「あと 2:48」の「2:48」（分:秒、秒は 2 桁）。1 時間未満の残りだけで、それ以外（0 以下・1 時間以上）は null。 */
export function formatCountdown(remainingSeconds: number): string | null {
  const total = Math.ceil(remainingSeconds);
  if (total <= 0 || total >= 3600) return null;
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

/** 読み上げの文。「開始まで あと 2 分 48 秒」。0 以下は「開始時刻になりました。入室できます」。0 の分・秒は省く。 */
export function countdownReadAloud(remainingSeconds: number): string {
  const total = Math.ceil(remainingSeconds);
  if (total <= 0) return "開始時刻になりました。入室できます";
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  const parts = [minutes > 0 ? `${minutes} 分` : null, seconds > 0 ? `${seconds} 秒` : null].filter(Boolean);
  return `開始まで あと ${parts.join(" ")}`;
}

/** 分の区切り（秒が 0 になる単位）の番号。1 分ごとの読み上げが変わるかを見るのに使う。 */
function minuteIndex(remainingSeconds: number): number {
  return Math.ceil(Math.max(0, Math.ceil(remainingSeconds)) / 60);
}

/**
 * role=status に置く文を、1 分ごと（分が変わったとき）と 0 になったときだけ新しくする。
 * 読む文は大きな表示と同じ残りを指す（切り上げた分にしない。開いたとき 2:22 なら「あと 2 分 22 秒」、次は 2:00 のとき「あと 2 分」）。
 * 1 時間以上は読まない（空）。
 */
export function useCountdownAnnouncement(remainingSeconds: number): string {
  const [last, setLast] = useState(() => ({ minute: minuteIndex(remainingSeconds), text: announce(remainingSeconds) }));
  const minute = minuteIndex(remainingSeconds);
  if (minute !== last.minute) {
    const next = { minute, text: announce(remainingSeconds) };
    setLast(next);
    return next.text;
  }
  return last.text;
}

function announce(remainingSeconds: number): string {
  return Math.ceil(remainingSeconds) >= 3600 ? "" : countdownReadAloud(remainingSeconds);
}
