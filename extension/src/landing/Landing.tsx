// The one-page website at the root of the GitHub Pages site. The web app lives
// next to it at review.html; links with ?pr= or ?demo are forwarded there.
import {
  ArrowRightIcon,
  BeakerIcon,
  CommentDiscussionIcon,
  CodeReviewIcon,
  DeviceMobileIcon,
  GitBranchIcon,
  GraphIcon,
  LightBulbIcon,
  LockIcon,
  MarkGithubIcon,
  ShieldCheckIcon,
  TerminalIcon,
  ZapIcon,
} from "@primer/octicons-react";
import type { ReactNode } from "react";
import { CopyButton } from "@/components/CopyButton";
import { EXTENSION_URL } from "@/lib/settings";

const REPO = "https://github.com/wearethefoos/ReviewHub";
const INSTALL = "curl -fsSL https://raw.githubusercontent.com/wearethefoos/ReviewHub/main/install.sh | sh -s -- init";

function Button({ href, children, primary }: { href: string; children: ReactNode; primary?: boolean }) {
  return (
    <a
      href={href}
      className={
        primary
          ? "inline-flex items-center gap-2 rounded-md bg-success-emphasis px-5 py-2.5 text-[15px] font-semibold text-white no-underline hover:opacity-90"
          : "inline-flex items-center gap-2 rounded-md border border-border bg-canvas px-5 py-2.5 text-[15px] font-semibold text-fg no-underline hover:bg-canvas-subtle"
      }
    >
      {children}
    </a>
  );
}

/** A screenshot in the page's colour scheme: `img/<name>-light.png` or `img/<name>-dark.png`. */
function Shot({ name, alt, className }: { name: string; alt: string; className?: string }) {
  return (
    <div className={"overflow-hidden rounded-xl border border-border bg-canvas shadow-[0_8px_30px_rgba(0,0,0,0.12)] " + (className ?? "")}>
      <div className="flex items-center gap-1.5 border-b border-border bg-canvas-subtle px-3 py-2">
        <span className="size-2.5 rounded-full bg-border" />
        <span className="size-2.5 rounded-full bg-border" />
        <span className="size-2.5 rounded-full bg-border" />
      </div>
      <picture>
        <source srcSet={`img/${name}-dark.png`} media="(prefers-color-scheme: dark)" />
        <img src={`img/${name}-light.png`} alt={alt} width={1280} height={800} className="block h-auto w-full" loading="lazy" />
      </picture>
    </div>
  );
}

function Feature({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <div className="rounded-xl border border-border bg-canvas p-5">
      <div className="mb-3 flex size-9 items-center justify-center rounded-lg bg-accent-subtle text-accent">{icon}</div>
      <h3 className="text-base font-semibold">{title}</h3>
      <p className="mt-1.5 text-[15px] leading-relaxed text-fg-muted">{children}</p>
    </div>
  );
}

function Spotlight({ eyebrow, title, children, image, alt, flip }: { eyebrow: string; title: string; children: ReactNode; image: string; alt: string; flip?: boolean }) {
  return (
    <div className="grid items-center gap-10 lg:grid-cols-[2fr_3fr]">
      <div className={flip ? "lg:order-2" : ""}>
        <p className="text-sm font-semibold uppercase tracking-wide text-accent">{eyebrow}</p>
        <h2 className="mt-2 text-2xl font-semibold leading-tight md:text-3xl">{title}</h2>
        <div className="mt-4 space-y-3 text-[16px] leading-relaxed text-fg-muted">{children}</div>
      </div>
      <Shot name={image} alt={alt} className={flip ? "lg:order-1" : ""} />
    </div>
  );
}

function Check({ children }: { children: ReactNode }) {
  return (
    <li className="flex gap-2">
      <span className="mt-1 text-success">
        <ShieldCheckIcon size={16} />
      </span>
      <span>{children}</span>
    </li>
  );
}

