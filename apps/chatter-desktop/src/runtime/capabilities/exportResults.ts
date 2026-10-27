import { DESKTOP_COMMANDS } from "../../protocol/desktopProtocol";
import type { DesktopTransport, ExportCapability } from "./contracts";
import { ExportDestination } from "./exportDestination";

export function createExportCapability(
  transport: Pick<DesktopTransport, "chooseExportPath" | "invoke">,
): ExportCapability {
  return {
    async chooseExportDestination() {
      const path = await transport.chooseExportPath();
      return path === null ? null : ExportDestination.fromPath(path);
    },

    async exportResults(results, destination) {
      // One request value, the Rust command's `ExportResultsRequest`.
      await transport.invoke(DESKTOP_COMMANDS.exportResults, {
        request: {
          results: JSON.stringify(results),
          format: destination.format,
          path: destination.path,
        },
      });
    },
  };
}
