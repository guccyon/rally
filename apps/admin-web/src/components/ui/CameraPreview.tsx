import { useEffect, useRef } from "react";

import { Icon } from "./Icon";

// カメラの映りの枠（specs/design/components.yaml の camera_preview。0.7.0）。admin-web は 16:9 で幅 512px まで。
// 映像は端末の中で映すだけ。stream を録画・静止画にしない、外へ送らない（must。governance の lint の media-no-record）。

/**
 * 枠の状態。live は映っている、waiting は許可待ち（loading）、off は切った（disabled）、
 * denied は使えない・none はない（error。入室は止めない）。
 */
export type CameraPreviewState = "live" | "waiting" | "off" | "denied" | "none";

/** placeholder の文（title と small）。errors の denied・none は components.yaml の camera_preview.errors の admin-web の形。 */
const PLACEHOLDERS: Record<Exclude<CameraPreviewState, "live">, { title: string; detail: string }> = {
  waiting: { title: "カメラの許可を待っています", detail: "ブラウザーの確認で「許可」を選ぶと、ここに映ります" },
  off: { title: "カメラを切っています", detail: "入室したあとも切ったままです" },
  denied: {
    title: "カメラが使えません",
    detail: "ブラウザーの設定で許可すると確かめられます。入室は開始時刻からできます。",
  },
  none: { title: "カメラが見つかりません", detail: "カメラがなくても、開始時刻から入室できます。" },
};

/**
 * カメラを開く・止めるのは呼ぶ側（枠を出しているあいだだけ開き、画面を離れたら止める）。ここは渡された stream を映すだけ。
 * 映っているときは role=img と「自分のカメラの映り（左右反転）」。placeholder は文として読み、変わったら role=status で 1 回読む。
 * 枠は inverse_surface の 1 か所に数える（同じ画面にほかの濃い面を置かない）。
 */
export function CameraPreview({
  state,
  stream,
  caption,
}: {
  state: CameraPreviewState;
  stream?: MediaStream | null;
  caption?: string;
}) {
  const video = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    if (video.current) video.current.srcObject = state === "live" ? (stream ?? null) : null;
  }, [state, stream]);

  const placeholder = state === "live" ? null : PLACEHOLDERS[state];
  // 許可待ちは濃い面のまま、切・使えない・ないは surface の面に text の文と videocam_off
  const dark = state === "live" || state === "waiting";
  return (
    <div className="flex w-full max-w-128 flex-col gap-2">
      <div
        role={state === "live" ? "img" : undefined}
        aria-label={state === "live" ? "自分のカメラの映り（左右反転）" : undefined}
        className={`relative flex aspect-video w-full flex-col items-center justify-center gap-2 overflow-hidden rounded-md px-4 text-center ${
          dark ? "bg-inverse-surface text-on-inverse-surface" : "bg-surface text-text"
        }`}
      >
        {state === "live" ? (
          <video
            ref={video}
            autoPlay
            muted
            playsInline
            aria-hidden="true"
            className="absolute inset-0 size-full -scale-x-100 object-cover"
          />
        ) : (
          <div role="status" className="flex flex-col items-center gap-2">
            {state !== "waiting" && <Icon name="videocam_off" size={24} />}
            <p className="text-xl font-black">{placeholder?.title}</p>
            <p className={`text-sm ${dark ? "text-on-inverse-surface-muted" : "text-text-muted"}`}>
              {placeholder?.detail}
            </p>
          </div>
        )}
      </div>
      {caption && <p className="text-sm text-text-muted">{caption}</p>}
    </div>
  );
}
