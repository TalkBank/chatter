import type {
  DesktopTransport,
  UpdatesCapability,
  UpdateOutcome,
} from "./contracts";

/** The transport primitives the update capability composes. */
type UpdatesTransport = Pick<
  DesktopTransport,
  | "checkForUpdate"
  | "askInstallUpdate"
  | "showMessage"
  | "onMenuCheckForUpdates"
>;

/**
 * Build the auto-update capability.
 *
 * The orchestration lives here (check, prompt, install) and is composed from
 * the transport's `@tauri-apps/plugin-updater`-backed primitives, so it is
 * unit-testable with a fake transport. Both flows are best-effort: a failure
 * never throws out of `checkOnLaunch` (a launch-time check must not block the
 * app) and is surfaced to the user in `checkNow` (a manual check must give
 * feedback).
 */
export function createUpdatesCapability(
  transport: UpdatesTransport,
): UpdatesCapability {
  // Shared check -> prompt -> install flow. Returns the outcome; any thrown
  // error propagates to the caller, which decides how to surface it.
  async function checkAndMaybeInstall(): Promise<UpdateOutcome> {
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
  }

  return {
    async checkOnLaunch(): Promise<UpdateOutcome> {
      try {
        return await checkAndMaybeInstall();
      } catch {
        return "error";
      }
    },

    async checkNow(): Promise<UpdateOutcome> {
      try {
        const outcome = await checkAndMaybeInstall();
        if (outcome === "no-update") {
          await transport.showMessage(
            "Chatter is up to date",
            "You are running the latest version.",
          );
        }
        return outcome;
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        await transport.showMessage(
          "Update check failed",
          `Could not check for updates: ${reason}`,
        );
        return "error";
      }
    },

    async onCheckRequested(handler: () => void): Promise<() => void> {
      return transport.onMenuCheckForUpdates(handler);
    },
  };
}
