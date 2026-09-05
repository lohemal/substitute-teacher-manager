import { useEffect, useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, Notice } from '@/components/ui'
import { assignApi, type BatchPick, type PlanSlot } from '@/ipc/assign'
import { DAY_LABEL, minToHm } from '@/ipc/bell'
import type { FindOptions } from '@/ipc/find'
import { errorDetail, errorMessage } from '@/ipc/invoke'
import { flagPicks, initialPicks, repeatIndex, type PickSlot, type Picks } from './planPicks'
import s from './BatchAssignModal.module.css'

interface Props {
  open: boolean
  /** 처음 열 때 보여 줄 날짜. 창 안에서 바꿀 수 있다 */
  date: string
  opt: FindOptions
  /** 처음 열 때 고를 대상 — 조회 화면에서 보던 학급 */
  defaultClassId: number | null
  /** 학급 대신 특정 선생님으로 바로 열 때 */
  defaultTeacherId?: number | null
  onClose: () => void
  onSaved: (count: number) => void
}

type SlotState = 'DONE' | 'PLANNED' | 'EMPTY' | 'BLOCKED'

const STATE_LABEL: Record<SlotState, string> = {
  DONE: '배정 완료',
  PLANNED: '배정 예정',
  EMPTY: '미배정',
  BLOCKED: '후보 없음',
}

function slotKey(x: PlanSlot): string {
  return `${x.classId}:${x.slotType}:${x.periodNo ?? 'L'}`
}

function weekdayOf(date: string): number {
  const js = new Date(`${date}T00:00:00`).getDay()
  return js === 0 ? 7 : js
}

