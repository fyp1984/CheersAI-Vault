// Copyright 2026 CheersAI. Licensed under Apache-2.0.
/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    include: [
      "src/**/*.spec.ts",
      "src/**/*.spec.tsx",
    ],
    exclude: [
      "node_modules",
      "dist",
      "src-tauri/**",
      "sdlc/**",
      "compliance/**",
      "tests/e2e/**",
      "apps/**",
      "src/**/node:test.*",
      "src/**/*.test.ts",
      "src/**/*.test.tsx",
    ],
    setupFiles: ["./vitest.setup.ts"],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      reportsDirectory: "test-results/vitest-coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.d.ts",
        "src/**/*.stories.*",
        "src/main.tsx",
        "src/**/*.test.*",
        "src/**/*.spec.*",
      ],
    },
    outputFile: {
      json: "test-results/vitest-results.json",
      junit: "test-results/vitest-junit.xml",
    },
  },
});
