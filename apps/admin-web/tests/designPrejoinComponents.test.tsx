// 入室前チェックの部品（specs/design/components.yaml の media_toggle・camera_preview・level_meter・join_bar。0.7.0）と
// 残り時間の表記（content.yaml の notation.datetime.countdown）。
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { useState } from "react";
import { expect, test, vi } from "vitest";

import { CameraPreview } from "@/components/ui/CameraPreview";
import { countdownAnnouncement, countdownReadAloud, formatCountdown } from "@/components/ui/countdown";
import { JoinBar } from "@/components/ui/JoinBar";
import { LevelMeter } from "@/components/ui/LevelMeter";
import { MediaToggle, mediaToggleLabel, type MediaToggleKind } from "@/components/ui/MediaToggle";

function Toggle({ kind, initial }: { kind: MediaToggleKind; initial: boolean }) {
  const [on, setOn] = useState(initial);
  return <MediaToggle kind={kind} on={on} onToggle={() => setOn((v) => !v)} />;
}

test("入・切の切り替えは押すとその場で文言とアイコンを入れ替え、面は secondary のまま、結果を role=status で読む", async () => {
  render(<Toggle kind="mic" initial />);
  const before = screen.getByRole("button", { name: "マイクを切る" });
  expect(before).toHaveClass("bg-primary-container", "rounded-md", "min-h-12");
  expect(before).not.toHaveAttribute("aria-pressed");
  expect(before.querySelector('svg[data-icon="mic"]')).toHaveAttribute("aria-hidden", "true");
  // 開いたときの状態は読まない
  expect(screen.getByRole("status")).toHaveTextContent("");

  await userEvent.click(before);
  const after = screen.getByRole("button", { name: "マイクを入れる" });
  expect(after).toHaveClass("bg-primary-container");
  expect(after.querySelector('svg[data-icon="mic_off"]')).not.toBeNull();
  expect(screen.getByRole("status")).toHaveTextContent("マイクを切りました");
});

test.each([
  ["mic", true, "マイクを切る"],
  ["mic", false, "マイクを入れる"],
  ["camera", true, "カメラを切る"],
  ["camera", false, "カメラを入れる"],
  ["speaker_test", true, "試し音を止める"],
  ["speaker_test", false, "スピーカーを試す"],
] as const)("%s の %s の文言は content.yaml の動詞（%s）", (kind, on, label) => {
  expect(mediaToggleLabel(kind, on)).toBe(label);
});

test("許可を求めているあいだは spinner を前に出して押せず、ない・許可がないときは無効", () => {
  const onToggle = vi.fn();
  render(
    <>
      <MediaToggle kind="camera" on onToggle={onToggle} loading />
      <MediaToggle kind="mic" on={false} onToggle={onToggle} disabled />
    </>,
  );
  const loading = screen.getByRole("button", { name: "カメラを切る" });
  expect(loading).toBeDisabled();
  expect(loading).toHaveAttribute("aria-busy", "true");
  expect(screen.getByTestId("spinner")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "マイクを入れる" })).toBeDisabled();
});

test("カメラの映りは role=img と左右反転のラベルで、映像は object-fit cover で左右反転、16:9 で幅 512px まで", () => {
  const { container } = render(<CameraPreview state="live" caption="映像はこの端末の中だけで映し、送りも残しもしません" />);
  const frame = screen.getByRole("img", { name: "自分のカメラの映り（左右反転）" });
  expect(frame).toHaveClass("aspect-video", "rounded-md", "overflow-hidden", "bg-inverse-surface");
  expect(frame.parentElement).toHaveClass("max-w-128");
  const video = container.querySelector("video");
  expect(video).toHaveClass("object-cover", "-scale-x-100");
  expect(video).toHaveProperty("muted", true);
  expect(screen.getByText("映像はこの端末の中だけで映し、送りも残しもしません")).toHaveClass("text-text-muted");
});

test("許可待ちは濃い面に文、切・使えない・ないは surface の面に videocam_off と、何が起きたかとどうすればよいか", () => {
  const { container, rerender } = render(<CameraPreview state="waiting" />);
  expect(screen.getByRole("status")).toHaveTextContent("カメラの許可を待っています");
  expect(container.querySelector(".bg-inverse-surface")).not.toBeNull();
  expect(container.querySelector('svg[data-icon="videocam_off"]')).toBeNull();

  rerender(<CameraPreview state="denied" />);
  expect(screen.getByRole("status")).toHaveTextContent(
    "カメラが使えませんブラウザーの設定で許可すると確かめられます。入室は開始時刻からできます。",
  );
  expect(container.querySelector(".bg-surface")).not.toBeNull();
  expect(container.querySelector('svg[data-icon="videocam_off"]')).toHaveAttribute("width", "24");
  expect(screen.queryByRole("img")).toBeNull();

  rerender(<CameraPreview state="none" />);
  expect(screen.getByRole("status")).toHaveTextContent("カメラがなくても、開始時刻から入室できます。");
});

