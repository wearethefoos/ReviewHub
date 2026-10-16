import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it } from "vitest";
import { loadCore } from "./core";
import { dismissUpdate, pendingUpdate, type UpdateEnv } from "./updates";

beforeAll(async () => {
  await loadCore(readFileSync(new URL("../wasm/reviewhub_wasm_bg.wasm", import.meta.url)));
});

const DAY = 24 * 60 * 60 * 1000;

/** A fake browser: in-memory storage, a GitHub that answers `latest` (or fails), and a settable clock. */
function fakeEnv(opts: { installType?: string; latest?: string | Error | number; version?: string } = {}) {
  const store: Record<string, unknown> = {};
  const fetches: { url: string; init?: RequestInit }[] = [];
  let latest = opts.latest ?? "v99.0.0";
  let time = 1_000_000;
  const env: UpdateEnv = {
    installType: async () => opts.installType ?? "development",
    version: opts.version ?? "0.6.0",
    storage: {
      get: async (key) => (key in store ? { [key]: store[key] } : {}),
      set: async (items) => void Object.assign(store, items),
    },
    fetch: (async (url: string, init?: RequestInit) => {
      fetches.push({ url, init });
      if (latest instanceof Error) throw latest;
      if (typeof latest === "number") return new Response("{}", { status: latest });
      return new Response(JSON.stringify({ tag_name: latest }));
    }) as typeof fetch,
    now: () => time,
  };
  return {
    env,
    store,
    fetches,
    advance: (ms: number) => void (time += ms),
    answer: (next: string | Error | number) => void (latest = next),
  };
}

describe("pendingUpdate", () => {
  it("tells unpacked installs about a newer release, asking GitHub anonymously", async () => {
    const { env, fetches, store } = fakeEnv();
    const pending = await pendingUpdate(env);
    expect(pending?.tag).toBe("v99.0.0");
    expect(pending?.text).toMatch(/^ReviewHub 99\.0\.0 is available \(you have 0\.6\.0\)\./);
    expect(pending?.text).toContain("chromewebstore.google.com");
    expect(fetches).toHaveLength(1);
    expect(fetches[0]?.url).toBe("https://api.github.com/repos/wearethefoos/ReviewHub/releases/latest");
    expect(new Headers(fetches[0]?.init?.headers).has("Authorization")).toBe(false);
    expect(store.latestRelease).toEqual({ tag: "v99.0.0", checked: 1_000_000 });
  });

  it("leaves store installs to Chrome, and says nothing when up to date", async () => {
    const store = fakeEnv({ installType: "normal" });
    expect(await pendingUpdate(store.env)).toBeNull();
    expect(store.fetches).toHaveLength(0);
    expect(await pendingUpdate(fakeEnv({ latest: "v0.6.0" }).env)).toBeNull();
  });

  it("asks GitHub at most once a day", async () => {
    const { env, fetches, advance, answer } = fakeEnv();
    await pendingUpdate(env);
    answer("v100.0.0");
    advance(DAY - 1);
    expect((await pendingUpdate(env))?.tag).toBe("v99.0.0");
    expect(fetches).toHaveLength(1);
    advance(1);
    expect((await pendingUpdate(env))?.tag).toBe("v100.0.0");
    expect(fetches).toHaveLength(2);
  });

  it("falls back to the last answer when GitHub fails", async () => {
    const { env, advance, answer } = fakeEnv();
    await pendingUpdate(env);
    advance(DAY);
    for (const failure of [new Error("offline"), 500, ""]) {
      answer(failure);
      expect((await pendingUpdate(env))?.tag).toBe("v99.0.0");
    }
    expect(await pendingUpdate(fakeEnv({ latest: new Error("offline") }).env)).toBeNull();
  });

  it("stays dismissed until the next release", async () => {
    const { env, advance, answer } = fakeEnv();
    await dismissUpdate(env, "v99.0.0");
    expect(await pendingUpdate(env)).toBeNull();
    answer("v100.0.0");
    advance(DAY);
    expect((await pendingUpdate(env))?.tag).toBe("v100.0.0");
  });
});
