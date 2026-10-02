const test = require("node:test");
const assert = require("node:assert/strict");

const {
  DESKTOP_COMMANDS,
} = require("../../.test-dist/src/protocol/desktopProtocol.js");
const {
  createClanCapability,
} = require("../../.test-dist/src/runtime/capabilities/clan.js");

// A serialized ParseError shaped like what the Rust bridge sends to the frontend.
function sampleError() {
  return {
    code: "E601",
    severity: "Error",
    location: { start: 168, end: 178, line: 8, column: 1 },
    labels: [],
    message: "Invalid dependent tier content",
  };
}

test("openInClan forwards the bare error.message as the CLAN highlight (matches the CLI)", async () => {
  const invocations = [];
  const transport = {
    async invoke(command, payload) {
      invocations.push([command, payload]);
      return undefined;
    },
  };

  const clan = createClanCapability(transport);
  const error = sampleError();
  await clan.openInClan({ file: "/corpus/E601.cha", error });

  assert.equal(invocations.length, 1, "exactly one invoke");
  const [command, payload] = invocations[0];
  assert.equal(command, DESKTOP_COMMANDS.openInClan);

  // The CLI/TUI sends the bare error.message; CLAN locates the highlight from
  // this text, so a code prefix would miss it. One request value, as the Rust
  // command takes it.
  assert.equal(payload.request.msg, "Invalid dependent tier content");
  assert.notEqual(payload.request.msg, "E601: Invalid dependent tier content");
});

test("openInClan forwards file, line, column and byte offset from the error", async () => {
  const invocations = [];
  const transport = {
    async invoke(command, payload) {
      invocations.push([command, payload]);
      return undefined;
    },
  };

  const clan = createClanCapability(transport);
  const error = sampleError();
  await clan.openInClan({ file: "/corpus/E601.cha", error });

  const [, payload] = invocations[0];
  assert.deepEqual(payload, {
    request: { file: "/corpus/E601.cha", line: 8, col: 1, byteOffset: 168, msg: sampleError().message },
  });
});
