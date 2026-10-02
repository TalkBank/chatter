import { useState } from "react";
import { parseJobCount, type ValidationSettings } from "../protocol/desktopProtocol";

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
  // The field's own text, so an entry that is not a count stays visible
  // (and says why) instead of being coerced or silently dropped.
  const [jobsText, setJobsText] = useState(settings.jobs === null ? "" : String(settings.jobs));
  const jobsInput = parseJobCount(jobsText);

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
            Strict cross-utterance linkers
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
              <option value="re2c">Re2c (experimental, incomplete)</option>
            </select>
          </label>

          <label>
            Parallel jobs
            <input
              type="number"
              min={1}
              step={1}
              placeholder="all CPUs"
              disabled={disabled}
              value={jobsText}
              aria-invalid={jobsInput.kind === "invalid"}
              onChange={(event) => {
                const raw = event.target.value;
                setJobsText(raw);
                const parsed = parseJobCount(raw);
                switch (parsed.kind) {
                  case "allCpus":
                    onChange({ ...settings, jobs: null });
                    break;
                  case "count":
                    onChange({ ...settings, jobs: parsed.count });
                    break;
                  case "invalid":
                    // The setting keeps its last valid value; the hint below
                    // says the entry was not taken.
                    break;
                }
              }}
            />
          </label>
          {jobsInput.kind === "invalid" && (
            <p className="validation-settings-hint" role="alert">
              Parallel jobs must be a whole number of at least 1, or empty for all CPUs.
              {" "}The next run uses{" "}
              {settings.jobs === null ? "all CPUs" : `${settings.jobs} job(s)`}, the last
              valid entry.
            </p>
          )}
        </div>
      )}
    </div>
  );
}
