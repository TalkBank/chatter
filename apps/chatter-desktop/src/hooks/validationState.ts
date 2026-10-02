import type { FileEntry, ValidationEvent, ValidationStats } from "../protocol/validation";

/**
 * Where a validation run is, WITH the data that phase and only that phase has.
 *
 * A discriminated union rather than a bare tag beside nullable fields, because
 * the flat shape made illegal combinations representable and two of them were
 * real bugs waiting to happen: `finished` with no stats, `aborted` with no
 * reason, and `idle` still carrying the previous run's results. Narrowing on
 * `kind` now yields exactly the fields that phase defines, so
 * `phase === "finished" && stats` (a runtime re-check of something the type
 * already knows) is gone from the render path.
 *
 * Two distinctions here are load-bearing and must not be collapsed:
 *
 * - `invoked` vs `discovering`. `invoked` is set locally the instant the Tauri
 *   command is sent; `discovering` is reachable ONLY from the backend's own
 *   event. When both were the single value "discovering", the UI could not
 *   tell "the backend never answered" from "the backend is working": no
 *   watchdog was possible, since a legitimately slow discovery looks exactly
 *   like silence, and the best bug report a user could file was the one
 *   received 2026-08-02, "it doesn't seem to go beyond the Discovering files
 *   step".
 * - `aborted` vs `finished`. A run that died produced no results; reporting it
 *   as a clean finish is how empty output reads as success.
 * - `finishedIncomplete` vs `finished`. A run whose workers abandoned files has
 *   perfectly ordinary-looking counts, because the missing files contributed to
 *   no counter. Only `finished` means "every discovered file was examined", so
 *   only `finished` may reach an all-valid claim. Folding the two together, or
 *   adding a `lostFiles` field to `finished`, would put the burden on every
 *   consumer to remember to check, which is the forgetting this shape exists to
 *   prevent.
 * - `stopped` vs `finished`. A cancelled run's counts describe only what it
 *   reached, so it is its own phase rather than a flag on `finished`.
 * - `nothingFound` vs `finished`. A target with no transcript validated
 *   nothing; it has no counts, and is never a clean finish.
 */
export type RunPhase =
  | { kind: "idle" }
  | { kind: "invoked" }
  | { kind: "discovering" }
  | { kind: "running"; totalFiles: number }
  | { kind: "finished"; stats: ValidationStats; passed: boolean }
  | { kind: "nothingFound" }
  | {
      kind: "finishedIncomplete";
      stats: ValidationStats;
      lostFiles: number;
      cause: string;
    }
  | { kind: "stopped"; stats: ValidationStats; unprocessedFiles: number; reason: string }
  | { kind: "aborted"; reason: string };

/**
 * State that accumulates ACROSS phases, beside the phase-specific data.
 *
 * Split this way because these genuinely span the run (files stream in
 * from `errors`/`fileComplete` regardless of phase), whereas `totalFiles`,
 * `stats` and the abort reason belong to exactly one phase each and live
 * there.
 */
export interface ValidationState {
  run: RunPhase;
  files: Map<string, FileEntry>;
  processedFiles: number;
  totalErrors: number;
  /** Why the cache would not open for this run, if it would not. */
  cacheUnavailable: string | null;
}

export function createInitialValidationState(): ValidationState {
  return {
    run: { kind: "idle" },
    files: new Map(),
    processedFiles: 0,
    totalErrors: 0,
    cacheUnavailable: null,
  };
}

/**
 * True while the command has been sent and the backend has said NOTHING yet.
 *
 * The only phase with no legitimate duration: everything between invoke and
 * the first event is building the config, opening the cache, and spawning a
 * thread. A run sitting here is evidence of a fault, which is what makes a
 * watchdog meaningful here and meaningless on `discovering`.
 */
export function isAwaitingBackend(run: RunPhase): boolean {
  return run.kind === "invoked";
}

/** True for every phase where a run is in flight and cancelling is meaningful. */
export function isRunPending(run: RunPhase): boolean {
  return run.kind === "invoked" || run.kind === "discovering" || run.kind === "running";
}

/**
 * True for a run that has ended, by any of its endings, so Re-validate is
 * offered. `idle` is excluded: there is no prior run to re-run. Exhaustive,
 * so a new phase is classified here or this fails to compile.
 */
export function isRunRecoverable(run: RunPhase): boolean {
  switch (run.kind) {
    case "finished":
    case "nothingFound":
    case "finishedIncomplete":
    case "stopped":
    case "aborted":
      return true;
    case "idle":
    case "invoked":
    case "discovering":
    case "running":
      return false;
  }
}

/**
 * How many files this run covers, DERIVED rather than stored.
 *
 * `running` learns the count from `started`; `finished` carries it inside
 * `stats`. Keeping a separate `totalFiles` field beside both was a second
 * representation of one fact, free to disagree with `stats.totalFiles`.
 */
export function totalFilesOf(run: RunPhase): number {
  switch (run.kind) {
    case "running":
      return run.totalFiles;
    case "finished":
    case "finishedIncomplete":
    case "stopped":
      // The count of files DISCOVERED, which an incomplete or stopped run
      // still knows; what it lacks is a result for each of them.
      return run.stats.totalFiles;
    case "idle":
    case "invoked":
    case "discovering":
    case "nothingFound":
    case "aborted":
      return 0;
  }
}

