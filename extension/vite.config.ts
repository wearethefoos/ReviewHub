import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Builds the extension into dist/: the review and options pages are bundled;
// public/ (manifest, background worker, content script, icons) is copied as is.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": resolve(import.meta.dirname, "src") } },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "chrome120",
    // The wasm module is ~2 MB; extensions load it from disk, so this is expected.
    chunkSizeWarningLimit: 3000,
    rollupOptions: {
      input: {
        review: resolve(import.meta.dirname, "review.html"),
        options: resolve(import.meta.dirname, "options.html"),
      },
    },
  },
  test: { include: ["src/**/*.test.ts"] },
});
