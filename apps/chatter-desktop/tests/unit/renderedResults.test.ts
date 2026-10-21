import { strict as assert } from "node:assert";
import { test } from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ErrorPanel from "../../src/components/ErrorPanel";
import FileTree from "../../src/components/FileTree";
import {
  applyValidationEvent,
  createInitialValidationState,
  relativeDisplayName,
  type ValidationState,
} from "../../src/hooks/validationState";
import type {
  FileStatus,
  RenderedDiagnostic,
  ValidationEvent,
  ValidationStats,
} from "../../src/types";

// Typed transport controls, not parsed CHAT or native-interaction evidence.
const root = "/inputs";
const path = `${root}/sample.cha`;

function stats(overrides: Partial<ValidationStats> = {}): ValidationStats {
  return {
    totalFiles: 1,
    validFiles: 1,
    invalidFiles: 0,
    cacheHits: 0,
    cacheMisses: 1,
    cacheErrors: 0,
    internalFailures: 0,
    roundtripPassed: 0,
    roundtripFailed: 0,
    ...overrides,
  };
}

function apply(state: ValidationState, event: ValidationEvent): ValidationState {
  return applyValidationEvent(state, event, (file) => relativeDisplayName(file, root));
}

function started(totalFiles = 1): ValidationState {
  return apply(createInitialValidationState(), { type: "started", totalFiles });
}

function tree(state: ValidationState): string {
  return renderToStaticMarkup(createElement(FileTree, {
    files: state.files,
    run: state.run,
    selectedFile: path,
    onSelectFile: () => assert.fail("static rendering must not select a file"),
  }));
}

function panel(state: ValidationState): string {
  const file = state.files.get(path);
  assert.ok(file, "event sequence must supply the selected file");
  return renderToStaticMarkup(createElement(ErrorPanel, {
    file,
    clanAvailable: false,
    onOpenInClan: () => assert.fail("static rendering must not launch CLAN"),
  }));
}

test("rendered results retain failures without diagnostic cards", () => {
  const cases: [FileStatus, string][] = [
    [{ type: "readError", message: "permission denied" }, "Read error: permission denied"],
    [{ type: "internalFailure", message: "validity undetermined" }, "Internal failure: validity undetermined"],
    [{ type: "roundtripFailed", reason: "model changed", cacheHit: false }, "Roundtrip failed: model changed"],
    [{ type: "invalid", errorCount: 2, cacheHit: true }, "Validation failed (2 diagnostics)"],
  ];
  for (const [status, expected] of cases) {
    const state = apply(started(), { type: "fileComplete", file: path, status });
    const detail = panel(state);
    assert.ok(detail.includes(expected), expected);
    assert.ok(detail.includes("var(--color-error)"));
    assert.ok(!detail.includes('class="error-card"'), "no diagnostic was supplied");
    const list = tree(state);
    assert.ok(list.includes("sample.cha"), "a failure must survive tree filtering");
    assert.ok(list.includes("1 file with errors / 1 total"));
    assert.ok(!list.includes("all valid"));
  }
});

test("rendered results distinguish pending from valid files", () => {
  const pending = apply(started(), {
    type: "errors", file: path, diagnostics: [], source: "",
  });
  assert.ok(panel(pending).includes("Validation pending"));
  assert.ok(panel(pending).includes("var(--color-text-secondary)"));
  assert.ok(!tree(pending).includes("all valid"));

  const valid = apply(pending, {
    type: "fileComplete", file: path, status: { type: "valid", cacheHit: false },
  });
  assert.ok(panel(valid).includes(">Valid<"));
  assert.ok(panel(valid).includes("var(--color-valid)"));
  assert.ok(!tree(valid).includes("sample.cha"), "clean valid files stay hidden");
  assert.ok(!tree(valid).includes("all valid"), "the run has not finished");
});

test("rendered file tree certifies only a completed passing run", () => {
  const valid = apply(started(), {
    type: "fileComplete", file: path, status: { type: "valid", cacheHit: false },
  });
  const incomplete = apply(started(2), {
    type: "fileComplete", file: path, status: { type: "valid", cacheHit: false },
  });
  const terminals: [ValidationState, ValidationEvent][] = [
    [valid, { type: "finished", stats: stats(), passed: false }],
    [incomplete, { type: "stopped", stats: stats({ totalFiles: 2 }), unprocessedFiles: 1, reason: "Cancelled" }],
    [incomplete, { type: "finishedIncomplete", stats: stats(), lostFiles: 1, cause: "worker failed" }],
    [createInitialValidationState(), { type: "aborted", reason: "startup failed" }],
    [createInitialValidationState(), { type: "nothingFound" }],
  ];
  assert.ok(!tree(valid).includes("all valid"), "streaming is not completion");
  for (const [previous, event] of terminals) {
    assert.ok(!tree(apply(previous, event)).includes("all valid"), event.type);
  }
  const complete = apply(valid, { type: "finished", stats: stats(), passed: true });
  assert.ok(tree(complete).includes("1 files, all valid"));
});

test("rendered file tree retains warnings and excludes clean valid files", () => {
  const diagnostic: RenderedDiagnostic = {
    error: {
      code: "W109", severity: "Warning", location: { kind: "known", start: 0, end: 1 },
      labels: [], message: "normalize media name",
    },
    sourceContext: { kind: "unavailable", reason: "transport control" },
    renderedHtml: "<span>normalize media name</span>",
    renderedText: "normalize media name",
  };
  let state = apply(started(2), {
    type: "errors", file: path, diagnostics: [diagnostic], source: "",
  });
  state = apply(state, {
    type: "fileComplete", file: path, status: { type: "valid", cacheHit: false },
  });
  state = apply(state, {
    type: "fileComplete", file: `${root}/clean.cha`, status: { type: "valid", cacheHit: false },
  });
  state = apply(state, {
    type: "finished", stats: stats({ totalFiles: 2, validFiles: 2, cacheMisses: 2 }), passed: true,
  });
  const list = tree(state);
  assert.ok(list.includes("sample.cha"));
  assert.ok(!list.includes("clean.cha"));
  assert.ok(list.includes("1 file with errors / 2 total"));
  assert.ok(!list.includes("all valid"), "warnings remain visible on a passing run");
  const detail = panel(state);
  assert.ok(detail.includes("[W109]"));
  assert.ok(detail.includes(diagnostic.renderedHtml), "use the supplied renderer output");
  assert.ok(detail.includes('class="error-code warning"'));
  assert.ok(!detail.includes("Open in CLAN"), "unavailable context cannot offer navigation");
});
