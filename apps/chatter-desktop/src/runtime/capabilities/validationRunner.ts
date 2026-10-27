import { createValidationRunId, DESKTOP_COMMANDS } from "../../protocol/desktopProtocol";
import type { DesktopTransport, ValidationRunnerCapability } from "./contracts";
import { disposeOnce } from "./shared";
import { ValidationBatcher, animationFrame, type FrameScheduler } from "./validationBatch";

type Subscription =
  | { kind: "pending" | "disposed" | "failed" }
  | { kind: "registered" | "invoking" | "active"; unlisten: () => void };

export function createValidationRunnerCapability(
  transport: Pick<DesktopTransport, "invoke" | "listenValidationEvent">,
  scheduleFrame: FrameScheduler = animationFrame,
): ValidationRunnerCapability {
  // Requests reach backend startup in UI order, even when listener registration
  // or the preceding startup is delayed. Failures do not poison the next run.
  let previousStartup: Promise<void> = Promise.resolve();

  return {
    startValidation(path, settings, onEvents) {
      const runId = createValidationRunId();
      const batch = new ValidationBatcher(onEvents, scheduleFrame);
      const request = { runId, path, ...settings };
      let subscription: Subscription = { kind: "pending" };
      const isDisposed = () => subscription.kind === "disposed";
      const preparation = new AbortController();
      const abandonedPreparation = new Promise<void>((resolve) => {
        preparation.signal.addEventListener("abort", () => resolve(), { once: true });
      });
      const predecessor = previousStartup;
      const cancelOwnRun = () => transport.invoke(DESKTOP_COMMANDS.cancelValidation, {
        request: { runId },
      });

      // Defer registration until the caller owns the handle, including when a
      // transport invokes a listener synchronously during registration.
      const ready = Promise.resolve().then(async () => {
        if (isDisposed()) return;
        const unlisten = disposeOnce(await transport.listenValidationEvent((message) => {
          if ((subscription.kind === "invoking" || subscription.kind === "active")
              && message.runId === runId) {
            batch.push(message.event);
          }
        }));
        if (isDisposed()) {
          unlisten();
          return;
        }
        subscription = { kind: "registered", unlisten };
        await predecessor;
        if (isDisposed()) return;
        subscription = { kind: "invoking", unlisten };
        try {
          await transport.invoke(DESKTOP_COMMANDS.validate, {
            request,
          });
        } catch (error) {
          unlisten();
          batch.dispose();
          if (!isDisposed()) subscription = { kind: "failed" };
          throw error;
        }
        if (!isDisposed()) subscription = { kind: "active", unlisten };
      });
      // A disposed subscription that never issued a command cannot overtake
      // anything. Its delayed listener must not hold the next startup hostage.
      // This is only a sequencing barrier; `ready` still exposes startup failure.
      previousStartup = Promise.race([ready.catch(() => {}), abandonedPreparation]);

      return {
        ready,
        async cancel() {
          await ready;
          if (subscription.kind === "active") await cancelOwnRun();
        },
        dispose() {
          batch.dispose();
          const previous = subscription;
          subscription = { kind: "disposed" };
          switch (previous.kind) {
            case "registered":
              previous.unlisten();
              preparation.abort();
              break;
            case "invoking":
            case "active":
              previous.unlisten();
              // Wait until backend registration has settled, so cancellation
              // cannot arrive before the request whose ownership it revokes.
              void ready.then(cancelOwnRun, () => {}).catch((error) => {
                console.error("disposed validation could not be cancelled:", error);
              });
              break;
            case "pending":
              preparation.abort();
              break;
            case "disposed":
            case "failed":
              break;
          }
        },
      };
    },
  };
}
