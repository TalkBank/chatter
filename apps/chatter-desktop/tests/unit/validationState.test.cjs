const test = require("node:test");
const assert = require("node:assert/strict");

const {
  applyValidationEvent,
  createInitialValidationState,
  isAwaitingBackend,
  isRunPending,
  isRunRecoverable,
  totalFilesOf,
  relativeDisplayName,
  shouldShowAllFilesValid,
  fileOutcome,
  fileStatusLabel,
  finishedRunSummary,
} = require("../../.test-dist/src/hooks/validationState.js");

// A valid `ValidationStats` literal, matching `src/protocol/validation.ts`.
// `cacheHitRate` is NOT one of them; this test file is untyped `.cjs`, so an
// invented field survives silently unless every literal is built here.
function stats(overrides = {}) {
  return {
    totalFiles: 2,
    validFiles: 2,
    invalidFiles: 0,
    cacheHits: 0,
    cacheMisses: 2,
    cacheErrors: 0,
    internalFailures: 0,
    roundtripPassed: 0,
    roundtripFailed: 0,
    ...overrides,
  };
}

test("terminal failures without diagnostics remain visible problems", () => {
  for (const status of [
    { type: "readError", message: "permission denied" },
    { type: "internalFailure", message: "CHAT validity was not determined" },
    { type: "roundtripFailed", reason: "model changed", cacheHit: false },
    { type: "invalid", errorCount: 2, cacheHit: true },
  ]) {
    const file = { path: "/sample.cha", name: "sample.cha", diagnostics: [], source: "", status };
    const outcome = fileOutcome(file);
    assert.equal(outcome.kind, "problem");
    assert.notEqual(outcome.message, "No errors");
    if (status.message || status.reason) {
      assert.ok(outcome.message.includes(status.message || status.reason));
    }
  }
});

// The one label a file's outcome reads as, on screen and in a text export.
test("a file's status label is its outcome in words", () => {
  const file = { path: "/sample.cha", name: "sample.cha", diagnostics: [], source: "", status: null };
  assert.equal(fileStatusLabel(file), "Validation pending");
  file.status = { type: "valid", cacheHit: false };
  assert.equal(fileStatusLabel(file), "Valid");
  file.status = { type: "readError", message: "permission denied" };
  assert.equal(fileStatusLabel(file), "Read error: permission denied");
});

test("only completed valid files without diagnostics have a valid outcome", () => {
  const file = { path: "/sample.cha", name: "sample.cha", diagnostics: [], source: "", status: null };
  assert.equal(fileOutcome(file).kind, "pending");
  file.status = { type: "valid", cacheHit: true };
  assert.equal(fileOutcome(file).kind, "valid");
  file.diagnostics = [diagnostic("W109", "normalize name")];
  assert.equal(fileOutcome(file).kind, "problem", "warnings stay visible");
});

// The all-valid claim is the runner's verdict (`passed`), never a rule the
// desktop computes over the counts: a failed run cannot be certified, a
// passed run is, and a passed run with warnings says so.
test("finished summaries certify exactly the runs the runner passed", () => {
  for (const overrides of [
    { validFiles: 1, internalFailures: 1 },
    { validFiles: 1, invalidFiles: 1 },
    { roundtripFailed: 1 },
  ]) {
    const failed = { kind: "finished", stats: stats(overrides), passed: false };
    assert.equal(shouldShowAllFilesValid(failed, 0), false);
    assert.ok(!finishedRunSummary(failed, 0, null).includes("files valid"));
  }
  const passed = { kind: "finished", stats: stats(), passed: true };
  assert.equal(finishedRunSummary(passed, 0, null), "All 2 files valid");
  assert.equal(finishedRunSummary(passed, 3, null), "All 2 files valid; 3 warnings");
  assert.equal(shouldShowAllFilesValid(passed, 1), false, "files with warnings stay shown");
});

// A target with no transcript is its own phase: no counts, no claim, and
// Re-validate offered.
test("a nothingFound event yields its own phase, which certifies nothing", () => {
  const next = applyValidationEvent(
    createInitialValidationState(),
    { type: "nothingFound" },
    (path) => path,
  );
  assert.equal(next.run.kind, "nothingFound");
  assert.equal(shouldShowAllFilesValid(next.run, 0), false);
  assert.ok(isRunRecoverable(next.run));
  assert.equal(totalFilesOf(next.run), 0);
});

