import { useEffect, useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, Notice } from '@/components/ui'
import { errorMessage } from '@/ipc/invoke'
import { teacherApi, type TeacherList } from '@/ipc/teacher'
import { classOnly } from '@/lib/classLabel'
import s from './QuickHomeroomModal.module.css'

interface Props {
  open: boolean
  data: TeacherList
  onClose: () => void
  onSaved: (list: TeacherList) => void
}

/** 담임이 비어 있는 학급에 이름만 넣어 교사를 한꺼번에 만든다. */
export function QuickHomeroomModal({ open, data, onClose, onSaved }: Props) {
  const qc = useQueryClient()
  const [names, setNames] = useState<Record<number, string>>({})
  const [done, setDone] = useState<{ created: number; problems: string[] } | null>(null)

  useEffect(() => {
    if (open) {
      setNames({})
      setDone(null)
    }
  }, [open])

  const empty = data.classes.filter((c) => c.homeroomTeacherId === null)
  const filled = data.classes.filter((c) => c.homeroomTeacherId !== null)

  const entries = Object.entries(names)
    .filter(([, v]) => v.trim().length > 0)
    .map(([classId, name]) => ({ classId: Number(classId), name: name.trim() }))

  const run = useMutation({
    mutationFn: () => teacherApi.quickAddHomerooms(entries),
    onSuccess: (res) => {
      qc.setQueryData(['teacher-list'], res.list)
      qc.invalidateQueries({ queryKey: ['setup-state'] })
      qc.invalidateQueries({ queryKey: ['teacher-readiness'] })
      onSaved(res.list)
      setDone({ created: res.created, problems: res.problems })
      setNames({})
    },
  })

  // 학년별로 묶는다
  const byGrade = new Map<number, typeof empty>()
  for (const c of empty) {
    if (!byGrade.has(c.grade)) byGrade.set(c.grade, [])
    byGrade.get(c.grade)!.push(c)
  }

  return (
    <Modal
      open={open}
      wide
      title="담임 빠른 등록"
      description="담임이 없는 학급에 이름만 적으면 교사가 함께 만들어집니다. 구분은 '담임', 보결 배정 대상은 켜진 상태로 등록됩니다."
      onClose={onClose}
      footer={
        <>
          <span className={s.footInfo}>
            {entries.length > 0
              ? `${entries.length}명 입력됨`
              : empty.length > 0
                ? '이름을 적어 주세요'
                : '담임이 비어 있는 학급이 없습니다'}
          </span>
          <div className={s.footRight}>
            <Button variant="ghost" onClick={onClose}>
              닫기
            </Button>
            <Button
              variant="primary"
              disabled={entries.length === 0 || run.isPending}
              onClick={() => run.mutate()}
            >
              {run.isPending ? '등록 중…' : `${entries.length}명 등록`}
            </Button>
          </div>
        </>
      }
    >
      {run.error && <Notice tone="danger">{errorMessage(run.error)}</Notice>}

      {done && (
        <Notice tone={done.problems.length > 0 ? 'warn' : 'info'}>
          <div>
            <strong>{done.created}명을 등록했습니다.</strong>
            {done.problems.length > 0 && (
              <ul className={s.problems}>
                {done.problems.map((p) => (
                  <li key={p}>{p}</li>
                ))}
              </ul>
            )}
          </div>
        </Notice>
      )}

      {empty.length === 0 ? (
        <p className={s.allDone}>모든 학급에 담임이 지정되어 있습니다.</p>
      ) : (
        <div className={s.grid}>
          {[...byGrade.entries()].map(([grade, list]) => (
            <div key={grade} className={s.gradeRow}>
              <span className={s.gradeName}>{grade}학년</span>
              <div className={s.cells}>
                {list.map((c) => (
                  <label key={c.classId} className={s.cell}>
                    <span className={s.classNo}>{classOnly(c.classNo, c.name)}</span>
                    <input
                      className={s.nameInput}
                      value={names[c.classId] ?? ''}
                      maxLength={20}
                      placeholder="이름"
                      onChange={(e) =>
                        setNames((n) => ({ ...n, [c.classId]: e.target.value }))
                      }
                    />
                  </label>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {filled.length > 0 && (
        <details className={s.already}>
          <summary>이미 담임이 있는 학급 {filled.length}개</summary>
          <div className={s.alreadyList}>
            {filled.map((c) => (
              <span key={c.classId} className={s.alreadyItem}>
                <strong>{c.label}</strong> {c.homeroomName}
                {c.homeroomActive === false && <span className={s.inactive}>비활성</span>}
              </span>
            ))}
          </div>
          <p className={s.alreadyHint}>
            담임을 바꾸려면 목록에서 그 선생님을 눌러 담당 학급을 고쳐 주세요.
          </p>
        </details>
      )}
    </Modal>
  )
}
