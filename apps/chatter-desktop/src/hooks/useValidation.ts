import { useCallback, useEffect, useRef, useState } from "react";
import { useValidationRunnerCapability } from "../runtime/DesktopRuntimeContext";
import type { ValidationRun } from "../runtime/desktopRuntime";
import type { ValidationSettings } from "../protocol/desktopProtocol";
import {
  applyValidationEvents,
  createInitialValidationState,
  isAwaitingBackend,
  relativeDisplayName,
  type ValidationState,
} from "./validationState";

export type { RunPhase, ValidationState } from "./validationState";

/**
 * How long the backend may stay silent after a validate command before the UI
 * says so. Everything in that window is config construction, opening the
 * cache, and spawning a thread, so there is no legitimate slow case here. This
 * deliberately does NOT apply to `discovering`, where a large tree genuinely
 * takes time; that distinction is only expressible because the two phases are
 * separate values.
 */
const BACKEND_SILENCE_WARNING_MS = 10_000;

/**
 * Hook that manages all validation state from the Tauri event stream.
 *
 * Accumulates per-file results from the desktop runtime's validation event stream.
 */
export function useValidation() {
  const validationRunner = useValidationRunnerCapability();
  const [state, setState] = useState<ValidationState>(createInitialValidationState);
  const runRef = useRef<ValidationRun | null>(null);

  // "The backend has been silent past BACKEND_SILENCE_WARNING_MS" is a
  // property of the RUN, not of whichever component happens to be mounted
  // watching it, so it lives here rather than in `ProgressBar`: a component
  // unmount must not silently restart the clock. Derived from the phase and a
  // timer rather than stored in `ValidationState` itself, since a mirrored
  // boolean there would be a second representation of "are we still
  // waiting", free to disagree with it.
  const [backendSilent, setBackendSilent] = useState(false);

  useEffect(() => {
    if (!isAwaitingBackend(state.run)) {
      setBackendSilent(false);
      return;
    }
    const timer = setTimeout(() => setBackendSilent(true), BACKEND_SILENCE_WARNING_MS);
    return () => clearTimeout(timer);
  }, [state.run]);

  const disposeRun = useCallback(() => {
    runRef.current?.dispose();
    runRef.current = null;
  }, []);

  const startValidation = useCallback(async (path: string, settings: ValidationSettings) => {
    disposeRun();

    // Requesting validation does not prove that backend discovery has begun.
    // See RunPhase's distinction between local requests and observed progress.
    setState({ ...createInitialValidationState(), run: { kind: "invoked" } });

    let ownedRun: ValidationRun | null = null;
    try {
      const run = validationRunner.startValidation(path, settings, (events) => {
        setState((prev) => runRef.current === run
          ? applyValidationEvents(prev, events, (fullPath) => relativeDisplayName(fullPath, path))
          : prev);
      });
      ownedRun = run;
      runRef.current = run;
      await run.ready;
    } catch (err) {
      if (runRef.current !== ownedRun) return;
      console.error("validate command failed:", err);
      window.alert(`Validation failed: ${String(err)}`);
      disposeRun();
      setState((prev) => ({
        ...prev,
        run: { kind: "aborted", reason: `Validation failed to start: ${String(err)}` },
      }));
    }
  }, [disposeRun, validationRunner]);

  const cancelValidation = useCallback(async () => {
    try {
      await runRef.current?.cancel();
    } catch (err) {
      console.error("cancel failed:", err);
    }
  }, []);

  const reset = useCallback(() => {
    disposeRun();
    setState(createInitialValidationState());
  }, [disposeRun]);

  useEffect(() => () => {
    disposeRun();
  }, [disposeRun]);

  return { state, startValidation, cancelValidation, reset, backendSilent };
}
