"use client";

import { type FormEvent, useState } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import { ResultMessage } from "@/components/ResultMessage";
import { lengthOf } from "@/components/coach/lessonMenuInput";
import { SPORT_LABELS, announcementTargetLabel } from "@/components/labels";

import {
  type AnnouncementInput,
  type AnnouncementInputErrors,
  BODY_MAX_LENGTH,
  EMPTY_INPUT,
  TITLE_MAX_LENGTH,
  draftTitleOf,
  targetSportOf,
  toAnnouncementRequest,
  validateAnnouncement,
  validateAnnouncementDraft,
} from "./announcementInput";

type Announcement = Schemas["Announcement"];
type AnnouncementDraft = Schemas["AnnouncementDraft"];

const SPORTS = Object.keys(SPORT_LABELS) as Schemas["Sport"][];

/** 対象の生徒がいないとき（契約の 404 `not_found`）の文言。汎用の「ID を確かめてください」では意味が通らないので、この画面で決める。 */
const NO_RECIPIENTS = "対象の生徒がいないため、お知らせは送られませんでした。対象を確かめてください。";

function Remaining({ max, length }: { max: number; length: number }) {
  const remaining = max - length;
  return (
    <span className={remaining < 0 ? "text-sm text-red-600" : "text-sm text-gray-500"}>
      {remaining < 0 ? `${max} 文字を ${-remaining} 文字超えています` : `あと ${remaining} 文字`}
    </span>
  );
}

function FieldError({ id, message }: { id: string; message?: string }) {
  if (!message) return null;
  return (
    <p id={id} className="text-sm text-red-600">
      {message}
    </p>
  );
}

type Props = {
  send: (request: Schemas["SendAnnouncementRequest"]) => Promise<Result<Announcement>>;
  onSent: (announcement: Announcement) => void;
  /** 「下書きに保存」で呼ぶ（作るか直すかは呼び出し側が決める）。なければボタンを出さない。 */
  saveDraft?: (request: Schemas["AnnouncementDraftRequest"]) => Promise<Result<AnnouncementDraft>>;
  /** 最初の入力（開いた下書きの中身）。なければ空。 */
  initial?: AnnouncementInput;
  /** 開いたらすぐに確かめに進む（下書きの一覧の「送る」）。入力に問題があれば項目の下に理由を出して入力に留まる。 */
  reviewOnOpen?: boolean;
};

/**
 * 本部の「お知らせを送る」。対象（生徒全員かスポーツ）・題・本文を書き、「内容を確かめる」でページの中の確認に進み
 * （ブラウザの confirm は使わない）、「送るのを確定」で送る（「直す」で入力に戻る）。サーバーが断る入力（題 1〜50 文字・
 * 本文 1〜2000 文字で、どちらも空白だけは不可）は確認に進まずに項目の下へ理由を出す。送れたら入力を空にして `onSent`。
 * 送れなければ入力を残して文言を出す。「下書きに保存」（`saveDraft`）は長すぎる入力だけを断り（空でも保存できる）、
 * 保存しても入力は残す。
 */
