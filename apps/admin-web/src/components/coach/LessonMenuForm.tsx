"use client";

import { type FormEvent, useState } from "react";

import type { Schemas } from "@/api/client";
import { LESSON_TYPE_LABELS, SPORT_LABELS } from "@/components/labels";

import {
  DESCRIPTION_MAX_LENGTH,
  DURATIONS,
  hasErrors,
  type InputErrors,
  type LessonMenuInput,
  lengthOf,
  NAME_MAX_LENGTH,
  validateInput,
} from "./lessonMenuInput";

const SPORTS = Object.keys(SPORT_LABELS) as Schemas["Sport"][];
const LESSON_TYPES = Object.keys(LESSON_TYPE_LABELS) as Schemas["LessonType"][];

function Remaining({ max, length }: { max: number; length: number }) {
  const remaining = max - length;
  return (
    <span className={remaining < 0 ? "text-sm text-danger" : "text-sm text-text-muted"}>
      {remaining < 0 ? `${max} 文字を ${-remaining} 文字超えています` : `あと ${remaining} 文字`}
    </span>
  );
}

function FieldError({ id, message }: { id: string; message?: string }) {
  if (!message) return null;
  return (
    <p id={id} className="text-sm text-danger">
      {message}
    </p>
  );
}

/**
 * レッスンメニューの作成・編集のフォーム。サーバーが断る入力（名前 1〜40 文字で空白だけは不可・説明 0〜400 文字・
 * 同期の時間は 15〜120 分の 15 分刻み）は送らずに項目の下へ理由を出す。送るのは `onSubmit` で、成否の表示は呼ぶ側。
 */
export function LessonMenuForm({
  initial,
  submitLabel,
  busy,
  onSubmit,
  onCancel,
}: {
  initial: LessonMenuInput;
  submitLabel: string;
  busy: boolean;
  onSubmit: (input: LessonMenuInput) => void;
  onCancel?: () => void;
}) {
  const [input, setInput] = useState<LessonMenuInput>(initial);
  const [errors, setErrors] = useState<InputErrors>({});

  function update(patch: Partial<LessonMenuInput>) {
    setInput((current) => ({ ...current, ...patch }));
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const found = validateInput(input);
    setErrors(found);
    if (hasErrors(found)) return;
    onSubmit(input);
  }

  return (
    <form onSubmit={handleSubmit} noValidate className="space-y-3">
      <div className="flex flex-wrap items-end gap-3">
        <label className="flex flex-col">
          スポーツ
          <select
            className="rounded-md border px-2 py-1"
            value={input.sport}
            onChange={(event) => update({ sport: event.target.value as Schemas["Sport"] })}
          >
            {SPORTS.map((value) => (
              <option key={value} value={value}>
                {SPORT_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col">
          レッスンの種類
          <select
            className="rounded-md border px-2 py-1"
            value={input.lessonType}
            onChange={(event) =>
              update({ lessonType: event.target.value as Schemas["LessonType"] })
            }
          >
            {LESSON_TYPES.map((value) => (
              <option key={value} value={value}>
                {LESSON_TYPE_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        {input.lessonType === "live_lesson" && (
          <label className="flex flex-col">
            1 回の時間
            <select
              className="rounded-md border px-2 py-1"
              value={input.durationMinutes}
              aria-invalid={errors.durationMinutes ? true : undefined}
              aria-describedby={errors.durationMinutes ? "lesson-menu-duration-error" : undefined}
              onChange={(event) => update({ durationMinutes: Number(event.target.value) })}
            >
              {DURATIONS.map((value) => (
                <option key={value} value={value}>
                  {value} 分
                </option>
              ))}
            </select>
          </label>
        )}
      </div>
      <p className="text-xs text-text-muted">
        スポーツは、あなたが提供しているスポーツから選んでください（提供していないスポーツのメニューは登録できません）。
      </p>
      <FieldError id="lesson-menu-duration-error" message={errors.durationMinutes} />
      <div>
        <label className="block">
          名前
          <input
            type="text"
            className="mt-1 block w-full rounded-md border px-2 py-1"
            value={input.name}
            aria-invalid={errors.name ? true : undefined}
            aria-describedby={errors.name ? "lesson-menu-name-error" : undefined}
            onChange={(event) => update({ name: event.target.value })}
          />
        </label>
        <Remaining max={NAME_MAX_LENGTH} length={lengthOf(input.name)} />
        <FieldError id="lesson-menu-name-error" message={errors.name} />
      </div>
      <div>
        <label className="block">
          説明（任意）
          <textarea
            className="mt-1 block w-full rounded-md border px-2 py-1"
            rows={4}
            value={input.description}
            aria-invalid={errors.description ? true : undefined}
            aria-describedby={errors.description ? "lesson-menu-description-error" : undefined}
            onChange={(event) => update({ description: event.target.value })}
          />
        </label>
        <Remaining max={DESCRIPTION_MAX_LENGTH} length={lengthOf(input.description)} />
        <FieldError id="lesson-menu-description-error" message={errors.description} />
      </div>
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={input.isPublished}
          onChange={(event) => update({ isPublished: event.target.checked })}
        />
        公開する（公開中のメニューだけが生徒に見えます）
      </label>
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={busy}
          className="rounded-pill bg-primary px-4 py-1 text-on-primary disabled:opacity-50"
        >
          {submitLabel}
        </button>
        {onCancel && (
          <button type="button" onClick={onCancel} className="rounded-pill border px-4 py-1">
            やめる
          </button>
        )}
      </div>
    </form>
  );
}
