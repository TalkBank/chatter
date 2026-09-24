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

type ActiveCheck = {
  phase: "checking";
  feedback: "silent" | "manual";
  result: Promise<UpdateOutcome>;
};
type CheckState = { phase: "idle" } | ActiveCheck;

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
  let state: CheckState = { phase: "idle" };
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

  async function perform(check: ActiveCheck): Promise<UpdateOutcome> {
    try {
      const outcome = await checkAndMaybeInstall();
      if (check.feedback === "manual" && outcome === "no-update") {
        await transport.showMessage(
          "Chatter is up to date",
          "You are running the latest version.",
        );
      }
      return outcome;
    } catch (error) {
      if (check.feedback === "manual") {
        const reason = error instanceof Error ? error.message : String(error);
        try {
          await transport.showMessage(
            "Update check failed",
            `Could not check for updates: ${reason}`,
          );
        } catch {
          // A failed native dialog cannot turn this best-effort command into
          // an unhandled rejection in the menu's fire-and-forget callback.
        }
      }
      return "error";
    }
  }

  function request(feedback: ActiveCheck["feedback"]): Promise<UpdateOutcome> {
    if (state.phase === "checking") {
      if (feedback === "manual") state.feedback = "manual";
      return state.result;
    }
    // Install the capability before starting any transport work. Launch,
    // periodic and manual triggers share its prompt/install and completion.
    const check: ActiveCheck = {
      phase: "checking",
      feedback,
      result: Promise.resolve().then(() => perform(check)).finally(() => {
        state = { phase: "idle" };
      }),
    };
    state = check;
    return check.result;
  }

  return {
    checkOnLaunch(): Promise<UpdateOutcome> {
      return request("silent");
    },

    checkNow(): Promise<UpdateOutcome> {
      return request("manual");
    },

    async onCheckRequested(handler: () => void): Promise<() => void> {
      return transport.onMenuCheckForUpdates(handler);
    },
  };
}
