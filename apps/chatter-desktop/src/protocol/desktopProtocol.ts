import type { ValidationEvent } from "./validation";

export const DESKTOP_COMMANDS = {
  validate: "validate",
  cancelValidation: "cancel_validation",
  checkClanAvailable: "check_clan_available",
  openInClan: "open_in_clan",
  exportResults: "export_results",
  revealInFileManager: "reveal_in_file_manager",
} as const;

export const DESKTOP_EVENTS = {
  validation: "validation-event",
} as const;

export type DesktopCommandName =
  (typeof DESKTOP_COMMANDS)[keyof typeof DESKTOP_COMMANDS];

export type DesktopEventName =
  (typeof DESKTOP_EVENTS)[keyof typeof DESKTOP_EVENTS];

export type ExportFormat = "json" | "text";

/** Which parser backend to validate with. Mirrors Rust `ParserKindRequest`. */
export type ParserKindSetting = "tree-sitter" | "re2c";

/** User-configurable validation settings, threaded through to `ValidationConfig`. */
export interface ValidationSettings {
  roundtrip: boolean;
  parserKind: ParserKindSetting;
  strictLinkers: boolean;
  /** Number of parallel validation jobs; `null` = use all CPUs. */
  jobs: number | null;
}

/** Matches `ValidationConfig::default()` on the Rust side. */
export const DEFAULT_VALIDATION_SETTINGS: ValidationSettings = {
  roundtrip: false,
  parserKind: "tree-sitter",
  strictLinkers: false,
  jobs: null,
};

export interface ValidateCommandArgs extends ValidationSettings {
  path: string;
}

export interface OpenInClanCommandArgs {
  file: string;
  line: number;
  col: number;
  byteOffset: number;
  msg: string;
}

export interface ExportResultsCommandArgs {
  results: string;
  format: ExportFormat;
  path: string;
}

export interface RevealInFileManagerCommandArgs {
  path: string;
}

export type DesktopCommandPayloadMap = {
  [DESKTOP_COMMANDS.validate]: ValidateCommandArgs;
  [DESKTOP_COMMANDS.cancelValidation]: undefined;
  [DESKTOP_COMMANDS.checkClanAvailable]: undefined;
  [DESKTOP_COMMANDS.openInClan]: OpenInClanCommandArgs;
  [DESKTOP_COMMANDS.exportResults]: ExportResultsCommandArgs;
  [DESKTOP_COMMANDS.revealInFileManager]: RevealInFileManagerCommandArgs;
};

export type DesktopCommandResultMap = {
  [DESKTOP_COMMANDS.validate]: void;
  [DESKTOP_COMMANDS.cancelValidation]: void;
  [DESKTOP_COMMANDS.checkClanAvailable]: boolean;
  [DESKTOP_COMMANDS.openInClan]: void;
  [DESKTOP_COMMANDS.exportResults]: void;
  [DESKTOP_COMMANDS.revealInFileManager]: void;
};

export type DesktopCommandArgs<C extends DesktopCommandName> =
  DesktopCommandPayloadMap[C] extends undefined
    ? []
    : [payload: DesktopCommandPayloadMap[C]];

export type ValidationEventPayload = ValidationEvent;
