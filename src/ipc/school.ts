import { invoke } from './invoke'

// 학교급은 순수 모듈 한 곳에 있다. 부르는 쪽이 바뀌지 않도록 다시 내보낸다.
export {
  SCHOOL_TYPE_LABEL,
  DEFAULT_GRADE_RANGE,
  CURRENT_SCHOOL_TYPE,
  isCurrentSchoolType,
} from '@/lib/schoolType'
export type { SchoolType } from '@/lib/schoolType'

import type { SchoolType } from '@/lib/schoolType'

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

export const schoolApi = {
  get: () => invoke<SchoolView | null>('school_get'),
  save: (input: SchoolInput) => invoke<SchoolView>('school_save', { input }),
}
