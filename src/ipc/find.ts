import { invoke } from './invoke'

export type SlotKind = 'PERIOD' | 'LUNCH'

export interface ClassChoice {
  classId: number
  grade: number
  classNo: number
  name: string | null
  /** '5-가람' */
  label: string
  /** '5학년 가람반' */
  fullLabel: string
  homeroomTeacherId: number | null
  homeroomName: string | null
}

export interface SlotChoice {
  grade: number
  dayOfWeek: number
  slotType: SlotKind
  periodNo: number | null
  label: string
  startMin: number
  endMin: number
}

export interface TeacherChoice {
  id: number
  name: string
  roleLabel: string
  duty: string
}

export interface ReasonChoice {
  code: string
  label: string
}

export interface FindOptions {
  classes: ClassChoice[]
  slots: SlotChoice[]
  teachers: TeacherChoice[]
  reasons: ReasonChoice[]
  schoolDays: number[]
  blockers: string[]
}

/**
 * 이 시간을 원래 담당하는 교사.
 * 전담이 들어오는 시간이면 그 전담교사, 그 밖에는 그 반 담임이다.
 */
export interface InCharge {
  teacherId: number | null
  name: string | null
  roleCode: string | null
  roleLabel: string | null
  subjectName: string | null
  /** 담임 대신 다른 교사가 수업하는 시간인가 */
  coveredByOther: boolean
  /** 그 반 담임이 담당하는 시간인가 */
  isHomeroom: boolean
}

/** 배정 전에 꼭 확인할 알림 */
export interface SlotNotice {
  kind: 'SPECIAL_LESSON' | 'NO_HOMEROOM'
  title: string
  body: string
}

export interface TargetSlot {
  date: string
  dayOfWeek: number
  classId: number
  grade: number
  classNo: number
  classLabel: string
  classFullLabel: string
  slotType: SlotKind
  periodNo: number | null
  slotLabel: string
  startMin: number
  endMin: number
  /** 원래 이 시간을 담당하는 교사 */
  inCharge: InCharge
}

export interface SubCounts {
  today: number
  month: number
  total: number
}

export type BusyKind =
  | 'LESSON'
  | 'HOMEROOM_LESSON'
  | 'LUNCH'
  | 'RECESS'
  | 'FIXED_DUTY'
  | 'NO_SUB_BLOCK'
  | 'SUBSTITUTION'
  | 'ABSENCE'

export interface BlockView {
  kind: BusyKind
  label: string
  startMin: number
  endMin: number
}

export interface Candidate {
  /** 추천 순위 (1부터). 불가능한 후보는 0 */
  rank: number
  /** 추천 근거 — '동학년 · 오늘 0회' */
  reason: string
  /** 바로 다음 후보와 순서를 가른 기준 */
  decidedBy: string | null
  teacherId: number
  name: string
  roleCode: 'HOMEROOM' | 'SPECIAL' | 'OTHER'
  roleLabel: string
  duty: string
  eligible: boolean
  /** 개발자 확인용 코드 — 화면에 그대로 쓰지 않는다 */
  reasonCode: string
  /** 화면에 보여 주는 한국어 상태 */
  statusLabel: string
  detail: string | null
  counts: SubCounts
  homeroomGrades: number[]
  blocks: BlockView[]
}

export interface FindResult {
  slot: TargetSlot
  eligible: Candidate[]
  excluded: Candidate[]
  warnings: string[]
  /** 배정 전에 꼭 확인할 것 (전담 시간 등) */
  notice: SlotNotice | null
}

/** 조회 전에 보여 주는 '이 시간 담당' 안내 */
export interface InChargeView {
  slot: TargetSlot
  notice: SlotNotice | null
}

export interface FindQuery {
  date: string
  classId: number
  slotType: SlotKind
  periodNo?: number | null
  absentTeacherId?: number | null
}

export const findApi = {
  options: () => invoke<FindOptions>('find_options'),
  weekday: (date: string) => invoke<number>('find_weekday', { date }),
  inCharge: (query: FindQuery) => invoke<InChargeView | null>('find_in_charge', { query }),
  candidates: (query: FindQuery) => invoke<FindResult>('find_candidates', { query }),
}

export const BUSY_KIND_LABEL: Record<BusyKind, string> = {
  LESSON: '수업',
  HOMEROOM_LESSON: '담임 수업',
  LUNCH: '점심 지도',
  RECESS: '중간놀이 지도',
  FIXED_DUTY: '고정 업무',
  NO_SUB_BLOCK: '보결 배정 불가',
  SUBSTITUTION: '보결',
  ABSENCE: '부재',
}
