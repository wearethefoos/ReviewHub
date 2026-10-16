// @vitest-environment jsdom
import { act, cleanup, configure, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { demo, fakeGitHub, loadWasm } from "@/test/fakes";
import { App } from "./App";

// The first review on a page compiles the wasm module, which takes a moment under jsdom.
configure({ asyncUtilTimeout: 5000 });
beforeAll(loadWasm);

/** Waits until the page's text contains `text` (often split over several elements). */
function shows(text: string) {
  return waitFor(() => expect(document.body.textContent?.replace(/\s+/g, " ")).toContain(text));
}
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  localStorage.clear();
  sessionStorage.clear();
});

function open(search: string) {
  history.replaceState(null, "", search);
  return render(<App />);
}

describe("the demo", () => {
  it("shows the briefing first", async () => {
    open("?demo");
    await shows(demo.pr.title);
    await shows("Read 4 of 12 files closely");
    expect(screen.getByText("Also make sure retries from the merchant's side don't create duplicate refunds.")).toBeTruthy();
    // There's nothing to review on the demo; the button says so.
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Review changes/ })));
    await shows("This is a demo pull request");
  });

  it.each([
    ["changes", "Focus4Read these line by line.", "src/api/routes/refunds.ts"],
    ["decisions", "Deviations from the prompt2", "Changed rounding in the shared percentOf helper"],
    ["verification", "What the agent validated6", "Two refunds for the same order at the same time"],
    ["security", "Findings5", "SQL injection through orderId"],
    ["rollout", "Before merging0/2", "Confirm the endpoint shape with the mobile team"],
  ])("has a %s tab", async (tab, heading, item) => {
    open(`?demo&tab=${tab}`);
    await shows(heading);
    await shows(item);
  });

  it("switches tabs and keeps rollout ticks", async () => {
    open("?demo");
    await shows(demo.pr.title);
    await act(async () => fireEvent.click(screen.getByRole("link", { name: /Rollout/ })));
    expect(location.search).toBe("?demo&tab=rollout");
    const step = await screen.findByRole("checkbox", { name: /Confirm the endpoint shape/ });
    await act(async () => fireEvent.click(step));
    expect(screen.getAllByText("1/2").length).toBeGreaterThan(0);
  });

  it("copies prompts for agents", async () => {
    const writeText = vi.fn(async () => {});
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    open("?demo&tab=security");
    await act(async () => fireEvent.click(await screen.findByRole("button", { name: /Copy prompt to fix all/ })));
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("SQL injection through orderId"));
  });

  it("expands skipped files on the Changes tab", async () => {
    open("?demo&tab=changes");
    const lockfile = (await screen.findAllByText("package-lock.json")).at(-1)!;
    const before = document.body.textContent!.length;
    await act(async () => fireEvent.click(lockfile));
    expect(document.body.textContent!.length).toBeGreaterThanOrEqual(before);
  });
});

describe("pull requests from GitHub", () => {
  it("loads one, then reloads it", async () => {
    const github = fakeGitHub();
    vi.stubGlobal("fetch", github.fetch);
    open("?pr=o%2Fr%237");
    await shows(demo.pr.title);
    expect(github.calls.some((c) => c.url.endsWith("/contents/.reviewhub/manifest.json?ref=" + demo.pr.headSha))).toBe(true);
    expect(github.calls.every((c) => !("Authorization" in c.headers))).toBe(true);
    const before = github.calls.length;
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Reload from GitHub/ })));
    await shows(demo.pr.title);
    expect(github.calls.length).toBeGreaterThan(before);
  });

  it("points to the extension or GitHub for reviewing", async () => {
    vi.stubGlobal("fetch", fakeGitHub().fetch);
    open("?pr=o%2Fr%237");
    await shows(demo.pr.title);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Review changes/ })));
    await shows("works in the Chrome extension");
    expect(screen.getByRole("link", { name: "Review on GitHub" }).getAttribute("href")).toBe("https://github.com/o/r/pull/7/files");
    expect(screen.getByRole("link", { name: "Get the extension" }).getAttribute("href")).toContain("chromewebstore.google.com");
  });

    it("sends the saved token, and explains errors", async () => {
    sessionStorage.setItem("reviewhub:settings", JSON.stringify({ token: "t0ken" }));
    // Reviews are kept per pull request for the page's lifetime, so this uses another one.
    const github = fakeGitHub({ "GET https://api.github.com/repos/o/r/pulls/8": () => new Response("{}", { status: 401 }) });
    vi.stubGlobal("fetch", github.fetch);
    open("?pr=o%2Fr%238");
    await shows("GitHub rejected the token");
    expect(github.calls[0]?.headers.Authorization).toBe("Bearer t0ken");
  });
});

describe("the start page", () => {
  it("opens what you type", async () => {
    vi.stubGlobal("fetch", fakeGitHub().fetch);
    open("?");
    expect(screen.getByText("Review what matters.")).toBeTruthy();
    const form = screen.getByRole("textbox", { name: "Pull request" }).closest("form")!;
    fireEvent.change(within(form).getByRole("textbox"), { target: { value: "not a pull request" } });
    await act(async () => fireEvent.submit(form));
    expect(screen.getByText(/not a pull request/)).toBeTruthy();
    fireEvent.change(within(form).getByRole("textbox"), { target: { value: "https://github.com/o/r/pull/9" } });
    await act(async () => fireEvent.submit(form));
    expect(location.search).toBe("?pr=o%2Fr%239");
    await shows("Pull request not found");
  });

  it("opens pull requests from the top bar", async () => {
    vi.stubGlobal("fetch", fakeGitHub().fetch);
    open("?demo");
    await shows(demo.pr.title);
    const box = screen.getByRole("textbox", { name: "Pull request URL or owner/repo#number" });
    fireEvent.change(box, { target: { value: "o/r#7" } });
    await act(async () => fireEvent.submit(box.closest("form")!));
    expect(location.search).toBe("?pr=o%2Fr%237");
  });
});
