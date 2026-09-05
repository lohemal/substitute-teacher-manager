import { invoke } from './invoke'

export type LessonType = 'SPECIAL' | 'CROSS' | 'CO' | 'ETC'

export const LESSON_TYPE_LABEL: Record<LessonType, string> = {
  SPECIAL: '전담 수업',
  CROSS: '교차 수업',
  CO: '공동 수업',
  ETC: '기타',
}

export const LESSON_TYPE_HINT: Record<LessonType, string> = {
  SPECIAL: '전담교사가 학급에 들어가는 보통의 수업. 그 시간 담임은 공강이 됩니다.',
  CROSS: '담임이 다른 반에 들어가는 수업. 그 반 담임은 공강이 됩니다.',
  CO: '담임과 함께 들어가는 수업. 담임은 공강이 되지 않습니다.',
  ETC: '그 밖의 수업.',
}

export interface LessonView {
  id: number
  dayOfWeek: number
  periodNo: number
  classId: number
  grade: number
  classNo: number
  /** '5-가람' */
  classLabel: string
  subjectId: number | null
  subjectName: string | null
  lessonType: LessonType
  replacesHomeroom: boolean
  note: string | null
  /** 학년별 시정표에서 계산한 실제 시각 */
  startMin: number | null
  endMin: number | null
}

export interface TeacherLessons {
  teacherId: number
  teacherName: string
  lessons: LessonView[]
  problems: string[]
}

export interface LessonTeacher {
  id: number
  name: string
  roleCode: 'HOMEROOM' | 'SPECIAL' | 'OTHER'
  roleLabel: string
  memo: string | null
  subjects: string[]
  subjectIds: number[]
  lessonCount: number
  homerooms: string[]
}

export interface ClassOption {
  classId: number
  grade: number
  classNo: number
  name: string | null
  label: string
  fullLabel: string
  homeroomName: string | null
}

export interface SubjectOption {
  id: number
  name: string
}

export interface GradeSlot {
  grade: number
  dayOfWeek: number
  periodNo: number
  startMin: number
  endMin: number
}

export interface LessonOverview {
  teachers: LessonTeacher[]
  classes: ClassOption[]
  subjects: SubjectOption[]
  schoolDays: number[]
  maxPeriod: number
  gradeSlots: GradeSlot[]
  bellMissing: boolean
}

export interface LessonInput {
  id: number | null
  teacherId: number
  dayOfWeek: number
  periodNo: number
  classId: number
  subjectId: number | null
  lessonType: LessonType
  note: string | null
}

export interface LessonRow {
  dayOfWeek: number
  periodNo: number
  classId: number
  subjectId: number | null
  lessonType: LessonType
  note?: string | null
}

export interface HomeroomFreeSlot {
  classId: number
  classLabel: string
  homeroomName: string | null
  dayOfWeek: number
  periodNo: number
  startMin: number
  endMin: number
  coveredBy: string
  subjectName: string | null
}

export const lessonApi = {
  overview: () => invoke<LessonOverview>('lesson_overview'),
  get: (teacherId: number) => invoke<TeacherLessons>('lesson_get', { teacherId }),
  upsert: (input: LessonInput) => invoke<TeacherLessons>('lesson_upsert', { input }),
  remove: (lessonId: number) => invoke<TeacherLessons>('lesson_delete', { lessonId }),
  check: (rows: LessonRow[]) => invoke<string[]>('lesson_check', { rows }),
  bulkReplace: (teacherId: number, rows: LessonRow[]) =>
    invoke<TeacherLessons>('lesson_bulk_replace', { teacherId, rows }),
  homeroomFree: () => invoke<HomeroomFreeSlot[]>('lesson_homeroom_free'),
  readiness: () => invoke<string[]>('lesson_readiness'),
  warnings: () => invoke<string[]>('lesson_warnings'),
  finishStep: () => invoke<string[]>('lesson_finish_step'),
}
