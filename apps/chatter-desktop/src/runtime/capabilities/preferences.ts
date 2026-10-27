/** Browser preferences are optional; persistence is never a prerequisite for use. */
export type ThemePreference = "system" | "light" | "dark";
export type OnboardingPreference = "show" | "dismissed";
export type PreferenceState<T, Initial = T> =
  | { kind: "absent"; value: Initial }
  | { kind: "stored"; value: T }
  | { kind: "session"; value: Initial; reason: "read-failed" | "invalid" }
  | { kind: "session"; value: T; reason: "write-failed" };

type Decoded<T> = { kind: "accepted"; value: T } | { kind: "invalid" };
export interface Preference<T, Initial = T> {
  readonly key: string;
  readonly initial: Initial;
  decode(raw: string): Decoded<T>;
  encode(value: T): string;
}

// A getter keeps even a throwing window.localStorage property inside the boundary.
export type PreferenceStorage = () => Pick<Storage, "getItem" | "setItem">;
const browserStorage: PreferenceStorage = () => window.localStorage;

export const themePreference: Preference<ThemePreference> = {
  key: "chatter-theme",
  initial: "system",
  decode: (raw) => raw === "system" || raw === "light" || raw === "dark"
    ? { kind: "accepted", value: raw } : { kind: "invalid" },
  encode: (value) => value,
};
export const onboardingPreference: Preference<OnboardingPreference> = {
  key: "chatter-onboarding-dismissed",
  initial: "show",
  decode: (raw) => raw === "1" ? { kind: "accepted", value: "dismissed" }
    : raw === "0" ? { kind: "accepted", value: "show" } : { kind: "invalid" },
  encode: (value) => value === "dismissed" ? "1" : "0",
};
export const targetPreference: Preference<string, null> = {
  key: "chatter-last-target",
  initial: null,
  decode: (raw) => raw.length > 0 ? { kind: "accepted", value: raw } : { kind: "invalid" },
  encode: (value) => value,
};

export function loadPreference<T, Initial>(
  preference: Preference<T, Initial>,
  storage: PreferenceStorage = browserStorage,
): PreferenceState<T, Initial> {
  let raw: string | null;
  try {
    raw = storage().getItem(preference.key);
  } catch {
    return { kind: "session", value: preference.initial, reason: "read-failed" };
  }
  if (raw === null) return { kind: "absent", value: preference.initial };
  const decoded = preference.decode(raw);
  switch (decoded.kind) {
    case "accepted": return { kind: "stored", value: decoded.value };
    case "invalid": return { kind: "session", value: preference.initial, reason: "invalid" };
  }
}

export function savePreference<T, Initial>(
  preference: Preference<T, Initial>,
  value: T,
  storage: PreferenceStorage = browserStorage,
): PreferenceState<T, Initial> {
  try {
    storage().setItem(preference.key, preference.encode(value));
    return { kind: "stored", value };
  } catch {
    return { kind: "session", value, reason: "write-failed" };
  }
}

export function preferenceWarning<T, Initial>(label: string, state: PreferenceState<T, Initial>): string | null {
  switch (state.kind) {
    case "absent":
    case "stored": return null;
    case "session":
      switch (state.reason) {
        case "read-failed": return `${label}: saved preference could not be read. Using the initial setting for this session.`;
        case "invalid": return `${label}: saved preference was not recognized. Using the initial setting for this session.`;
        case "write-failed": return `${label}: this choice could not be saved. It applies to this session only.`;
      }
  }
}
