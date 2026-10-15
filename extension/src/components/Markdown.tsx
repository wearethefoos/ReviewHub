import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import remarkGfm from "remark-gfm";
import { languages } from "@/lib/highlight";
import { cx } from "./ui";

// Fenced code blocks with a language (```ts) are highlighted; a review comment's ```suggestion block isn't a language.
const highlight: [typeof rehypeHighlight, { languages: typeof languages; detect: false; plainText: string[] }] = [
  rehypeHighlight,
  { languages, detect: false, plainText: ["suggestion", "text", "txt"] },
];

/** Renders untrusted markdown. Raw HTML is not rendered and unsafe URLs are stripped. */
export function Markdown({ children, className }: { children: string; className?: string }) {
  return (
    <div className={cx("markdown", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[highlight]}>
        {children}
      </ReactMarkdown>
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
