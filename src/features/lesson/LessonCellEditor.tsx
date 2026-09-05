import { useEffect, useMemo, useState } from 'react'

import { Modal } from '@/components/Modal'
import { Button, Notice } from '@/components/ui'
import { DAY_LABEL, minToHm } from '@/ipc/bell'
import {
  LESSON_TYPE_HINT,
  LESSON_TYPE_LABEL,
  type ClassOption,
  type GradeSlot,
  type LessonType,
  type LessonView,
  type SubjectOption,
} from '@/ipc/lesson'
import s from './LessonCellEditor.module.css'

export interface CellTarget {
  dayOfWeek: number
  periodNo: number
  /** 이미 있는 수업을 고치는 경우 */
  lesson?: LessonView
}

export interface CellValue {
  classId: number
  subjectId: number | null
  lessonType: LessonType
}

interface Props {
  open: boolean
  target: CellTarget | null
  classes: ClassOption[]
  subjects: SubjectOption[]
  gradeSlots: GradeSlot[]
  /** 이 교사가 담당하는 과목 (먼저 보여 준다) */
  teacherSubjectIds: number[]
  /** 직전에 넣은 값 — 연속 입력을 빠르게 하기 위해 미리 골라 둔다 */
  lastUsed: CellValue | null
  saving: boolean
  error: string | null
  onClose: () => void
  onSave: (v: CellValue, andNext: boolean) => void
  onDelete: (lessonId: number) => void
}

