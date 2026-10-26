// @vitest-environment node
// API の型が契約から生成したものと一致すること（手書き・生成忘れを防ぐ）。直すときは `npm run gen:api`。
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import openapiTS, { astToString } from "openapi-typescript";
import { expect, test } from "vitest";

const contract = new URL("../../../specs/contracts/api/admin-web/openapi.yaml", import.meta.url);
const generated = fileURLToPath(new URL("../src/api/schema.ts", import.meta.url));

test("src/api/schema.ts は admin-web の OpenAPI から生成したものと一致する", async () => {
  const expected = astToString(await openapiTS(contract));
  const actual = readFileSync(generated, "utf8");
  // CLI はファイルの先頭に生成の注記を付ける。本文だけを比べる。
  expect(actual.endsWith(expected)).toBe(true);
});
