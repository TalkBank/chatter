import type { AboutCapability, DesktopTransport } from "./contracts";

/** The transport primitives the About capability composes. */
type AboutTransport = Pick<
  DesktopTransport,
  "onMenuAbout" | "openExternalUrl" | "getAppVersion"
>;

/**
 * Build the "About Chatter" capability: subscribe to the About menu item,
 * report the app version, and open the modal's links in the OS browser. Thin
 * over the transport so `@tauri-apps/*` stays behind the runtime seam and the
 * `AboutModal` component depends only on this capability.
 */
export function createAboutCapability(
  transport: AboutTransport,
): AboutCapability {
  return {
    onAboutRequested(handler: () => void): Promise<() => void> {
      return transport.onMenuAbout(handler);
    },
    openExternal(url: string): Promise<void> {
      return transport.openExternalUrl(url);
    },
    version(): Promise<string> {
      return transport.getAppVersion();
    },
  };
}
