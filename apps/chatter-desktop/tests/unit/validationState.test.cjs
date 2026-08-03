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
} = require("../../.test-dist/src/hooks/validationState.js");

// A valid `ValidationStats` literal (9 fields, `src/protocol/validation.ts`).
// `cacheHitRate` is NOT one of them; this test file is untyped `.cjs`, so an
// invented field survives silently unless every literal is built here.
function stats(overrides = {}) {
  return {
    totalFiles: 2,
    validFiles: 2,
    invalidFiles: 0,
    cacheHits: 0,
    cacheMisses: 2,
    parseErrors: 0,
    roundtripPassed: 0,
    roundtripFailed: 0,
    cancelled: false,
    ...overrides,
  };
}

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

// REGRESSION GUARD: before this fix, FileTree derived "all valid" from
// `errorFileCount === 0` alone, which is also true for the entire window
// between "discovery done" and "last file actually validated" whenever no
// error has streamed in yet - not the same thing as the run being finished.
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
    shouldShowAllFilesValid({ kind: "finished", stats: stats() }, 0),
    true,
    "must claim all-valid once finished with zero error files",
  );
  assert.equal(
    shouldShowAllFilesValid({ kind: "finished", stats: stats() }, 2),
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

// REGRESSION GUARD: `aborted` used to be a dead end (no Re-validate button,
// unlike `finished`), because only `finished` was checked at the call sites
// that decide whether to offer Re-validate. Covers all six `RunPhase`
// variants so a future phase is a deliberate yes/no here, not a silent
// omission at whichever call site someone remembered to update.
test("isRunRecoverable is true only for finished and aborted", () => {
  assert.ok(!isRunRecoverable({ kind: "idle" }), "nothing to re-run before a run has started");
  assert.ok(!isRunRecoverable({ kind: "invoked" }), "a run in flight is not done yet");
  assert.ok(!isRunRecoverable({ kind: "discovering" }), "a run in flight is not done yet");
  assert.ok(!isRunRecoverable({ kind: "running", totalFiles: 3 }), "a run in flight is not done yet");
  assert.ok(
    isRunRecoverable({ kind: "finished", stats: stats() }),
    "a completed run can be re-validated",
  );
  assert.ok(
    isRunRecoverable({ kind: "aborted", reason: "died" }),
    "a dead run must not be a dead end either",
  );
});

// REGRESSION GUARD: a run whose workers abandoned files reports perfectly
// ordinary-looking counts, because the missing files contributed to no
// counter. A 500-file corpus could validate 480 and show "all files valid",
// which is the worst possible failure for a tool that tells researchers
// whether their data is sound.
test("an incomplete run never claims all files valid", () => {
  assert.ok(
    !shouldShowAllFilesValid(
      { kind: "finishedIncomplete", stats: stats({ totalFiles: 5, validFiles: 3 }), lostFiles: 2 },
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
    },
    (path) => path,
  );

  assert.equal(next.run.kind, "finishedIncomplete");
  assert.equal(next.run.lostFiles, 2);
});

// An incomplete run is still re-runnable: re-running is exactly what a user
// should do after files were skipped.
test("isRunRecoverable includes finishedIncomplete", () => {
  assert.ok(
    isRunRecoverable({
      kind: "finishedIncomplete",
      stats: stats({ totalFiles: 5, validFiles: 3 }),
      lostFiles: 2,
    }),
  );
});
