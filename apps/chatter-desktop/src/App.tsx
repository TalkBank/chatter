import { useCallback, useEffect, useState } from "react";
import AboutModal from "./components/AboutModal";
import DropZone from "./components/DropZone";
import ErrorPanel from "./components/ErrorPanel";
import FileTree from "./components/FileTree";
import OnboardingOverlay from "./components/OnboardingOverlay";
import ProgressBar from "./components/ProgressBar";
import ValidationSettingsPanel from "./components/ValidationSettingsPanel";
import { useTheme } from "./hooks/useTheme";
import { useValidation } from "./hooks/useValidation";
import { DEFAULT_VALIDATION_SETTINGS, type ValidationSettings } from "./protocol/desktopProtocol";
import type { ParseError } from "./protocol/validation";
import {
  useAboutCapability,
  useClanCapability,
  useExportCapability,
  useUpdatesCapability,
} from "./runtime/DesktopRuntimeContext";

export default function App() {
  const { theme, setTheme } = useTheme();
  const clan = useClanCapability();
  const exportCapability = useExportCapability();
  const updates = useUpdatesCapability();
  const about = useAboutCapability();
  const { state, startValidation, cancelValidation, reset } = useValidation();
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [clanAvailable, setClanAvailable] = useState(false);
  const [lastTarget, setLastTarget] = useState<string | null>(
    () => localStorage.getItem("chatter-last-target"),
  );
  const [startTime, setStartTime] = useState<number | null>(null);
  const [validationSettings, setValidationSettings] = useState<ValidationSettings>(
    DEFAULT_VALIDATION_SETTINGS,
  );

  useEffect(() => {
    clan.checkClanAvailable().then(setClanAvailable).catch(() => {});
  }, [clan]);

  // Update checks: on launch, periodically in the background, and on demand
  // from the "Check for Updates..." app-menu item. The launch-only check
  // missed long-running or rarely-relaunched installs (a host sat weeks
  // behind), so a 6-hour background check plus a manual trigger close that
  // gap. Every path is best-effort and never throws.
  useEffect(() => {
    void updates.checkOnLaunch();

    const SIX_HOURS_MS = 6 * 60 * 60 * 1000;
    const interval = setInterval(() => {
      void updates.checkOnLaunch();
    }, SIX_HOURS_MS);

    // Guard the async listener registration against cleanup running before the
    // subscribe promise resolves (React StrictMode double-mounts in dev, or any
    // effect re-run). Without the flag, `unsubscribe` is still undefined at
    // cleanup, the pending listener is never removed, and multiple live
    // listeners stack, so one menu click fires `checkNow` (and its dialog)
    // several times.
    let cancelled = false;
    let unsubscribe: (() => void) | undefined;
    void updates
      .onCheckRequested(() => {
        void updates.checkNow();
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unsubscribe = fn;
        }
      });

    return () => {
      cancelled = true;
      clearInterval(interval);
      unsubscribe?.();
    };
  }, [updates]);

  // Open the About modal when the "About Chatter" menu item fires. Same
  // cancelled-flag-safe async subscription as the update listener above.
  useEffect(() => {
    let cancelled = false;
    let unsubscribe: (() => void) | undefined;
    void about
      .onAboutRequested(() => {
        setAboutOpen(true);
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unsubscribe = fn;
        }
      });
    return () => {
      cancelled = true;
      unsubscribe?.();
    };
  }, [about]);

  // Track validation start time for ETA
  useEffect(() => {
    if (state.phase === "running" && startTime === null) {
      setStartTime(Date.now());
    }
    if (state.phase === "finished" || state.phase === "idle") {
      setStartTime(null);
    }
  }, [state.phase, startTime]);

  // Update window title based on validation state
  useEffect(() => {
    switch (state.phase) {
      case "idle":
        document.title = "Chatter";
        break;
      case "discovering":
        document.title = "Chatter \u00b7 Discovering files\u2026";
        break;
      case "running":
        document.title = `Chatter \u00b7 Validating (${state.processedFiles}/${state.totalFiles})`;
        break;
      case "finished": {
        if (state.stats) {
          const { invalidFiles, totalFiles } = state.stats;
          if (invalidFiles === 0) {
            document.title = `Chatter \u00b7 All ${totalFiles} files valid`;
          } else {
            document.title = `Chatter \u00b7 ${state.totalErrors} errors in ${invalidFiles} files`;
          }
        } else {
          document.title = "Chatter";
        }
        break;
      }
    }
  }, [state.phase, state.processedFiles, state.totalFiles, state.totalErrors, state.stats]);

  // Send notification when validation finishes and window is not focused
  useEffect(() => {
    if (state.phase !== "finished" || !state.stats) return;
    if (document.hasFocus()) return;

    const { invalidFiles } = state.stats;
    const body =
      invalidFiles === 0
        ? `All ${state.stats.totalFiles} files valid`
        : `${state.totalErrors} errors in ${invalidFiles} files`;

    if ("Notification" in window && Notification.permission === "granted") {
      new Notification("Validation complete", { body });
    } else if ("Notification" in window && Notification.permission !== "denied") {
      void Notification.requestPermission().then((perm) => {
        if (perm === "granted") {
          new Notification("Validation complete", { body });
        }
      });
    }
  }, [state.phase, state.stats, state.totalErrors]);

  const handlePath = useCallback(
    (path: string) => {
      setLastTarget(path);
      localStorage.setItem("chatter-last-target", path);
      setSelectedFile(null);
      setStartTime(null);
      startValidation(path, validationSettings);
    },
    [startValidation, validationSettings],
  );

  const handleRevalidate = useCallback(() => {
    if (lastTarget) {
      reset();
      setSelectedFile(null);
      setStartTime(null);
      startValidation(lastTarget, validationSettings);
    }
  }, [lastTarget, reset, startValidation, validationSettings]);

  const handleOpenInClan = useCallback(
    async (file: string, error: ParseError) => {
      try {
        await clan.openInClan({ file, error });
      } catch (err) {
        console.error("open_in_clan failed:", err);
        alert(`Open in CLAN failed: ${err}`);
      }
    },
    [clan],
  );

  const handleRevealFile = useCallback(async (path: string) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("reveal_in_file_manager", { path });
    } catch (err) {
      console.error("reveal_in_file_manager failed:", err);
    }
  }, []);

  const handleExport = useCallback(async () => {
    // Guard explicitly rather than relying solely on the Export button's own
    // `phase === "finished"` gating in ProgressBar: `state.files` only holds a
    // complete, stable result set once the run has actually finished, and this
    // handler should not derive an export from a still-streaming partial set.
    if (state.phase !== "finished") {
      console.error("export requested before validation finished; ignoring");
      return;
    }

    try {
      const path = await exportCapability.chooseExportPath();
      if (!path) return;

      const format = path.endsWith(".json") ? "json" : "text";
      const results = [...state.files.values()].map((file) => ({
        path: file.path,
        errors: file.diagnostics.map((diagnostic) => ({
          ...diagnostic.error,
          renderedText: diagnostic.renderedText,
        })),
        status: file.status,
      }));

      await exportCapability.exportResults(results, format, path);
    } catch (err) {
      console.error("export failed:", err);
      alert(`Export failed: ${err}`);
    }
  }, [exportCapability, state.files, state.phase]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const isRunning = state.phase === "running" || state.phase === "discovering";

      if ((event.ctrlKey || event.metaKey) && event.key === "o") {
        event.preventDefault();
      }

      if ((event.ctrlKey || event.metaKey) && event.key === "r") {
        event.preventDefault();
        if (!isRunning && lastTarget) {
          handleRevalidate();
        }
      }

      if (event.key === "Escape" && isRunning) {
        event.preventDefault();
        cancelValidation();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [state.phase, lastTarget, handleRevalidate, cancelValidation]);

  const selectedEntry = selectedFile ? state.files.get(selectedFile) ?? null : null;
  const isRunning = state.phase === "running" || state.phase === "discovering";

  return (
    <div className="app">
      <OnboardingOverlay />
      <AboutModal open={aboutOpen} onClose={() => setAboutOpen(false)} />
      <div className="drop-zone-area">
        <span className="app-wordmark">
          Chatter
          <span className="mark" aria-hidden="true">
            {"¶"}
          </span>
        </span>
        <DropZone
          onPath={handlePath}
          disabled={isRunning}
          lastTarget={state.phase === "idle" ? lastTarget : null}
          theme={theme}
          onThemeChange={setTheme}
        />
        <ValidationSettingsPanel
          settings={validationSettings}
          onChange={setValidationSettings}
          disabled={isRunning}
        />
      </div>
      <div className="main-panels">
        <FileTree
          files={state.files}
          totalFiles={state.totalFiles}
          phase={state.phase}
          selectedFile={selectedFile}
          onSelectFile={setSelectedFile}
        />
        <ErrorPanel
          file={selectedEntry}
          clanAvailable={clanAvailable}
          onOpenInClan={handleOpenInClan}
          onRevealFile={handleRevealFile}
        />
      </div>
      <ProgressBar
        phase={state.phase}
        processedFiles={state.processedFiles}
        totalFiles={state.totalFiles}
        totalErrors={state.totalErrors}
        stats={state.stats}
        startTime={startTime}
        onRevalidate={handleRevalidate}
        onCancel={cancelValidation}
        onExport={handleExport}
      />
    </div>
  );
}
