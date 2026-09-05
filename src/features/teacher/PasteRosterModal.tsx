import { useEffect, useMemo, useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, Notice } from '@/components/ui'
import { errorMessage } from '@/ipc/invoke'
import { teacherApi, type RoleCode, type TeacherList } from '@/ipc/teacher'
import { parseRoster } from './parseRoster'
import s from './PasteRosterModal.module.css'

const ROLE_LABEL: Record<RoleCode, string> = {
  HOMEROOM: '담임',
  SPECIAL: '전담',
  OTHER: '기타',
}

const SAMPLE = `김민수\t담임\t1-1
이영희\t담임\t1학년 2반
박지훈\t전담\t체육, 음악
최수정\t기타\t보건교사`

interface Props {
  open: boolean
  onClose: () => void
  onSaved: (list: TeacherList) => void
}

export function PasteRosterModal({ open, onClose, onSaved }: Props) {
  const qc = useQueryClient()
  const [text, setText] = useState('')
  const [done, setDone] = useState<{ created: number; skipped: number; problems: string[] } | null>(
    null,
  )

  useEffect(() => {
    if (open) {
      setText('')
      setDone(null)
    }
  }, [open])

  const rows = useMemo(() => parseRoster(text), [text])

  const run = useMutation({
    mutationFn: () =>
      teacherApi.bulkCreate(
        rows.map((r) => ({
          name: r.name,
          roleCode: r.roleCode ?? null,
          memo: r.memo ?? null,
          grade: r.grade ?? null,
          classNo: r.classNo ?? null,
          className: r.className ?? null,
          subjectNames: r.subjectNames ?? [],
        })),
      ),
    onSuccess: (res) => {
      qc.setQueryData(['teacher-list'], res.list)
      qc.invalidateQueries({ queryKey: ['setup-state'] })
      qc.invalidateQueries({ queryKey: ['teacher-readiness'] })
      onSaved(res.list)
      setDone({ created: res.created, skipped: res.skipped, problems: res.problems })
      setText('')
    },
  })

  return (
    <Modal
      open={open}
      wide
      title="명단 붙여넣기"
      description="엑셀이나 한글에서 교사 명단을 복사해 아래에 붙여넣으세요. 미리보기를 확인한 뒤 등록합니다."
      onClose={onClose}
      footer={
        <>
          <span className={s.footInfo}>
            {rows.length > 0 ? `${rows.length}명 읽음` : '붙여넣으면 미리보기가 나타납니다'}
          </span>
          <div className={s.footRight}>
            <Button variant="ghost" onClick={onClose}>
              닫기
            </Button>
            <Button
              variant="primary"
              disabled={rows.length === 0 || run.isPending}
              onClick={() => run.mutate()}
            >
              {run.isPending ? '등록 중…' : `${rows.length}명 등록`}
            </Button>
          </div>
        </>
      }
    >
      {run.error && <Notice tone="danger">{errorMessage(run.error)}</Notice>}

      {done && (
        <Notice tone={done.problems.length > 0 ? 'warn' : 'info'}>
          <div>
            <strong>
              {done.created}명을 등록했습니다.
              {done.skipped > 0 && ` (${done.skipped}줄은 이름이 없어 건너뜀)`}
            </strong>
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
          placeholder={'김민수\t담임\t1-1\n박지훈\t전담\t체육\n최수정\t기타\t보건교사'}
          onChange={(e) => setText(e.target.value)}
        />
        <p className={s.hint}>
          한 줄에 한 명씩. <strong>이름만 있어도 됩니다.</strong> 구분(담임·전담·기타), 학급(1-1 또는
          1학년 2반), 과목, 담당 업무는 순서에 상관없이 알아서 구분합니다.
        </p>
      </div>

      {rows.length > 0 && (
        <div>
          <p className={s.previewTitle}>미리보기 — 이대로 등록됩니다</p>
          <div className={s.tableWrap}>
            <table className={s.table}>
              <thead>
                <tr>
                  <th>이름</th>
                  <th>구분</th>
                  <th>담당 학급</th>
                  <th>과목</th>
                  <th>담당 업무</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r, i) => (
                  <tr key={`${r.name}-${i}`}>
                    <td className={s.nameCell}>{r.name}</td>
                    <td>{r.roleCode ? ROLE_LABEL[r.roleCode] : '—'}</td>
                    <td>
                      {r.grade != null
                        ? r.className
                          ? `${r.grade}-${r.className}`
                          : r.classNo != null
                            ? `${r.grade}-${r.classNo}`
                            : '—'
                        : '—'}
                    </td>
                    <td>{(r.subjectNames ?? []).join(', ') || '—'}</td>
                    <td className={s.memoCell}>{r.memo ?? '—'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className={s.hint}>
            잘못 읽은 줄이 있으면 붙여넣은 내용을 고치거나, 등록한 뒤 목록에서 수정하면 됩니다.
          </p>
        </div>
      )}
    </Modal>
  )
}
