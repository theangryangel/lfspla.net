import tailwindcss from "@tailwindcss/vite";
import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

export default defineConfig({
  resolve: {
    alias: {
      $assets: fileURLToPath(new URL("../assets", import.meta.url)),
    },
  },
  plugins: [
    tailwindcss(),
    sveltekit({
      compilerOptions: {
        // Force runes mode for the project, except for libraries. Can be removed in svelte 6.
        runes: ({ filename }) =>
          filename.split(/[/\\]/).includes("node_modules") ? undefined : true,
      },
      // Single-page app: the Rust backend serves dist/ and falls back to index.html.
      adapter: adapter({
        pages: "dist",
        assets: "dist",
        fallback: "index.html",
        strict: false,
      }),
    }),
  ],
  server: {
    host: "127.0.0.1",
    fs: {
      // Catalogue artwork lives beside the frontend, under the repository's
      // shared assets directory.
      allow: [fileURLToPath(new URL("..", import.meta.url))],
    },
    proxy: {
      // API and OAuth routes are served by Rust.
      "/api": { target: "http://localhost:8000", changeOrigin: true },
      "/auth": { target: "http://localhost:8000", changeOrigin: true },
    },
  },
  preview: { host: "127.0.0.1" },
});
