import { useEffect, useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, Notice } from '@/components/ui'
import { errorMessage, isAppError } from '@/ipc/invoke'
import {
  ROLE_HINT,
  teacherApi,
  type RoleCode,
  type TeacherList,
  type TeacherView,
} from '@/ipc/teacher'
import { classOnly } from '@/lib/classLabel'
import s from './TeacherEditor.module.css'

interface Props {
  open: boolean
  /** null이면 새로 등록 */
  teacher: TeacherView | null
  data: TeacherList
  onClose: () => void
  onSaved: (list: TeacherList) => void
}

interface Form {
  name: string
  roleCode: RoleCode
  isSubstitutable: boolean
  memo: string
  homeroomClassIds: number[]
  subjectIds: number[]
}

function toForm(t: TeacherView | null): Form {
  if (!t) {
    return {
      name: '',
      roleCode: 'HOMEROOM',
      isSubstitutable: true,
      memo: '',
      homeroomClassIds: [],
      subjectIds: [],
    }
  }
  return {
    name: t.name,
    roleCode: t.roleCode,
    isSubstitutable: t.isSubstitutable,
    memo: t.memo ?? '',
    homeroomClassIds: t.homerooms.map((h) => h.classId),
    subjectIds: t.subjects.map((x) => x.id),
  }
}