export function Landing() {
  return (
    <div className="min-h-screen bg-canvas text-fg">
      <header className="bg-header text-header-fg">
        <div className="mx-auto flex max-w-[1200px] items-center gap-3 px-4 py-3 md:px-6">
          <img src="icons/48.png" alt="" className="size-7 rounded-md" />
          <span className="font-semibold">ReviewHub</span>
          <nav className="ml-auto flex items-center gap-5 text-sm">
            <a href="#features" className="hidden text-header-fg/80 no-underline hover:text-header-fg sm:inline">
              Features
            </a>
            <a href="#setup" className="hidden text-header-fg/80 no-underline hover:text-header-fg sm:inline">
              Set up
            </a>
            <a href="review.html" className="text-header-fg/80 no-underline hover:text-header-fg">
              Open the app
            </a>
            <a href={REPO} className="text-header-fg/80 hover:text-header-fg" aria-label="ReviewHub on GitHub">
              <MarkGithubIcon size={20} />
            </a>
          </nav>
        </div>
      </header>

      <main>
        <section className="border-b border-border bg-canvas-subtle">
          <div className="mx-auto max-w-[1200px] px-4 pb-16 pt-14 text-center md:px-6 md:pt-20">
            <p className="inline-flex items-center gap-2 rounded-full border border-border bg-canvas px-3 py-1 text-sm text-fg-muted">
              <ZapIcon size={14} className="text-attention" /> Free and open source · nothing hosted
            </p>
            <h1 className="mx-auto mt-6 max-w-4xl text-4xl font-semibold leading-[1.1] tracking-tight md:text-6xl">
              Your agents write the code.
              <br />
              <span className="text-accent">Review what matters.</span>
            </h1>
            <p className="mx-auto mt-6 max-w-2xl text-lg leading-relaxed text-fg-muted md:text-xl">
              ReviewHub opens an agent&apos;s pull request with its account of the work: what was asked, where it deviated, what it decided and what it
              couldn&apos;t check. An independent security review and your CI&apos;s coverage rank every file, so you read 4 of 12 files closely instead of
              all of them.
            </p>
            <div className="mt-8 flex flex-wrap justify-center gap-3">
              <Button href={EXTENSION_URL} primary>
                Add to Chrome <ArrowRightIcon size={16} />
              </Button>
              <Button href="review.html?demo">Try the demo</Button>
              <Button href={REPO}>
                <MarkGithubIcon size={16} /> GitHub
              </Button>
            </div>
            <Shot name="briefing" alt="The ReviewHub briefing for an agent-authored pull request" className="mx-auto mt-14 max-w-5xl text-left" />
          </div>
        </section>

        <section className="mx-auto max-w-[1200px] px-4 py-16 md:px-6">
          <div className="grid gap-8 text-center md:grid-cols-3">
            {[
              ["The ask", "What the user wanted, next to what the agent delivered, so drift is obvious."],
              ["The judgement calls", "Every deviation and open decision, with the alternatives and a prompt to switch."],
              ["The blind spots", "What the agent couldn't validate, and exactly how to close each gap."],
            ].map(([title, text]) => (
              <div key={title}>
                <h3 className="text-lg font-semibold">{title}</h3>
                <p className="mt-2 text-fg-muted">{text}</p>
              </div>
            ))}
          </div>
        </section>

        <section id="features" className="mx-auto max-w-[1200px] space-y-24 px-4 pb-20 md:px-6">
          <Spotlight
            eyebrow="Focus ranking"
            title="Read the 4 files that matter. Skim the rest."
            image="changes"
            alt="The Changes tab, with files grouped into Focus, Skim and Skip"
          >
            <p>
              Every changed file gets a focus score from security findings, the agent&apos;s deviations and decisions, validation gaps, test coverage of the
              changed lines, complexity and sensitive paths. The diff opens on <strong className="text-fg">Focus</strong> files and collapses the
              generated and mechanical ones.
            </p>
            <p>The agent can ask for a file to be skipped, but only independent signals can confirm it.</p>
          </Spotlight>

          <Spotlight
            flip
            eyebrow="Independent security review"
            title="A second agent that didn't write the code checks it."
            image="security"
            alt="Security findings, each with a suggested fix as a patch"
          >
            <p>
              A separate reviewer gets only the pull request, never the author&apos;s conversation. Each finding comes with a patch to apply or a prompt to
              hand to an agent.
            </p>
            <p>
              It runs in CI when you add an Anthropic key, in the agent&apos;s own session otherwise, or on <code className="font-mono text-sm">git push</code>{" "}
              with the pre-push hook. Reports that contain anything credential-shaped are rejected before they&apos;re posted.
            </p>
          </Spotlight>

          <Spotlight
            eyebrow="Review without leaving"
            title="Turn findings into GitHub comments in one click."
            image="review"
            alt="A security finding turned into a pending review comment with a suggested change"
          >
            <p>
              Comment on a line or drag over several, turn any finding into a comment with the fix as a GitHub suggestion, then submit everything as one
              review: Comment, Approve or Request changes.
            </p>
            <p>Syntax highlighting, dark mode, and the GitHub look you already know.</p>
          </Spotlight>
        </section>

        <section className="border-y border-border bg-canvas-subtle">
          <div className="mx-auto max-w-[1200px] px-4 py-20 md:px-6">
            <h2 className="text-center text-2xl font-semibold md:text-3xl">Everything a reviewer needs, none of the reading you don&apos;t</h2>
            <div className="mt-10 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
              <Feature icon={<CommentDiscussionIcon size={18} />} title="The ask, without the secrets">
                The agent summarizes the request for reviewers. Prompts can be private, so they&apos;re never pasted verbatim.
              </Feature>
              <Feature icon={<GitBranchIcon size={18} />} title="Deviations">
                Anywhere the delivery differs from the request, with the agent&apos;s reason and how much it matters.
              </Feature>
              <Feature icon={<LightBulbIcon size={18} />} title="Decisions with alternatives">
                Questions the prompt left open, what the agent chose, and ready-to-run prompts for the paths it didn&apos;t take.
              </Feature>
              <Feature icon={<BeakerIcon size={18} />} title="Evidence, not claims">
                Only checks the agent actually ran, with the command and output. Failures and skips come first.
              </Feature>
              <Feature icon={<GraphIcon size={18} />} title="Coverage and complexity from CI">
                <code className="font-mono text-sm">reviewhub init</code> detects Rust, JS/TS and Python and publishes which changed lines no test runs.
              </Feature>
              <Feature icon={<CodeReviewIcon size={18} />} title="A briefing on every PR">
                One comment on each pull request with blockers and files to read, linked into the diff. Great in the GitHub mobile app.
              </Feature>
              <Feature icon={<DeviceMobileIcon size={18} />} title="Review from your phone">
                The web app and the briefing comment work anywhere, and repo-level setup works in Claude Code cloud sessions.
              </Feature>
              <Feature icon={<LockIcon size={18} />} title="Nothing hosted">
                Your browser talks to GitHub directly. Your token stays on your device. No servers, no tracking.
              </Feature>
            </div>
          </div>
        </section>

        <section className="mx-auto max-w-[1200px] px-4 py-20 md:px-6">
          <h2 className="text-center text-2xl font-semibold md:text-3xl">Three reports, three authors</h2>
          <p className="mx-auto mt-3 max-w-2xl text-center text-fg-muted">No agent grades its own work. ReviewHub only trusts reports from people with write access, the PR author and logins you choose.</p>
          <div className="mt-10 grid gap-4 md:grid-cols-3">
            {[
              ["Review manifest", "The authoring agent", "The ask, deviations, decisions, validation, gaps and rollout steps."],
              ["Security report", "A separate reviewer agent", "Findings with severity, location and a patch or prompt to fix each one."],
              ["Metrics report", "Your CI", "Coverage of the changed lines and how much more complex each file got."],
            ].map(([title, author, text], i) => (
              <div key={title} className="relative rounded-xl border border-border p-5">
                <span className="absolute -top-3 left-5 rounded-full bg-accent-emphasis px-2 text-xs font-semibold leading-6 text-white">{i + 1}</span>
                <h3 className="font-semibold">{title}</h3>
                <p className="text-sm text-accent">by {author}</p>
                <p className="mt-2 text-[15px] text-fg-muted">{text}</p>
              </div>
            ))}
          </div>
        </section>

        <section id="setup" className="bg-header text-header-fg">
          <div className="mx-auto grid max-w-[1200px] gap-10 px-4 py-20 md:px-6 lg:grid-cols-2">
            <div>
              <h2 className="text-2xl font-semibold md:text-3xl">Set up a repository in one command</h2>
              <p className="mt-3 text-header-fg/75">Run it in your repository&apos;s root and commit what it lists.</p>
              <div className="mt-6 flex items-start gap-2 rounded-lg border border-white/15 bg-black/30 p-3">
                <TerminalIcon size={16} className="mt-1 shrink-0 text-header-fg/60" />
                <code className="min-w-0 flex-1 break-all font-mono text-sm leading-6">{INSTALL}</code>
                <CopyButton text={INSTALL} className="shrink-0" />
              </div>
            </div>
            <ul className="space-y-3 text-[15px] leading-relaxed text-header-fg/85">
              <Check>Claude Code publishes a review manifest with every pull request it opens, including in cloud sessions from your phone.</Check>
              <Check>An independent security review runs on every pull request: in CI, in the session, or on push.</Check>
              <Check>CI publishes coverage and complexity for the test stacks it finds.</Check>
              <Check>Every pull request gets a briefing comment for reviewers.</Check>
            </ul>
          </div>
        </section>

        <section className="mx-auto max-w-[1200px] px-4 py-20 md:px-6">
          <h2 className="text-center text-2xl font-semibold md:text-3xl">Review wherever you are</h2>
          <div className="mt-10 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {[
              ["Chrome", "Open in ReviewHub on any pull request on github.com.", EXTENSION_URL, "Add to Chrome"],
              ["Any browser or phone", "The same review page as a web app, no install.", "review.html?demo", "Open the demo"],
              ["Claude Code", "/reviewhub:review walks you through what needs your judgement.", `${REPO}#claude-code-plugin`, "Install the plugin"],
              ["Terminal", "reviewhub review owner/repo#123, with --diff for the Focus files.", `${REPO}#cli-on-its-own`, "Get the CLI"],
            ].map(([title, text, href, cta]) => (
              <a key={title} href={href} className="group rounded-xl border border-border p-5 text-fg no-underline hover:border-accent">
                <h3 className="font-semibold">{title}</h3>
                <p className="mt-1.5 text-[15px] text-fg-muted">{text}</p>
                <p className="mt-3 inline-flex items-center gap-1 text-sm font-semibold text-accent">
                  {cta} <ArrowRightIcon size={14} className="transition-transform group-hover:translate-x-0.5" />
                </p>
              </a>
            ))}
          </div>
        </section>
      </main>

      <footer className="border-t border-border bg-canvas-subtle">
        <div className="mx-auto flex max-w-[1200px] flex-wrap items-center gap-x-6 gap-y-2 px-4 py-6 text-sm text-fg-muted md:px-6">
          <span>© 2026 Big Fat Coder B.V. · Apache License 2.0</span>
          <a href={REPO} className="ml-auto">
            GitHub
          </a>
          <a href={`${REPO}/blob/main/PRIVACY.md`}>Privacy</a>
          <a href={`${REPO}/releases`}>Releases</a>
        </div>
      </footer>
    </div>
  );
}
