import { invoke } from './invoke'
import type { HistoryFilter } from './assign'
import type { StatsQuery } from './stats'
import type { PayPolicy, PayQuery } from './pay'

// ---------- 백업 ----------

export type BackupKind = 'MANUAL' | 'AUTO' | 'SAFETY' | 'UPGRADE'

export interface BackupFile {
  name: string
  path: string
  kind: BackupKind
  kindLabel: string
  madeAt: string
  sizeKb: number
  schemaVersion: number | null
  restorable: boolean
  problem: string | null
}

export interface BackupResult {
  name: string
  path: string
  folder: string
}

export interface InspectResult {
  ok: boolean
  schemaVersion: number | null
  problem: string | null
}

export interface RestoreResult {
  safetyBackup: string
  schemaVersion: number
}

// ---------- 동작 옵션 ----------

export type BackupMode = 'OFF' | 'DAILY' | 'ON_EXIT'

export const BACKUP_MODE_LABEL: Record<BackupMode, string> = {
  OFF: '사용 안 함',
  DAILY: '하루 1회',
  ON_EXIT: '앱 종료 시',
}

export interface OptionRow {
  key: string
  label: string
  hint: string
  value: boolean
  isDefault: boolean
}

/** 고를 수 있는 보결 수당 지급 기준. 정의는 Rust 한 곳에만 있다 */
export interface PolicyRow {
  code: PayPolicy
  label: string
  hint: string
}

export interface SettingsView {
  engine: OptionRow[]
  autoBackupMode: BackupMode
  autoBackupKeep: number
  changed: boolean
  dbPath: string
  backupDir: string
  exportDir: string
  fixedNote: string
  /** 1회 보결 수당(원) */
  subPayPerCase: number
  subPayPolicy: PayPolicy
  subPayPolicies: PolicyRow[]
}

export interface SettingsInput {
  engine?: { key: string; value: boolean }[]
  autoBackupMode?: BackupMode | null
  autoBackupKeep?: number | null
  subPayPerCase?: number | null
  subPayPolicy?: PayPolicy | null
}

// ---------- 학기 ----------

export interface TermRow {
  id: number
  schoolYear: number
  semester: number
  name: string
  startDate: string | null
  endDate: string | null
  effectiveFrom: string
  effectiveTo: string
  explicit: boolean
  isCurrent: boolean
  classCount: number
  lessonCount: number
  subCount: number
}

export interface TermDates {
  id: number
  name?: string | null
  startDate?: string | null
  endDate?: string | null
}

export interface NewTermInput {
  schoolYear: number
  semester: number
  name?: string | null
  startDate?: string | null
  endDate?: string | null
  copyClasses: boolean
  copyHomerooms: boolean
  copyBells: boolean
  copyLessons: boolean
  setCurrent: boolean
}

export interface NewTermResult {
  termId: number
  name: string
  copiedClasses: number
  copiedHomerooms: number
  copiedBells: number
  copiedLessons: number
  nextSteps: string[]
}

// ---------- 내보내기 · 초기화 ----------

export interface ExportResult {
  name: string
  path: string
  folder: string
  rows: number
}

export interface DataResetResult {
  backup: string
  folder: string
}

/** 자료 전체 초기화에서 사용자가 그대로 입력해야 하는 문구 */
export const RESET_PHRASE = '전체 초기화'

export const adminApi = {
  // 백업
  backupCreate: () => invoke<BackupResult>('backup_create'),
  backupList: () => invoke<BackupFile[]>('backup_list'),
  backupDelete: (name: string) => invoke<void>('backup_delete', { name }),
  backupInspect: (path: string) => invoke<InspectResult>('backup_inspect', { path }),
  backupRestore: (src: { name?: string; path?: string }) =>
    invoke<RestoreResult>('backup_restore', {
      name: src.name ?? null,
      path: src.path ?? null,
    }),
  backupAutoNow: () => invoke<string | null>('backup_auto_now'),
  openBackupFolder: () => invoke<void>('backup_open_folder'),
  openExportFolder: () => invoke<void>('export_open_folder'),
  openDataFolder: () => invoke<void>('data_open_folder'),

  // 동작 옵션
  settings: () => invoke<SettingsView>('settings_view'),
  saveSettings: (input: SettingsInput) => invoke<void>('settings_save', { input }),
  resetSettings: () => invoke<void>('settings_reset'),

  // 학기
  termList: () => invoke<TermRow[]>('term_list'),
  termSaveDates: (input: TermDates) => invoke<void>('term_save_dates', { input }),
  termSetCurrent: (id: number) => invoke<void>('term_set_current', { id }),
  termStartNew: (input: NewTermInput) => invoke<NewTermResult>('term_start_new', { input }),

  // 내보내기 · 초기화
  exportHistory: (filter?: HistoryFilter) =>
    invoke<ExportResult>('export_history_xlsx', { filter: filter ?? null }),
  exportStats: (query?: StatsQuery) =>
    invoke<ExportResult>('export_stats_xlsx', { query: query ?? null }),
  exportPay: (query?: PayQuery) =>
    invoke<ExportResult>('export_pay_xlsx', { query: query ?? null }),
  dataReset: (confirm: string) => invoke<DataResetResult>('data_reset', { confirm }),
}
