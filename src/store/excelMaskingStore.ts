/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0.
 *
 * Full license text: http://www.apache.org/licenses/LICENSE-2.0
 */
import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { ExcelMaskingConfig } from "@/types/commands";

interface ExcelMaskingState {
  privacy: {
    excelAutoMaskDialog: boolean;
    excelDefaultRetainEncryptedSource: boolean;
  };
  setPrivacy: (p: Partial<ExcelMaskingState["privacy"]>) => void;
  lastConfig: ExcelMaskingConfig | null;
  setLastConfig: (c: ExcelMaskingConfig | null) => void;
  templates: { name: string; path: string }[];
  addTemplate: (t: { name: string; path: string }) => void;
}

export const useExcelMaskingStore = create<ExcelMaskingState>()(
  persist(
    (set) => ({
      privacy: {
        excelAutoMaskDialog: true,
        excelDefaultRetainEncryptedSource: false,
      },
      setPrivacy: (p) =>
        set((state) => ({
          privacy: { ...state.privacy, ...p },
        })),
      lastConfig: null,
      setLastConfig: (c) => set({ lastConfig: c }),
      templates: [],
      addTemplate: (t) =>
        set((state) => ({
          templates: [...state.templates.filter((e) => e.path !== t.path), t],
        })),
    }),
    {
      name: "excel-masking-store",
      partialize: (state) => ({
        privacy: state.privacy,
        lastConfig: state.lastConfig,
      }),
    }
  )
);
