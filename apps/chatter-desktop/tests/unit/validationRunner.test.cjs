const test = require("node:test");
const assert = require("node:assert/strict");

const {
  DESKTOP_COMMANDS,
  DESKTOP_EVENTS,
  parseJobCount,
} = require("../../.test-dist/src/protocol/desktopProtocol.js");
const {
  createValidationRunnerCapability,
} = require("../../.test-dist/src/runtime/capabilities/validationRunner.js");

test("desktop protocol names stay centralized and stable", () => {
  assert.deepEqual(DESKTOP_COMMANDS, {
    validate: "validate",
    cancelValidation: "cancel_validation",
    checkClanAvailable: "check_clan_available",
    openInClan: "open_in_clan",
    exportResults: "export_results",
    revealInFileManager: "reveal_in_file_manager",
  });
  assert.deepEqual(DESKTOP_EVENTS, {
    validation: "validation-event",
  });
});

test("validation runner listens before invoking and disposes once", async () => {
  const seenEvents = [];
  const invocations = [];
  let disposeCalls = 0;

  const transport = {
    async invoke(command, payload) {
      invocations.push([command, payload]);
      return undefined;
    },
    async listenValidationEvent(listener) {
      listener({ type: "discovering" });
      listener({ type: "started", totalFiles: 2 });
      return () => {
        disposeCalls += 1;
      };
    },
  };

  const settings = {
    roundtrip: false,
    parserKind: "tree-sitter",
    strictLinkers: false,
    jobs: null,
  };

  const validationRunner = createValidationRunnerCapability(transport);
  const run = await validationRunner.startValidation("/tmp/reference", settings, (event) => {
    seenEvents.push(event);
  });

  assert.deepEqual(seenEvents, [
    { type: "discovering" },
    { type: "started", totalFiles: 2 },
  ]);
  assert.deepEqual(invocations, [
    [
      DESKTOP_COMMANDS.validate,
      {
        request: {
          path: "/tmp/reference",
          roundtrip: false,
          parserKind: "tree-sitter",
          strictLinkers: false,
          jobs: null,
        },
      },
    ],
  ]);

  await run.cancel();
  assert.deepEqual(invocations[1], [DESKTOP_COMMANDS.cancelValidation, undefined]);

  run.dispose();
  run.dispose();
  assert.equal(disposeCalls, 1);
});

test("validation runner disposes the listener if validate fails", async () => {
  let disposed = false;

  const validationRunner = createValidationRunnerCapability({
    async invoke() {
      throw new Error("boom");
    },
    async listenValidationEvent() {
      return () => {
        disposed = true;
      };
    },
  });

  await assert.rejects(
    validationRunner.startValidation(
      "/tmp/reference",
      { roundtrip: false, parserKind: "tree-sitter", strictLinkers: false, jobs: null },
      () => {},
    ),
    /boom/,
  );
  assert.equal(disposed, true);
});

test("the jobs field becomes a positive whole count, all CPUs, or invalid", () => {
  assert.deepEqual(parseJobCount(""), { kind: "allCpus" });
  assert.deepEqual(parseJobCount("  "), { kind: "allCpus" });
  assert.deepEqual(parseJobCount("4"), { kind: "count", count: 4 });
  for (const raw of ["0", "1.5", "-2", "abc", "1e3", "99999999999999999999"]) {
    assert.deepEqual(parseJobCount(raw), { kind: "invalid" }, raw);
  }
});
