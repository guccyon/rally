"use client";

import { useState } from "react";

import type { Schemas } from "@/api/client";
import { announcementTargetLabel, formatDateTime } from "@/components/labels";
import { Button } from "@/components/ui/Button";

import { draftTitleOf } from "./announcementInput";

type AnnouncementDraft = Schemas["AnnouncementDraft"];

/** 行の「消す」。押したら行の中で確かめ（ブラウザの confirm は使わない）、「消すのを確定」で `onDelete`。 */
function DeleteDraftAction({ onDelete }: { onDelete: () => Promise<void> }) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  async function confirm() {
    setBusy(true);
    await onDelete();
    setBusy(false);
    setConfirming(false);
  }

  if (!confirming) {
    return (
      <Button variant="secondary" onClick={() => setConfirming(true)}>
        消す
      </Button>
    );
  }
  return (
    <div role="group" aria-label="下書きを消す前の確認" className="space-y-1">
      <p className="text-xs">この下書きを消しますか？消した下書きは戻せません。</p>
      <div className="flex gap-2">
        <Button variant="danger" disabled={busy} onClick={() => void confirm()}>
          消すのを確定
        </Button>
        <Button variant="secondary" disabled={busy} onClick={() => setConfirming(false)}>
          やめる
        </Button>
      </div>
    </div>
  );
}

type Props = {
  drafts: AnnouncementDraft[];
  /** フォームで開いている下書き（なければ null） */
  openedId: string | null;
  onOpen: (draft: AnnouncementDraft) => void;
  onSend: (draft: AnnouncementDraft) => void;
  onDelete: (draft: AnnouncementDraft) => Promise<void>;
};

/**
 * お知らせの下書きの一覧。並びは渡された順（API の順＝最後に直した時刻の新しい順）のまま、題（空なら「（題なし）」）・
 * 対象・最後に直した日時を出し、行から開いて直す・送る・消す。0 件の表示は呼び出し側。
 */
export function AnnouncementDraftList({ drafts, openedId, onOpen, onSend, onDelete }: Props) {
  return (
    <ul aria-label="下書きの一覧" className="space-y-3">
      {drafts.map((draft) => (
        <li key={draft.announcement_draft_id} className="rounded-md border p-3">
          <p className="font-semibold">
            {draftTitleOf(draft)}
            {draft.announcement_draft_id === openedId && (
              <span className="ml-2 text-xs font-normal text-primary">（開いています）</span>
            )}
          </p>
          <p className="text-xs text-text-muted">
            {announcementTargetLabel(draft.sport)}・最後に直した日時 {formatDateTime(draft.updated_at)}
          </p>
          <div className="mt-2 flex flex-wrap items-start gap-2">
            <Button variant="secondary" onClick={() => onOpen(draft)}>
              開いて直す
            </Button>
            <Button variant="secondary" onClick={() => onSend(draft)}>
              送る
            </Button>
            <DeleteDraftAction onDelete={() => onDelete(draft)} />
          </div>
        </li>
      ))}
    </ul>
  );
}
