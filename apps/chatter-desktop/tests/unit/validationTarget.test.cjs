const assert = require("node:assert/strict");
const test = require("node:test");
const { createValidationTargetCapability } = require("../../.test-dist/src/runtime/capabilities/validationTarget.js");
const { TargetSelection } = require("../../.test-dist/src/runtime/capabilities/targetSelection.js");

function deferred() {
  return Promise.withResolvers();
}

test("retiring a picker prevents its late result from replacing a newer target", async () => {
  const dialog = deferred();
  const paths = [];
  const reports = [];
  const old = new TargetSelection({ chooseValidationFile: () => dialog.promise },
    (path) => paths.push(path), (message) => reports.push(message));
  const pending = old.choose("file");
  old.close();
  const current = new TargetSelection({}, (path) => paths.push(path), (message) => reports.push(message));
  current.select(["/transcripts/current.cha"]);
  dialog.resolve("/transcripts/stale.cha");
  await pending;
  old.select(["/transcripts/also-stale.cha"]);
  assert.deepEqual(paths, ["/transcripts/current.cha"]);
  assert.deepEqual(reports, []);
});

test("one picker owns selection until cancellation, then a retry may start", async () => {
  const dialog = deferred();
  const paths = [];
  let folders = 0;
  const selection = new TargetSelection({
    chooseValidationFile: () => dialog.promise,
    async chooseValidationFolder() { folders += 1; return "/transcripts/folder"; },
  }, (path) => paths.push(path), assert.fail);
  const pending = selection.choose("file");
  await selection.choose("folder");
  selection.select(["/transcripts/drop.cha"]);
  assert.equal(folders, 0);
  assert.deepEqual(paths, []);
  dialog.resolve(null);
  await pending;
  await selection.choose("folder");
  selection.select(["/transcripts/duplicate.cha"]);
  assert.equal(folders, 1);
  assert.deepEqual(paths, ["/transcripts/folder"]);
});

test("live picker failures remain visible and permit retry; retired failures are quiet", async () => {
  const dialog = deferred();
  const reports = [];
  const paths = [];
  const selection = new TargetSelection({
    chooseValidationFile: () => dialog.promise,
    async chooseValidationFolder() { return "/transcripts/retry"; },
  }, (path) => paths.push(path), (message) => reports.push(message));
  const pending = selection.choose("file");
  dialog.reject(new Error("picker unavailable"));
  await pending;
  assert.deepEqual(reports, ["Choose file failed: Error: picker unavailable"]);
  await selection.choose("folder");
  assert.deepEqual(paths, ["/transcripts/retry"]);

  const late = deferred();
  const retired = new TargetSelection({ chooseValidationFolder: () => late.promise }, assert.fail, assert.fail);
  const abandoned = retired.choose("folder");
  retired.close();
  late.reject(new Error("late failure"));
  await abandoned;
});

test("multiple-target refusal preserves the enabled interval and accepts exactly one original path", () => {
  const paths = [];
  const reports = [];
  const selection = new TargetSelection({}, (path) => paths.push(path), (message) => reports.push(message));
  selection.select([]);
  selection.select([""]);
  selection.select(["first.cha", "second.cha"]);
  assert.equal(reports.length, 1);
  assert.deepEqual(paths, []);
  selection.select(["/transcripts/café.cha"]);
  selection.select(["/transcripts/café.cha"]);
  assert.deepEqual(paths, ["/transcripts/café.cha"]);
});

test("a failed start remains visible and does not strand target selection", async () => {
  const reports = [];
  let attempts = 0;
  const selection = new TargetSelection({ async chooseValidationFile() { return "sample.cha"; } }, () => {
    attempts += 1;
    if (attempts === 1) throw new Error("start unavailable");
  }, (message) => reports.push(message));
  await selection.choose("file");
  assert.deepEqual(reports, ["Start validation failed: Error: start unavailable"]);
  await selection.choose("file");
  assert.equal(attempts, 2);
});

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
