// @vitest-environment jsdom
// Writing a review in the extension: line comments, comments from findings, and submitting it to GitHub.
import { act, cleanup, configure, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "@/review/App";
import { type Call, demo, fakeChrome, fakeGitHub, loadWasm } from "@/test/fakes";

configure({ asyncUtilTimeout: 5000 });
beforeAll(loadWasm);

const REVIEWS = "POST https://api.github.com/repos/o/r/pulls/7/reviews";
let github: ReturnType<typeof fakeGitHub>;

function setup(token = "t0ken", review: (call: Call) => Response = () => github.json({ html_url: "https://github.com/o/r/pull/7#pullrequestreview-1" })) {
  github = fakeGitHub({ [REVIEWS]: review });
  vi.stubGlobal("fetch", github.fetch);
  vi.stubGlobal("chrome", fakeChrome(token ? { token } : {}).chrome);
}

beforeEach(() => setup());
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  localStorage.clear();
});

async function open(tab: string) {
  history.replaceState(null, "", `?pr=o%2Fr%237&tab=${tab}`);
  render(<App />);
  await waitFor(() => expect(document.body.textContent).toContain(demo.pr.title));
}

function editor() {
  return screen.getByPlaceholderText(/Leave a comment \(Markdown/) as HTMLTextAreaElement;
}

function submitted(): { event: string; body?: string; comments: { path: string; line: number; start_line?: number; body: string }[] } {
  return github.calls.find((c) => c.method === "POST")!.body as never;
}

describe("line comments", () => {
  it("comments on one line, edits and deletes it", async () => {
    await open("changes");
    const plus = screen.getAllByRole("button", { name: "Comment on this line" });
    await act(async () => fireEvent.mouseDown(plus[1]!));
    await act(async () => fireEvent.mouseUp(window));
    expect(document.body.textContent).toMatch(/Comment on line \d+ of/);
    fireEvent.change(editor(), { target: { value: "Why this import?" } });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Add review comment" })));
    expect(screen.getByText("Pending")).toBeTruthy();
    expect(screen.getByText("Why this import?")).toBeTruthy();

    await act(async () => fireEvent.click(screen.getByTitle("Edit")));
    fireEvent.change(editor(), { target: { value: "Why this import, though?" } });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Update comment" })));
    expect(screen.getByText("Why this import, though?")).toBeTruthy();

    await act(async () => fireEvent.click(screen.getByTitle("Delete")));
    expect(screen.queryByText("Pending")).toBeNull();
  });

  it("selects several lines by dragging or shift-clicking", async () => {
    await open("changes");
    const plus = screen.getAllByRole("button", { name: "Comment on this line" });
    await act(async () => fireEvent.mouseDown(plus[1]!));
    await act(async () => fireEvent.mouseEnter(plus[3]!.closest("tr")!));
    await act(async () => fireEvent.mouseUp(window));
    expect(document.body.textContent).toMatch(/Comment on lines \d+–\d+ of/);
    // Escape cancels.
    await act(async () => fireEvent.keyDown(editor(), { key: "Escape" }));
    expect(screen.queryByPlaceholderText(/Leave a comment \(Markdown/)).toBeNull();

    // Click one line, shift-click another; Ctrl+Enter adds the comment.
    await act(async () => fireEvent.mouseDown(plus[2]!));
    await act(async () => fireEvent.mouseUp(window));
    await act(async () => fireEvent.keyDown(editor(), { key: "Escape" }));
    await act(async () => fireEvent.mouseDown(plus[4]!, { shiftKey: true }));
    expect(document.body.textContent).toMatch(/Comment on lines \d+–\d+ of/);
    fireEvent.change(editor(), { target: { value: "These three lines" } });
    await act(async () => fireEvent.keyDown(editor(), { key: "Enter", ctrlKey: true }));
    expect(screen.getByText("These three lines")).toBeTruthy();
  });

  it("drafts comments from findings and annotations on the diff", async () => {
    await open("changes");
    const comment = screen.getAllByRole("button", { name: "Comment" });
    await act(async () => fireEvent.click(comment[0]!));
    expect(editor().value.length).toBeGreaterThan(0);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Add review comment" })));
    expect(screen.getAllByText("Pending")).toHaveLength(1);
  });
});

describe("the review", () => {
  it("adds findings from the Security tab and submits everything as one review", async () => {
    await open("security");
    const add = screen.getAllByRole("button", { name: "Add to review" });
    await act(async () => fireEvent.click(add[0]!));
    expect(screen.getByRole("button", { name: /Added to your review/ })).toBeTruthy();

    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Review changes/ })));
    expect(screen.getByText("1 line comment will be submitted.")).toBeTruthy();
    fireEvent.change(screen.getByPlaceholderText("Leave a comment"), { target: { value: "Fix the injection first." } });
    await act(async () => fireEvent.click(screen.getByRole("radio", { name: /Request changes/ })));
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Submit review" })));
    await waitFor(() => expect(screen.getByText("Review submitted")).toBeTruthy());

    const review = submitted();
    expect(review.event).toBe("REQUEST_CHANGES");
    expect(review.body).toBe("Fix the injection first.");
    expect(review.comments).toHaveLength(1);
    expect(review.comments[0]!.path).toBe("src/payments/refunds.ts");
    expect(github.calls.find((c) => c.method === "POST")!.headers.Authorization).toBe("Bearer t0ken");
    // Submitted drafts are gone.
    expect(localStorage.length).toBe(0);
  });

  it("needs a comment unless approving", async () => {
    await open("briefing");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Review changes/ })));
    const submit = screen.getByRole("button", { name: "Submit review" }) as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    await act(async () => fireEvent.click(screen.getByRole("radio", { name: /Approve/ })));
    expect(submit.disabled).toBe(false);
    await act(async () => fireEvent.click(submit));
    await waitFor(() => expect(submitted().event).toBe("APPROVE"));
  });

  it("explains a missing token and GitHub's refusals", async () => {
    setup("");
    await open("briefing");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Review changes/ })));
    await act(async () => fireEvent.click(screen.getByRole("radio", { name: /Approve/ })));
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Submit review" })));
    expect(screen.getByText(/Reviewing needs a GitHub token/)).toBeTruthy();
    expect(github.calls.some((c) => c.method === "POST")).toBe(false);
    cleanup();

    for (const [status, body, message] of [
      [422, { message: "Unprocessable Entity", errors: ["Can not approve your own pull request"] }, "GitHub didn't accept the review: Unprocessable Entity: Can not approve your own pull request"],
      [403, {}, "GitHub refused the review"],
      [401, {}, "GitHub rejected the token"],
    ] as const) {
      setup("t0ken", () => github.json(body, status));
      await open("briefing");
      await act(async () => fireEvent.click(screen.getByRole("button", { name: /Review changes/ })));
      await act(async () => fireEvent.click(screen.getByRole("radio", { name: /Approve/ })));
      await act(async () => fireEvent.click(screen.getByRole("button", { name: "Submit review" })));
      await waitFor(() => expect(document.body.textContent).toContain(message));
      cleanup();
    }
  });

  it("keeps drafts per pull request, and doesn't submit ones from an earlier commit", async () => {
    const draft = { path: "src/payments/refunds.ts", target: { line: 2, side: "RIGHT" }, body: "Old remark", id: "a", commitId: "earlier" };
    localStorage.setItem("reviewhub:review:o/r#7", JSON.stringify({ drafts: [draft], body: "" }));
    await open("changes");
    expect(screen.getByText("Outdated")).toBeTruthy();
    expect(screen.queryByTitle("Edit")).toBeNull();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Review changes/ })));
    expect(screen.getByText(/1 written on an earlier commit won't be/)).toBeTruthy();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Close" })));
  });
});
