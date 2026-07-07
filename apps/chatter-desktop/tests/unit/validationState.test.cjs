const test = require("node:test");
const assert = require("node:assert/strict");

const {
  applyValidationEvent,
  createInitialValidationState,
  relativeDisplayName,
  shouldShowAllFilesValid,
} = require("../../.test-dist/src/hooks/validationState.js");

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
  assert.equal(state.phase, "running");
  assert.equal(state.totalFiles, 1);
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
test("shouldShowAllFilesValid requires phase to be finished, not just zero errors", () => {
  assert.equal(
    shouldShowAllFilesValid("running", 0),
    false,
    "must not claim all-valid mid-run even with zero errors observed so far",
  );
  assert.equal(
    shouldShowAllFilesValid("discovering", 0),
    false,
    "must not claim all-valid while still discovering files",
  );
  assert.equal(
    shouldShowAllFilesValid("idle", 0),
    false,
    "must not claim all-valid before a run has started",
  );
  assert.equal(
    shouldShowAllFilesValid("finished", 0),
    true,
    "must claim all-valid once finished with zero error files",
  );
  assert.equal(
    shouldShowAllFilesValid("finished", 2),
    false,
    "must not claim all-valid when finished with error files present",
  );
});
