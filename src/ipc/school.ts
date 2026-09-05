import { invoke } from './invoke'

export type SchoolType = 'ELEMENTARY' | 'MIDDLE' | 'HIGH'

export interface ClassCount {
  grade: number
  count: number
}

export interface GradeNames {
  grade: number
  names: string[]
}

/** 반을 숫자로 부를지 이름으로 부를지 */
export interface ClassNaming {
  mode: 'NUMBER' | 'NAME'
  /** 학년마다 다른 이름을 쓰는가 */
  perGrade: boolean
  /** perGrade=false 일 때 모든 학년이 함께 쓰는 이름 */
  sharedNames: string[]
  /** perGrade=true 일 때 학년별 이름 */
  gradeNames: GradeNames[]
}

export const DEFAULT_NAMING: ClassNaming = {
  mode: 'NUMBER',
  perGrade: false,
  sharedNames: [],
  gradeNames: [],
}

export interface SchoolInput {
  name: string
  schoolType: SchoolType
  minGrade: number
  maxGrade: number
  /** 1=월 … 7=일 */
  schoolDays: number[]
  schoolYear: number
  semester: number
  classCounts: ClassCount[]
  naming: ClassNaming
}

export interface SchoolView extends SchoolInput {
  termId: number
  termName: string
}

export const SCHOOL_TYPE_LABEL: Record<SchoolType, string> = {
  ELEMENTARY: '초등학교',
  MIDDLE: '중학교',
  HIGH: '고등학교',
}

/** 학교 구분별 기본 학년 범위 */
export const DEFAULT_GRADE_RANGE: Record<SchoolType, [number, number]> = {
  ELEMENTARY: [1, 6],
  MIDDLE: [1, 3],
  HIGH: [1, 3],
}

export const schoolApi = {
  get: () => invoke<SchoolView | null>('school_get'),
  save: (input: SchoolInput) => invoke<SchoolView>('school_save', { input }),
}
