import type {
  DesktopTransport,
  UpdatesCapability,
  UpdateOutcome,
} from "./contracts";

/**
 * Build the launch-time auto-update capability.
 *
 * The orchestration lives here (check, prompt, install) and is composed from
 * the transport's two `@tauri-apps/plugin-updater`-backed primitives, so it is
 * unit-testable with a fake transport. The whole flow is best-effort: any
 * failure resolves to an "error" outcome rather than throwing, because a
 * failed update check must never block or crash a clinician's app.
 */
export function createUpdatesCapability(
  transport: Pick<DesktopTransport, "checkForUpdate" | "askInstallUpdate">,
): UpdatesCapability {
  return {
    async checkOnLaunch(): Promise<UpdateOutcome> {
      try {
        const update = await transport.checkForUpdate();
        if (update === null) {
          return "no-update";
        }
        const accepted = await transport.askInstallUpdate(
          update.version,
          update.currentVersion,
          update.notes,
        );
        if (!accepted) {
          return "declined";
        }
        await update.install();
        return "installing";
      } catch {
        return "error";
      }
    },
  };
}
