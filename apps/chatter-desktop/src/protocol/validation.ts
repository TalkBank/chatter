/** Mirrors Rust `Severity` enum from talkbank-model */
export type Severity = "Error" | "Warning";

/** Mirrors Rust `Span` struct */
export interface Span {
  start: number;
  end: number;
}

/**
 * Mirrors Rust `DiagnosticLocation` (its tagged wire form). A known location
 * carries its line and column together or not at all; an unlocated diagnostic
 * concerns a value with no source position and has no offsets to show.
 */
export type DiagnosticLocation =
  | ({ kind: "known"; start: number; end: number } & (
      | { line: number; column: number }
      | { line?: never; column?: never }
    ))
  | { kind: "unlocated" };

/** Mirrors Rust `FrontendSourceLocation`: an admitted presentation's position. */
export interface SourceContextLocation {
  start: number;
  end: number;
  line: number;
  column: number;
}

/** A stable key fragment for a diagnostic location. */
export function locationKey(location: DiagnosticLocation): string {
  switch (location.kind) {
    case "known":
      return `${location.start}`;
    case "unlocated":
      return "unlocated";
  }
}

/** Mirrors Rust `ErrorLabel` */
export interface ErrorLabel {
  start: number;
  end: number;
  message: string;
}

/** Mirrors Rust `ParseError` (serialized form) */
export interface ParseError {
  code: string;
  severity: Severity;
  location: DiagnosticLocation;
  labels: ErrorLabel[];
  message: string;
  suggestion?: string;
  help_url?: string;
}

/** File validation status, mirrors Rust `FrontendFileStatus` */
export type FileStatus =
  | { type: "valid"; cacheHit: boolean }
  | { type: "invalid"; errorCount: number; cacheHit: boolean }
  | { type: "roundtripFailed"; cacheHit: boolean; reason: string }
  | { type: "internalFailure"; message: string }
  | { type: "readError"; message: string };

/** Mirrors Rust `FrontendStats` */
export interface ValidationStats {
  totalFiles: number;
  validFiles: number;
  invalidFiles: number;
  cacheHits: number;
  cacheMisses: number;
  /** Cache reads or writes that failed; those files were validated without it. */
  cacheErrors: number;
  internalFailures: number;
  roundtripPassed: number;
  roundtripFailed: number;
}

/** Rust-issued content identity; the frontend forwards it unchanged. */
export interface SourceRevision {
  readonly digest: readonly number[];
}

export type DiagnosticSourceContext =
  | { kind: "available"; location: SourceContextLocation; sourceRevision: SourceRevision }
  | { kind: "unavailable"; reason: string };

/** A parse diagnostic paired with pre-rendered miette HTML from Rust */
export interface RenderedDiagnostic {
  /** Untouched evidence; labels and location keep absolute file offsets. */
  error: ParseError;
  sourceContext: DiagnosticSourceContext;
  renderedHtml: string;
  /** Plain text rendering (no ANSI) for clipboard copy */
  renderedText: string;
}

/** Events emitted from the Rust backend via Tauri's event bridge */
export type ValidationEvent =
  | { type: "discovering" }
  | { type: "started"; totalFiles: number }
  | {
      type: "errors";
      file: string;
      diagnostics: RenderedDiagnostic[];
      source: string;
    }
  | { type: "fileComplete"; file: string; status: FileStatus }
  /** The cache would not open; the run goes on without it. Sent first. */
  | { type: "cacheUnavailable"; reason: string }
  | { type: "aborted"; reason: string }
  /**
   * The run ended without covering every file it discovered. `stats` describes
   * ONLY the files that were processed, so nothing here supports a claim about
   * the whole input.
   */
  | {
      type: "finishedIncomplete";
      stats: ValidationStats;
      lostFiles: number;
      cause: string;
    }
  /**
   * The run was cancelled and left `unprocessedFiles` unchecked. Its own
   * event, never a flag on `finished`, so no all-valid claim is reachable.
   */
  | {
      type: "stopped";
      stats: ValidationStats;
      unprocessedFiles: number;
      reason: string;
    }
  /**
   * Every discovered file was accounted for. `passed` is the runner's own
   * verdict (the CLI's exit status reads the same one): the only basis for
   * an all-valid claim.
   */
  | { type: "finished"; stats: ValidationStats; passed: boolean }
  /** The target held no CHAT transcript: nothing was validated. */
  | { type: "nothingFound" };

/** Per-file state accumulated from the event stream */
export interface FileEntry {
  path: string;
  /** Display name (relative to the validated root) */
  name: string;
  diagnostics: RenderedDiagnostic[];
  source: string;
  status: FileStatus | null;
}

/** Tree node for the collapsible file tree */
export interface TreeNode {
  name: string;
  path: string;
  children: TreeNode[];
  file: FileEntry | null;
}
