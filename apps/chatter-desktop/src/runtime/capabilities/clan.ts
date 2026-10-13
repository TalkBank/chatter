import { DESKTOP_COMMANDS } from "../../protocol/desktopProtocol";
import type { ClanCapability, DesktopTransport } from "./contracts";

export function createClanCapability(
  transport: Pick<DesktopTransport, "invoke">,
): ClanCapability {
  return {
    async checkClanAvailable() {
      return transport.invoke(DESKTOP_COMMANDS.checkClanAvailable);
    },

    async openInClan({ file, error, sourceContext }) {
      // One request value, the Rust command's `OpenInClanRequest`.
      await transport.invoke(DESKTOP_COMMANDS.openInClan, {
        request: {
          file,
          line: sourceContext.location.line,
          col: sourceContext.location.column,
          byteOffset: sourceContext.location.start,
          // Bare message, matching the CLI/TUI: CLAN locates the highlight
          // from this text.
          msg: error.message,
        },
      });
    },
  };
}