test("a stopped run is its own phase and never certifies anything", () => {
  const stopped = {
    kind: "stopped",
    stats: stats({ totalFiles: 5, validFiles: 3 }),
    unprocessedFiles: 2,
    reason: "Cancelled",
  };
  assert.equal(shouldShowAllFilesValid(stopped, 0), false);
  assert.equal(isRunRecoverable(stopped), true);
  assert.equal(totalFilesOf(stopped), 5);
});

function diagnostic(code, message, start = 1) {
  return {
    error: {
      code,
      severity: "Error",
      location: { start, end: start + 1, line: 1, column: 1 },
      labels: [],
      message,
    },
    renderedHtml: `<span>${message}</span>`,
  };
}

test("validation state accumulates diagnostics and file status immutably", () => {
  const root = "/tmp/corpus";
  const file = "/tmp/corpus/nested/sample.cha";
  const relative = (path) => relativeDisplayName(path, root);

  let state = createInitialValidationState();
  state = applyValidationEvent(state, { type: "discovering" }, relative);
  state = applyValidationEvent(state, { type: "started", totalFiles: 1 }, relative);
  state = applyValidationEvent(
    state,
    {
      type: "errors",
      file,
      diagnostics: [diagnostic("E001", "missing header")],
      source: "*CHI:\thello .",
    },
    relative,
  );
  state = applyValidationEvent(
    state,
    {
      type: "fileComplete",
      file,
      status: { type: "invalid", errorCount: 1, cacheHit: false },
    },
    relative,
  );

  const entry = state.files.get(file);
  assert.ok(entry);
  assert.equal(entry.name, "nested/sample.cha");
  assert.equal(entry.diagnostics.length, 1);
  assert.equal(entry.status.type, "invalid");
  assert.equal(state.run.kind, "running");
  assert.equal(totalFilesOf(state.run), 1);
  assert.equal(state.processedFiles, 1);
  assert.equal(state.totalErrors, 1);
});

test("relative display names handle file roots and Windows separators", () => {
  assert.equal(
    relativeDisplayName("/tmp/corpus/sample.cha", "/tmp/corpus/sample.cha"),
    "sample.cha",
  );
  assert.equal(
    relativeDisplayName(
      "C:\\Corpora\\nested\\sample.cha",
      "C:\\Corpora",
    ),
    "nested/sample.cha",
  );
  assert.equal(
    relativeDisplayName("/tmp/corpus/nested/sample.cha", "/tmp/corpus"),
    "nested/sample.cha",
  );
});

// "All valid" is never derived from `errorFileCount === 0` alone, which is
// also true for the whole window between "discovery done" and "last file
// validated" while no error has streamed in yet.
test("an aborted run never claims all files valid", () => {
  assert.equal(
    shouldShowAllFilesValid({ kind: "aborted", reason: "the validator stopped" }, 0),
    false,
    "a run that died produced no results, so 'all valid' would describe nothing",
  );
});

test("shouldShowAllFilesValid requires phase to be finished, not just zero errors", () => {
  assert.equal(
    shouldShowAllFilesValid({ kind: "running", totalFiles: 2 }, 0),
    false,
    "must not claim all-valid mid-run even with zero errors observed so far",
  );
  assert.equal(
    shouldShowAllFilesValid({ kind: "discovering" }, 0),
    false,
    "must not claim all-valid while still discovering files",
  );
  assert.equal(
    shouldShowAllFilesValid({ kind: "idle" }, 0),
    false,
    "must not claim all-valid before a run has started",
  );
  assert.equal(
    shouldShowAllFilesValid({ kind: "finished", stats: stats(), passed: true }, 0),
    true,
    "must claim all-valid once finished with zero error files",
  );
  assert.equal(
    shouldShowAllFilesValid({ kind: "finished", stats: stats(), passed: true }, 2),
    false,
    "must not claim all-valid when finished with error files present",
  );
});

// The optimistic "we asked" state must be DISTINCT from the backend's
// confirmed "I am discovering" state.
//
// They were the same value ("discovering"), set both locally at invoke time
// (useValidation.ts) and from the backend's Discovering event. So the UI could
// not tell "the backend never answered" from "the backend is working", no
// watchdog was possible (a slow discovery is indistinguishable from silence),
// and the only bug report a user could file was the one actually received on
// 2026-08-02: "it doesn't seem to go beyond the Discovering files step".
test("invoked is distinct from discovering, so backend silence is observable", () => {
  const relative = (path) => path;

  // Optimistic: the command has been sent, the backend has not spoken.
  const invoked = { ...createInitialValidationState(), run: { kind: "invoked" } };
  assert.equal(invoked.run.kind, "invoked");
  assert.ok(isAwaitingBackend(invoked.run), "invoked means no backend event yet");

  // The backend's first event moves it on, and that transition is the ONLY
  // way to reach "discovering".
  const discovering = applyValidationEvent(invoked, { type: "discovering" }, relative);
  assert.equal(discovering.run.kind, "discovering");
  assert.ok(
    !isAwaitingBackend(discovering.run),
    "discovering means the backend has spoken, so no watchdog applies",
  );
});