function shiftDate(date: string, days: number): string {
  const d = new Date(`${date}T00:00:00`)
  d.setDate(d.getDate() + days)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

/**
 * 한 반(또는 한 선생님)의 하루치 보결을 한 번에 배정한다.
 *
 * 날짜와 대상을 고르면 그 날 보결이 필요한 시간을 모두 뽑아 시간대마다
 * 추천 1순위를 골라 둔다. 최종 선택은 담당자가 한다.
 *
 * 같은 선생님이 여러 교시에 들어가는 것은 막지 않는다. 대신 **중복**으로
 * 표시해 담당자가 보고 정하게 한다. 다만 시각이 겹치는 경우는 저장할 수
 * 없으므로 따로 알리고 [배정]을 잠근다.
 *
 * 저장은 **전체 성공 또는 전체 실패**다.
 */
export function BatchAssignModal({
  open,
  date,
  opt,
  defaultClassId,
  defaultTeacherId,
  onClose,
  onSaved,
}: Props) {
  const qc = useQueryClient()

  // ---------- 날짜 ----------
  const [day, setDay] = useState(date)
  useEffect(() => {
    if (open) setDay(date)
  }, [open, date])

  // ---------- 대상 ----------
  // 값은 항상 교사 id다. 학급을 고르면 그 반 담임이 된다.
  const classTargets = useMemo(
    () => opt.classes.filter((c) => c.homeroomTeacherId != null),
    [opt.classes],
  )
  const homeroomIds = useMemo(
    () => new Set(classTargets.map((c) => c.homeroomTeacherId as number)),
    [classTargets],
  )
  const otherTargets = useMemo(
    () => opt.teachers.filter((t) => !homeroomIds.has(t.id)),
    [opt.teachers, homeroomIds],
  )

  const initialTarget =
    defaultTeacherId ??
    classTargets.find((c) => c.classId === defaultClassId)?.homeroomTeacherId ??
    classTargets[0]?.homeroomTeacherId ??
    otherTargets[0]?.id ??
    null

  const [target, setTarget] = useState<number | null>(initialTarget)
  useEffect(() => {
    if (open) setTarget(initialTarget)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  const { data: plan, isLoading, error } = useQuery({
    queryKey: ['day-plan', day, target],
    queryFn: () => assignApi.dayPlan(day, target as number),
    enabled: open && target != null,
  })

  // ---------- 선택 ----------
  const [picks, setPicks] = useState<Picks>({})

  const pickSlots: PickSlot[] = useMemo(
    () =>
      (plan?.slots ?? []).map((x) => ({
        key: slotKey(x),
        startMin: x.startMin,
        endMin: x.endMin,
        candidateIds: x.candidates.map((c) => c.teacherId),
        taken: x.existingSubId != null,
      })),
    [plan],
  )

  // 계획을 받으면 추천 1순위를 골라 둔다 (자동 저장은 하지 않는다)
  useEffect(() => {
    setPicks(initialPicks(pickSlots))
  }, [pickSlots])

  const flags = useMemo(() => flagPicks(pickSlots, picks), [pickSlots, picks])
  const overlapCount = Object.values(flags).filter((f) => f === 'OVERLAP').length
  const duplicateCount = Object.values(flags).filter((f) => f === 'DUPLICATE').length

  const stateOf = (x: PlanSlot): SlotState => {
    if (x.existingSubId != null) return 'DONE'
    if (x.candidates.length === 0) return 'BLOCKED'
    return picks[slotKey(x)] != null ? 'PLANNED' : 'EMPTY'
  }

  const planned = useMemo(() => {
    if (!plan) return [] as BatchPick[]
    return plan.slots
      .filter((x) => x.existingSubId == null && picks[slotKey(x)] != null)
      .map((x) => ({
        classId: x.classId,
        slotType: x.slotType,
        periodNo: x.periodNo,
        subTeacherId: picks[slotKey(x)] as number,
      }))
  }, [plan, picks])

  const save = useMutation({
    mutationFn: () =>
      assignApi.batch({
        date: day,
        absentTeacherId: target as number,
        absenceId: plan?.absenceId ?? null,
        picks: planned,
      }),
    onSuccess: async (out) => {
      await Promise.all([
        qc.invalidateQueries({ queryKey: ['assign-history'] }),
        qc.invalidateQueries({ queryKey: ['day-plan'] }),
        qc.invalidateQueries({ queryKey: ['absences'] }),
      ])
      onSaved(out.saved.length)
      onClose()
    },
  })

  const noticeCount = plan?.slots.filter((x) => x.notice).length ?? 0
  const canSave = planned.length > 0 && overlapCount === 0 && !save.isPending

  return (
    <Modal
      open={open}
      wide
      title="다건 보결 배정"
      description="날짜와 대상을 고르면 그 날 보결이 필요한 시간을 모두 찾아 드립니다."
      onClose={onClose}
      footer={
        <>
          <span className={s.footInfo}>
            배정 예정 <strong className="num">{planned.length}</strong>건
            {plan && plan.slots.length > planned.length && (
              <span className={s.footRest}>
                / 전체 {plan.slots.length}건 (나머지는 미배정으로 남습니다)
              </span>
            )}
          </span>
          <div className={s.footBtns}>
            <Button variant="ghost" onClick={onClose}>
              닫기
            </Button>
            <Button variant="primary" icon="check" disabled={!canSave} onClick={() => save.mutate()}>
              {save.isPending ? '확인하는 중…' : `${planned.length}건 배정`}
            </Button>
          </div>
        </>
      }
    >
      {/* ---------- 날짜 · 대상 ---------- */}
      <div className={s.headRow}>
        <span className={s.headLabel}>날짜</span>
        <div className={s.dateBox}>
          <button
            type="button"
            className={s.stepBtn}
            onClick={() => setDay(shiftDate(day, -1))}
            aria-label="하루 전"
          >
            ‹
          </button>
          <input
            type="date"
            className={s.dateInput}
            value={day}
            onChange={(e) => setDay(e.target.value || date)}
          />
          <button
            type="button"
            className={s.stepBtn}
            onClick={() => setDay(shiftDate(day, 1))}
            aria-label="하루 뒤"
          >
            ›
          </button>
          <span className={s.weekday}>{DAY_LABEL[weekdayOf(day)]}요일</span>
        </div>

        <span className={s.headLabel}>대상</span>
        <select
          className={s.targetSelect}
          value={target ?? ''}
          onChange={(e) => setTarget(e.target.value ? Number(e.target.value) : null)}
        >
          <optgroup label="학급 (담임이 자리를 비울 때)">
            {classTargets.map((c) => (
              <option key={c.classId} value={c.homeroomTeacherId as number}>
                {c.label} — 담임 {c.homeroomName}
              </option>
            ))}
          </optgroup>
          {otherTargets.length > 0 && (
            <optgroup label="전담 · 그 밖의 선생님">
              {otherTargets.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name} ({t.roleLabel}
                  {t.duty ? ` · ${t.duty}` : ''})
                </option>
              ))}
            </optgroup>
          )}
        </select>
        {plan?.absenceLabel && <span className={s.absenceTag}>{plan.absenceLabel}</span>}
      </div>

      {isLoading && <p className={s.center}>불러오는 중…</p>}
      {error && <Notice tone="danger">{errorMessage(error)}</Notice>}

      {save.error && (
        <Notice tone="danger">
          <div>
            <strong>배정하지 못했습니다</strong>
            <p className={s.errText}>{errorMessage(save.error)}</p>
            {errorDetail(save.error) && (
              <details className={s.detail}>
                <summary>자세히</summary>
                <pre className="selectable">{errorDetail(save.error)}</pre>
              </details>
            )}
          </div>
        </Notice>
      )}

      {plan?.warnings.map((w) => (
        <Notice key={w} tone="warn">
          {w}
        </Notice>
      ))}

      {overlapCount > 0 && (
        <Notice tone="danger">
          같은 시각에 같은 선생님을 두 곳에 넣을 수는 없습니다. 빨간색{' '}
          <b>시각 겹침</b> 표시가 있는 칸의 선생님을 바꿔 주세요.
        </Notice>
      )}

      {noticeCount > 0 && (
        <p className={s.infoLine}>
          <span className={s.infoDot} aria-hidden="true" />
          전담 수업이 들어오는 시간이 {noticeCount}칸 있습니다. 표의 <b>ⓘ</b> 표시를 확인해
          주세요 — 보결이 필요 없을 수 있습니다.
        </p>
      )}

      {plan && plan.slots.length > 0 && (
        <div className={s.tableWrap}>
          <table className={s.table}>
            <thead>
              <tr>
                <th className={s.timeCol}>시간</th>
                <th className={s.targetCol}>대상</th>
                <th className={s.pickCol}>선택 교사</th>
                <th className={s.stateCol}>상태</th>
              </tr>
            </thead>
            <tbody>
              {plan.slots.map((x) => {
                const key = slotKey(x)
                const st = stateOf(x)
                const flag = flags[key] ?? 'NONE'
                const chosen = picks[key] ?? null
                const chosenC = x.candidates.find((c) => c.teacherId === chosen)
                const nth = repeatIndex(pickSlots, picks, key)
                return (
                  <tr key={key} className={st === 'DONE' ? s.rowDone : undefined}>
                    <td className={s.timeCol}>
                      <span className={s.slotLabel}>{x.slotLabel}</span>
                      <span className={`${s.time} num`}>
                        {minToHm(x.startMin)}~{minToHm(x.endMin)}
                      </span>
                    </td>

                    <td className={s.targetCol}>
                      <span className={s.cls}>{x.classLabel}</span>
                      <span className={s.kind}>
                        {x.kindLabel}
                        {x.subjectName ? ` · ${x.subjectName}` : ''}
                      </span>
                      {x.notice && (
                        <span className={s.noticeTag} title={x.notice.body}>
                          ⓘ {x.notice.title}
                        </span>
                      )}
                    </td>

                    <td className={s.pickCol}>
                      {st === 'DONE' ? (
                        <span className={s.doneName}>{x.existingSubName}</span>
                      ) : (
                        <>
                          <select
                            className={s.select}
                            value={chosen ?? ''}
                            disabled={x.candidates.length === 0}
                            onChange={(e) =>
                              setPicks((prev) => ({
                                ...prev,
                                [key]: e.target.value ? Number(e.target.value) : null,
                              }))
                            }
                          >
                            <option value="">미배정으로 두기</option>
                            {x.candidates.map((c) => (
                              <option key={c.teacherId} value={c.teacherId}>
                                {c.rank}순위 {c.name} ({c.roleLabel}
                                {c.duty ? ` · ${c.duty}` : ''}) — 오늘 {c.counts.today}회 · 누적{' '}
                                {c.counts.total}회
                              </option>
                            ))}
                          </select>
                          {chosenC && (
                            <span className={s.pickWhy}>
                              {chosenC.rank}순위 · {chosenC.reason}
                            </span>
                          )}
                          {x.candidates.length === 0 && (
                            <span className={s.none}>이 시간에 가능한 선생님이 없습니다</span>
                          )}
                        </>
                      )}
                    </td>

                    <td className={s.stateCol}>
                      <span className={`${s.state} ${s[`state_${st}`]}`}>{STATE_LABEL[st]}</span>
                      {flag === 'DUPLICATE' && (
                        <span
                          className={s.dupTag}
                          title={`${chosenC?.name ?? '이 선생님'}이 이 날 이 묶음에서 ${nth}번째로 들어갑니다`}
                        >
                          중복 {nth > 1 ? `(${nth}번째)` : ''}
                        </span>
                      )}
                      {flag === 'OVERLAP' && <span className={s.clashTag}>시각 겹침</span>}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}

      {plan && plan.slots.length > 0 && (
        <p className={s.foot}>
          각 칸에 추천 1순위를 골라 두었습니다. 같은 선생님이 여러 교시에 들어가도 되며, 그럴
          때는 <b>중복</b>으로만 표시합니다{duplicateCount > 0 ? ` (지금 ${duplicateCount}칸)` : ''}
          . 프로그램은 추천만 하며 [배정]을 눌러야 저장됩니다. 한 칸이라도 배정할 수 없으면{' '}
          <b>아무것도 저장하지 않습니다.</b>
        </p>
      )}
    </Modal>
  )
}
