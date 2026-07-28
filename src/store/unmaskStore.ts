import { create } from "zustand";

interface UnmaskStore {
  maskedFile: string;
  mappingFile: string;
  setMaskedFile: (path: string) => void;
  setMappingFile: (path: string) => void;
}

export const useUnmaskStore = create<UnmaskStore>((set) => ({
  maskedFile: "",
  mappingFile: "",
  setMaskedFile: (maskedFile) => set({ maskedFile }),
  setMappingFile: (mappingFile) => set({ mappingFile }),
}));
