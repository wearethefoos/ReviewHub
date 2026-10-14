"use client";

import { useEffect } from "react";

/** Opens a collapsed <details> when the URL hash points at it, so links to skipped files work. */
export function OpenOnHash() {
  useEffect(() => {
    function open() {
      const id = decodeURIComponent(window.location.hash.slice(1));
      const el = id ? document.getElementById(id) : null;
      if (el instanceof HTMLDetailsElement && !el.open) {
        el.open = true;
        el.scrollIntoView();
      }
    }
    open();
    window.addEventListener("hashchange", open);
    return () => window.removeEventListener("hashchange", open);
  }, []);
  return null;
}
