const test = require("node:test");
const assert = require("node:assert/strict");

const {
  createUpdatesCapability,
} = require("../../.test-dist/src/runtime/capabilities/updates.js");

// A fake transport exposing the two update primitives the capability composes.
// `events` records the order of calls so we can assert the orchestration.
function fakeTransport({ update, accept, throwOnCheck }) {
  const events = [];
  const messages = [];
  let installed = false;
  return {
    events,
    messages,
    wasInstalled: () => installed,
    async checkForUpdate() {
      events.push("check");
      if (throwOnCheck) {
        throw new Error("network down");
      }
      if (!update) {
        return null;
      }
      return {
        version: update.version,
        currentVersion: update.currentVersion,
        notes: update.notes ?? null,
        async install() {
          events.push("install");
          installed = true;
        },
      };
    },
    async askInstallUpdate() {
      events.push("ask");
      return accept;
    },
    async showMessage(title, body) {
      events.push("message");
      messages.push({ title, body });
    },
  };
}

test("checkOnLaunch reports no-update and never prompts when none is available", async () => {
  const transport = fakeTransport({ update: null });
  const updates = createUpdatesCapability(transport);

  const outcome = await updates.checkOnLaunch();

  assert.equal(outcome, "no-update");
  assert.deepEqual(transport.events, ["check"]);
});

test("checkOnLaunch installs the update when the user accepts the prompt", async () => {
  const transport = fakeTransport({
    update: { version: "0.1.1", currentVersion: "0.1.0", notes: "Fixes." },
    accept: true,
  });
  const updates = createUpdatesCapability(transport);

  const outcome = await updates.checkOnLaunch();

  assert.equal(outcome, "installing");
  assert.ok(transport.wasInstalled(), "the update should have been installed");
  assert.deepEqual(transport.events, ["check", "ask", "install"]);
});

test("checkOnLaunch does not install when the user declines", async () => {
  const transport = fakeTransport({
    update: { version: "0.1.1", currentVersion: "0.1.0" },
    accept: false,
  });
  const updates = createUpdatesCapability(transport);

  const outcome = await updates.checkOnLaunch();

  assert.equal(outcome, "declined");
  assert.ok(!transport.wasInstalled(), "a declined update must not install");
  assert.deepEqual(transport.events, ["check", "ask"]);
});

test("checkOnLaunch swallows errors so a failed check never breaks the app", async () => {
  const transport = fakeTransport({ throwOnCheck: true });
  const updates = createUpdatesCapability(transport);

  const outcome = await updates.checkOnLaunch();

  assert.equal(outcome, "error");
});

test("checkNow tells the user they are up to date when no update exists", async () => {
  const transport = fakeTransport({ update: null });
  const updates = createUpdatesCapability(transport);

  const outcome = await updates.checkNow();

  assert.equal(outcome, "no-update");
  assert.deepEqual(transport.events, ["check", "message"]);
  assert.equal(transport.messages.length, 1, "an up-to-date dialog must show");
});

test("checkNow installs the update when the user accepts the prompt", async () => {
  const transport = fakeTransport({
    update: { version: "0.1.1", currentVersion: "0.1.0", notes: "Fixes." },
    accept: true,
  });
  const updates = createUpdatesCapability(transport);

  const outcome = await updates.checkNow();

  assert.equal(outcome, "installing");
  assert.ok(transport.wasInstalled(), "the update should have been installed");
  assert.deepEqual(transport.events, ["check", "ask", "install"]);
  assert.equal(transport.messages.length, 0, "no dialog when an update is found");
});

test("checkNow reports the failure to the user when the check throws", async () => {
  const transport = fakeTransport({ throwOnCheck: true });
  const updates = createUpdatesCapability(transport);

  const outcome = await updates.checkNow();

  assert.equal(outcome, "error");
  assert.equal(transport.messages.length, 1, "a failure dialog must show");
});
