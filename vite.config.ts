import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import fs from "node:fs";

const host = process.env.TAURI_DEV_HOST;
const packageJson = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, "package.json"), "utf8")
);

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  define: {
    "import.meta.env.VITE_APP_VERSION": JSON.stringify(packageJson.version),
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
