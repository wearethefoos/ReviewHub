import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { cx } from "./ui";

/** Renders untrusted markdown. Raw HTML is not rendered and unsafe URLs are stripped. */
export function Markdown({ children, className }: { children: string; className?: string }) {
  return (
    <div className={cx("markdown", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{children}</ReactMarkdown>
    </div>
  );
}

/** Single-line markdown (titles), rendered inline without paragraph wrappers. */
export function InlineMarkdown({ children }: { children: string }) {
  return (
    <span className="markdown">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={{ p: ({ children }) => <>{children}</> }}>
        {children}
      </ReactMarkdown>
    </span>
  );
}
