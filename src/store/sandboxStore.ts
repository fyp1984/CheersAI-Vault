import { create } from "zustand";
import type { SandboxFile } from "@/types/sandbox";

interface SandboxStore {
  locked: boolean;
  files: SandboxFile[];
  setLocked: (locked: boolean) => void;
  setFiles: (files: SandboxFile[]) => void;
  addFile: (file: SandboxFile) => void;
  removeFile: (name: string) => void;
}

export const useSandboxStore = create<SandboxStore>((set) => ({
  locked: true,
  files: [],

  setLocked: (locked) => set({ locked }),
  setFiles: (files) => set({ files }),
  addFile: (file) =>
    set((state) => ({ files: [...state.files, file] })),
  removeFile: (name) =>
    set((state) => ({ files: state.files.filter((f) => f.name !== name) })),
}));
