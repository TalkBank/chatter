const test = require("node:test");
const assert = require("node:assert/strict");
const { createExportCapability } = require("../../.test-dist/src/runtime/capabilities/exportResults.js");
const { DESKTOP_COMMANDS } = require("../../.test-dist/src/protocol/desktopProtocol.js");

test("export selections carry format through to the command without changing the path", async () => {
  for (const [path, format] of [
    ["/reports/results.json", "json"],
    ["/reports/results.JSON", "json"],
    ["C:\\reports\\results.JsOn", "json"],
    ["/reports/évidence.txt", "text"],
    ["/reports/json/final-report", "text"],
    ["/reports/results.json.txt", "text"],
  ]) {
    const invocations = [];
    const capability = createExportCapability({
      async chooseExportPath() { return path; },
      async invoke(command, payload) { invocations.push({ command, payload }); },
    });
    const results = [{ path: "/corpus/source.cha", errors: [], status: { type: "valid", cacheHit: false }, statusLabel: "Valid" }];
    const destination = await capability.chooseExportDestination();
    assert.notEqual(destination, null);
    await capability.exportResults(results, destination);
    assert.deepEqual(invocations, [{
      command: DESKTOP_COMMANDS.exportResults,
      payload: { request: { results: JSON.stringify(results), path, format } },
    }], path);
  }
});

test("cancelled or failed export selection never sends a write command", async () => {
  const invoke = async () => assert.fail("no write without a destination");
  const cancelled = createExportCapability({ async chooseExportPath() { return null; }, invoke });
  assert.equal(await cancelled.chooseExportDestination(), null);
  const empty = createExportCapability({ async chooseExportPath() { return ""; }, invoke });
  await assert.rejects(empty.chooseExportDestination(), /export file name/);
  const failed = createExportCapability({ async chooseExportPath() { throw new Error("dialog failed"); }, invoke });
  await assert.rejects(failed.chooseExportDestination(), /dialog failed/);
});

test("export write failures reach the caller", async () => {
  const capability = createExportCapability({
    async chooseExportPath() { return "/reports/result.json"; },
    async invoke() { throw new Error("destination refused"); },
  });
  const destination = await capability.chooseExportDestination();
  await assert.rejects(capability.exportResults([], destination), /destination refused/);
});
