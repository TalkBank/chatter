import { useCallback, useEffect, useLayoutEffect, useRef, useState, type DragEvent } from "react";
import {
  useDesktopEnvironmentCapability,
  useValidationTargetCapability,
} from "../runtime/DesktopRuntimeContext";
import type { ThemePreference } from "../hooks/useTheme";
import { TargetSelection } from "../runtime/capabilities/targetSelection";

interface Props {
  onPath: (path: string) => void;
  disabled: boolean;
  lastTarget: string | null;
  theme: ThemePreference;
  onThemeChange: (theme: ThemePreference) => void;
}

export default function DropZone({
  onPath,
  disabled,
  lastTarget,
  theme,
  onThemeChange,
}: Props) {
  const environment = useDesktopEnvironmentCapability();
  const validationTarget = useValidationTargetCapability();
  const [dragging, setDragging] = useState(false);
  const selection = useRef<TargetSelection | null>(null);

  useLayoutEffect(() => {
    if (disabled) return;
    const current = new TargetSelection(validationTarget, onPath, (message) => window.alert(message));
    selection.current = current;
    return () => {
      current.close();
      selection.current = null;
    };
  }, [disabled, onPath, validationTarget]);

  const handleDragOver = useCallback((event: DragEvent) => {
    event.preventDefault();
    event.stopPropagation();
    if (!environment.isNativeDesktop()) {
      setDragging(true);
    }
  }, [environment]);

  const handleDragLeave = useCallback((event: DragEvent) => {
    event.preventDefault();
    event.stopPropagation();
    if (!environment.isNativeDesktop()) {
      setDragging(false);
    }
  }, [environment]);

  const handleDrop = useCallback((event: DragEvent) => {
    event.preventDefault();
    event.stopPropagation();
    if (!environment.isNativeDesktop()) {
      setDragging(false);
    }
  }, [environment]);

  const handleSelectedTargets = useCallback(
    (paths: string[]) => {
      selection.current?.select(paths);
    },
    [],
  );

  useEffect(() => {
    if (!environment.isNativeDesktop()) return;

    let disposed = false;
    let unlisten: (() => void) | undefined;

    void validationTarget
      .onValidationDragDrop((event) => {
        switch (event.type) {
          case "enter":
          case "over":
            setDragging(true);
            break;
          case "leave":
            setDragging(false);
            break;
          case "drop":
            setDragging(false);
            handleSelectedTargets(event.paths);
            break;
        }
      })
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
      })
      .catch((error) => {
        console.error("failed to attach native drag-and-drop listener:", error);
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [environment, handleSelectedTargets, validationTarget]);

  const shortTarget = lastTarget
    ? lastTarget.split(/[\\/]/).slice(-2).join("/")
    : null;

  return (
    <div
      className={`drop-zone ${dragging ? "dragging" : ""}`}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      <button onClick={() => void selection.current?.choose("file")} disabled={disabled}>
        Choose File
      </button>
      <button onClick={() => void selection.current?.choose("folder")} disabled={disabled}>
        Choose Folder
      </button>
      {!disabled && lastTarget && shortTarget ? (
        <span
          className="last-target-hint"
          onClick={() => handleSelectedTargets([lastTarget])}
          title={lastTarget}
        >
          Last: {shortTarget}. Re-validate?
        </span>
      ) : (
        <span className="hint">
          {dragging
            ? "Drop one file or folder to validate"
            : "or drag one .cha file or folder here"}
        </span>
      )}

      <div className="theme-toggle">
        {(["system", "light", "dark"] as const).map((t) => (
          <button
            key={t}
            className={theme === t ? "active" : ""}
            onClick={() => onThemeChange(t)}
          >
            {t === "system" ? "System" : t === "light" ? "Light" : "Dark"}
          </button>
        ))}
      </div>
    </div>
  );
}
