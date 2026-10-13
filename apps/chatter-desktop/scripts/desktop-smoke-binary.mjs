import { accessSync, constants, statSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Admit one exact standalone-driver application, without launching it.
 * Paths use the host filesystem; a platform override tests suffix policy only.
 * @param {{configUrl: string, platform?: NodeJS.Platform,
 *   binaryPath?: string, targetDir?: string}} options
 * @returns {{readonly application: string}}
 */
export function admitDesktopSmokeBinary({
  configUrl,
  platform = process.platform,
  binaryPath,
  targetDir,
}) {
  const executableName = (() => {
    switch (platform) {
      case "linux": return "chatter-desktop";
      case "win32": return "chatter-desktop.exe";
      default:
        throw new Error(`Standalone tauri-driver does not support ${platform}; use Linux or Windows.`);
    }
  })();

  if (binaryPath !== undefined && targetDir !== undefined) {
    throw new Error("Configure only one of CHATTER_DESKTOP_BINARY and CHATTER_DESKTOP_TARGET_DIR.");
  }
  for (const [name, path] of [
    ["CHATTER_DESKTOP_BINARY", binaryPath],
    ["CHATTER_DESKTOP_TARGET_DIR", targetDir],
  ]) {
    if (path !== undefined && (path === "" || !isAbsolute(path))) {
      throw new Error(`${name} must be an absolute path; relative Cargo build paths are not inferred.`);
    }
  }

  const workspaceRoot = resolve(dirname(fileURLToPath(configUrl)), "../..");
  const application = binaryPath ?? join(
    targetDir ?? join(workspaceRoot, "target"), "debug", executableName,
  );
  try {
    if (!statSync(application).isFile()) {
      throw new Error("not a regular file");
    }
    // Windows access(X_OK) is not an executable-format check. The driver owns
    // actual process admission; Linux can additionally check execute permission.
    accessSync(application, platform === "linux" ? constants.X_OK : constants.F_OK);
  } catch (cause) {
    throw new Error(`Desktop smoke binary is not usable: ${application}. Build it or set CHATTER_DESKTOP_BINARY.`, { cause });
  }
  return Object.freeze({ application });
}
