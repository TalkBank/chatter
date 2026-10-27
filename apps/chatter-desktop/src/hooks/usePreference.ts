import { useCallback, useState } from "react";
import { loadPreference, savePreference, type Preference } from "../runtime/capabilities/preferences";

/** Keep the active choice and persistence outcome in one state transition. */
export function usePreference<T, Initial>(preference: Preference<T, Initial>) {
  const [state, setState] = useState(() => loadPreference(preference));
  const setValue = useCallback((value: T) => {
    // Do not perform storage writes in a React state updater (which can replay).
    setState(savePreference(preference, value));
  }, [preference]);
  return [state, setValue] as const;
}
