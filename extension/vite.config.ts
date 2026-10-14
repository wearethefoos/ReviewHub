import { cpSync } from "node:fs";
import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const root = import.meta.dirname;

// Two builds of the same pages:
// - `vite build` → dist/: the browser extension (public/: manifest, background worker, content script, icons);
// - `vite build --mode web` → dist-web/: the web app for GitHub Pages (index redirect, web manifest, icons).
export default defineConfig(({ mode }) => {
  const web = mode === "web";
  return {
    base: "./",
    plugins: [
      react(),
      tailwindcss(),
      web && {
        name: "reviewhub-web-assets",
        transformIndexHtml: () => [
          { tag: "link", attrs: { rel: "manifest", href: "manifest.webmanifest" }, injectTo: "head" as const },
          { tag: "link", attrs: { rel: "apple-touch-icon", href: "icons/128.png" }, injectTo: "head" as const },
          { tag: "meta", attrs: { name: "theme-color", content: "#1f2328" }, injectTo: "head" as const },
        ],
        closeBundle() {
          cpSync(resolve(root, "public/icons"), resolve(root, "dist-web/icons"), { recursive: true });
          cpSync(resolve(root, "web/manifest.webmanifest"), resolve(root, "dist-web/manifest.webmanifest"));
        },
      },
    ],
    resolve: { alias: { "@": resolve(root, "src") } },
    publicDir: web ? false : "public",
    build: {
      outDir: web ? "dist-web" : "dist",
      emptyOutDir: true,
      target: "chrome120",
      // The wasm module is ~2 MB; it's loaded once and cached.
      chunkSizeWarningLimit: 3000,
      rollupOptions: {
        input: {
          review: resolve(root, "review.html"),
          options: resolve(root, "options.html"),
          ...(web && { index: resolve(root, "index.html") }),
        },
      },
    },
    test: { include: ["src/**/*.test.ts"] },
  };
});
