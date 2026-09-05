import { invoke } from './invoke'
import type { SlotKind, SlotNotice, SubCounts } from './find'

// ---------- 결근 ----------

export interface AbsenceInput {
  teacherId: number
  /** YYYY-MM-DD */
  date: string
  isAllDay: boolean
  startMin?: number | null
  endMin?: number | null
  reasonCode?: string | null
  reasonText?: string | null
}

export interface AbsenceRow {
  id: number
  date: string
  teacherId: number
  teacherName: string
  roleLabel: string
  isAllDay: boolean
  startMin: number | null
  endMin: number | null
  /** '연가' 등 화면에 그대로 쓰는 문구 */
  reasonLabel: string
  reasonCode: string | null
  reasonText: string | null
  status: 'ACTIVE' | 'CANCELLED'
  createdAt: string
  cancelledAt: string | null
  /** 이 결근으로 배정된 활성 보결 건수 */
  subCount: number
}

// ---------- 배정 ----------

export interface AssignInput {
  date: string
  classId: number
  slotType: SlotKind
  periodNo?: number | null
  absentTeacherId?: number | null
  subTeacherId: number
  absenceId?: number | null
  reasonCode?: string | null
  reasonText?: string | null
}

export interface AssignSaved {
  id: number
  date: string
  classLabel: string
  classFullLabel: string
  slotLabel: string
  startMin: number
  endMin: number
  subTeacherName: string
  absentTeacherName: string | null
  /** 저장은 되었지만 확인하면 좋은 내용 (전담 시간 등) */
  notice: SlotNotice | null
}

// ---------- 하루 일괄 보결 ----------

export type DutyKind = 'LESSON' | 'HOMEROOM_LESSON' | 'LUNCH' | 'RECESS'

export interface PlanCandidate {
  teacherId: number
  name: string
  roleLabel: string
  duty: string
  rank: number
  reason: string
  counts: SubCounts
}

export interface PlanSlot {
  classId: number
  classLabel: string
  slotType: SlotKind
  periodNo: number | null
  slotLabel: string
  startMin: number
  endMin: number
  kind: DutyKind
  /** '정규 수업' 등 */
  kindLabel: string
  subjectName: string | null
  candidates: PlanCandidate[]
  existingSubId: number | null
  existingSubName: string | null
  notice: SlotNotice | null
}

export interface DayPlan {
  date: string
  dayOfWeek: number
  teacherId: number
  teacherName: string
  absenceId: number | null
  absenceLabel: string | null
  slots: PlanSlot[]
  warnings: string[]
}

export interface BatchPick {
  classId: number
  slotType: SlotKind
  periodNo?: number | null
  subTeacherId: number
}

export interface BatchInput {
  date: string
  absentTeacherId: number
  absenceId?: number | null
  picks: BatchPick[]
}

// ---------- 배정 내역 ----------

export type SubStatus = 'ASSIGNED' | 'CANCELLED'

export interface HistoryFilter {
  from?: string | null
  to?: string | null
  keyword?: string | null
  status?: SubStatus | 'ALL' | null
  limit?: number | null
}

export interface HistoryRow {
  id: number
  date: string
  dayOfWeek: number
  /** 기록 당시의 표기. 반 이름이 바뀌어도 그대로다 */
  classLabel: string
  grade: number
  slotType: SlotKind
  slotLabel: string
  startMin: number
  endMin: number
  absentTeacherId: number | null
  absentTeacherName: string | null
  subTeacherId: number
  subTeacherName: string
  subjectName: string | null
  reasonLabel: string | null
  status: SubStatus
  recommendRank: number | null
  recommendReason: string | null
  createdAt: string
  cancelledAt: string | null
  cancelReason: string | null
}

export interface HistoryView {
  rows: HistoryRow[]
  assignedCount: number
  cancelledCount: number
  truncated: boolean
}

export const assignApi = {
  // 결근
  absenceCreate: (input: AbsenceInput) => invoke<number>('absence_create', { input }),
  absenceCancel: (id: number) => invoke<number>('absence_cancel', { id }),
  absenceList: (date: string, all = false) =>
    invoke<AbsenceRow[]>('absence_list', { date, all }),

  // 배정
  create: (input: AssignInput) => invoke<AssignSaved>('assign_create', { input }),
  dayPlan: (date: string, teacherId: number) =>
    invoke<DayPlan>('assign_day_plan', { date, teacherId }),
  batch: (input: BatchInput) => invoke<{ saved: AssignSaved[] }>('assign_batch', { input }),
  cancel: (id: number, reason?: string | null) =>
    invoke<void>('assign_cancel', { id, reason: reason ?? null }),
  history: (filter?: HistoryFilter) =>
    invoke<HistoryView>('assign_history', { filter: filter ?? null }),
}

/** 취소 사유 고르기 — 직접 입력도 가능하다 */
export const CANCEL_REASONS = [
  '결근이 취소되어 담임이 출근함',
  '다른 선생님으로 변경',
  '수업이 없어짐 (행사·단축수업 등)',
  '잘못 배정함',
] as const
