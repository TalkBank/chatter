import { useCallback, useEffect, useState } from "react";
import { useAboutCapability } from "../runtime/DesktopRuntimeContext";

const TALKBANK_URL = "https://talkbank.org";
const REPO_URL = "https://github.com/TalkBank/chatter";

interface Props {
  open: boolean;
  onClose: () => void;
}

/**
 * The "About Chatter" modal, opened from the app menu (the native about panel
 * cannot show clickable links). Links open in the OS browser via the `about`
 * capability's `openExternal`, never a bare `<a href>` that would navigate the
 * app webview away from itself.
 */
export default function AboutModal({ open, onClose }: Props) {
  const about = useAboutCapability();
  const [version, setVersion] = useState<string | null>(null);

  // Load the version when the modal opens; guard against unmount/close before
  // the promise resolves.
  useEffect(() => {
    if (!open) {
      return;
    }
    let cancelled = false;
    void about
      .version()
      .then((value) => {
        if (!cancelled) {
          setVersion(value);
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [open, about]);

  // Escape closes the modal.
  useEffect(() => {
    if (!open) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  const openLink = useCallback(
    (url: string) => {
      void about.openExternal(url).catch(() => {});
    },
    [about],
  );

  if (!open) {
    return null;
  }

  return (
    <div className="about-backdrop" onClick={onClose}>
      <div className="about-card" onClick={(event) => event.stopPropagation()}>
        <h2>Chatter</h2>
        <p className="about-version">
          {version ? `Version ${version}` : " "}
        </p>
        <p className="about-desc">
          A validator for CHAT transcripts, part of the TalkBank toolchain. It
          checks <code>.cha</code> files against the CHAT format and shows
          each error with full source context.
        </p>
        <p className="about-credit">By TalkBank and Brian MacWhinney.</p>
        <div className="about-links">
          <button
            type="button"
            className="about-link"
            onClick={() => openLink(TALKBANK_URL)}
          >
            talkbank.org
          </button>
          <button
            type="button"
            className="about-link"
            onClick={() => openLink(REPO_URL)}
          >
            GitHub repository
          </button>
        </div>
        <button
          type="button"
          className="primary about-close-btn"
          onClick={onClose}
        >
          Close
        </button>
      </div>
    </div>
  );
}
