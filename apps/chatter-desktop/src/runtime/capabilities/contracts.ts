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
  /**
   * Check for a newer release on demand (the "Check for Updates..." menu
   * item). Behaves like {@link checkOnLaunch} when an update exists, but when
   * the app is already current it tells the user so, since a manual check
   * must give visible feedback. Never throws.
   */
  checkNow(): Promise<UpdateOutcome>;
  /**
   * Subscribe to the "Check for Updates..." app-menu item. Returns an
   * unsubscribe function. Kept here (not as a raw event listener in a
   * component) so `@tauri-apps/*` stays behind the runtime seam.
   */
  onCheckRequested(handler: () => void): Promise<() => void>;
}

/**
 * The "About Chatter" surface: menu-triggered modal, its version string, and
 * opening its links in the OS browser. Behind the runtime seam so components
 * do not touch `@tauri-apps/*` directly.
 */
export interface AboutCapability {
  /**
   * Subscribe to the "About Chatter" app-menu item. Returns an unsubscribe
   * function.
   */
  onAboutRequested(handler: () => void): Promise<() => void>;
  /** Open an external `http(s)` URL in the user's default browser. */
  openExternal(url: string): Promise<void>;
  /** The running app version (e.g. "0.3.2"), for display in the About modal. */
  version(): Promise<string>;
}

export interface DesktopRuntime {
  environment: DesktopEnvironmentCapability;
  validationRunner: ValidationRunnerCapability;
  validationTarget: ValidationTargetCapability;
  clan: ClanCapability;
  exports: ExportCapability;
  updates: UpdatesCapability;
  about: AboutCapability;
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
  /**
   * Show a simple informational dialog. Used by a manual update check to
   * report "you are up to date" or a check failure, cases where the
   * launch-time flow would otherwise stay silent.
   */
  showMessage(title: string, message: string): Promise<void>;
  /**
   * Subscribe to the native "Check for Updates..." menu item (the backend
   * emits `menu://check-for-updates` on click). Returns an unsubscribe
   * function; a no-op unsubscribe in the non-desktop shell.
   */
  onMenuCheckForUpdates(listener: () => void): Promise<() => void>;
  /**
   * Subscribe to the native "About Chatter" menu item (the backend emits
   * `menu://about` on click). Returns an unsubscribe function; a no-op
   * unsubscribe in the non-desktop shell.
   */
  onMenuAbout(listener: () => void): Promise<() => void>;
  /**
   * Open an external `http(s)` URL in the OS default browser (via the
   * `open_external` command). Rejected for non-http(s) URLs by the backend.
   */
  openExternalUrl(url: string): Promise<void>;
  /** The running app version string, from the Tauri app metadata. */
  getAppVersion(): Promise<string>;
}
