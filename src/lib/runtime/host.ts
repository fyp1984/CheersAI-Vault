/**
 * 宿主检测单一入口。
 *
 * 页面不得各自猜测运行环境；一律通过本模块判断当前是 Tauri 桌面宿主还是
 * 普通浏览器宿主。检测本身只读取 Tauri 官方注入的 `globalThis.isTauri`
 * 标记（`@tauri-apps/api/core` 的 `isTauri()`），不调用 `invoke`、不注册
 * 事件、不访问文件系统或对话框，因此在普通浏览器中调用是完全安全的。
 */
export type HostKind = "tauri" | "browser";

/**
 * 与 `@tauri-apps/api/core` 的 `isTauri()` 语义保持一致的只读全局标记检测，
 * 但不静态引用 `@tauri-apps/*`，避免普通浏览器主入口打包 Tauri 依赖。
 * 参考实现：node_modules/@tauri-apps/api/core.js 的 `isTauri()`。
 */
function readTauriFlag(): boolean {
  return !!(globalThis as { isTauri?: boolean }).isTauri;
}

export function detectHost(): HostKind {
  return readTauriFlag() ? "tauri" : "browser";
}

export function isTauriHost(): boolean {
  return detectHost() === "tauri";
}

export function isBrowserHost(): boolean {
  return detectHost() === "browser";
}