test("a run still in flight counts as running for UI purposes from invoke onward", () => {
  assert.ok(isRunPending({ kind: "invoked" }), "a sent-but-unanswered command is in flight");
  assert.ok(isRunPending({ kind: "discovering" }));
  assert.ok(isRunPending({ kind: "running", totalFiles: 3 }));
  assert.ok(!isRunPending({ kind: "idle" }));
  assert.ok(!isRunPending({ kind: "aborted", reason: "died" }));
});

// Every ended phase offers Re-validate and no other does, so a new phase is
// a deliberate yes or no here.
test("isRunRecoverable is true exactly for the ended phases", () => {
  assert.ok(!isRunRecoverable({ kind: "idle" }), "nothing to re-run before a run has started");
  assert.ok(!isRunRecoverable({ kind: "invoked" }), "a run in flight is not done yet");
  assert.ok(!isRunRecoverable({ kind: "discovering" }), "a run in flight is not done yet");
  assert.ok(!isRunRecoverable({ kind: "running", totalFiles: 3 }), "a run in flight is not done yet");
  assert.ok(
    isRunRecoverable({ kind: "finished", stats: stats(), passed: true }),
    "a completed run can be re-validated",
  );
  assert.ok(
    isRunRecoverable({ kind: "aborted", reason: "died" }),
    "a dead run must not be a dead end either",
  );
});

// A run whose workers abandoned files reports ordinary-looking counts,
// because the missing files contributed to no counter, so it can never reach
// the all-valid claim.
test("an incomplete run never claims all files valid", () => {
  assert.ok(
    !shouldShowAllFilesValid(
      {
        kind: "finishedIncomplete",
        stats: stats({ totalFiles: 5, validFiles: 3 }),
        lostFiles: 2,
        cause: "1 worker(s) failed with an internal error.",
      },
      0,
    ),
    "zero errors among the files that WERE checked is not a verdict on the ones that were not",
  );
});

test("a finishedIncomplete event yields the incomplete phase, not finished", () => {
  const next = applyValidationEvent(
    createInitialValidationState(),
    {
      type: "finishedIncomplete",
      stats: stats({ totalFiles: 5, validFiles: 3 }),
      lostFiles: 2,
      cause: "1 worker(s) failed with an internal error.",
    },
    (path) => path,
  );

  assert.equal(next.run.kind, "finishedIncomplete");
  assert.equal(next.run.lostFiles, 2);
  // The runner's own sentence for why, carried through unchanged.
  assert.equal(next.run.cause, "1 worker(s) failed with an internal error.");
});

// An incomplete run is still re-runnable: re-running is exactly what a user
// should do after files were skipped.
test("isRunRecoverable includes finishedIncomplete", () => {
  assert.ok(
    isRunRecoverable({
      kind: "finishedIncomplete",
      stats: stats({ totalFiles: 5, validFiles: 3 }),
      lostFiles: 2,
      cause: "1 worker(s) failed with an internal error.",
    }),
  );
});

test("a failing cache is said in the finished summary", () => {
  const run = { kind: "finished", stats: stats({ cacheErrors: 3 }), passed: true };
  const summary = finishedRunSummary(run, 0, null);
  assert.ok(summary.includes("3 cache failures"), summary);
  assert.ok(
    !finishedRunSummary({ kind: "finished", stats: stats(), passed: true }, 0, null).includes("cache"),
  );
});

// A cache that would not open is said in the summary, with its reason, and
// the reducer keeps it beside the run until the next run starts.
test("a cache that would not open is said in the finished summary", () => {
  const next = applyValidationEvent(
    createInitialValidationState(),
    { type: "cacheUnavailable", reason: "database is locked" },
    (path) => path,
  );
  assert.equal(next.cacheUnavailable, "database is locked");
  assert.equal(next.run.kind, "idle", "the note does not move the run's phase");
  const summary = finishedRunSummary(
    { kind: "finished", stats: stats(), passed: true },
    0,
    next.cacheUnavailable,
  );
  assert.ok(summary.includes("cache unavailable"), summary);
  assert.ok(summary.includes("database is locked"), summary);
});