export function AnnouncementForm({ send, onSent, saveDraft, initial = EMPTY_INPUT, reviewOnOpen = false }: Props) {
  const [input, setInput] = useState<AnnouncementInput>(initial);
  const [errors, setErrors] = useState<AnnouncementInputErrors>(() =>
    reviewOnOpen ? validateAnnouncement(initial) : {},
  );
  const [confirming, setConfirming] = useState(
    () => reviewOnOpen && Object.keys(validateAnnouncement(initial)).length === 0,
  );
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);

  function update(patch: Partial<AnnouncementInput>) {
    setInput((current) => ({ ...current, ...patch }));
  }

  function review(event: FormEvent) {
    event.preventDefault();
    const found = validateAnnouncement(input);
    setErrors(found);
    if (Object.keys(found).length > 0) return;
    setResult(null);
    setConfirming(true);
  }

  async function save() {
    if (!saveDraft) return;
    const found = validateAnnouncementDraft(input);
    setErrors(found);
    if (Object.keys(found).length > 0) return;
    setBusy(true);
    const response = await saveDraft(toAnnouncementRequest(input));
    setBusy(false);
    setResult(
      response.ok
        ? {
            ok: true,
            message: `下書き「${draftTitleOf(response.data)}」を保存しました。生徒にはまだ届いていません。`,
          }
        : { ok: false, message: response.message },
    );
  }

  async function confirm() {
    setBusy(true);
    const response = await send(toAnnouncementRequest(input));
    setBusy(false);
    setConfirming(false);
    if (response.ok) {
      setInput(EMPTY_INPUT);
      setResult({
        ok: true,
        message: `お知らせ「${response.data.title}」を ${response.data.recipient_count} 人の生徒に送りました。`,
      });
      onSent(response.data);
    } else {
      setResult({
        ok: false,
        message: response.code === "not_found" ? NO_RECIPIENTS : response.message,
      });
    }
  }

  if (confirming) {
    const target = announcementTargetLabel(targetSportOf(input));
    return (
      <div role="group" aria-label="送る前の確認" className="space-y-3">
        <p className="text-sm">
          次の内容で<strong>{target}</strong>
          に送りますか？送るとすぐに生徒に届き、プッシュ通知は取り消せません。
        </p>
        <dl className="space-y-2 rounded border bg-gray-50 p-3 text-sm">
          <div>
            <dt className="text-gray-600">対象</dt>
            <dd>{target}</dd>
          </div>
          <div>
            <dt className="text-gray-600">題</dt>
            <dd>{input.title}</dd>
          </div>
          <div>
            <dt className="text-gray-600">本文</dt>
            <dd className="whitespace-pre-wrap">{input.body}</dd>
          </div>
        </dl>
        <div className="flex gap-2">
          <button
            type="button"
            disabled={busy}
            className="rounded bg-blue-600 px-4 py-1 text-white disabled:opacity-50"
            onClick={() => void confirm()}
          >
            {busy ? "送っています…" : "送るのを確定"}
          </button>
          <button
            type="button"
            disabled={busy}
            className="rounded border px-4 py-1 disabled:opacity-50"
            onClick={() => setConfirming(false)}
          >
            直す
          </button>
        </div>
      </div>
    );
  }

  return (
    <form onSubmit={review} noValidate className="space-y-3">
      <ResultMessage result={result} />
      <label className="block">
        対象
        <select
          className="mt-1 block rounded border px-2 py-1"
          value={input.sport}
          onChange={(event) => update({ sport: event.target.value as Schemas["Sport"] | "" })}
        >
          <option value="">生徒全員（ゴルフの生徒も含む）</option>
          {SPORTS.map((value) => (
            <option key={value} value={value}>
              {announcementTargetLabel(value)}
            </option>
          ))}
        </select>
      </label>
      <div>
        <label className="block">
          題
          <input
            type="text"
            className="mt-1 block w-full rounded border px-2 py-1"
            value={input.title}
            aria-invalid={errors.title ? true : undefined}
            aria-describedby={errors.title ? "announcement-title-error" : undefined}
            onChange={(event) => update({ title: event.target.value })}
          />
        </label>
        <Remaining max={TITLE_MAX_LENGTH} length={lengthOf(input.title)} />
        <FieldError id="announcement-title-error" message={errors.title} />
      </div>
      <div>
        <label className="block">
          本文
          <textarea
            className="mt-1 block w-full rounded border px-2 py-1"
            rows={6}
            value={input.body}
            aria-invalid={errors.body ? true : undefined}
            aria-describedby={errors.body ? "announcement-body-error" : undefined}
            onChange={(event) => update({ body: event.target.value })}
          />
        </label>
        <Remaining max={BODY_MAX_LENGTH} length={lengthOf(input.body)} />
        <FieldError id="announcement-body-error" message={errors.body} />
      </div>
      <p className="text-xs text-gray-600">
        お知らせは対象の生徒全員のお知らせの一覧に届きます。プッシュ通知は、お知らせの通知を受け取る設定にしている生徒にだけ送ります。
      </p>
      <div className="flex gap-2">
        <button type="submit" disabled={busy} className="rounded bg-blue-600 px-4 py-1 text-white disabled:opacity-50">
          内容を確かめる
        </button>
        {saveDraft && (
          <button
            type="button"
            disabled={busy}
            className="rounded border px-4 py-1 disabled:opacity-50"
            onClick={() => void save()}
          >
            {busy ? "保存しています…" : "下書きに保存"}
          </button>
        )}
      </div>
      {saveDraft && (
        <p className="text-xs text-gray-600">
          下書きは生徒に届きません。送るまでは本部の画面にだけ残り、下の「下書き」から開いて直す・送る・消すができます。
        </p>
      )}
    </form>
  );
}
