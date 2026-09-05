import { useEffect, useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { TimeInput } from '@/components/TimeInput'
import { Button, Notice } from '@/components/ui'
import { assignApi, type AbsenceInput } from '@/ipc/assign'
import type { ReasonChoice, TeacherChoice } from '@/ipc/find'
import { errorMessage } from '@/ipc/invoke'
import s from './AbsenceModal.module.css'

interface Props {
  open: boolean
  /** 처음 열 때 보여 줄 날짜 (창 안에서 바꿀 수 있다) */
  date: string
  teachers: TeacherChoice[]
  reasons: ReasonChoice[]
  /** 미리 골라 둘 교사 */
  defaultTeacherId?: number | null
  onClose: () => void
  onSaved: (teacherId: number, date: string, teacherName: string) => void
}

const HM_0900 = 9 * 60
const HM_1200 = 12 * 60

/**
 * 결근 등록.
 *
 * 종일이 대부분이므로 종일을 기본으로 두고, 반차·조퇴처럼 일부 시간만
 * 비우는 경우에만 시각을 입력하게 한다. 등록하면 그 시간대 보결 조회에서
 * 곧바로 제외된다.
 */
export function AbsenceModal({
  open,
  date,
  teachers,
  reasons,
  defaultTeacherId,
  onClose,
  onSaved,
}: Props) {
  const qc = useQueryClient()
  const [day, setDay] = useState(date)
  const [teacherId, setTeacherId] = useState<number | null>(defaultTeacherId ?? null)
  const [isAllDay, setAllDay] = useState(true)
  const [startMin, setStartMin] = useState(HM_0900)
  const [endMin, setEndMin] = useState(HM_1200)
  const [reasonCode, setReasonCode] = useState<string>('')
  const [reasonText, setReasonText] = useState('')

  // 열릴 때마다 처음 상태로 되돌린다
  useEffect(() => {
    if (!open) return
    setDay(date)
    setTeacherId(defaultTeacherId ?? null)
    setAllDay(true)
    setStartMin(HM_0900)
    setEndMin(HM_1200)
    setReasonCode(reasons[0]?.code ?? '')
    setReasonText('')
  }, [open, date, defaultTeacherId, reasons])

  const save = useMutation({
    mutationFn: () => {
      const input: AbsenceInput = {
        teacherId: teacherId!,
        date: day,
        isAllDay,
        startMin: isAllDay ? null : startMin,
        endMin: isAllDay ? null : endMin,
        reasonCode: reasonCode || null,
        reasonText: reasonText.trim() || null,
      }
      return assignApi.absenceCreate(input)
    },
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ['absences'] })
      await qc.invalidateQueries({ queryKey: ['day-plan'] })
      const who = teachers.find((t) => t.id === teacherId)?.name ?? ''
      onSaved(teacherId!, day, who)
      onClose()
    },
  })

  const timeBad = !isAllDay && endMin <= startMin
  const canSave = teacherId != null && !timeBad && !save.isPending

  return (
    <Modal
      open={open}
      title="결근 등록"
      description="등록하면 그 시간대 보결 조회에서 곧바로 제외됩니다."
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            닫기
          </Button>
          <Button variant="primary" icon="check" disabled={!canSave} onClick={() => save.mutate()}>
            {save.isPending ? '등록 중…' : '결근 등록'}
          </Button>
        </>
      }
    >
      {save.error && <Notice tone="danger">{errorMessage(save.error)}</Notice>}

      <label className={s.field}>
        <span className={s.label}>날짜</span>
        <input
          type="date"
          className={s.select}
          value={day}
          onChange={(e) => setDay(e.target.value || date)}
        />
      </label>

      <label className={s.field}>
        <span className={s.label}>결근 교사</span>
        <select
          className={s.select}
          value={teacherId ?? ''}
          onChange={(e) => setTeacherId(e.target.value ? Number(e.target.value) : null)}
        >
          <option value="">선택해 주세요</option>
          {teachers.map((t) => (
            <option key={t.id} value={t.id}>
              {t.name} ({t.roleLabel}
              {t.duty ? ` · ${t.duty}` : ''})
            </option>
          ))}
        </select>
      </label>

      <div className={s.field}>
        <span className={s.label}>결근 범위</span>
        <div className={s.segment}>
          <button
            type="button"
            className={isAllDay ? `${s.seg} ${s.segOn}` : s.seg}
            onClick={() => setAllDay(true)}
          >
            종일
          </button>
          <button
            type="button"
            className={!isAllDay ? `${s.seg} ${s.segOn}` : s.seg}
            onClick={() => setAllDay(false)}
          >
            일부 시간
          </button>
        </div>
        {!isAllDay && (
          <div className={s.timeRow}>
            <TimeInput value={startMin} onChange={setStartMin} aria-label="결근 시작 시각" />
            <span className={s.tilde}>~</span>
            <TimeInput value={endMin} onChange={setEndMin} aria-label="결근 종료 시각" />
            <span className={s.hint}>이 시각과 겹치는 수업만 보결 대상이 됩니다</span>
          </div>
        )}
        {timeBad && <p className={s.bad}>종료 시각이 시작 시각보다 뒤여야 합니다.</p>}
      </div>

      <label className={s.field}>
        <span className={s.label}>사유</span>
        <div className={s.chips}>
          {reasons.map((r) => (
            <button
              key={r.code}
              type="button"
              className={r.code === reasonCode ? `${s.chip} ${s.chipOn}` : s.chip}
              onClick={() => setReasonCode(r.code)}
            >
              {r.label}
            </button>
          ))}
        </div>
      </label>

      <label className={s.field}>
        <span className={s.label}>
          메모 <span className={s.optional}>(선택)</span>
        </span>
        <input
          type="text"
          className={s.text}
          value={reasonText}
          maxLength={100}
          placeholder="예) 오후 교육청 회의"
          onChange={(e) => setReasonText(e.target.value)}
        />
      </label>
    </Modal>
  )
}
