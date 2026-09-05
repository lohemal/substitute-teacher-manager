import { invoke } from './invoke'

export type RoleCode = 'HOMEROOM' | 'SPECIAL' | 'OTHER'

export interface ClassRef {
  classId: number
  grade: number
  classNo: number
  /** 반 이름. 숫자로 부르는 학교면 null */
  name: string | null
  /** '5-가람' 또는 '5-1' */
  label: string
}

export interface SubjectRef {
  id: number
  name: string
}

export interface RoleRef {
  code: RoleCode
  label: string
  sortOrder: number
}

export interface TeacherView {
  id: number
  name: string
  roleCode: RoleCode
  roleLabel: string
  isSubstitutable: boolean
  memo: string | null
  active: boolean
  homerooms: ClassRef[]
  subjects: SubjectRef[]
  lessonCount: number
  subCountTotal: number
  /** 시간표·보결 기록에 쓰이고 있어 완전 삭제가 불가능 */
  hasRecords: boolean
}

export interface ClassSlot {
  classId: number
  grade: number
  classNo: number
  name: string | null
  /** '5-가람' 또는 '5-1' */
  label: string
  /** '5학년 가람반' 또는 '5학년 1반' */
  fullLabel: string
  homeroomTeacherId: number | null
  homeroomName: string | null
  homeroomActive: boolean | null
}

export interface TeacherCounts {
  total: number
  homeroom: number
  special: number
  other: number
  substitutable: number
  inactive: number
}

export interface TeacherList {
  teachers: TeacherView[]
  roles: RoleRef[]
  subjects: SubjectRef[]
  classes: ClassSlot[]
  counts: TeacherCounts
}

export interface SetActiveResult {
  list: TeacherList
  unassignedClasses: string[]
}

export interface TeacherInput {
  id: number | null
  name: string
  roleCode: RoleCode
  isSubstitutable: boolean
  memo: string | null
  homeroomClassIds: number[]
  subjectIds: number[]
  replaceExistingHomeroom: boolean
}

export interface BulkTeacherRow {
  name: string
  roleCode?: RoleCode | null
  memo?: string | null
  grade?: number | null
  classNo?: number | null
  /** 반 이름으로 부르는 학교용 */
  className?: string | null
  subjectNames?: string[]
}

export interface BulkResult {
  created: number
  skipped: number
  problems: string[]
  list: TeacherList
}

export const ROLE_HINT: Record<RoleCode, string> = {
  HOMEROOM: '학급을 맡은 선생님',
  SPECIAL: '과목을 맡은 선생님',
  OTHER: '교장·교감·보건·상담·사서 등',
}

export const teacherApi = {
  list: () => invoke<TeacherList>('teacher_list'),
  upsert: (input: TeacherInput) => invoke<TeacherList>('teacher_upsert', { input }),
  setActive: (teacherId: number, active: boolean) =>
    invoke<SetActiveResult>('teacher_set_active', { teacherId, active }),
  remove: (teacherId: number) => invoke<TeacherList>('teacher_delete', { teacherId }),
  setSubstitutableBulk: (teacherIds: number[], value: boolean) =>
    invoke<TeacherList>('teacher_set_substitutable_bulk', { teacherIds, value }),
  setActiveBulk: (teacherIds: number[], active: boolean) =>
    invoke<SetActiveResult>('teacher_set_active_bulk', { teacherIds, active }),
  quickAddHomerooms: (entries: { classId: number; name: string }[]) =>
    invoke<BulkResult>('teacher_quick_add_homerooms', { entries }),
  bulkCreate: (rows: BulkTeacherRow[]) => invoke<BulkResult>('teacher_bulk_create', { rows }),
  upsertSubject: (name: string) => invoke<TeacherList>('subject_upsert', { name }),
  readiness: () => invoke<string[]>('teacher_readiness'),
  finishStep: () => invoke<string[]>('teacher_finish_step'),
}
