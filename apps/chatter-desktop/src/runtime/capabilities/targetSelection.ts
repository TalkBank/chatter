import type { ValidationTargetCapability } from "./contracts";

type PickerKind = "file" | "folder";
type SelectionState =
  | { kind: "ready" }
  | { kind: "picking" }
  | { kind: "closed" };

/** One enabled interval owns at most one accepted validation target. */
export class TargetSelection {
  #state: SelectionState = { kind: "ready" };

  constructor(
    private readonly capability: Pick<ValidationTargetCapability,
      "chooseValidationFile" | "chooseValidationFolder">,
    private readonly onPath: (path: string) => void,
    private readonly report: (message: string) => void,
  ) {}

  /** Retiring an interval also invalidates its outstanding native dialog. */
  close(): void {
    this.#state = { kind: "closed" };
  }

  /** Drops and recent-target clicks share the same admission as pickers. */
  select(paths: readonly string[]): void {
    if (this.#state.kind !== "ready") return;
    const selected = paths.filter(Boolean);
    if (selected.length === 0) return;
    if (selected.length !== 1) {
      this.report("Chatter validates one .cha file or one folder at a time.");
      return;
    }
    this.close();
    try {
      this.onPath(selected[0]);
    } catch (error) {
      this.#state = { kind: "ready" };
      this.report(`Start validation failed: ${error}`);
    }
  }

  async choose(kind: PickerKind): Promise<void> {
    if (this.#state.kind !== "ready") return;
    const pending: SelectionState = { kind: "picking" };
    this.#state = pending;
    let selected: string | null;
    try {
      selected = kind === "file"
        ? await this.capability.chooseValidationFile()
        : await this.capability.chooseValidationFolder();
    } catch (error) {
      if (this.#state === pending) {
        this.#state = { kind: "ready" };
        this.report(`Choose ${kind} failed: ${error}`);
      }
      return;
    }
    if (this.#state !== pending) return;
    this.#state = { kind: "ready" };
    if (selected !== null) this.select([selected]);
  }
}
