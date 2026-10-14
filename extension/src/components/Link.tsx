import type { AnchorHTMLAttributes, MouseEvent } from "react";
import { navigate } from "@/lib/router";

/** In-page navigation for links that change the query string (tabs); everything else is a plain link. */
export function Link({ href, onClick, ...props }: AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) {
  function handle(event: MouseEvent<HTMLAnchorElement>) {
    onClick?.(event);
    if (event.defaultPrevented || !href.startsWith("?")) return;
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    navigate(href);
  }
  return <a href={href} onClick={handle} {...props} />;
}
