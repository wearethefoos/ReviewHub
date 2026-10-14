import { CheckCircleFillIcon, CodeReviewIcon, XCircleFillIcon } from "@primer/octicons-react";
import { type FormEvent, StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "@/globals.css";
import { Code } from "@/components/ui";
import { createClient } from "@/lib/github";
import { loadSettings, saveSettings } from "@/lib/settings";

type Check = { ok: boolean; message: string } | null;

async function checkToken(token: string): Promise<Check> {
  try {
    const client = createClient(token);
    const user = token ? ((await client.get("/user")) as { login: string }) : null;
    const limits = (await client.get("/rate_limit")) as { resources: { core: { remaining: number; limit: number } } };
    const { remaining, limit } = limits.resources.core;
    return { ok: true, message: `${user ? `Signed in as ${user.login}. ` : "No token: public repositories only. "}${remaining} of ${limit} API requests left this hour.` };
  } catch (err) {
    return { ok: false, message: err instanceof Error ? err.message : String(err) };
  }
}

function Options() {
  const [token, setToken] = useState("");
  const [trusted, setTrusted] = useState("");
  const [saved, setSaved] = useState(false);
  const [check, setCheck] = useState<Check>(null);

  useEffect(() => {
    loadSettings().then((s) => {
      setToken(s.token);
      setTrusted(s.trustedAuthors.join(", "));
    });
  }, []);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const trustedAuthors = trusted.split(",").map((s) => s.trim()).filter(Boolean);
    await saveSettings({ token: token.trim(), trustedAuthors });
    setSaved(true);
    setCheck(await checkToken(token.trim()));
  }

  return (
    <main className="mx-auto max-w-2xl px-6 py-10">
      <h1 className="flex items-center gap-2 text-2xl font-semibold">
        <CodeReviewIcon size={24} /> ReviewHub settings
      </h1>
      <form onSubmit={submit} className="mt-6 space-y-6">
        <div>
          <label htmlFor="token" className="font-semibold">
            GitHub token
          </label>
          <p className="mt-1 text-sm text-fg-muted">
            Needed for private repositories, and raises GitHub&apos;s rate limit from 60 to 5,000 requests an hour. Create a{" "}
            <a href="https://github.com/settings/personal-access-tokens/new" target="_blank" rel="noreferrer">
              fine-grained token
            </a>{" "}
            with read-only <span className="font-medium">Pull requests</span> and <span className="font-medium">Contents</span> access to the repositories you
            review. It is stored in this browser only and sent only to <Code>api.github.com</Code>.
          </p>
          <input
            id="token"
            type="password"
            autoComplete="off"
            value={token}
            onChange={(e) => {
              setToken(e.target.value);
              setSaved(false);
            }}
            placeholder="github_pat_…"
            className="mt-2 w-full rounded-md border border-border bg-canvas px-3 py-1.5 font-mono text-sm focus:border-accent focus:outline-none"
          />
        </div>
        <div>
          <label htmlFor="trusted" className="font-semibold">
            Also trust reports from
          </label>
          <p className="mt-1 text-sm text-fg-muted">
            ReviewHub reads report comments from repository owners, members and collaborators, the pull request&apos;s author and{" "}
            <Code>github-actions[bot]</Code>. Add other logins (for example your CI bot) separated by commas.
          </p>
          <input
            id="trusted"
            value={trusted}
            onChange={(e) => {
              setTrusted(e.target.value);
              setSaved(false);
            }}
            placeholder="my-ci-bot, security-agent[bot]"
            className="mt-2 w-full rounded-md border border-border bg-canvas px-3 py-1.5 text-sm focus:border-accent focus:outline-none"
          />
        </div>
        <div className="flex items-center gap-3">
          <button type="submit" className="rounded-md bg-success-emphasis px-4 py-1.5 font-medium text-white hover:opacity-90">
            Save and test
          </button>
          {saved && <span className="text-sm text-fg-muted">Saved.</span>}
        </div>
        {check && (
          <p className={`flex items-start gap-2 text-sm ${check.ok ? "text-success" : "text-danger"}`}>
            {check.ok ? <CheckCircleFillIcon size={16} className="mt-0.5" /> : <XCircleFillIcon size={16} className="mt-0.5" />}
            {check.message}
          </p>
        )}
      </form>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Options />
  </StrictMode>,
);
