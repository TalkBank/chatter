import type { ExportFormat } from "../protocol/desktopProtocol";
import { createAboutCapability } from "./capabilities/about";
import { createClanCapability } from "./capabilities/clan";
import type {
  AboutCapability,
  ClanCapability,
  DesktopEnvironmentCapability,
  DesktopRuntime,
  DesktopTransport,
  ExportCapability,
  OpenInClanRequest,
  UpdatesCapability,
  ValidationDragDropEvent,
  ValidationExportEntry,
  ValidationRun,
  ValidationRunnerCapability,
  ValidationTargetCapability,
} from "./capabilities/contracts";
import { createDesktopEnvironmentCapability } from "./capabilities/environment";
import { createExportCapability } from "./capabilities/exportResults";
import { createUpdatesCapability } from "./capabilities/updates";
import { createValidationRunnerCapability } from "./capabilities/validationRunner";
import { createValidationTargetCapability } from "./capabilities/validationTarget";
import { tauriTransport } from "./tauriTransport";

export function createDesktopRuntime(
  transport: DesktopTransport = tauriTransport,
): DesktopRuntime {
  return {
    environment: createDesktopEnvironmentCapability(transport),
    validationRunner: createValidationRunnerCapability(transport),
    validationTarget: createValidationTargetCapability(transport),
    clan: createClanCapability(transport),
    exports: createExportCapability(transport),
    updates: createUpdatesCapability(transport),
    about: createAboutCapability(transport),
  };
}

export const desktopRuntime = createDesktopRuntime();

export type {
  AboutCapability,
  ClanCapability,
  DesktopEnvironmentCapability,
  DesktopRuntime,
  ExportCapability,
  OpenInClanRequest,
  UpdatesCapability,
  ValidationDragDropEvent,
  ValidationExportEntry,
  ValidationRun,
  ValidationRunnerCapability,
  ValidationTargetCapability,
};
export type { ExportFormat };
