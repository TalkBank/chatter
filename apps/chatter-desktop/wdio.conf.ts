/**
 * WebdriverIO configuration for Chatter Desktop E2E tests.
 *
 * Uses tauri-driver as the WebDriver backend to drive the real Tauri app.
 *
 * Prerequisites:
 *   cargo install tauri-driver
 *   cargo tauri build --debug    (or use the dev binary)
 *
 * Run:
 *   npx wdio run wdio.conf.ts
 */

import type {} from "@wdio/types";
import { admitDesktopSmokeBinary } from "./scripts/desktop-smoke-binary.mjs";

const isCI = !!process.env.CI;

// The standalone driver's vendor extension is not in WebdriverIO's base types.
const tauriCapabilities: WebdriverIO.Capabilities & {
  "tauri:options": ReturnType<typeof admitDesktopSmokeBinary>;
} = {
  "tauri:options": admitDesktopSmokeBinary({
    configUrl: import.meta.url,
    binaryPath: process.env.CHATTER_DESKTOP_BINARY,
    targetDir: process.env.CHATTER_DESKTOP_TARGET_DIR,
  }),
};

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./tests/e2e/**/*.spec.ts"],

  maxInstances: 1,
  capabilities: [tauriCapabilities],

  logLevel: isCI ? "warn" : "info",
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  // tauri-driver listens on port 4444 by default
  port: 4444,
  hostname: "localhost",

  services: [],
  framework: "mocha",
  reporters: ["spec"],

  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },
};
