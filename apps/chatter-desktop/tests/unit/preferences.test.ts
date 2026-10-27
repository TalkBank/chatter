import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  loadPreference, savePreference, preferenceWarning,
  themePreference, onboardingPreference, targetPreference,
  type PreferenceStorage,
} from "../../src/runtime/capabilities/preferences";

function memory(values: Record<string, string> = {}): PreferenceStorage {
  const entries = new Map(Object.entries(values));
  return () => ({
    getItem: (key) => entries.get(key) ?? null,
    setItem: (key, value) => { entries.set(key, value); },
  });
}
const denied: PreferenceStorage = () => { throw new Error("Storage denied"); };

test("missing preferences have explicit initial states without writing storage", () => {
  const storage: PreferenceStorage = () => ({
    getItem: () => null,
    setItem: () => assert.fail("loading must not write"),
  });
  assert.deepEqual(loadPreference(themePreference, storage), { kind: "absent", value: "system" });
  assert.deepEqual(loadPreference(onboardingPreference, storage), { kind: "absent", value: "show" });
  assert.deepEqual(loadPreference(targetPreference, storage), { kind: "absent", value: null });
});

test("existing wire values remain readable, including exact Unicode target paths", () => {
  const storage = memory({
    "chatter-theme": "dark",
    "chatter-onboarding-dismissed": "1",
    "chatter-last-target": "/inputs/日本語/sample.cha",
  });
  assert.deepEqual(loadPreference(themePreference, storage), { kind: "stored", value: "dark" });
  assert.deepEqual(loadPreference(onboardingPreference, storage), { kind: "stored", value: "dismissed" });
  assert.deepEqual(loadPreference(targetPreference, storage), { kind: "stored", value: "/inputs/日本語/sample.cha" });
});

test("invalid persisted data is distinguished from absence and retained untouched", () => {
  const storage = memory({ "chatter-theme": "sepia", "chatter-onboarding-dismissed": "yes", "chatter-last-target": "" });
  assert.deepEqual(loadPreference(themePreference, storage), { kind: "session", value: "system", reason: "invalid" });
  assert.deepEqual(loadPreference(onboardingPreference, storage), { kind: "session", value: "show", reason: "invalid" });
  assert.deepEqual(loadPreference(targetPreference, storage), { kind: "session", value: null, reason: "invalid" });
  assert.equal(storage().getItem("chatter-theme"), "sepia");
});

test("throwing storage acquisition and reads return explicit session states", () => {
  const readDenied: PreferenceStorage = () => ({
    getItem: () => { throw new Error("Read denied"); },
    setItem: () => assert.fail("loading must not write"),
  });
  for (const storage of [denied, readDenied]) {
    assert.deepEqual(loadPreference(themePreference, storage), { kind: "session", value: "system", reason: "read-failed" });
    assert.deepEqual(loadPreference(onboardingPreference, storage), { kind: "session", value: "show", reason: "read-failed" });
    assert.deepEqual(loadPreference(targetPreference, storage), { kind: "session", value: null, reason: "read-failed" });
  }
});

test("write failures preserve all active choices and permit a later successful save", () => {
  const full: PreferenceStorage = () => ({
    getItem: () => null,
    setItem: () => { throw new Error("Quota exceeded"); },
  });
  for (const storage of [denied, full]) {
    assert.deepEqual(savePreference(themePreference, "light", storage), { kind: "session", value: "light", reason: "write-failed" });
    assert.deepEqual(savePreference(onboardingPreference, "dismissed", storage), { kind: "session", value: "dismissed", reason: "write-failed" });
    assert.deepEqual(savePreference(targetPreference, "/inputs/sample.cha", storage), { kind: "session", value: "/inputs/sample.cha", reason: "write-failed" });
  }
  const recovered = memory();
  assert.deepEqual(savePreference(themePreference, "dark", recovered), { kind: "stored", value: "dark" });
  assert.equal(recovered().getItem("chatter-theme"), "dark");
  savePreference(onboardingPreference, "dismissed", recovered);
  assert.equal(recovered().getItem("chatter-onboarding-dismissed"), "1");
  savePreference(targetPreference, "/inputs/sample.cha", recovered);
  assert.equal(recovered().getItem("chatter-last-target"), "/inputs/sample.cha");
});

test("feedback distinguishes unreadable, invalid and unsaved preferences", () => {
  assert.equal(preferenceWarning("Theme", { kind: "absent", value: "system" }), null);
  assert.equal(preferenceWarning("Theme", { kind: "stored", value: "dark" }), null);
  assert.match(preferenceWarning("Theme", loadPreference(themePreference, denied))!, /could not be read/);
  assert.match(preferenceWarning("Theme", loadPreference(themePreference, memory({ "chatter-theme": "bad" })))!, /not recognized/);
  assert.match(preferenceWarning("Theme", savePreference(themePreference, "dark", denied))!, /session only/);
});
