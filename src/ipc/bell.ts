import { invoke } from './invoke'

export type SlotType = 'PERIOD' | 'LUNCH' | 'OTHER'

export interface Slot {
  dayOfWeek: number
  slotType: SlotType
  periodNo: number | null
  label: string
  startMin: number
  endMin: number
}

export interface BellProfile {
  id: number
  name: string
  grades: number[]
  slots: Slot[]
}

export interface BellOverview {
  profiles: BellProfile[]
  unassignedGrades: number[]
  minGrade: number
  maxGrade: number
  schoolDays: number[]
}

export interface GenParams {
  firstStartMin: number
  lessonMinutes: number
  breakMinutes: number
  /** 0이면 점심 없음 */
  lunchAfterPeriod: number
  lunchMinutes: number
  /** 0이면 중간놀이 없음 */
  recessAfterPeriod: number
  recessMinutes: number
  recessLabel: string
  /** [요일, 교시 수] */
  periodsByDay: [number, number][]
}

/** 교시 수와 점심·중간놀이 위치는 그대로 두고 길이만 다시 계산할 때 쓰는 값 */
export interface ReflowParams {
  firstStartMin: number
  lessonMinutes: number
  breakMinutes: number
  lunchMinutes: number
  /** 중간놀이 등 그 밖의 구간 길이 */
  otherMinutes: number
}

export type BellTemplate = 'SAME' | 'TWO' | 'THREE'

export const bellApi = {
  overview: () => invoke<BellOverview>('bell_overview'),
  applyTemplate: (templateKey: BellTemplate) =>
    invoke<BellOverview>('bell_apply_template', { templateKey }),
  generate: (params: GenParams) => invoke<Slot[]>('bell_generate', { params }),
  reflow: (profileIds: number[], params: ReflowParams) =>
    invoke<BellOverview>('bell_reflow', { profileIds, params }),
  checkSlots: (slots: Slot[]) => invoke<string[]>('bell_check_slots', { slots }),
  createProfile: (name: string) => invoke<BellOverview>('bell_create_profile', { name }),
  renameProfile: (profileId: number, name: string) =>
    invoke<BellOverview>('bell_rename_profile', { profileId, name }),
  deleteProfile: (profileId: number) => invoke<BellOverview>('bell_delete_profile', { profileId }),
  saveProfile: (profileId: number, grades: number[], slots: Slot[]) =>
    invoke<BellOverview>('bell_save_profile', { profileId, grades, slots }),
  finishStep: () => invoke<string[]>('bell_finish_step'),
  readiness: () => invoke<string[]>('bell_readiness'),
}

/* ---------- 화면 표시용 ---------- */

export const DAY_LABEL = ['', '월', '화', '수', '목', '금', '토', '일']

export { minToHm, hmToMin } from '@/lib/time'
