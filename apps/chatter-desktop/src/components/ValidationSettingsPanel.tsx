import { useState } from "react";
import type { ValidationSettings } from "../protocol/desktopProtocol";

interface Props {
  settings: ValidationSettings;
  onChange: (settings: ValidationSettings) => void;
  disabled: boolean;
}

/**
 * Settings popover for the validation-runner config knobs
 * (`talkbank_transform::validation_runner::ValidationConfig`). These are
 * genuinely honored now that the desktop backend routes both single-file and
 * directory targets through the same shared streaming entrypoints the CLI
 * uses; before that unification the single-file path had no way to reach
 * `roundtrip`/`parser_kind`/`strict_linkers` at all.
 */
export default function ValidationSettingsPanel({ settings, onChange, disabled }: Props) {
  const [open, setOpen] = useState(false);

  return (
    <div className="validation-settings">
      <button
        type="button"
        className="validation-settings-toggle"
        onClick={() => setOpen((prev) => !prev)}
        disabled={disabled}
        aria-expanded={open}
        title="Validation settings"
      >
        {"⚙"} Settings
      </button>

      {open && (
        <div className="validation-settings-popover">
          <label>
            <input
              type="checkbox"
              checked={settings.roundtrip}
              disabled={disabled}
              onChange={(event) =>
                onChange({ ...settings, roundtrip: event.target.checked })
              }
            />
            Roundtrip check (serialize {"→"} re-parse {"→"} compare)
          </label>

          <label>
            <input
              type="checkbox"
              checked={settings.strictLinkers}
              disabled={disabled}
              onChange={(event) =>
                onChange({ ...settings, strictLinkers: event.target.checked })
              }
            />
            Strict cross-utterance linkers (E351-E355)
          </label>

          <label>
            Parser
            <select
              value={settings.parserKind}
              disabled={disabled}
              onChange={(event) =>
                onChange({
                  ...settings,
                  parserKind: event.target.value as ValidationSettings["parserKind"],
                })
              }
            >
              <option value="tree-sitter">Tree-sitter (default)</option>
              <option value="re2c">Re2c</option>
            </select>
          </label>

          <label>
            Parallel jobs
            <input
              type="number"
              min={1}
              placeholder="all CPUs"
              disabled={disabled}
              value={settings.jobs ?? ""}
              onChange={(event) => {
                const raw = event.target.value;
                onChange({ ...settings, jobs: raw === "" ? null : Number(raw) });
              }}
            />
          </label>
        </div>
      )}
    </div>
  );
}
