import { create } from "zustand";

const ACTIVE_PREVIEW_SESSION_KEY = "cheersai:active_preview_id";

function readSessionPreviewId(): string | null {
  try {
    const raw = window.sessionStorage.getItem(ACTIVE_PREVIEW_SESSION_KEY);
    if (!raw) return null;
    // 严格校验：必须是 UUID 格式，拒绝路径式、超长或非法内容。
    const trimmed = raw.trim();
    if (
      trimmed.length > 64 ||
      !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        trimmed
      )
    ) {
      window.sessionStorage.removeItem(ACTIVE_PREVIEW_SESSION_KEY);
      return null;
    }
    return trimmed;
  } catch {
    return null;
  }
}

function writeSessionPreviewId(id: string | null) {
  try {
    if (id === null) {
      window.sessionStorage.removeItem(ACTIVE_PREVIEW_SESSION_KEY);
    } else {
      window.sessionStorage.setItem(ACTIVE_PREVIEW_SESSION_KEY, id);
    }
  } catch {
    // sessionStorage 不可用不应阻断主链路。
  }
}

interface AppStore {
  sidebarCollapsed: boolean;
  setSidebarCollapsed: (v: boolean) => void;
  toggleSidebar: () => void;
  /** 当前浏览器会话中活动 preview 的 UUID，仅用于切页/刷新后恢复入口。 */
  activePreviewId: string | null;
  setActivePreviewId: (id: string | null) => void;
}

export const useAppStore = create<AppStore>((set) => ({
  sidebarCollapsed: false,
  setSidebarCollapsed: (v) => set({ sidebarCollapsed: v }),
  toggleSidebar: () =>
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  activePreviewId: readSessionPreviewId(),
  setActivePreviewId: (id) => {
    writeSessionPreviewId(id);
    set({ activePreviewId: id });
  },
}));
