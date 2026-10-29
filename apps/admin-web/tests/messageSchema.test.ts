// @vitest-environment node
// チャットのメッセージの型（契約の `Message`）。種類（`kind`）と種類ごとの中身、送り手の `system` が生成した型にあること。
// 型の検査は `npm run typecheck`（tsc）で効く。
import { expectTypeOf, test } from "vitest";

import type { components } from "@/api/schema";

type Message = components["schemas"]["Message"];

test("Message は種類と、種類ごとの中身（動画の ID・添削の ID は種類が持つときだけ）を持つ", () => {
  expectTypeOf<Message["kind"]>().toEqualTypeOf<"text" | "video_review_request" | "video_review_returned">();
  expectTypeOf<Message["body"]>().toEqualTypeOf<string>();
  expectTypeOf<Message["video_id"]>().toEqualTypeOf<string | undefined>();
  expectTypeOf<Message["review_id"]>().toEqualTypeOf<string | undefined>();
});

test("送り手は生徒・コーチ・システム", () => {
  expectTypeOf<Message["sender_role"]>().toEqualTypeOf<"student" | "coach" | "system">();
});
