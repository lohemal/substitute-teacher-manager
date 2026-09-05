import { useEffect, useMemo, useState } from 'react'
import { useMutation } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, Notice } from '@/components/ui'
import { DAY_LABEL } from '@/ipc/bell'
import { errorMessage } from '@/ipc/invoke'
import { lessonApi, type LessonOverview, type LessonRow, type TeacherLessons } from '@/ipc/lesson'
import { parseLessonGrid, type ParsedCell } from './parseLessonGrid'
import s from './PasteGridModal.module.css'

interface Props {
  open: boolean
  teacherId: number
  teacherName: string
  teacherSubjectIds: number[]
  data: LessonOverview
  onClose: () => void
  onApplied: (next: TeacherLessons) => void
}

const SAMPLE = [
  '\t월\t화\t수\t목\t금',
  '1교시\t4-1 영어\t\t5-1 영어\t\t',
  '2교시\t4-2 영어\t5-1 영어\t\t6-1 영어\t',
  '3교시\t\t5-2 영어\t4-1 영어\t\t6-2 영어',
].join('\n')

export function PasteGridModal({
  open,
  teacherId,
  teacherName,
  teacherSubjectIds,
  data,
  onClose,
  onApplied,
}: Props) {
  const [text, setText] = useState('')
  const [problems, setProblems] = useState<string[]>([])
  const [checking, setChecking] = useState(false)

  useEffect(() => {
    if (open) {
      setText('')
      setProblems([])
    }
  }, [open])

  const defaultSubject = data.subjects.find((x) => x.id === teacherSubjectIds[0])

  const result = useMemo(
    () =>
      text.trim()
        ? parseLessonGrid(text, {
            classes: data.classes,
            subjects: data.subjects,
            defaultSubjectId: defaultSubject?.id,
            defaultSubjectName: defaultSubject?.name,
          })
        : null,
    [text, data.classes, data.subjects, defaultSubject],
  )

  const good: ParsedCell[] = result?.cells.filter((c) => c.classId != null) ?? []
  const bad: ParsedCell[] = result?.cells.filter((c) => c.classId == null) ?? []
  const unsureSubject = good.filter((c) => c.problem != null)

  const rows: LessonRow[] = good.map((c) => ({
    dayOfWeek: c.dayOfWeek,
    periodNo: c.periodNo,
    classId: c.classId!,
    subjectId: c.subjectId ?? null,
    lessonType: c.lessonType,
  }))

  // 읽은 결과를 백엔드 규칙(시각 겹침 등)으로 미리 검사한다
  useEffect(() => {
    if (rows.length === 0) {
      setProblems([])
      return
    }
    setChecking(true)
    const t = setTimeout(() => {
      lessonApi
        .check(rows)
        .then(setProblems)
        .catch(() => setProblems([]))
        .finally(() => setChecking(false))
    }, 300)
    return () => clearTimeout(t)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(rows)])

  const apply = useMutation({
    mutationFn: () => lessonApi.bulkReplace(teacherId, rows),
    onSuccess: (next) => {
      onApplied(next)
      onClose()
    },
  })

  const blocked = problems.length > 0 || rows.length === 0

  return (
    <Modal
      open={open}
      wide
      title={`${teacherName} 선생님 시간표 붙여넣기`}
      description="엑셀이나 한글에서 시간표 표를 복사해 붙여넣으세요. 미리보기를 확인한 뒤 반영합니다."
      onClose={onClose}
      footer={
        <>
          <span className={s.footInfo}>
            {result == null
              ? '붙여넣으면 미리보기가 나타납니다'
              : `읽은 수업 ${good.length}개${bad.length > 0 ? ` · 못 읽은 칸 ${bad.length}개` : ''}`}
          </span>
          <div className={s.footRight}>
            <Button variant="ghost" onClick={onClose} disabled={apply.isPending}>
              취소
            </Button>
            <Button
              variant="primary"
              disabled={blocked || checking || apply.isPending}
              onClick={() => apply.mutate()}
            >
              {apply.isPending ? '반영 중…' : `${rows.length}개 수업으로 바꾸기`}
            </Button>
          </div>
        </>
      }
    >
      {apply.error && <Notice tone="danger">{errorMessage(apply.error)}</Notice>}

      <div>
        <div className={s.taLabel}>
          <span>붙여넣기</span>
          <button type="button" className={s.sampleBtn} onClick={() => setText(SAMPLE)}>
            예시 넣어 보기
          </button>
        </div>
        <textarea
          className={`${s.textarea} selectable`}
          value={text}
          rows={7}
          placeholder={'\t월\t화\t수\n1교시\t4-1 영어\t\t5-1 영어\n2교시\t4-2 영어\t5-1 영어\t'}
          onChange={(e) => setText(e.target.value)}
        />
        <p className={s.hint}>
          첫 줄에 <strong>요일</strong>, 첫 칸에 <strong>교시</strong>가 있는 표를 넣으면 됩니다.
          (요일과 교시가 바뀐 표도 알아서 읽습니다) 칸에는 <strong>4-1 영어</strong>,{' '}
          <strong>4학년 1반 영어</strong>, <strong>3-가람 체육</strong> 모두 됩니다.
          {defaultSubject && (
            <>
              {' '}
              과목을 안 적으면 <strong>{defaultSubject.name}</strong>으로 넣습니다.
            </>
          )}
        </p>
      </div>

      {result?.fatal && <Notice tone="danger">{result.fatal}</Notice>}

      {bad.length > 0 && (
        <Notice tone="warn">
          <div>
            <strong>못 읽은 칸 {bad.length}개는 빠집니다</strong>
            <ul className={s.list}>
              {bad.slice(0, 6).map((c, i) => (
                <li key={i}>
                  {DAY_LABEL[c.dayOfWeek]}요일 {c.periodNo}교시 &lsquo;{c.raw}&rsquo; — {c.problem}
                </li>
              ))}
              {bad.length > 6 && <li>… 그 밖 {bad.length - 6}개</li>}
            </ul>
          </div>
        </Notice>
      )}

      {unsureSubject.length > 0 && (
        <Notice tone="info">
          <div>
            <strong>과목을 알아보지 못한 칸 {unsureSubject.length}개</strong>
            <ul className={s.list}>
              {unsureSubject.slice(0, 4).map((c, i) => (
                <li key={i}>
                  {DAY_LABEL[c.dayOfWeek]}요일 {c.periodNo}교시 {c.classLabel} — {c.problem}
                </li>
              ))}
            </ul>
            학급은 정상이므로 과목 없이 넣고, 반영 뒤 칸을 눌러 과목만 고쳐도 됩니다.
          </div>
        </Notice>
      )}

      {problems.length > 0 && (
        <Notice tone="danger">
          <div>
            <strong>이대로는 반영할 수 없습니다</strong>
            <ul className={s.list}>
              {problems.map((p) => (
                <li key={p}>{p}</li>
              ))}
            </ul>
          </div>
        </Notice>
      )}

      {good.length > 0 && (
        <div>
          <p className={s.previewTitle}>
            미리보기 — 이대로 <strong>{teacherName}</strong> 선생님의 시간표를 바꿉니다
            <span className={s.previewWarn}> (기존 시간표는 지워집니다)</span>
          </p>
          <div className={s.tableWrap}>
            <table className={s.table}>
              <thead>
                <tr>
                  <th className={s.corner}>교시</th>
                  {data.schoolDays.map((d) => (
                    <th key={d}>{DAY_LABEL[d]}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {Array.from({ length: data.maxPeriod }, (_, i) => i + 1).map((p) => (
                  <tr key={p}>
                    <th className={s.rowHead}>{p}교시</th>
                    {data.schoolDays.map((d) => {
                      const cell = result?.cells.find(
                        (c) => c.dayOfWeek === d && c.periodNo === p,
                      )
                      if (!cell) return <td key={d} className={s.empty}>—</td>
                      if (cell.classId == null)
                        return (
                          <td key={d} className={s.badCell} title={cell.problem}>
                            {cell.raw}
                          </td>
                        )
                      return (
                        <td key={d} className={s.goodCell}>
                          <span className={s.cellClass}>{cell.classLabel}</span>
                          {cell.subjectName && (
                            <span className={s.cellSubject}>{cell.subjectName}</span>
                          )}
                        </td>
                      )
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </Modal>
  )
}
