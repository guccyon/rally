"use client";

import { useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { formatDate } from "@/components/labels";
import { Button } from "@/components/ui/Button";

type Student = Schemas["Student"];

type Students = { kind: "idle" } | { kind: "loading" } | { kind: "loaded"; students: Student[] } | { kind: "failed" };

/** 候補に並べる人数の上限（多すぎるときは入力を足して絞ってもらう）。 */
const MAX_CANDIDATES = 20;

/**
 * 予約の一覧の「生徒」の絞り込み。生徒の ID を知らなくても、表示名の一部を入れて候補から選べる。
 * 候補は生徒の一覧（listStudents）を最初に入力したときに 1 回読み、表示名の部分一致で絞る。
 * 同じ表示名の生徒を見分けられるよう、候補には登録日を添える。選んだ生徒は「外す」で外せる。
 */
export function BookingStudentPicker({
  api,
  selected,
  onSelect,
}: {
  api: Pick<HeadquartersApi, "listStudents">;
  selected: { studentId: string; name: string } | null;
  onSelect: (student: { studentId: string; name: string } | null) => void;
}) {
  const [text, setText] = useState("");
  const [students, setStudents] = useState<Students>({ kind: "idle" });
  const requested = useRef(false);

  function change(value: string) {
    setText(value);
    if (requested.current) return;
    requested.current = true;
    setStudents({ kind: "loading" });
    void api.listStudents().then((response) => {
      setStudents(response.ok ? { kind: "loaded", students: response.data.students } : { kind: "failed" });
      if (!response.ok) requested.current = false;
    });
  }

  const query = text.trim();
  const matches =
    students.kind === "loaded" && query !== ""
      ? students.students.filter((student) => student.display_name.includes(query))
      : [];

  return (
    <div className="space-y-2">
      <label className="block">
        生徒の名前
        <input
          type="search"
          className="mt-1 block rounded-md border px-2 py-1"
          placeholder="表示名の一部"
          value={text}
          onChange={(event) => change(event.target.value)}
        />
      </label>
      {selected !== null && (
        <p className="text-sm">
          生徒を選んでいます: <span className="font-semibold">{selected.name}</span>{" "}
          <Button variant="text" onClick={() => onSelect(null)}>
            生徒の絞り込みを外す
          </Button>
        </p>
      )}
      {query !== "" && students.kind === "loading" && <p className="text-sm text-text-muted">生徒を探しています…</p>}
      {query !== "" && students.kind === "failed" && (
        <p className="text-sm text-danger">生徒の一覧を読めませんでした。もう一度入力してください</p>
      )}
      {query !== "" && students.kind === "loaded" && matches.length === 0 && (
        <p className="text-sm">{`「${query}」に一致する生徒はいません`}</p>
      )}
      {matches.length > 0 && (
        <ul aria-label="生徒の候補" className="space-y-1 text-sm">
          {matches.slice(0, MAX_CANDIDATES).map((student) => (
            <li key={student.student_id}>
              <Button
                variant="text"
                onClick={() => {
                  setText("");
                  onSelect({ studentId: student.student_id, name: student.display_name });
                }}
              >
                {`${student.display_name}（登録日 ${formatDate(student.registered_at)}）`}
              </Button>
            </li>
          ))}
          {matches.length > MAX_CANDIDATES && (
            <li className="text-text-muted">{`ほかに ${matches.length - MAX_CANDIDATES} 人います。名前をもう少し入れて絞ってください`}</li>
          )}
        </ul>
      )}
    </div>
  );
}
