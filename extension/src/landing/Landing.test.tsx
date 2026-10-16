// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Landing } from "./Landing";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("introduces ReviewHub and links to the demo, the store and the web app", () => {
  vi.stubGlobal("matchMedia", (query: string) => ({ matches: query.includes("dark"), media: query, addEventListener() {}, removeEventListener() {} }));
  render(<Landing />);
  expect(screen.getByRole("heading", { level: 1 })).toBeTruthy();
  const links = screen.getAllByRole("link").map((a) => a.getAttribute("href") ?? "");
  expect(links.some((href) => href.includes("review.html?demo"))).toBe(true);
  expect(links.some((href) => href.includes("chromewebstore.google.com"))).toBe(true);
  // Screenshots come in the colour scheme of the page.
  const images = screen.getAllByRole("img").map((img) => img.getAttribute("src") ?? "");
  expect(images.length).toBeGreaterThan(0);
});
