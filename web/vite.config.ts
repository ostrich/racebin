import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    conditions: ["browser"]
  },
  // RichTextEditor is loaded on demand, so Vite cannot discover these dependencies
  // during its initial source scan. Pre-bundle them to prevent a first-use reload.
  optimizeDeps: {
    include: [
      "@tiptap/core",
      "@tiptap/extension-table",
      "@tiptap/extension-task-item",
      "@tiptap/extension-task-list",
      "@tiptap/markdown",
      "@tiptap/starter-kit"
    ]
  },
  build: {
    rollupOptions: {
      output: {
        entryFileNames: "assets/app.js",
        chunkFileNames: "assets/[name]-[hash].js",
        assetFileNames: asset => asset.names.some(name => name.endsWith(".woff2"))
          ? "assets/[name][extname]"
          : "assets/app[extname]"
      }
    }
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.ts"]
  }
});
