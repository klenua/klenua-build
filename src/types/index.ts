export interface VersionTarget {
  id: string;
  platform: string;
  file: string;
  version?: string;
  build?: string;
  editable: boolean;
  sourceOfTruth: boolean;
  note?: string;
}

export interface GitInfo { branch?: string; status?: string; }
export interface ProjectScan {
  root: string; projectName: string; projectType: string; platforms: string[];
  version?: string; build?: string; sourceOfTruth?: string; targets: VersionTarget[];
  warnings: string[]; git?: GitInfo; appName?: string; bundleId?: string;
}
export interface LineChange { before: string; after: string; }
export interface FileChange { file: string; changes: LineChange[]; }
export interface Preview { changes: FileChange[]; warnings: string[]; }
export interface RecentProject { path: string; name: string; }
export interface AppSettings { saveProjects: boolean; autoUpdate: boolean; autoSyncAfterApply: boolean; uploadBackupFiles: boolean; }
export interface KlenuaAccount { name: string; email: string; }
export interface KlenuaSyncStatus { syncedAt: string; }
export interface CheckFinding {
  severity: "error" | "warning" | "info";
  rule: string;
  file: string;
  message: string;
  fixHint: string;
}
export interface CheckReport { findings: CheckFinding[]; hasErrors: boolean; }
export interface ManualAttention { file: string; reason: string; }
export interface RenamePreview {
  preview: Preview;
  warning?: string;
  manualAttention: ManualAttention[];
  outOfScope: string[];
}
