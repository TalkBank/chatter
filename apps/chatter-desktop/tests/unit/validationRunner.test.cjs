const test = require("node:test");
const assert = require("node:assert/strict");
const {
  DESKTOP_COMMANDS, DESKTOP_EVENTS, parseJobCount,
} = require("../../.test-dist/src/protocol/desktopProtocol.js");
const {
  createValidationRunnerCapability,
} = require("../../.test-dist/src/runtime/capabilities/validationRunner.js");

const settings = { roundtrip: false, parserKind: "tree-sitter", strictLinkers: false, jobs: null };
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function harness() {
  const listeners = new Set();
  const invocations = [];
  return {
    listeners, invocations,
    emit(runId, event) { for (const listener of listeners) listener({ runId, event }); },
    transport: {
      async invoke(command, payload) { invocations.push([command, payload]); },
      async listenValidationEvent(listener) {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
    },
  };
}

test("desktop protocol names stay centralized and stable", () => {
  assert.deepEqual(DESKTOP_COMMANDS, {
    validate: "validate", cancelValidation: "cancel_validation", checkClanAvailable: "check_clan_available",
    openInClan: "open_in_clan", exportResults: "export_results", revealInFileManager: "reveal_in_file_manager",
  });
  assert.deepEqual(DESKTOP_EVENTS, { validation: "validation-event" });
});

test("validation runner subscribes before invoke and scopes events and cancellation", async () => {
  const h = harness();
  const seen = [];
  let stopCalls = 0;
  const runner = createValidationRunnerCapability({
    async listenValidationEvent(listener) {
      h.listeners.add(listener);
      return () => { stopCalls++; h.listeners.delete(listener); };
    },
    async invoke(command, payload) {
      h.invocations.push([command, payload]);
      if (command === DESKTOP_COMMANDS.validate) {
        assert.equal(h.listeners.size, 1);
        h.emit(payload.request.runId, { type: "discovering" });
        h.emit(payload.request.runId, { type: "started", totalFiles: 2 });
      }
    },
  });
  const run = runner.startValidation("/tmp/reference", settings, events => seen.push(...events));
  assert.equal(typeof run.dispose, "function", "ownership exists before asynchronous startup");
  await run.ready;
  assert.deepEqual(seen, [{ type: "discovering" }, { type: "started", totalFiles: 2 }]);
  const request = h.invocations[0][1].request;
  assert.match(request.runId, /^[0-9a-f-]{36}$/);
  assert.deepEqual(request, { runId: request.runId, path: "/tmp/reference", ...settings });
  await run.cancel();
  assert.deepEqual(h.invocations[1], [DESKTOP_COMMANDS.cancelValidation, { request: { runId: request.runId } }]);
  run.dispose();
  run.dispose();
  await Promise.resolve();
  assert.equal(stopCalls, 1);
});

test("validation runner disposes the listener when validate fails", async () => {
  let disposed = 0;
  const runner = createValidationRunnerCapability({
    async invoke() { throw new Error("boom"); },
    async listenValidationEvent() { return () => disposed++; },
  });
  const run = runner.startValidation("/tmp/reference", settings, () => {});
  await assert.rejects(run.ready, /boom/);
  run.dispose();
  assert.equal(disposed, 1);
});

test("a run retains its selected settings while startup is pending", async () => {
  const h = harness();
  const runner = createValidationRunnerCapability(h.transport);
  const selected = { ...settings };
  const run = runner.startValidation("/tmp/reference", selected, () => {});
  selected.parserKind = "re2c";
  await run.ready;
  assert.equal(h.invocations[0][1].request.parserKind, "tree-sitter");
  run.dispose();
});

test("a replaced validation cannot deliver its late events to the next run", async () => {
  const h = harness();
  const runner = createValidationRunnerCapability(h.transport);
  const oldSeen = [];
  const first = runner.startValidation("/tmp/first", settings, events => oldSeen.push(...events));
  await first.ready;
  const firstId = h.invocations[0][1].request.runId;
  const queuedOldListener = [...h.listeners][0];
  first.dispose();
  const seen = [];
  const second = runner.startValidation("/tmp/second", settings, events => seen.push(...events));
  await second.ready;
  const secondId = h.invocations.filter(([command]) => command === DESKTOP_COMMANDS.validate)[1][1].request.runId;
  assert.notEqual(firstId, secondId);
  h.emit(firstId, { type: "started", totalFiles: 42 });
  queuedOldListener({ runId: firstId, event: { type: "started", totalFiles: 42 } });
  assert.deepEqual(seen, [], "the old run must not update the replacement");
  assert.deepEqual(oldSeen, [], "disposed callbacks cannot revive the old run");
  h.emit(secondId, { type: "started", totalFiles: 2 });
  assert.deepEqual(seen, [{ type: "started", totalFiles: 2 }]);
  second.dispose();
});

test("disposal during listener registration cleans up without issuing validation", async () => {
  const registering = deferred();
  const registered = deferred();
  let disposed = 0;
  const calls = [];
  const runner = createValidationRunnerCapability({
    async listenValidationEvent() { registering.resolve(); return registered.promise; },
    async invoke(...args) { calls.push(args); },
  });
  const run = runner.startValidation("/tmp/reference", settings, () => assert.fail("disposed event"));
  await registering.promise;
  run.dispose();
  registered.resolve(() => disposed++);
  await run.ready;
  run.dispose();
  assert.equal(disposed, 1);
  assert.deepEqual(calls, []);
});

test("a disposed pending listener cannot block the next startup", async () => {
  const registering = deferred();
  const delayedListener = deferred();
  const h = harness();
  let registrations = 0;
  let oldDisposals = 0;
  const runner = createValidationRunnerCapability({
    ...h.transport,
    async listenValidationEvent(listener) {
      if (++registrations === 1) {
        registering.resolve();
        return delayedListener.promise;
      }
      return h.transport.listenValidationEvent(listener);
    },
  });
  const first = runner.startValidation("/tmp/first", settings, () => {});
  await registering.promise;
  first.dispose();
  const second = runner.startValidation("/tmp/second", settings, () => {});
  await second.ready;
  assert.equal(h.invocations[0][1].request.path, "/tmp/second");
  delayedListener.resolve(() => oldDisposals++);
  await first.ready;
  assert.equal(oldDisposals, 1);
  assert.equal(h.invocations.filter(([c]) => c === DESKTOP_COMMANDS.validate).length, 1);
  second.dispose();
});

test("cancel requested during startup waits for registration and targets that run", async () => {
  const starting = deferred();
  const started = deferred();
  const calls = [];
  const runner = createValidationRunnerCapability({
    async listenValidationEvent() { return () => {}; },
    async invoke(command, payload) {
      calls.push([command, payload]);
      if (command === DESKTOP_COMMANDS.validate) { starting.resolve(); await started.promise; }
    },
  });
  const run = runner.startValidation("/tmp/reference", settings, () => {});
  const cancel = run.cancel();
  await starting.promise;
  assert.equal(calls.length, 1);
  started.resolve();
  await cancel;
  assert.deepEqual(calls[1], [DESKTOP_COMMANDS.cancelValidation, { request: { runId: calls[0][1].request.runId } }]);
  run.dispose();
});

test("disposed startup cannot overtake a newer request and its cancel stays scoped", async () => {
  const h = harness();
  const starting = deferred();
  const started = deferred();
  const cancelled = deferred();
  const runner = createValidationRunnerCapability({
    ...h.transport,
    async invoke(command, payload) {
      h.invocations.push([command, payload]);
      if (command === DESKTOP_COMMANDS.validate && payload.request.path === "/tmp/first") {
        starting.resolve();
        await started.promise;
      }
      if (command === DESKTOP_COMMANDS.cancelValidation) cancelled.resolve(payload.request.runId);
    },
  });
  const first = runner.startValidation("/tmp/first", settings, () => {});
  await starting.promise;
  first.dispose();
  const second = runner.startValidation("/tmp/second", settings, () => {});
  assert.equal(h.invocations.filter(([c]) => c === DESKTOP_COMMANDS.validate).length, 1);
  started.resolve();
  await second.ready;
  const firstId = h.invocations[0][1].request.runId;
  assert.equal(await cancelled.promise, firstId);
  assert.deepEqual(h.invocations.filter(([c]) => c === DESKTOP_COMMANDS.validate).map(([,p]) => p.request.path), ["/tmp/first", "/tmp/second"]);
  second.dispose();
});

test("failed startup does not prevent the next run from starting", async () => {
  const h = harness();
  const starting = deferred();
  const failure = deferred();
  const runner = createValidationRunnerCapability({
    ...h.transport,
    async invoke(command, payload) {
      if (command === DESKTOP_COMMANDS.validate && payload.request.path === "/tmp/first") {
        starting.resolve();
        await failure.promise;
      }
      h.invocations.push([command, payload]);
    },
  });
  const first = runner.startValidation("/tmp/first", settings, () => {});
  const failed = assert.rejects(first.ready, /old failure/);
  await starting.promise;
  first.dispose();
  const second = runner.startValidation("/tmp/second", settings, () => {});
  failure.reject(new Error("old failure"));
  await second.ready;
  assert.equal(h.invocations[0][1].request.path, "/tmp/second");
  second.dispose();
  await failed;
});

test("the jobs field becomes a positive whole count, all CPUs, or invalid", () => {
  assert.deepEqual(parseJobCount(""), { kind: "allCpus" });
  assert.deepEqual(parseJobCount("  "), { kind: "allCpus" });
  assert.deepEqual(parseJobCount("4"), { kind: "count", count: 4 });
  for (const raw of ["0", "1.5", "-2", "abc", "1e3", "99999999999999999999"]) {
    assert.deepEqual(parseJobCount(raw), { kind: "invalid" }, raw);
  }
});

const { ValidationBatcher } = require("../../.test-dist/src/runtime/capabilities/validationBatch.js");
const { applyValidationEvents, createInitialValidationState } = require("../../.test-dist/src/hooks/validationState.js");

function frames() {
  const scheduled = [];
  return {
    scheduled,
    schedule(callback) {
      const frame = { callback, cancelled: false };
      scheduled.push(frame);
      return () => { frame.cancelled = true; };
    },
  };
}
const completedFile = (i) => ({ type: "fileComplete", file: `/reference/${i}.cha`, status: { type: "valid", cacheHit: false } });
const runStats = (totalFiles, validFiles) => ({
  totalFiles, validFiles, invalidFiles: 0, cacheHits: 0, cacheMisses: validFiles,
  cacheErrors: 0, internalFailures: 0, roundtripPassed: 0, roundtripFailed: 0,
});

test("streaming batches retain every file and publish at the 50-file boundary", () => {
  const clock = frames();
  const deliveries = [];
  const batcher = new ValidationBatcher(batch => deliveries.push(batch), clock.schedule);
  for (let i = 0; i < 49; i++) batcher.push(completedFile(i));
  assert.equal(deliveries.length, 0);
  assert.equal(clock.scheduled.length, 1);
  batcher.push(completedFile(49));
  assert.equal(deliveries.length, 1);
  assert.deepEqual(deliveries[0], Array.from({ length: 50 }, (_, i) => completedFile(i)));
  assert.equal(clock.scheduled[0].cancelled, true);
  batcher.push(completedFile(50));
  clock.scheduled[0].callback();
  assert.equal(deliveries.length, 1, "an obsolete frame cannot flush its successor");
  clock.scheduled[1].callback();
  assert.deepEqual(deliveries[1], [completedFile(50)]);
  assert.equal(deliveries[0].length, 50, "later appends cannot mutate a delivered batch");
  batcher.dispose();
});

test("terminal outcomes flush retained files immediately and close the batcher", () => {
  for (const ending of [
    { type: "finished", stats: runStats(1, 1), passed: true },
    { type: "finishedIncomplete", stats: runStats(2, 1), lostFiles: 1, cause: "worker stopped" },
    { type: "stopped", stats: runStats(2, 1), unprocessedFiles: 1, reason: "Cancelled" },
    { type: "aborted", reason: "stream ended" },
  ]) {
    const clock = frames();
    const deliveries = [];
    const batcher = new ValidationBatcher(batch => deliveries.push(batch), clock.schedule);
    batcher.push(completedFile(0));
    batcher.push(ending);
    assert.deepEqual(deliveries, [[completedFile(0), ending]], ending.type);
    clock.scheduled[0].callback();
    batcher.push(completedFile(1));
    assert.equal(deliveries.length, 1);
  }
  const empty = [];
  const batcher = new ValidationBatcher(batch => empty.push(batch), () => assert.fail("no frame needed"));
  batcher.push({ type: "nothingFound" });
  assert.deepEqual(empty, [[{ type: "nothingFound" }]]);
});

test("lifecycle and cache events are immediate while disposal discards only unpublished updates", () => {
  const clock = frames();
  const deliveries = [];
  const batcher = new ValidationBatcher(batch => deliveries.push(batch), clock.schedule);
  for (const event of [{ type: "cacheUnavailable", reason: "offline" }, { type: "discovering" }, { type: "started", totalFiles: 1 }]) {
    batcher.push(event);
    assert.deepEqual(deliveries.at(-1), [event]);
  }
  assert.equal(clock.scheduled.length, 0);
  batcher.push(completedFile(0));
  batcher.dispose();
  clock.scheduled[0].callback();
  assert.equal(deliveries.length, 3);
  assert.equal(clock.scheduled[0].cancelled, true);
});

test("the capability flushes its current run before completion and clears buffered old runs", async () => {
  const h = harness();
  const clock = frames();
  const runner = createValidationRunnerCapability(h.transport, clock.schedule);
  const old = [];
  const first = runner.startValidation("/tmp/first", settings, batch => old.push(batch));
  await first.ready;
  const firstId = h.invocations[0][1].request.runId;
  h.emit(firstId, completedFile(0));
  assert.equal(old.length, 0);
  first.dispose();
  const delivered = [];
  let state = createInitialValidationState();
  const second = runner.startValidation("/tmp/second", settings, batch => {
    delivered.push(batch);
    state = applyValidationEvents(state, batch, path => path);
  });
  await second.ready;
  const secondId = h.invocations.filter(([c]) => c === DESKTOP_COMMANDS.validate)[1][1].request.runId;
  clock.scheduled[0].callback();
  h.emit(firstId, completedFile(1));
  for (let i = 0; i < 51; i++) h.emit(secondId, completedFile(i));
  assert.equal(delivered.length, 1);
  h.emit(secondId, { type: "finished", stats: runStats(51, 51), passed: true });
  assert.equal(delivered.length, 2);
  assert.equal(state.files.size, 51);
  assert.equal(state.processedFiles, 51);
  assert.equal(state.run.kind, "finished");
  assert.equal(old.length, 0);
  second.dispose();
});
