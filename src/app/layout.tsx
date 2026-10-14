import { CodeReviewIcon } from "@primer/octicons-react";
import type { Metadata } from "next";
import Link from "next/link";
import type { ReactNode } from "react";
import "./globals.css";

export const metadata: Metadata = {
  title: { default: "ReviewHub", template: "%s · ReviewHub" },
  description: "Review agent-authored pull requests by what matters, not by line count.",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body className="min-h-screen">
        <header className="bg-header text-header-fg">
          <div className="mx-auto flex max-w-[1280px] items-center gap-4 px-4 py-3 md:px-6">
            <Link href="/" className="flex items-center gap-2 font-semibold text-header-fg">
              <CodeReviewIcon size={24} />
              ReviewHub
            </Link>
            <form action="/open" className="ml-auto w-full max-w-sm">
              <input
                name="pr"
                placeholder="Open a pull request: owner/repo#123 or URL"
                aria-label="Pull request URL or owner/repo#number"
                className="w-full rounded-md border border-white/25 bg-transparent px-3 py-1 text-sm text-header-fg placeholder:text-white/60 focus:border-white/60 focus:outline-none"
              />
            </form>
          </div>
        </header>
        {children}
      </body>
    </html>
  );
}
