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
 */
export type RunPhase =
  | { kind: "idle" }
  | { kind: "invoked" }
  | { kind: "discovering" }
  | { kind: "running"; totalFiles: number }
  | { kind: "finished"; stats: ValidationStats }
  | { kind: "finishedIncomplete"; stats: ValidationStats; lostFiles: number }
  | { kind: "aborted"; reason: string };

/**
 * State that accumulates ACROSS phases, beside the phase-specific data.
 *
 * Split this way because these three genuinely span the run (files stream in
 * from `errors`/`fileComplete` regardless of phase), whereas `totalFiles`,
 * `stats` and the abort reason belong to exactly one phase each and now live
 * there.
 */
export interface ValidationState {
  run: RunPhase;
  files: Map<string, FileEntry>;
  processedFiles: number;
  totalErrors: number;
}

export function createInitialValidationState(): ValidationState {
  return {
    run: { kind: "idle" },
    files: new Map(),
    processedFiles: 0,
    totalErrors: 0,
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
 * True for a run that has stopped with a specific outcome from which
 * Re-validate should be offered: it either produced results (`finished`) or
 * it died (`aborted`). `idle` is deliberately excluded: there is no prior run
 * to re-run.
 *
 * `aborted` used to be a dead end with no way forward except dragging a
 * target in again, because only `finished` offered Re-validate. Naming the
 * shared condition means a future terminal phase is either included here
 * deliberately or the exhaustive switch below fails to compile; it cannot be
 * forgotten silently the way three independent ad hoc `run.kind === "..."`
 * checks could be.
 */
export function isRunRecoverable(run: RunPhase): boolean {
  switch (run.kind) {
    case "finished":
    case "finishedIncomplete":
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
      // The count of files DISCOVERED, which an incomplete run still knows;
      // what it lacks is a result for each of them.
      return run.stats.totalFiles;
    case "idle":
    case "invoked":
    case "discovering":
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
        },
      };

    case "finished":
      return { ...prev, run: { kind: "finished", stats: event.stats } };
  }

  return assertNever(event);
}

/**
 * Whether the file tree may claim "all valid". This must be gated on the run
 * having actually finished, not merely on the error-file count being zero:
 * `errorFileCount` only reflects files that have streamed a result *so far*,
 * so it reads as zero for the entire window between "discovery done" and
 * "last file actually validated" whenever no error has arrived yet. See
 * apps/chatter-desktop/CLAUDE.md's parity notes for the desktop-vs-CLI
 * divergence this guards against. An `aborted` run is deliberately excluded:
 * it produced no results, so "all valid" would be a claim about nothing. So is
 * `finishedIncomplete`: "all valid" is a claim about every discovered file, and
 * an incomplete run never opened some of them, so a clean-looking result there
 * is a false clean bill of health rather than a verdict.
 */
export function shouldShowAllFilesValid(run: RunPhase, errorFileCount: number): boolean {
  return run.kind === "finished" && errorFileCount === 0;
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
