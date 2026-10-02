import { useEffect, useState } from "react";
import type { RunPhase } from "../hooks/useValidation";
import { finishedRunSummary, isRunRecoverable, totalFilesOf } from "../hooks/validationState";

interface Props {
  run: RunPhase;
  /** Whether the backend has been silent past the run hook's watchdog window. */
  backendSilent: boolean;
  processedFiles: number;
  totalErrors: number;
  /** Why the cache would not open for this run, if it would not. */
  cacheUnavailable: string | null;
  startTime: number | null;
  onRevalidate: () => void;
  onCancel: () => void;
  onExport: () => void;
}

function formatEta(seconds: number): string {
  if (seconds < 60) return `~${Math.ceil(seconds)}s remaining`;
  const m = Math.floor(seconds / 60);
  const s = Math.ceil(seconds % 60);
  return `~${m}m ${s}s remaining`;
}

export default function ProgressBar({
  run,
  backendSilent,
  processedFiles,
  totalErrors,
  cacheUnavailable,
  startTime,
  onRevalidate,
  onCancel,
  onExport,
}: Props) {
  const totalFiles = totalFilesOf(run);
  const pct = totalFiles > 0 ? (processedFiles / totalFiles) * 100 : 0;

  // Update ETA every second during validation
  const [, setTick] = useState(0);
  useEffect(() => {
    if (run.kind !== "running" || !startTime) return;
    const id = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(id);
  }, [run, startTime]);

  let etaText: string | null = null;
  if (run.kind === "running" && startTime && processedFiles >= 5 && processedFiles < totalFiles) {
    const elapsed = (Date.now() - startTime) / 1000;
    const perFile = elapsed / processedFiles;
    const remaining = perFile * (totalFiles - processedFiles);
    etaText = formatEta(remaining);
  }

  return (
    <div className="status-bar">
      {run.kind === "idle" && <span>Ready</span>}

      {run.kind === "invoked" && (
        <span>
          Starting{"…"}
          {backendSilent && (
            <span className="warning-text">
              {" "}
              The validator has not responded yet. If this persists, please
              report it: nothing has begun scanning, so the fault is at startup
              rather than in your files.
            </span>
          )}
        </span>
      )}

      {run.kind === "aborted" && (
        <span className="error-count-text">{run.reason}</span>
      )}

      {/* Leads with what was NOT checked, because the counts beside it are
          about the rest and would otherwise read as the run's totals. */}
      {run.kind === "finishedIncomplete" && (
        <span className="error-count-text">
          Incomplete: {run.lostFiles} of {run.stats.totalFiles} files were never
          checked: {run.cause} Of the rest, {run.stats.validFiles} valid,{" "}
          {run.stats.invalidFiles} invalid.
        </span>
      )}

      {/* Like incomplete, it leads with what was NOT checked. */}
      {run.kind === "stopped" && (
        <span className="error-count-text">
          {run.reason}: {run.unprocessedFiles} of {run.stats.totalFiles} files
          were not checked. Of the rest, {run.stats.validFiles} valid,{" "}
          {run.stats.invalidFiles} invalid.
        </span>
      )}

      {run.kind === "nothingFound" && (
        <span className="error-count-text">No CHAT files found</span>
      )}

      {run.kind === "discovering" && <span>Discovering files...</span>}

      {run.kind === "running" && (
        <>
          <div className="progress-track">
            <div className="progress-fill" style={{ width: `${pct}%` }} />
          </div>
          <span className="progress-text">
            {processedFiles}/{totalFiles}
          </span>
          {totalErrors > 0 && (
            <span className="error-count-text">{totalErrors} errors</span>
          )}
          {etaText && <span className="eta-text">{etaText}</span>}
        </>
      )}

      {run.kind === "finished" && (
        <span>
          {finishedRunSummary(run, totalErrors, cacheUnavailable)}
        </span>
      )}

      <div className="actions">
        {run.kind === "running" && (
          <button onClick={onCancel}>Cancel</button>
        )}
        {isRunRecoverable(run) && (
          <button className="primary" onClick={onRevalidate}>
            Re-validate
          </button>
        )}
        {/* Export stays finished-only: an aborted run produced no results to
            export, so isRunRecoverable is deliberately not used here. */}
        {run.kind === "finished" && <button onClick={onExport}>Export</button>}
      </div>
    </div>
  );
}
