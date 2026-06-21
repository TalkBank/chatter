import { DESKTOP_COMMANDS } from "../../protocol/desktopProtocol";
import type { ClanCapability, DesktopTransport } from "./contracts";

export function createClanCapability(
  transport: Pick<DesktopTransport, "invoke">,
): ClanCapability {
  return {
    async checkClanAvailable() {
      return transport.invoke(DESKTOP_COMMANDS.checkClanAvailable);
    },

    async openInClan({ file, error }) {
      await transport.invoke(DESKTOP_COMMANDS.openInClan, {
        file,
        line: error.location.line ?? 0,
        col: error.location.column ?? 0,
        byteOffset: error.location.start,
        // Bare message, matching the CLI/TUI. CLAN locates the highlight from
        // this text, so prefixing it with the error code (the old behavior)
        // diverged from the working CLI.
        msg: error.message,
      });
    },
  };
}
