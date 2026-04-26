export interface SandboxFile {
  name: string;
  path: string;
  size: number;
  modified: string;
}

export interface SandboxState {
  locked: boolean;
  files: SandboxFile[];
}
