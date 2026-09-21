import { invoke } from './invoke'

/**
 * 전담교사 식사시간.
 *
 * 학급의 '점심'이나 업무 개념인 '점심 보결'과 다른 말이다. 여기서 다루는
 * 것은 전담 선생님이 **밥 먹는 시간**이고, 그 시간에는 일반 수업 보결을
 * 맡기지 않는다. 점심 보결은 그대로 맡을 수 있다.
 */

/** 무엇으로 정해졌는가 */
export type MealSource = 'MANUAL' | 'AUTO' | 'DEFAULT' | 'UNKNOWN'

export interface DayWindow {
  dayOfWeek: number
  startMin: number
  endMin: number
}

/** 학교에 실제로 있는 점심시간 하나 */
export interface MealPattern {
  /** 학교 기본 식사시간으로 저장할 값 */
  bellScheduleId: number
  bellName: string
  grades: number[]
  /** '1·2·3학년' */
  gradeLabel: string
  byDay: DayWindow[]
  /** '12:10~13:10' 또는 '요일마다 다름' */
  timeLabel: string
}

export interface MealDay {
  dayOfWeek: number
  startMin: number | null
  endMin: number | null
  source: MealSource
  /** '자동' | '직접 지정' | '학교 기본' | '확인 필요' */
  sourceLabel: string
  /** '13:10~14:00' 또는 빈 문자열 */
  timeLabel: string
  /** 정하지 못했을 때 그 까닭 */
  note: string | null
  /** 그 날 이 교사의 수업 시간 */
  lessons: DayWindow[]
}

export interface MealView {
  patterns: MealPattern[]
  defaultBellId: number | null
  /** 기본 식사시간을 정해야 판단할 수 있는 상태인가 */
  needsDefault: boolean
  schoolDays: number[]
  teacherId: number | null
  teacherName: string | null
  isSpecial: boolean
  days: MealDay[]
}

export interface MealOverrideInput {
  teacherId: number
  dayOfWeek: number
  /** 비우면 수동 지정을 없애고 자동 판정으로 되돌린다 */
  startMin?: number | null
  endMin?: number | null
}

export const mealApi = {
  view: (teacherId?: number | null) =>
    invoke<MealView>('meal_view', { teacherId: teacherId ?? null }),
  setDefault: (bellScheduleId: number | null, teacherId?: number | null) =>
    invoke<MealView>('meal_set_default', {
      bellScheduleId,
      teacherId: teacherId ?? null,
    }),
  setOverride: (input: MealOverrideInput) => invoke<MealView>('meal_set_override', { input }),
}
