import { useCallback, useEffect, useRef, useState } from "react";
import { useValidationRunnerCapability } from "../runtime/DesktopRuntimeContext";
import type { ValidationRun } from "../runtime/desktopRuntime";
import type { ValidationSettings } from "../protocol/desktopProtocol";
import {
  applyValidationEvent,
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
  /** Selected validation target used for computing relative display names */
  const rootRef = useRef<string>("");

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

  const relativeName = useCallback((fullPath: string): string => {
    return relativeDisplayName(fullPath, rootRef.current);
  }, []);

  const startValidation = useCallback(async (path: string, settings: ValidationSettings) => {
    disposeRun();

    rootRef.current = path;

    // "invoked", NOT "discovering": the backend has not spoken yet, and the
    // difference is what makes backend silence observable. See RunPhase's docs.
    setState({ ...createInitialValidationState(), run: { kind: "invoked" } });

    try {
      runRef.current = await validationRunner.startValidation(path, settings, (event) => {
        setState((prev) => applyValidationEvent(prev, event, relativeName));
      });
    } catch (err) {
      console.error("validate command failed:", err);
      window.alert(`Validation failed: ${String(err)}`);
      disposeRun();
      setState((prev) => ({
        ...prev,
        run: { kind: "aborted", reason: `Validation failed to start: ${String(err)}` },
      }));
    }
  }, [disposeRun, relativeName, validationRunner]);

  const cancelValidation = useCallback(async () => {
    try {
      await runRef.current?.cancel();
    } catch (err) {
      console.error("cancel failed:", err);
    }
  }, []);

  const reset = useCallback(() => {
    rootRef.current = "";
    disposeRun();
    setState(createInitialValidationState());
  }, [disposeRun]);

  useEffect(() => () => {
    disposeRun();
  }, [disposeRun]);

  return { state, startValidation, cancelValidation, reset, backendSilent };
}
