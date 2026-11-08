import { useState } from "react";

import { Button } from "./Button";
import { Icon } from "./Icon";
import type { IconName } from "./icons/paths";

// 入・切の切り替えボタン（specs/design/components.yaml の media_toggle。0.7.0）。
// 面は button の secondary のまま変えず、入・切は文言とアイコンを入れ替えて示す。aria-pressed は付けない（文言が操作を言う）。

/** 切り替えの種類（components.yaml の media_toggle.variants）。 */
export type MediaToggleKind = "mic" | "camera" | "speaker_test";

type Face = { label: string; icon: IconName; result: string };

/**
 * 種類 × 今の状態（on: マイク・カメラは入、試し音は鳴らしている）ごとの文言・アイコン・読み上げる結果。
 * 文言は content.yaml の actions.verbs（mic_off・mic_on・camera_off・camera_on・test_speaker・stop_test_sound）。
 */
const FACES: Record<MediaToggleKind, { on: Face; off: Face }> = {
  mic: {
    on: { label: "マイクを切る", icon: "mic", result: "マイクを入れました" },
    off: { label: "マイクを入れる", icon: "mic_off", result: "マイクを切りました" },
  },
  camera: {
    on: { label: "カメラを切る", icon: "videocam", result: "カメラを入れました" },
    off: { label: "カメラを入れる", icon: "videocam_off", result: "カメラを切りました" },
  },
  speaker_test: {
    on: { label: "試し音を止める", icon: "stop_circle", result: "試し音を鳴らしています" },
    off: { label: "スピーカーを試す", icon: "volume_up", result: "試し音を止めました" },
  },
};

/** 種類と今の状態に合うボタンの文言（画面のテストとキャプチャの一覧で使う）。 */
export function mediaToggleLabel(kind: MediaToggleKind, on: boolean): string {
  return FACES[kind][on ? "on" : "off"].label;
}

/**
 * 押したら確認せずに onToggle を呼ぶ（入・切を変えるのは呼ぶ側）。on が変わったら結果を role=status で 1 回読む。
 * loading は許可を求めているあいだ（spinner を前に出し押せない）、disabled は端末にない・許可がないとき（理由は近くの文で）。
 * 切り替えに失敗したら、呼ぶ側が on を元に戻して notice の inline（danger）を出す。
 * 2 つ並べるときは親を `flex gap-2` にし、className に `flex-1` を渡して幅を等分する。
 */
export function MediaToggle({
  kind,
  on,
  onToggle,
  loading = false,
  disabled = false,
  className,
}: {
  kind: MediaToggleKind;
  on: boolean;
  onToggle: () => void;
  loading?: boolean;
  disabled?: boolean;
  className?: string;
}) {
  const face = FACES[kind][on ? "on" : "off"];
  // 押したあとだけ結果を読む（画面を開いたときの状態は読まない）
  const [pressed, setPressed] = useState(false);
  return (
    <>
      <Button
        variant="secondary"
        loading={loading}
        disabled={disabled}
        className={className}
        onClick={() => {
          setPressed(true);
          onToggle();
        }}
      >
        {!loading && <Icon name={face.icon} size={20} />}
        {face.label}
      </Button>
      <span role="status" className="sr-only">
        {pressed ? face.result : ""}
      </span>
    </>
  );
}
