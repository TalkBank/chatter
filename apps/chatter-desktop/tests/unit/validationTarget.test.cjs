const assert = require("node:assert/strict");
const test = require("node:test");
const { createValidationTargetCapability } = require("../../.test-dist/src/runtime/capabilities/validationTarget.js");

test("reveal delegates the exact path to the typed desktop command", async () => {
  const calls = [];
  const target = createValidationTargetCapability({
    async invoke(command, args) { calls.push({ command, args }); },
  });
  await target.revealFile("/transcripts/café.cha");
  assert.deepEqual(calls, [{
    command: "reveal_in_file_manager",
    args: { path: "/transcripts/café.cha" },
  }]);
});

test("reveal preserves native failure rather than claiming success", async () => {
  const failure = new Error("file manager unavailable");
  const target = createValidationTargetCapability({
    async invoke() { throw failure; },
  });
  await assert.rejects(target.revealFile("/transcripts/input.cha"), failure);
});
