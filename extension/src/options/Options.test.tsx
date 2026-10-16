// @vitest-environment jsdom
import { act, cleanup, configure, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fakeChrome, fakeGitHub } from "@/test/fakes";
import { Options } from "./Options";

configure({ asyncUtilTimeout: 5000 });
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  localStorage.clear();
  sessionStorage.clear();
});

const API = "https://api.github.com";

function github(status = 200) {
  const fake = fakeGitHub({
    [`GET ${API}/user`]: () => fake.json({ login: "octocat" }, status),
    [`GET ${API}/rate_limit`]: () => fake.json({ resources: { core: { remaining: 4999, limit: 5000 } } }),
  });
  vi.stubGlobal("fetch", fake.fetch);
  return fake;
}

/** Renders the page once its saved settings have loaded (before that, loading would overwrite what's typed). */
async function mount() {
  render(<Options />);
  await act(async () => {});
}

async function save(fields: { token?: string; trusted?: string; remember?: boolean }) {
  if (fields.token !== undefined) fireEvent.change(screen.getByLabelText("GitHub token"), { target: { value: fields.token } });
  if (fields.trusted !== undefined) fireEvent.change(screen.getByLabelText("Also trust reports from"), { target: { value: fields.trusted } });
  if (fields.remember) fireEvent.click(screen.getByRole("checkbox", { name: /Remember the token/ }));
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Save and test" })));
}

describe("in the web app", () => {
  it("keeps the token in this tab unless remembered, and tests it", async () => {
    const fake = github();
    await mount();
    await save({ token: " ghp_test ", trusted: "ci-bot, , sec[bot]" });
    await waitFor(() => expect(document.body.textContent).toContain("Signed in as octocat. 4999 of 5000 API requests left this hour."));
    expect(fake.calls[0]?.headers.Authorization).toBe("Bearer ghp_test");
    expect(JSON.parse(sessionStorage.getItem("reviewhub:settings")!)).toMatchObject({ token: "ghp_test", trustedAuthors: ["ci-bot", "sec[bot]"] });
    expect(JSON.parse(localStorage.getItem("reviewhub:settings")!).token).toBe("");

    await save({ remember: true });
    expect(JSON.parse(localStorage.getItem("reviewhub:settings")!).token).toBe("ghp_test");
    cleanup();

    // Read back on the next visit.
    sessionStorage.clear();
    await mount();
    await waitFor(() => expect((screen.getByLabelText("GitHub token") as HTMLInputElement).value).toBe("ghp_test"));
    expect((screen.getByLabelText("Also trust reports from") as HTMLInputElement).value).toBe("ci-bot, sec[bot]");
  });

  it("works without a token, and reports a rejected one", async () => {
    github();
    await mount();
    await save({ token: "" });
    await waitFor(() => expect(document.body.textContent).toContain("No token: public repositories only."));
    cleanup();

    github(401);
    await mount();
    await save({ token: "bad" });
    await waitFor(() => expect(document.body.textContent).toContain("GitHub rejected the token"));
  });
});

describe("in the extension", () => {
  it("stores settings in chrome.storage", async () => {
    github();
    const { chrome, store } = fakeChrome({ token: "old", trustedAuthors: ["bot"] });
    vi.stubGlobal("chrome", chrome);
    await mount();
    await waitFor(() => expect((screen.getByLabelText("GitHub token") as HTMLInputElement).value).toBe("old"));
    expect(screen.queryByRole("checkbox", { name: /Remember the token/ })).toBeNull();
    await save({ token: "new", trusted: "" });
    expect(store).toMatchObject({ token: "new", trustedAuthors: [] });
    expect(sessionStorage.length + localStorage.length).toBe(0);
  });
});
