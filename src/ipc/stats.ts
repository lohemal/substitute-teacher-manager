import { invoke } from './invoke'
import type { SlotKind } from './find'

export type Preset = 'TODAY' | 'WEEK' | 'MONTH' | 'TERM' | 'CUSTOM'

export const PRESET_LABEL: Record<Preset, string> = {
  TODAY: '오늘',
  WEEK: '이번 주',
  MONTH: '이번 달',
  TERM: '이번 학기',
  CUSTOM: '기간 지정',
}

/** 같은 구분 안에서의 참고 표시. 평가가 아니다. */
export type Band = 'MORE' | 'TYPICAL' | 'LESS' | 'NONE'

export interface StatsQuery {
  preset?: Preset
  from?: string | null
  to?: string | null
}

export interface Summary {
  absentTeachers: number
  absenceCount: number
  required: number
  /** 필요한 시간 가운데 배정을 마친 수 (required = covered + unassigned) */
  covered: number
  /** 기간 안의 배정 건수 전체. 결근 등록 없이 배정한 건도 들어간다 */
  assigned: number
  unassigned: number
  cancelled: number
  subTeachers: number
}

export interface TeacherStat {
  teacherId: number
  name: string
  roleCode: 'HOMEROOM' | 'SPECIAL' | 'OTHER'
  roleLabel: string
  duty: string
  active: boolean
  isSubstitutable: boolean
  period: number
  today: number
  week: number
  month: number
  term: number
  total: number
  band: Band
  bandLabel: string
  lastDate: string | null
}

export interface RoleSpread {
  roleCode: string
  roleLabel: string
  people: number
  total: number
  avg: number
  max: number
  min: number
  spread: number
  maxName: string | null
  minName: string | null
  comparable: boolean
}

export interface AbsenceStat {
  teacherId: number
  name: string
  roleLabel: string
  count: number
  allDay: number
  partial: number
  reasons: string
  required: number
  assigned: number
  unassigned: number
}

export interface DayStat {
  date: string
  dayOfWeek: number
  absentTeachers: number
  absentNames: string
  required: number
  assigned: number
  unassigned: number
  cancelled: number
}

export interface OpenSlot {
  date: string
  dayOfWeek: number
  classId: number
  classLabel: string
  slotType: SlotKind
  periodNo: number | null
  slotLabel: string
  startMin: number
  endMin: number
  kindLabel: string
  absentTeacherId: number
  absentTeacherName: string
}

export interface StatsView {
  from: string
  to: string
  rangeLabel: string
  preset: Preset
  daysTruncated: boolean
  summary: Summary
  teachers: TeacherStat[]
  spreads: RoleSpread[]
  fairnessNote: string
  absences: AbsenceStat[]
  days: DayStat[]
  openSlots: OpenSlot[]
}

export const statsApi = {
  view: (query: StatsQuery) => invoke<StatsView>('stats_view', { query }),
}
