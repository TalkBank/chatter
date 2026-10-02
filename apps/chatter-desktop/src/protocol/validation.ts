/** Mirrors Rust `Severity` enum from talkbank-model */
export type Severity = "Error" | "Warning";

/** Mirrors Rust `Span` struct */
export interface Span {
  start: number;
  end: number;
}

/** Mirrors Rust `SourceLocation` */
export interface SourceLocation {
  start: number;
  end: number;
  line?: number;
  column?: number;
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
  location: SourceLocation;
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

/** A parse diagnostic paired with pre-rendered miette HTML from Rust */
export interface RenderedDiagnostic {
  error: ParseError;
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
