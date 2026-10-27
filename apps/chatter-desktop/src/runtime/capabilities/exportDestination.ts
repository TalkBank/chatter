import type { ExportFormat } from "../../protocol/desktopProtocol";

/** One selected path with its derived format; callers cannot pair them freely. */
export class ExportDestination {
  readonly #format: ExportFormat;

  private constructor(readonly path: string) {
    this.#format = path.toLowerCase().endsWith(".json") ? "json" : "text";
  }

  static fromPath(path: string): ExportDestination {
    if (path.length === 0) throw new Error("Choose an export file name.");
    return new ExportDestination(path);
  }

  get format(): ExportFormat {
    return this.#format;
  }
}
