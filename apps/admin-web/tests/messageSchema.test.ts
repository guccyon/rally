// @vitest-environment node
// チャットのメッセージの型（契約の `Message`・`SendMessageRequest`）。種類（`kind`）と種類ごとの中身、送り手の `system`、
// 送信の本文かスタンプが生成した型にあること。
// 型の検査は `npm run typecheck`（tsc）で効く。
import { expectTypeOf, test } from "vitest";

import type { components } from "@/api/schema";

type Message = components["schemas"]["Message"];

type StampId = components["schemas"]["StampId"];
type SendMessageRequest = components["schemas"]["SendMessageRequest"];

test("Message は種類と、種類ごとの中身（動画の ID・添削の ID・スタンプは種類が持つときだけ）を持つ", () => {
  expectTypeOf<Message["kind"]>().toEqualTypeOf<"text" | "video_review_request" | "video_review_returned" | "stamp">();
  expectTypeOf<Message["body"]>().toEqualTypeOf<string>();
  expectTypeOf<Message["video_id"]>().toEqualTypeOf<string | undefined>();
  expectTypeOf<Message["review_id"]>().toEqualTypeOf<string | undefined>();
  expectTypeOf<Message["stamp_id"]>().toEqualTypeOf<StampId | undefined>();
});

test("スタンプは決まった 8 種類の ID", () => {
  expectTypeOf<StampId>().toEqualTypeOf<
    "thanks" | "good" | "got_it" | "lets_go" | "nice_shot" | "tired" | "question" | "ok"
  >();
});

test("送信は本文かスタンプ（どちらも任意の項目。どちらか 1 つは API が 422 で守る）", () => {
  expectTypeOf<SendMessageRequest["body"]>().toEqualTypeOf<string | undefined>();
  expectTypeOf<SendMessageRequest["stamp_id"]>().toEqualTypeOf<StampId | undefined>();
});

test("送り手は生徒・コーチ・システム", () => {
  expectTypeOf<Message["sender_role"]>().toEqualTypeOf<"student" | "coach" | "system">();
});
