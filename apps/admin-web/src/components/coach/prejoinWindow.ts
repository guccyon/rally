import { useEffect, useState } from "react";

import { formatTime, toJstDate } from "@/components/labels";

/**
 * 入室前チェック（glossary の prejoin_check）を開ける、スロットの開始の何分前か。admin-web はこの 1 つだけを参照する。
 * 入室（通話セッションの開始）は開始時刻ちょうどから（#899・#921 の slot_not_started）。環境ごとに変えない。
 */
export const PREJOIN_CHECK_LEAD_MINUTES = 5;

/** 入室前チェックを開けるようになる時刻（epoch ミリ秒）。 */
export function prejoinCheckOpensAt(startsAt: string): number {
  return new Date(startsAt).getTime() - PREJOIN_CHECK_LEAD_MINUTES * 60_000;
}

/** 今（epoch ミリ秒）入室前チェックを開けるか（開始 5 分前を過ぎたか）。 */
export function canOpenPrejoinCheck(startsAt: string, now: number): boolean {
  return now >= prejoinCheckOpensAt(startsAt);
}

/** 開始時刻までの残りの秒（開始時刻を過ぎたら 0 以下）。 */
export function secondsUntilStart(startsAt: string, now: number): number {
  return (new Date(startsAt).getTime() - now) / 1000;
}

const WEEKDAYS = ["日", "月", "火", "水", "木", "金", "土"];

/**
 * 入室前チェックを開ける時刻の表示。今日（日本時間）なら「14:55」、ほかの日なら「11/9（月）14:55」。
 */
export function formatPrejoinOpensAt(startsAt: string, now: number): string {
  const opensAt = prejoinCheckOpensAt(startsAt);
  const time = formatTime(new Date(opensAt).toISOString());
  const day = toJstDate(opensAt);
  if (day === toJstDate(now)) return time;
  const [year, month, date] = day.split("-").map(Number) as [number, number, number];
  const weekday = WEEKDAYS[new Date(Date.UTC(year, month - 1, date)).getUTCDay()];
  return `${month}/${date}（${weekday}）${time}`;
}

/**
 * 今の時刻（epoch ミリ秒）を 1 秒ごとに書き換える。画面を開いたまま開始 5 分前・開始時刻をまたいだら、読み込み直さずに切り替えるのに使う。
 */
export function useNow(intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(timer);
  }, [intervalMs]);
  return now;
}