export function TeacherEditor({ open, teacher, data, onClose, onSaved }: Props) {
  const qc = useQueryClient()
  const [form, setForm] = useState<Form>(() => toForm(teacher))
  const [askReplace, setAskReplace] = useState<string | null>(null)
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [newSubject, setNewSubject] = useState('')

  useEffect(() => {
    if (open) {
      setForm(toForm(teacher))
      setAskReplace(null)
      setConfirmDelete(false)
      setNewSubject('')
    }
  }, [open, teacher])

  const patch = (p: Partial<Form>) => setForm((f) => ({ ...f, ...p }))

  const afterChange = (list: TeacherList) => {
    qc.setQueryData(['teacher-list'], list)
    qc.invalidateQueries({ queryKey: ['setup-state'] })
    qc.invalidateQueries({ queryKey: ['teacher-readiness'] })
    onSaved(list)
  }

  const save = useMutation({
    mutationFn: (replace: boolean) =>
      teacherApi.upsert({
        id: teacher?.id ?? null,
        name: form.name.trim(),
        roleCode: form.roleCode,
        isSubstitutable: form.isSubstitutable,
        memo: form.memo.trim() || null,
        homeroomClassIds: form.roleCode === 'HOMEROOM' ? form.homeroomClassIds : [],
        subjectIds: form.subjectIds,
        replaceExistingHomeroom: replace,
      }),
    onSuccess: (list) => {
      afterChange(list)
      onClose()
    },
    onError: (e) => {
      // 담임이 겹치면 확인을 받고 다시 시도한다
      if (isAppError(e) && e.code === 'HOMEROOM_TAKEN') setAskReplace(e.userMessage)
    },
  })

  const setActive = useMutation({
    mutationFn: (active: boolean) => teacherApi.setActive(teacher!.id, active),
    onSuccess: (res) => {
      afterChange(res.list)
      onClose()
    },
  })

  const remove = useMutation({
    mutationFn: () => teacherApi.remove(teacher!.id),
    onSuccess: (list) => {
      afterChange(list)
      onClose()
    },
  })

  const addSubject = useMutation({
    mutationFn: (name: string) => teacherApi.upsertSubject(name),
    onSuccess: (list) => {
      qc.setQueryData(['teacher-list'], list)
      const added = list.subjects.find((x) => x.name === newSubject.trim())
      if (added) patch({ subjectIds: [...form.subjectIds, added.id] })
      setNewSubject('')
      onSaved(list)
    },
  })

  const toggleClass = (classId: number) => {
    patch({
      homeroomClassIds: form.homeroomClassIds.includes(classId)
        ? form.homeroomClassIds.filter((x) => x !== classId)
        : [...form.homeroomClassIds, classId],
    })
  }

  const toggleSubject = (id: number) => {
    patch({
      subjectIds: form.subjectIds.includes(id)
        ? form.subjectIds.filter((x) => x !== id)
        : [...form.subjectIds, id],
    })
  }

  const nameEmpty = form.name.trim().length === 0
  const busy = save.isPending || setActive.isPending || remove.isPending

  // 학년별로 학급을 묶어서 보여 준다
  const byGrade = new Map<number, typeof data.classes>()
  for (const c of data.classes) {
    if (!byGrade.has(c.grade)) byGrade.set(c.grade, [])
    byGrade.get(c.grade)!.push(c)
  }

  return (
    <Modal
      open={open}
      title={teacher ? `${teacher.name} 선생님 정보` : '교사 추가'}
      description={
        teacher
          ? '고친 내용은 앞으로의 보결 조회에 적용됩니다. 지난 보결 기록은 그대로 남습니다.'
          : undefined
      }
      onClose={onClose}
      footer={
        <>
          <div className={s.footLeft}>
            {teacher && !confirmDelete && (
              <>
                {teacher.active ? (
                  <button
                    type="button"
                    className={s.subtleBtn}
                    disabled={busy}
                    onClick={() => setActive.mutate(false)}
                  >
                    비활성으로 바꾸기
                  </button>
                ) : (
                  <button
                    type="button"
                    className={s.subtleBtn}
                    disabled={busy}
                    onClick={() => setActive.mutate(true)}
                  >
                    다시 활성으로
                  </button>
                )}
                {!teacher.hasRecords && (
                  <button
                    type="button"
                    className={s.dangerLink}
                    onClick={() => setConfirmDelete(true)}
                  >
                    완전히 지우기
                  </button>
                )}
              </>
            )}
            {teacher && confirmDelete && (
              <span className={s.confirmRow}>
                <span className={s.confirmText}>되돌릴 수 없습니다. 지울까요?</span>
                <button
                  type="button"
                  className={s.confirmYes}
                  disabled={busy}
                  onClick={() => remove.mutate()}
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
            <Button variant="ghost" onClick={onClose} disabled={busy}>
              취소
            </Button>
            <Button
              variant="primary"
              disabled={nameEmpty || busy}
              onClick={() => save.mutate(false)}
            >
              {save.isPending ? '저장 중…' : '저장'}
            </Button>
          </div>
        </>
      }
    >
      {askReplace && (
        <Notice tone="warn">
          <div>
            <p>{askReplace}</p>
            <div className={s.replaceActions}>
              <Button variant="primary" onClick={() => save.mutate(true)} disabled={busy}>
                네, 담임을 바꿉니다
              </Button>
              <Button variant="ghost" onClick={() => setAskReplace(null)}>
                아니요
              </Button>
            </div>
          </div>
        </Notice>
      )}
      {save.error && !askReplace && <Notice tone="danger">{errorMessage(save.error)}</Notice>}
      {setActive.error && <Notice tone="danger">{errorMessage(setActive.error)}</Notice>}
      {remove.error && <Notice tone="danger">{errorMessage(remove.error)}</Notice>}

      {teacher && !teacher.active && (
        <Notice tone="info">
          이 선생님은 <strong>비활성</strong> 상태입니다. 보결 조회에 나타나지 않지만 지난 기록에는
          그대로 남아 있습니다.
        </Notice>
      )}

      <label className={s.field}>
        <span className={s.label}>이름</span>
        <input
          className={s.input}
          value={form.name}
          maxLength={20}
          placeholder="예) 김민수"
          onChange={(e) => patch({ name: e.target.value })}
          autoFocus
        />
      </label>

      <div className={s.field}>
        <span className={s.label}>구분</span>
        <div className={s.roleChips}>
          {data.roles.map((r) => (
            <button
              key={r.code}
              type="button"
              className={form.roleCode === r.code ? `${s.roleChip} ${s.roleChipOn}` : s.roleChip}
              onClick={() => patch({ roleCode: r.code })}
            >
              <span className={s.roleName}>{r.label}</span>
              <span className={s.roleHint}>{ROLE_HINT[r.code]}</span>
            </button>
          ))}
        </div>
      </div>

      {form.roleCode === 'HOMEROOM' && (
        <div className={s.field}>
          <span className={s.label}>담당 학급</span>
          <div className={s.classGroups}>
            {[...byGrade.entries()].map(([grade, list]) => (
              <div key={grade} className={s.classGroup}>
                <span className={s.gradeName}>{grade}학년</span>
                <div className={s.classChips}>
                  {list.map((c) => {
                    const on = form.homeroomClassIds.includes(c.classId)
                    const other =
                      c.homeroomTeacherId !== null && c.homeroomTeacherId !== teacher?.id
                    return (
                      <button
                        key={c.classId}
                        type="button"
                        className={on ? `${s.classChip} ${s.classChipOn}` : s.classChip}
                        onClick={() => toggleClass(c.classId)}
                        title={other ? `지금은 ${c.homeroomName} 선생님이 담임입니다` : undefined}
                      >
                        {classOnly(c.classNo, c.name)}
                        {other && !on && <span className={s.taken}>{c.homeroomName}</span>}
                      </button>
                    )
                  })}
                </div>
              </div>
            ))}
          </div>
          <p className={s.hint}>
            복식학급이면 두 개 이상 고를 수 있습니다. 다른 선생님이 맡고 있는 반을 고르면 저장할 때
            확인을 물어봅니다.
          </p>
        </div>
      )}

      <div className={s.field}>
        <span className={s.label}>
          담당 과목
          {form.roleCode === 'SPECIAL' && <span className={s.req}> · 전담은 꼭 지정해 주세요</span>}
        </span>
        <div className={s.subjectChips}>
          {data.subjects.map((sub) => (
            <button
              key={sub.id}
              type="button"
              className={
                form.subjectIds.includes(sub.id)
                  ? `${s.subjectChip} ${s.subjectChipOn}`
                  : s.subjectChip
              }
              onClick={() => toggleSubject(sub.id)}
            >
              {sub.name}
            </button>
          ))}
        </div>
        <div className={s.addSubject}>
          <input
            className={s.inputSmall}
            value={newSubject}
            maxLength={20}
            placeholder="없는 과목 추가"
            onChange={(e) => setNewSubject(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && newSubject.trim()) addSubject.mutate(newSubject.trim())
            }}
          />
          <Button
            variant="ghost"
            disabled={!newSubject.trim() || addSubject.isPending}
            onClick={() => addSubject.mutate(newSubject.trim())}
          >
            추가
          </Button>
        </div>
        {addSubject.error && <Notice tone="danger">{errorMessage(addSubject.error)}</Notice>}
      </div>

      <label className={s.field}>
        <span className={s.label}>담당 업무 · 메모</span>
        <input
          className={s.input}
          value={form.memo}
          maxLength={40}
          placeholder="예) 보건교사, 교감, 3학년 부장"
          onChange={(e) => patch({ memo: e.target.value })}
        />
        <p className={s.hint}>목록에 함께 표시됩니다. 비워 두어도 됩니다.</p>
      </label>

      <div className={s.switchRow}>
        <label className={s.switch}>
          <input
            type="checkbox"
            checked={form.isSubstitutable}
            onChange={(e) => patch({ isSubstitutable: e.target.checked })}
          />
          <span>
            <strong>보결 배정 대상</strong>
            <span className={s.switchHint}>
              끄면 보결 조회 결과에 나타나지 않습니다. 교장·교감·보건 등에 사용하세요.
            </span>
          </span>
        </label>
      </div>
    </Modal>
  )
}
