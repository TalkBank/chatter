import type { DesktopTransport, ValidationTargetCapability } from "./contracts";
import { DESKTOP_COMMANDS } from "../../protocol/desktopProtocol";
import { disposeOnce, singlePathSelection } from "./shared";

export function createValidationTargetCapability(
  transport: Pick<
    DesktopTransport,
    "invoke" | "chooseValidationFile" | "chooseValidationFolder" | "onValidationDragDrop"
  >,
): ValidationTargetCapability {
  return {
    async chooseValidationFile() {
      return singlePathSelection(await transport.chooseValidationFile());
    },

    async chooseValidationFolder() {
      return singlePathSelection(await transport.chooseValidationFolder());
    },

    async revealFile(path) {
      await transport.invoke(DESKTOP_COMMANDS.revealInFileManager, { path });
    },

    async onValidationDragDrop(listener) {
      return disposeOnce(await transport.onValidationDragDrop(listener));
    },
  };
}
