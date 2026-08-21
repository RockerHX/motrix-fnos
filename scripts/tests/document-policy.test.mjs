import test from "node:test";
import assert from "node:assert/strict";
import { checkDocumentPolicy } from "../verify/check-document-policy.mjs";

test("仓库文档符合生命周期白名单", () => {
  assert.deepEqual(checkDocumentPolicy(), []);
});
