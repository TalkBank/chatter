import type {
  DesktopCommandArgs,
  DesktopCommandName,
  DesktopCommandResultMap,
  ExportFormat,
  ValidationEventPayload,
  ValidationSettings,
} from "../../protocol/desktopProtocol";
import type { FileStatus, ParseError, ValidationEvent } from "../../protocol/validation";

/** A `ParseError` paired with its pre-rendered miette text, for text export.
 *
 * Carrying `renderedText` through lets the Rust export command reuse the
 * canonical rendering already computed once in `events.rs` instead of
 * hand-rebuilding a poorer "path:line: code msg" line from raw fields.
 */
export interface ValidationExportError extends ParseError {
  renderedText: string;
}

export interface ValidationExportEntry {
  path: string;
  errors: ValidationExportError[];
  status: FileStatus | null;
}

export type { ParserKindSetting, ValidationSettings } from "../../protocol/desktopProtocol";

export interface OpenInClanRequest {
  file: string;
  error: ParseError;
}

export type ValidationDragDropEvent =
  | { type: "enter" | "over" | "leave" }
  | { type: "drop"; paths: string[] };

export interface ValidationRun {
  cancel(): Promise<void>;
  dispose(): void;
}

export interface DesktopEnvironmentCapability {
  isNativeDesktop(): boolean;
}

export interface ValidationRunnerCapability {
  startValidation(
    path: string,
    settings: ValidationSettings,
    onEvent: (event: ValidationEvent) => void,
  ): Promise<ValidationRun>;
}

export interface ValidationTargetCapability {
  chooseValidationFile(): Promise<string | null>;
  chooseValidationFolder(): Promise<string | null>;
  onValidationDragDrop(
    listener: (event: ValidationDragDropEvent) => void,
  ): Promise<() => void>;
}

export interface ClanCapability {
  checkClanAvailable(): Promise<boolean>;
  openInClan(request: OpenInClanRequest): Promise<void>;
}

export interface ExportCapability {
  chooseExportPath(): Promise<string | null>;
  exportResults(
    results: ValidationExportEntry[],
    format: ExportFormat,
    path: string,
  ): Promise<void>;
}

/**
 * An update that the updater found to be newer than the running app.
 * `install()` downloads, installs, and relaunches into the new version.
 */
export interface AvailableUpdate {
  version: string;
  currentVersion: string;
  notes: string | null;
  install(): Promise<void>;
}

/**
 * Outcome of a best-effort launch-time update check. A failed check is an
 * "error" outcome, never a thrown exception: update checking must never
 * block or crash the app.
 */
export type UpdateOutcome =
  | "no-update"
  | "declined"
  | "installing"
  | "error";

export interface UpdatesCapability {
  /**
   * Check for a newer release on launch; if one exists, prompt the user and,
   * on acceptance, install it and relaunch. Never throws.
   */
  checkOnLaunch(): Promise<UpdateOutcome>;
}

export interface DesktopRuntime {
  environment: DesktopEnvironmentCapability;
  validationRunner: ValidationRunnerCapability;
  validationTarget: ValidationTargetCapability;
  clan: ClanCapability;
  exports: ExportCapability;
  updates: UpdatesCapability;
}

export interface DesktopTransport {
  isNativeDesktop(): boolean;
  invoke<C extends DesktopCommandName>(
    command: C,
    ...args: DesktopCommandArgs<C>
  ): Promise<DesktopCommandResultMap[C]>;
  listenValidationEvent(
    listener: (event: ValidationEventPayload) => void,
  ): Promise<() => void>;
  chooseValidationFile(): Promise<string | string[] | null>;
  chooseValidationFolder(): Promise<string | string[] | null>;
  chooseExportPath(): Promise<string | null>;
  onValidationDragDrop(
    listener: (event: ValidationDragDropEvent) => void,
  ): Promise<() => void>;
  /**
   * Check for an available update. Returns the update (with an `install()`
   * that downloads, installs, and relaunches) or null when none is available
   * or the app is not running as a native desktop build.
   */
  checkForUpdate(): Promise<AvailableUpdate | null>;
  /** Ask the user whether to install the named update. Returns their choice. */
  askInstallUpdate(
    version: string,
    currentVersion: string,
    notes: string | null,
  ): Promise<boolean>;
}
