import { useEffect, useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Icon } from '@/components/Icon'
import { Button, Notice } from '@/components/ui'
import { DAY_LABEL, minToHm } from '@/ipc/bell'
import { errorMessage } from '@/ipc/invoke'
import {
  lessonApi,
  type LessonOverview,
  type LessonType,
  type LessonView,
  type TeacherLessons,
} from '@/ipc/lesson'
import { LessonCellEditor, type CellTarget, type CellValue } from './LessonCellEditor'
import { MealCard } from './MealCard'
import { PasteGridModal } from './PasteGridModal'
import s from './LessonEditor.module.css'

export function LessonEditor() {
  const qc = useQueryClient()
  const { data: ov, isLoading, error } = useQuery({
    queryKey: ['lesson-overview'],
    queryFn: lessonApi.overview,
  })

  const [teacherId, setTeacherId] = useState<number | null>(null)
  const [target, setTarget] = useState<CellTarget | null>(null)
  const [editorOpen, setEditorOpen] = useState(false)
  const [pasteOpen, setPasteOpen] = useState(false)
  const [lastUsed, setLastUsed] = useState<CellValue | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)

  // 처음 열면 첫 전담교사를 고른다
  useEffect(() => {
    if (!ov || teacherId != null) return
    const first = ov.teachers.find((t) => t.roleCode === 'SPECIAL') ?? ov.teachers[0]
    if (first) setTeacherId(first.id)
  }, [ov, teacherId])

  const { data: sheet } = useQuery({
    queryKey: ['lesson-sheet', teacherId],
    queryFn: () => lessonApi.get(teacherId!),
    enabled: teacherId != null,
  })

  const afterChange = (next: TeacherLessons) => {
    qc.setQueryData(['lesson-sheet', next.teacherId], next)
    qc.invalidateQueries({ queryKey: ['lesson-overview'] })
    qc.invalidateQueries({ queryKey: ['lesson-warnings'] })
    qc.invalidateQueries({ queryKey: ['teacher-list'] })
    qc.invalidateQueries({ queryKey: ['setup-state'] })
  }

  const save = useMutation({
    mutationFn: (v: { value: CellValue; target: CellTarget }) =>
      lessonApi.upsert({
        id: v.target.lesson?.id ?? null,
        teacherId: teacherId!,
        dayOfWeek: v.target.dayOfWeek,
        periodNo: v.target.periodNo,
        classId: v.value.classId,
        subjectId: v.value.subjectId,
        lessonType: v.value.lessonType,
        note: null,
      }),
    onSuccess: (next) => {
      afterChange(next)
      setSaveError(null)
    },
    onError: (e) => setSaveError(errorMessage(e)),
  })

  const remove = useMutation({
    mutationFn: (lessonId: number) => lessonApi.remove(lessonId),
    onSuccess: (next) => {
      afterChange(next)
      setEditorOpen(false)
      setSaveError(null)
    },
    onError: (e) => setSaveError(errorMessage(e)),
  })

  const teacher = ov?.teachers.find((t) => t.id === teacherId) ?? null

  /** (요일, 교시) -> 수업 */
  const byCell = useMemo(() => {
    const m = new Map<string, LessonView>()
    for (const l of sheet?.lessons ?? []) m.set(`${l.dayOfWeek}:${l.periodNo}`, l)
    return m
  }, [sheet])

  if (isLoading) return <div className={s.center}>불러오는 중…</div>
  if (error || !ov) return <Notice tone="danger">{errorMessage(error)}</Notice>

  if (ov.bellMissing) {
    return (
      <Notice tone="warn">
        시정표가 아직 없어 시간표를 넣어도 실제 시각을 알 수 없습니다. 먼저{' '}
        <strong>시정표 · 점심시간</strong>을 설정해 주세요.
      </Notice>
    )
  }
  if (ov.teachers.length === 0) {
    return (
      <Notice tone="warn">
        등록된 교사가 없습니다. <strong>교사 관리</strong>에서 선생님을 먼저 등록해 주세요.
      </Notice>
    )
  }

  const specials = ov.teachers.filter((t) => t.roleCode === 'SPECIAL')
  const others = ov.teachers.filter((t) => t.roleCode !== 'SPECIAL')

  const openCell = (dayOfWeek: number, periodNo: number) => {
    setTarget({ dayOfWeek, periodNo, lesson: byCell.get(`${dayOfWeek}:${periodNo}`) })
    setSaveError(null)
    setEditorOpen(true)
  }

  /** 저장 후 같은 요일의 다음 빈 교시를 이어서 연다 */
  const openNextEmpty = (dayOfWeek: number, fromPeriod: number) => {
    for (let p = fromPeriod + 1; p <= ov.maxPeriod; p++) {
      if (!byCell.has(`${dayOfWeek}:${p}`)) {
        setTarget({ dayOfWeek, periodNo: p })
        return true
      }
    }
    return false
  }

  const handleSave = (value: CellValue, andNext: boolean) => {
    if (!target) return
    const at = target
    setLastUsed(value)
    save.mutate(
      { value, target: at },
      {
        onSuccess: () => {
          if (andNext) {
            // 다음 칸으로 이어 가되, 없으면 닫는다
            const moved = openNextEmpty(at.dayOfWeek, at.periodNo)
            if (!moved) setEditorOpen(false)
          } else {
            setEditorOpen(false)
          }
        },
      },
    )
  }

  const typeMark = (t: LessonType) => (t === 'CO' ? '공동' : t === 'CROSS' ? '교차' : null)

  return (
    <div className={s.wrap}>
      {/* ---- 교사 목록 ---- */}
      <div className={s.layout}>
        <aside className={s.side}>
          <p className={s.sideTitle}>전담교사</p>
          {specials.length === 0 && (
            <p className={s.sideEmpty}>
              전담교사가 없습니다. 교사 관리에서 구분을 &lsquo;전담&rsquo;으로 지정하면 여기 나타납니다.
            </p>
          )}
          {specials.map((t) => (
            <button
              key={t.id}
              type="button"
              className={t.id === teacherId ? `${s.tItem} ${s.tItemOn}` : s.tItem}
              onClick={() => setTeacherId(t.id)}
            >
              <span className={s.tName}>{t.name}</span>
              <span className={s.tSub}>
                {t.subjects.length > 0 ? t.subjects.join(', ') : '과목 미지정'}
              </span>
              <span className={t.lessonCount > 0 ? s.tCount : `${s.tCount} ${s.tCountZero}`}>
                {t.lessonCount > 0 ? `${t.lessonCount}시간` : '비어 있음'}
              </span>
            </button>
          ))}

          {others.length > 0 && (
            <details className={s.otherBox}>
              <summary>담임·기타 교사 {others.length}명</summary>
              <p className={s.otherHint}>
                담임이 다른 반에 들어가는 교차수업이 있을 때만 쓰세요.
              </p>
              {others.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  className={t.id === teacherId ? `${s.tItem} ${s.tItemOn}` : s.tItem}
                  onClick={() => setTeacherId(t.id)}
                >
                  <span className={s.tName}>{t.name}</span>
                  <span className={s.tSub}>
                    {t.homerooms.length > 0 ? t.homerooms.join(', ') : (t.memo ?? t.roleLabel)}
                  </span>
                  {t.lessonCount > 0 && <span className={s.tCount}>{t.lessonCount}시간</span>}
                </button>
              ))}
            </details>
          )}
        </aside>

        {/* ---- 시간표 ---- */}
        <section className={s.main}>
          {teacher && (
            <div className={s.head}>
              <div>
                <h3 className={s.headName}>
                  {teacher.name} <span className={s.headRole}>{teacher.roleLabel}</span>
                </h3>
                <p className={s.headSub}>
                  {teacher.subjects.length > 0 && <>담당 과목 {teacher.subjects.join(', ')} · </>}
                  입력된 수업 {sheet?.lessons.length ?? 0}시간
                </p>
              </div>
              <div className={s.headActions}>
                <Button variant="ghost" onClick={() => setPasteOpen(true)}>
                  붙여넣기
                </Button>
              </div>
            </div>
          )}

          {sheet && sheet.problems.length > 0 && (
            <Notice tone="danger">
              <div>
                <strong>고쳐야 할 부분이 있습니다</strong>
                <ul className={s.list}>
                  {sheet.problems.map((p) => (
                    <li key={p}>{p}</li>
                  ))}
                </ul>
              </div>
            </Notice>
          )}

          <div className={s.gridWrap}>
            <table className={s.grid}>
              <thead>
                <tr>
                  <th className={s.corner}>교시</th>
                  {ov.schoolDays.map((d) => (
                    <th key={d}>{DAY_LABEL[d]}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {Array.from({ length: ov.maxPeriod }, (_, i) => i + 1).map((p) => (
                  <tr key={p}>
                    <th className={s.rowHead}>{p}교시</th>
                    {ov.schoolDays.map((d) => {
                      const l = byCell.get(`${d}:${p}`)
                      const anyGrade = ov.gradeSlots.some(
                        (g) => g.dayOfWeek === d && g.periodNo === p,
                      )
                      if (!anyGrade) {
                        return (
                          <td key={d} className={s.noSlot} title="이 요일에는 이 교시가 없습니다">
                            —
                          </td>
                        )
                      }
                      return (
                        <td key={d} className={s.cellWrap}>
                          <button
                            type="button"
                            className={l ? `${s.cell} ${s.cellFilled}` : s.cell}
                            onClick={() => openCell(d, p)}
                          >
                            {l ? (
                              <>
                                <span className={s.cellMain}>
                                  {l.classLabel}
                                  {l.subjectName && (
                                    <>
                                      {' · '}
                                      <span className={s.cellSubject}>{l.subjectName}</span>
                                    </>
                                  )}
                                </span>
                                <span className={s.cellTime}>
                                  {l.startMin != null && l.endMin != null
                                    ? `${minToHm(l.startMin)}~${minToHm(l.endMin)}`
                                    : '시각 없음'}
                                </span>
                                {typeMark(l.lessonType) && (
                                  <span className={s.cellTag}>{typeMark(l.lessonType)}</span>
                                )}
                              </>
                            ) : (
                              <span className={s.cellAdd}>
                                <Icon name="plus" size={14} />
                              </span>
                            )}
                          </button>
                        </td>
                      )
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <p className={s.gridHint}>
            빈 칸을 눌러 수업을 넣고, 채워진 칸을 눌러 고치거나 지웁니다. 시각은 그 학급 학년의
            시정표에서 자동으로 정해집니다.
          </p>

          {/* 전담 선생님이 일반 보결을 맡지 않는 시간 */}
          <MealCard teacherId={teacherId} />
        </section>
      </div>

      {teacher && (
        <>
          <LessonCellEditor
            open={editorOpen}
            target={target}
            classes={ov.classes}
            subjects={ov.subjects}
            gradeSlots={ov.gradeSlots}
            teacherSubjectIds={teacher.subjectIds}
            lastUsed={lastUsed}
            saving={save.isPending || remove.isPending}
            error={saveError}
            onClose={() => setEditorOpen(false)}
            onSave={handleSave}
            onDelete={(id) => remove.mutate(id)}
          />
          <PasteGridModal
            open={pasteOpen}
            teacherId={teacher.id}
            teacherName={teacher.name}
            teacherSubjectIds={teacher.subjectIds}
            data={ov as LessonOverview}
            onClose={() => setPasteOpen(false)}
            onApplied={afterChange}
          />
        </>
      )}
    </div>
  )
}