export function applyValidationEvent(
  prev: ValidationState,
  event: ValidationEvent,
  relativeName: (path: string) => string,
): ValidationState {
  switch (event.type) {
    case "cacheUnavailable":
      return { ...prev, cacheUnavailable: event.reason };

    case "discovering":
      return { ...prev, run: { kind: "discovering" } };

    case "started":
      return { ...prev, run: { kind: "running", totalFiles: event.totalFiles } };

    case "errors": {
      const files = new Map(prev.files);
      const existing = files.get(event.file);

      files.set(
        event.file,
        existing
          ? {
              ...existing,
              diagnostics: [...existing.diagnostics, ...event.diagnostics],
              source: event.source,
            }
          : {
              path: event.file,
              name: relativeName(event.file),
              diagnostics: [...event.diagnostics],
              source: event.source,
              status: null,
            },
      );

      return {
        ...prev,
        files,
        totalErrors: prev.totalErrors + event.diagnostics.length,
      };
    }

    case "fileComplete": {
      const files = new Map(prev.files);
      const existing = files.get(event.file);

      files.set(
        event.file,
        existing
          ? { ...existing, status: event.status }
          : {
              path: event.file,
              name: relativeName(event.file),
              diagnostics: [],
              source: "",
              status: event.status,
            },
      );

      return { ...prev, files, processedFiles: prev.processedFiles + 1 };
    }

    case "aborted":
      return { ...prev, run: { kind: "aborted", reason: event.reason } };

    case "finishedIncomplete":
      return {
        ...prev,
        run: {
          kind: "finishedIncomplete",
          stats: event.stats,
          lostFiles: event.lostFiles,
          cause: event.cause,
        },
      };

    case "stopped":
      return {
        ...prev,
        run: {
          kind: "stopped",
          stats: event.stats,
          unprocessedFiles: event.unprocessedFiles,
          reason: event.reason,
        },
      };

    case "finished":
      return { ...prev, run: { kind: "finished", stats: event.stats, passed: event.passed } };

    case "nothingFound":
      return { ...prev, run: { kind: "nothingFound" } };
  }

  return assertNever(event);
}

/**
 * Whether the file tree may claim "all valid": the run finished and PASSED
 * (the runner's own verdict, the one the CLI's exit status reads), and no
 * file has anything to show. Only `finished` carries a verdict, so a run
 * that is still going, stopped, lost files, found nothing or died can never
 * reach the claim; and `errorFileCount` keeps a passed run with warnings
 * showing its files.
 */
export function shouldShowAllFilesValid(run: RunPhase, errorFileCount: number): boolean {
  return run.kind === "finished" && run.passed && errorFileCount === 0;
}

/** One projection for tree visibility and detail rendering, including failures
 * that have no CHAT diagnostic (for example, a file that could not be read). */
export type FileOutcome =
  | { kind: "pending" }
  | { kind: "valid" }
  | { kind: "problem"; message: string };

export function fileOutcome(file: FileEntry): FileOutcome {
  if (file.diagnostics.length > 0) {
    return { kind: "problem", message: `${file.diagnostics.length} diagnostics` };
  }
  const status = file.status;
  if (status === null) return { kind: "pending" };
  switch (status.type) {
    case "valid": return { kind: "valid" };
    case "invalid": return { kind: "problem", message: `Validation failed (${status.errorCount} diagnostics)` };
    case "readError": return { kind: "problem", message: `Read error: ${status.message}` };
    case "internalFailure": return { kind: "problem", message: `Internal failure: ${status.message}` };
    case "roundtripFailed": return { kind: "problem", message: `Roundtrip failed: ${status.reason}` };
  }
  return assertNever(status);
}

/** The file's outcome as one line for a person: what the error panel
 * shows and what a text export writes, from this one owner. */
export function fileStatusLabel(file: FileEntry): string {
  const outcome = fileOutcome(file);
  switch (outcome.kind) {
    case "problem": return outcome.message;
    case "valid": return "Valid";
    case "pending": return "Validation pending";
  }
}

/** Finished is not necessarily successful: the runner's verdict decides,
 * so the title, notification and status bar agree with the CLI's exit
 * status. A cancelled run is never `finished`; it is `stopped`. */
export function finishedRunSummary(
  run: Extract<RunPhase, { kind: "finished" }>,
  diagnostics: number,
  cacheUnavailable: string | null,
): string {
  const stats = run.stats;
  // A failing cache is said, never shown as a cold one: those files were
  // validated without it, and the results stand.
  const cacheNote = cacheUnavailable !== null
    ? `; cache unavailable (validated without it): ${cacheUnavailable}`
    : stats.cacheErrors > 0
      ? `; ${stats.cacheErrors} cache failures (validated without the cache)`
      : "";
  if (run.passed) {
    // Warnings do not fail a run; they are counted, not hidden.
    const warnings = diagnostics > 0 ? `; ${diagnostics} warnings` : "";
    return `All ${stats.totalFiles} files valid${warnings}${cacheNote}`;
  }
  return `${diagnostics} diagnostics; ${stats.invalidFiles} invalid or unreadable files, ${stats.internalFailures} internal failures, ${stats.roundtripFailed} roundtrip failures${cacheNote}`;
}

export function relativeDisplayName(fullPath: string, targetPath: string): string {
  if (!targetPath) return normalizeDisplayPath(fullPath);
  if (fullPath === targetPath) return basename(fullPath);

  const targetWithSeparator = withTrailingSeparator(targetPath);
  if (fullPath.startsWith(targetWithSeparator)) {
    return normalizeDisplayPath(fullPath.slice(targetWithSeparator.length));
  }

  return normalizeDisplayPath(fullPath);
}

function normalizeDisplayPath(path: string): string {
  return path.replace(/\\/g, "/");
}

function basename(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] ?? path;
}

function withTrailingSeparator(path: string): string {
  if (path === "" || /[\\/]$/.test(path)) return path;
  const separator = path.includes("\\") ? "\\" : "/";
  return `${path}${separator}`;
}

function assertNever(value: never): never {
  throw new Error(`Unhandled validation event: ${JSON.stringify(value)}`);
}
