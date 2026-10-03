import type { CategoryFilter } from '$lib/types';
export type ImportOptions = { history: boolean; profiles: boolean; preferences: boolean };
export type Counts = { added: number; existing: number; conflicts: number };
export type ImportSummary = {
  sessions: Counts;
  categories: Counts;
  profiles: Counts;
  preferences: boolean;
};
export type ImportPreview = {
  token: string;
  filename: string;
  exported_at: string;
  app_version: string;
  source_timezone: string | null;
  start: number | null;
  end: number | null;
  has_profiles: boolean;
  has_preferences: boolean;
  duplicates: number;
  options: ImportOptions;
  summary: ImportSummary;
  conflicts: number;
  renames: number;
  digest: string;
};
export type Conflict = {
  kind: string;
  label: string;
  differences: { field: string; local: string; file: string }[];
};
export type Rename = { kind: string; from: string; to: string };
export type Saved = {
  filename: string;
  rows: number;
  categories: number;
  profiles: number;
  warning: boolean;
};
export type CommitResult = {
  committed: boolean;
  refresh_failed: boolean;
  preview: ImportPreview | null;
  summary: ImportSummary;
};
export type ReportScope = {
  kind: 'daily' | 'sessions';
  start: string | null;
  end: string | null;
  filter: CategoryFilter;
  hour: number | null;
};
export type ReportInfo = {
  scope: ReportScope;
  rows: number;
  focus_secs: number;
  filename: string;
  timezone: string | null;
};
export type DistributionRow = {
  category_id: number | null;
  name: string | null;
  archived: boolean;
  completed: number;
  focus_secs: number;
};
export type Distribution = { rows: DistributionRow[]; total_focus_secs: number };