export function LessonCellEditor({
  open,
  target,
  classes,
  subjects,
  gradeSlots,
  teacherSubjectIds,
  lastUsed,
  saving,
  error,
  onClose,
  onSave,
  onDelete,
}: Props) {
  const [classId, setClassId] = useState<number | null>(null)
  const [subjectId, setSubjectId] = useState<number | null>(null)
  const [lessonType, setLessonType] = useState<LessonType>('SPECIAL')
  const [showAllTypes, setShowAllTypes] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)

  useEffect(() => {
    if (!open || !target) return
    if (target.lesson) {
      setClassId(target.lesson.classId)
      setSubjectId(target.lesson.subjectId)
      setLessonType(target.lesson.lessonType)
    } else {
      // 새로 넣을 때는 직전 입력을 미리 골라 둔다
      setClassId(lastUsed?.classId ?? null)
      setSubjectId(lastUsed?.subjectId ?? teacherSubjectIds[0] ?? null)
      setLessonType(lastUsed?.lessonType ?? 'SPECIAL')
    }
    setShowAllTypes(false)
    setConfirmDelete(false)
  }, [open, target, lastUsed, teacherSubjectIds])

  /** 이 요일·교시에 수업이 가능한 학급만 남긴다 (학년마다 교시 수가 다르므로) */
  const available = useMemo(() => {
    if (!target) return []
    const ok = new Set(
      gradeSlots
        .filter((g) => g.dayOfWeek === target.dayOfWeek && g.periodNo === target.periodNo)
        .map((g) => g.grade),
    )
    return classes.filter((c) => ok.has(c.grade))
  }, [classes, gradeSlots, target])

  const byGrade = useMemo(() => {
    const m = new Map<number, ClassOption[]>()
    for (const c of available) {
      if (!m.has(c.grade)) m.set(c.grade, [])
      m.get(c.grade)!.push(c)
    }
    return [...m.entries()]
  }, [available])

  const chosen = available.find((c) => c.classId === classId) ?? null
  const time = target
    ? gradeSlots.find(
        (g) =>
          g.grade === chosen?.grade &&
          g.dayOfWeek === target.dayOfWeek &&
          g.periodNo === target.periodNo,
      )
    : undefined

  // 이 교사 담당 과목을 먼저, 나머지는 뒤에
  const mine = subjects.filter((x) => teacherSubjectIds.includes(x.id))
  const others = subjects.filter((x) => !teacherSubjectIds.includes(x.id))

  if (!target) return null

  const title = `${DAY_LABEL[target.dayOfWeek]}요일 ${target.periodNo}교시`

  return (
    <Modal
      open={open}
      title={target.lesson ? `${title} 수업 고치기` : `${title} 수업 넣기`}
      description={
        available.length === 0
          ? undefined
          : '학급을 고르면 그 학년 시정표의 실제 시각으로 저장됩니다.'
      }
      onClose={onClose}
      footer={
        <>
          <div className={s.footLeft}>
            {target.lesson && !confirmDelete && (
              <button
                type="button"
                className={s.deleteLink}
                onClick={() => setConfirmDelete(true)}
              >
                이 수업 지우기
              </button>
            )}
            {target.lesson && confirmDelete && (
              <span className={s.confirmRow}>
                <span className={s.confirmText}>지울까요?</span>
                <button
                  type="button"
                  className={s.confirmYes}
                  onClick={() => onDelete(target.lesson!.id)}
                  disabled={saving}
                >
                  지우기
                </button>
                <button
                  type="button"
                  className={s.subtleBtn}
                  onClick={() => setConfirmDelete(false)}
                >
                  취소
                </button>
              </span>
            )}
          </div>
          <div className={s.footRight}>
            <Button variant="ghost" onClick={onClose} disabled={saving}>
              취소
            </Button>
            {!target.lesson && (
              <Button
                variant="ghost"
                disabled={classId == null || saving}
                onClick={() => onSave({ classId: classId!, subjectId, lessonType }, true)}
                title="저장하고 같은 요일의 다음 교시를 바로 엽니다"
              >
                저장하고 다음 칸 →
              </Button>
            )}
            <Button
              variant="primary"
              disabled={classId == null || saving}
              onClick={() => onSave({ classId: classId!, subjectId, lessonType }, false)}
            >
              {saving ? '저장 중…' : '저장'}
            </Button>
          </div>
        </>
      }
    >
      {error && <Notice tone="danger">{error}</Notice>}

      {available.length === 0 ? (
        <Notice tone="warn">
          {DAY_LABEL[target.dayOfWeek]}요일 {target.periodNo}교시에 수업이 있는 학년이 없습니다.
          시정표에서 그 요일의 교시 수를 확인해 주세요.
        </Notice>
      ) : (
        <>
          <div className={s.field}>
            <span className={s.label}>학급</span>
            <div className={s.classGroups}>
              {byGrade.map(([grade, list]) => (
                <div key={grade} className={s.classGroup}>
                  <span className={s.gradeName}>{grade}학년</span>
                  <div className={s.classChips}>
                    {list.map((c) => (
                      <button
                        key={c.classId}
                        type="button"
                        className={
                          c.classId === classId ? `${s.classChip} ${s.classChipOn}` : s.classChip
                        }
                        onClick={() => setClassId(c.classId)}
                        title={c.homeroomName ? `담임 ${c.homeroomName}` : undefined}
                      >
                        {c.name ?? `${c.classNo}반`}
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </div>
            {time && (
              <p className={s.timeHint}>
                실제 시각 <strong>{minToHm(time.startMin)}~{minToHm(time.endMin)}</strong>
              </p>
            )}
          </div>

          <div className={s.field}>
            <span className={s.label}>과목</span>
            <div className={s.subjectChips}>
              {mine.map((x) => (
                <button
                  key={x.id}
                  type="button"
                  className={
                    x.id === subjectId ? `${s.subjectChip} ${s.subjectChipOn}` : s.subjectChip
                  }
                  onClick={() => setSubjectId(x.id === subjectId ? null : x.id)}
                >
                  {x.name}
                </button>
              ))}
              {mine.length > 0 && others.length > 0 && <span className={s.divider} />}
              {others.map((x) => (
                <button
                  key={x.id}
                  type="button"
                  className={
                    x.id === subjectId
                      ? `${s.subjectChip} ${s.subjectChipOn}`
                      : `${s.subjectChip} ${s.subjectChipOther}`
                  }
                  onClick={() => setSubjectId(x.id === subjectId ? null : x.id)}
                >
                  {x.name}
                </button>
              ))}
            </div>
          </div>

          {showAllTypes ? (
            <div className={s.field}>
              <span className={s.label}>수업 구분</span>
              <div className={s.typeList}>
                {(Object.keys(LESSON_TYPE_LABEL) as LessonType[]).map((t) => (
                  <button
                    key={t}
                    type="button"
                    className={t === lessonType ? `${s.typeRow} ${s.typeRowOn}` : s.typeRow}
                    onClick={() => setLessonType(t)}
                  >
                    <span className={s.typeName}>{LESSON_TYPE_LABEL[t]}</span>
                    <span className={s.typeHint}>{LESSON_TYPE_HINT[t]}</span>
                  </button>
                ))}
              </div>
            </div>
          ) : (
            <button type="button" className={s.moreLink} onClick={() => setShowAllTypes(true)}>
              수업 구분 바꾸기 (지금 {LESSON_TYPE_LABEL[lessonType]})
            </button>
          )}
        </>
      )}
    </Modal>
  )
}
