import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import fs from "node:fs";
import type { ProxyOptions } from "vite";

const host = process.env.TAURI_DEV_HOST;
const devPort = Number(process.env.VITE_DEV_PORT || "3000");
const previewPort = Number(process.env.VITE_PREVIEW_PORT || `${devPort}`);
const runtimeProxyTarget =
  process.env.VITE_RUNTIME_PROXY_TARGET || "http://127.0.0.1:8787";
const packageJson = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, "package.json"), "utf8")
);

function createRuntimeProxy(): ProxyOptions {
  return {
    target: runtimeProxyTarget,
    changeOrigin: true,
    configure: (proxy) => {
      proxy.on("proxyReq", (proxyReq) => {
        // Browser-hosted local dev/preview uses a same-origin `/api` proxy and
        // does not need the front-end origin forwarded to Runtime. Stripping
        // these headers keeps proxy behavior aligned with the docs test server
        // and avoids Runtime CORS false negatives for local ports like 3001/5174.
        proxyReq.removeHeader("origin");
        proxyReq.removeHeader("referer");
        proxyReq.removeHeader("sec-fetch-site");
        proxyReq.removeHeader("sec-fetch-mode");
        proxyReq.removeHeader("sec-fetch-dest");
        proxyReq.removeHeader("sec-fetch-user");
      });
    },
  };
}

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
    port: devPort,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 3001,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
    // 开发模式下把同源 `/api` 请求代理到本机企业 Runtime，配合
    // `src/lib/runtime/client.ts` 默认的同源相对地址使用；不需要 Runtime
    // 时该代理不会被触发，不影响其余开发流程。
    proxy: {
      "/api": createRuntimeProxy(),
    },
  },
  // `vite preview` 用于本地验证生产构建，也保留 `/api` 代理，
  // 使浏览器访问 preview server 时仍能到达本机 Runtime。
  preview: {
    port: previewPort,
    strictPort: true,
    host: false,
    proxy: {
      "/api": createRuntimeProxy(),
    },
  },
}));