test("音の大きさは 10 段で届いた段まで primary、止めているときは全部 surface。段は読まず、文を role=status で読む", () => {
  const { rerender } = render(<LevelMeter level={6} label="音が届いています" />);
  const meter = screen.getByTestId("level-meter");
  expect(meter).toHaveAttribute("aria-hidden", "true");
  expect(meter).toHaveClass("h-6", "gap-1");
  const segments = [...meter.children];
  expect(segments).toHaveLength(10);
  expect(segments.filter((s) => s.classList.contains("bg-primary"))).toHaveLength(6);
  expect(segments.filter((s) => s.classList.contains("bg-primary-container"))).toHaveLength(4);
  for (const segment of segments) {
    expect(segment).toHaveClass("h-2", "rounded-pill");
    expect(segment.className).not.toMatch(/transition/);
  }
  expect(screen.getByRole("status")).toHaveTextContent("音が届いています");

  rerender(<LevelMeter level={6} label="マイクを切っています" stopped />);
  expect([...screen.getByTestId("level-meter").children].every((s) => s.classList.contains("bg-surface"))).toBe(true);
});

test("残り時間は 1 時間未満だけ「分:秒」で、読み上げは分と秒の文", () => {
  expect(formatCountdown(168)).toBe("2:48");
  expect(formatCountdown(65)).toBe("1:05");
  expect(formatCountdown(3599)).toBe("59:59");
  expect(formatCountdown(3600)).toBeNull();
  expect(formatCountdown(0)).toBeNull();
  expect(countdownReadAloud(168)).toBe("開始まで あと 2 分 48 秒");
  expect(countdownReadAloud(120)).toBe("開始まで あと 2 分");
  expect(countdownReadAloud(30)).toBe("開始まで あと 30 秒");
  expect(countdownReadAloud(0)).toBe("開始時刻になりました。入室できます");
});

test("読み上げの文は 1 分ごとと 0 になったときだけ変わる", () => {
  expect(countdownAnnouncement(180)).toBe("開始まで あと 3 分");
  expect(countdownAnnouncement(168)).toBe("開始まで あと 3 分");
  expect(countdownAnnouncement(121)).toBe("開始まで あと 3 分");
  expect(countdownAnnouncement(120)).toBe("開始まで あと 2 分");
  expect(countdownAnnouncement(0)).toBe("開始時刻になりました。入室できます");
  expect(countdownAnnouncement(7200)).toBe("");
});

test("入室の帯は開始時刻の前は「入室する」を無効にし、理由と残り時間を出す。開始時刻になったら開き直さずに押せる", async () => {
  const onJoin = vi.fn();
  const { rerender } = render(<JoinBar startTime="12:00" remainingSeconds={168} onJoin={onJoin} />);
  const region = screen.getByRole("region", { name: "入室" });
  expect(region).toHaveClass("sticky", "bottom-0", "bg-background", "border-t", "border-border");
  expect(region.className).not.toMatch(/shadow/);
  expect(screen.getByRole("button", { name: "入室する" })).toBeDisabled();
  expect(screen.getByText("開始時刻になると入室できます")).toHaveClass("text-text-muted");
  expect(screen.getByText("2:48")).toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent("開始まで あと 3 分");

  rerender(<JoinBar startTime="12:00" remainingSeconds={0} onJoin={onJoin} />);
  expect(screen.queryByText("2:48")).toBeNull();
  expect(screen.getByRole("status")).toHaveTextContent("開始時刻になりました。入室できます");
  expect(screen.getByText("カメラとマイクの入・切は、そのまま通話に引き継ぎます")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "入室する" }));
  expect(onJoin).toHaveBeenCalledTimes(1);
});

test("入室に失敗したらボタンの上に失敗の知らせを出し、ボタンは押せるまま。左に secondary を 1 つ置ける", () => {
  render(
    <JoinBar
      startTime="12:00"
      remainingSeconds={0}
      error="この予約の開始時刻になっていないため、ビデオ通話を始められません。開始時刻になってから、もう一度始めてください。"
      onJoin={vi.fn()}
      secondaryAction={{ label: "予約の一覧に戻る", onClick: vi.fn() }}
    />,
  );
  expect(screen.getByRole("alert")).toHaveTextContent("ビデオ通話を始められません");
  expect(screen.getByRole("button", { name: "入室する" })).toBeEnabled();
  const buttons = screen.getAllByRole("button");
  expect(buttons.map((b) => b.textContent)).toEqual(["予約の一覧に戻る", "入室する"]);
  expect(buttons[0]).toHaveClass("bg-primary-container");
});

// 部品は見た目だけを持ち、映像（stream）と音の大きさ（level）は呼ぶ側から受け取る（#1041）。
// カメラ・マイク・スピーカーを開く・計るのは入室前チェックの画面の側で、components/ui には書かない。
test("部品はカメラ・マイクを開かず、音を計らない（映像と音の大きさは呼ぶ側から受け取る）", () => {
  const dir = resolve(process.cwd(), "src/components/ui");
  const offenders = readdirSync(dir)
    .filter((name) => /\.tsx?$/.test(name))
    .filter((name) =>
      /\b(mediaDevices|getUserMedia|getDisplayMedia|enumerateDevices|AudioContext|AnalyserNode|setSinkId)\b/.test(
        readFileSync(join(dir, name), "utf8"),
      ),
    );
  expect(offenders).toEqual([]);
});
