import { fileURLToPath, URL } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [
    // macOS writes "._*" sidecar files on non-Apple drives; never treat them as routes.
    tanstackRouter({ target: "react", autoCodeSplitting: true, routeFileIgnorePattern: "^\\._" }),
    react(),
    tailwindcss(),
  ],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  server: { proxy: { "/api": "http://127.0.0.1:4747" } },
  test: {
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    exclude: ["**/node_modules/**", "**/dist/**", "**/._*"],
  },
});
