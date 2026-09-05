import { useEffect, useMemo, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Icon } from '@/components/Icon'
import { Button, Notice } from '@/components/ui'
import { DAY_LABEL, minToHm } from '@/ipc/bell'
import {
  BUSY_KIND_LABEL,
  findApi,
  type Candidate,
  type FindResult,
  type InCharge,
  type SlotKind,
  type SlotNotice,
} from '@/ipc/find'
import { assignApi, type AbsenceRow, type AssignSaved } from '@/ipc/assign'
import { errorDetail, errorMessage } from '@/ipc/invoke'
import { priorityApi } from '@/ipc/priority'
import { useRemembered } from '@/lib/remember'
import { AbsenceDeleteModal } from '@/features/assign/AbsenceDeleteModal'
import { AbsenceModal } from '@/features/assign/AbsenceModal'
import { AssignConfirmModal } from '@/features/assign/AssignConfirmModal'
import { BatchAssignModal } from '@/features/assign/BatchAssignModal'
import s from './FindPanel.module.css'

function todayStr(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

function shiftDate(date: string, days: number): string {
  const d = new Date(`${date}T00:00:00`)
  d.setDate(d.getDate() + days)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

/** 화면에서 바로 계산하는 요일 (1=월 … 7=일). 저장은 백엔드가 다시 계산한다. */
function weekdayOf(date: string): number {
  const d = new Date(`${date}T00:00:00`)
  const js = d.getDay() // 0=일
  return js === 0 ? 7 : js
}

export function FindPanel() {
  const { data: opt, isLoading, error } = useQuery({
    queryKey: ['find-options'],
    queryFn: findApi.options,
  })

  // 추천 기준이 바뀌면 결과를 다시 받아 즉시 새 순서로 보여 준다
  const { data: priority } = useQuery({
    queryKey: ['priority-rules'],
    queryFn: priorityApi.list,
  })

  const [date, setDate] = useState(todayStr)
  // 날짜는 늘 오늘로 시작하지만, 보던 학급은 기억해 둔다
  const [grade, setGrade] = useRemembered<number | null>('find.grade', null)
  const [classId, setClassId] = useRemembered<number | null>('find.classId', null)
  const [slotType, setSlotType] = useState<SlotKind>('PERIOD')
  const [periodNo, setPeriodNo] = useState<number | null>(null)
  const [result, setResult] = useState<FindResult | null>(null)
  const [showExcluded, setShowExcluded] = useState(false)
  const [showDebug, setShowDebug] = useState(false)

  // Phase 8 — 배정
  const qc = useQueryClient()
  const [absenceOpen, setAbsenceOpen] = useState(false)
  /** 삭제 확인 중인 결근 */
  const [absenceToDelete, setAbsenceToDelete] = useState<AbsenceRow | null>(null)
  /** 다른 날짜에 결근을 등록했을 때 그 날로 건너뛸 수 있게 알려 준다 */
  const [absenceSaved, setAbsenceSaved] = useState<{ date: string; name: string } | null>(null)
  /** 다건 배정 창. 특정 선생님으로 바로 열 때는 그 id를 담는다 */
  const [batch, setBatch] = useState<{ teacherId: number | null } | null>(null)
  const [picked, setPicked] = useState<Candidate | null>(null)
  const [done, setDone] = useState<AssignSaved | null>(null)

  const dayOfWeek = weekdayOf(date)

  const grades = useMemo(
    () => [...new Set(opt?.classes.map((c) => c.grade) ?? [])].sort((a, b) => a - b),
    [opt],
  )

  // 기억해 둔 학급으로 열되, 그 사이 없어졌으면(새 학기 등) 첫 학급으로 되돌린다
  useEffect(() => {
    if (!opt || opt.classes.length === 0) return
    const kept = opt.classes.find((c) => c.classId === classId)
    if (kept) {
      if (kept.grade !== grade) setGrade(kept.grade)
      return
    }
    const first = opt.classes.find((c) => c.grade === grade) ?? opt.classes[0]
    setGrade(first.grade)
    setClassId(first.classId)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [opt])

  const classesOfGrade = useMemo(
    () => opt?.classes.filter((c) => c.grade === grade) ?? [],
    [opt, grade],
  )

  // 이 학년·요일에 고를 수 있는 시간
  const timeChoices = useMemo(() => {
    if (!opt || grade == null) return []
    return opt.slots
      .filter((x) => x.grade === grade && x.dayOfWeek === dayOfWeek)
      .sort((a, b) => a.startMin - b.startMin)
  }, [opt, grade, dayOfWeek])

  const chosenTime = timeChoices.find(
    (x) => x.slotType === slotType && (x.periodNo ?? null) === (periodNo ?? null),
  )

  // 학년이나 날짜가 바뀌어 고른 시간이 없어지면 비운다
  useEffect(() => {
    if (timeChoices.length === 0) return
    if (!chosenTime) {
      const first = timeChoices.find((x) => x.slotType === 'PERIOD') ?? timeChoices[0]
      setSlotType(first.slotType)
      setPeriodNo(first.periodNo)
    }
  }, [timeChoices, chosenTime])

  // 조회 버튼을 누르기 전에 '이 시간을 누가 담당하는지'를 미리 알려 준다.
  // 전담 시간인데 모르고 보결을 배정하는 일을 막는 것이 목적이다.
  const { data: charge } = useQuery({
    queryKey: ['find-in-charge', date, classId, slotType, periodNo],
    queryFn: () =>
      findApi.inCharge({
        date,
        classId: classId!,
        slotType,
        periodNo: slotType === 'PERIOD' ? periodNo : null,
      }),
    enabled: classId != null && !!chosenTime,
  })

  // 이 날 등록된 결근이 있으면 알려 준다 (등록은 배정 내역 화면에서 한다)
  const { data: absences } = useQuery({
    queryKey: ['absences', date],
    queryFn: () => assignApi.absenceList(date),
  })

  const search = useMutation({
    mutationFn: () =>
      findApi.candidates({
        date,
        classId: classId!,
        slotType,
        periodNo: slotType === 'PERIOD' ? periodNo : null,
      }),
    onSuccess: (r) => {
      setResult(r)
      setShowExcluded(false)
      setDone(null)
    },
  })

  // 추천 기준이 바뀌면(설정 화면에서 저장) 지금 결과를 새 기준으로 다시 정렬한다
  const priorityStamp = useMemo(
    () =>
      (priority?.rules ?? [])
        .filter((r) => r.enabled)
        .map((r) => r.ruleKey)
        .join('>'),
    [priority],
  )
  const lastStamp = useRef<string | null>(null)
  useEffect(() => {
    if (!result) {
      lastStamp.current = priorityStamp
      return
    }
    if (lastStamp.current !== null && lastStamp.current !== priorityStamp) {
      search.mutate()
    }
    lastStamp.current = priorityStamp
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [priorityStamp])

  const changeGrade = (g: number) => {
    setGrade(g)
    const first = opt?.classes.find((c) => c.grade === g)
    setClassId(first?.classId ?? null)
    setResult(null)
  }

  if (isLoading) return <div className={s.center}>불러오는 중…</div>
  if (error || !opt) {
    return (
      <Notice tone="danger">
        <div>
          <p>{errorMessage(error)}</p>
          {errorDetail(error) && (
            <details className={s.detailBox}>
              <summary>자세히</summary>
              <pre className="selectable">{errorDetail(error)}</pre>
            </details>
          )}
        </div>
      </Notice>
    )
  }

  const canSearch = classId != null && !!chosenTime && !search.isPending

  return (
    <div
      className={s.wrap}
      onKeyDown={(e) => {
        // 조건을 고른 뒤 Enter 를 누르면 바로 조회한다
        const tag = (e.target as HTMLElement).tagName
        if (e.key === 'Enter' && canSearch && tag !== 'BUTTON' && tag !== 'TEXTAREA') {
          e.preventDefault()
          search.mutate()
        }
      }}
    >
      {/* 날짜만 정하고 바로 들어가면 되는 자리 */}
      <div className={s.topBar}>
        <Button variant="ghost" icon="plus" onClick={() => setAbsenceOpen(true)}>
          결근 등록
        </Button>
        <Button variant="accent" icon="plus" onClick={() => setBatch({ teacherId: null })}>
          다건 배정
        </Button>
      </div>

      {/* 오늘이 아닌 날짜에 등록했으면 그 날로 갈 수 있게 알려 준다 */}
      {absenceSaved && absenceSaved.date !== date && (
        <div className={s.jump} role="status">
          <span>
            <strong>{absenceSaved.name}</strong> 선생님의{' '}
            <span className="num">{absenceSaved.date}</span> 결근을 등록했습니다.
          </span>
          <button
            type="button"
            className={s.jumpBtn}
            onClick={() => {
              setDate(absenceSaved.date)
              setResult(null)
              setAbsenceSaved(null)
            }}
          >
            그 날짜로 이동
          </button>
          <button
            type="button"
            className={s.jumpClose}
            onClick={() => setAbsenceSaved(null)}
            aria-label="닫기"
          >
            ×
          </button>
        </div>
      )}

      {opt.blockers.length > 0 && (
        <Notice tone="warn">
          <div>
            <strong>결과가 정확하지 않을 수 있습니다</strong>
            <ul className={s.list}>
              {opt.blockers.map((b) => (
                <li key={b}>{b}</li>
              ))}
            </ul>
          </div>
        </Notice>
      )}

      {/* ---------- 조회 조건 ---------- */}
      <section className={s.card}>
        <div className={s.row}>
          <label className={s.field}>
            <span className={s.label}>날짜</span>
            <div className={s.dateBox}>
              <input
                type="date"
                className={s.dateInput}
                value={date}
                onChange={(e) => {
                  setDate(e.target.value || todayStr())
                  setResult(null)
                }}
              />
              <span className={s.weekday}>{DAY_LABEL[dayOfWeek]}요일</span>
            </div>
          </label>

          <div className={s.quickDates}>
            <button
              type="button"
              className={s.quickBtn}
              onClick={() => {
                setDate(todayStr())
                setResult(null)
              }}
            >
              오늘
            </button>
            <button
              type="button"
              className={s.quickBtn}
              onClick={() => {
                setDate(shiftDate(todayStr(), 1))
                setResult(null)
              }}
            >
              내일
            </button>
            <button
              type="button"
              className={s.quickBtn}
              onClick={() => {
                setDate(shiftDate(date, -1))
                setResult(null)
              }}
              aria-label="하루 전"
            >
              ‹
            </button>
            <button
              type="button"
              className={s.quickBtn}
              onClick={() => {
                setDate(shiftDate(date, 1))
                setResult(null)
              }}
              aria-label="하루 뒤"
            >
              ›
            </button>
          </div>
        </div>

        <div className={s.row}>
          <div className={s.field}>
            <span className={s.label}>학년</span>
            <div className={s.chips}>
              {grades.map((g) => (
                <button
                  key={g}
                  type="button"
                  className={g === grade ? `${s.chip} ${s.chipOn}` : s.chip}
                  onClick={() => changeGrade(g)}
                >
                  {g}학년
                </button>
              ))}
            </div>
          </div>

          <div className={s.field}>
            <span className={s.label}>반</span>
            <div className={s.chips}>
              {classesOfGrade.map((c) => (
                <button
                  key={c.classId}
                  type="button"
                  className={c.classId === classId ? `${s.chip} ${s.chipOn}` : s.chip}
                  onClick={() => {
                    setClassId(c.classId)
                    setResult(null)
                  }}
                  title={c.homeroomName ? `담임 ${c.homeroomName}` : '담임 미지정'}
                >
                  {c.name ?? `${c.classNo}반`}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className={s.field}>
          <span className={s.label}>보결 시간</span>
          {timeChoices.length === 0 ? (
            <p className={s.warnText}>
              {grade}학년은 {DAY_LABEL[dayOfWeek]}요일에 수업이 없습니다. 다른 날짜나 학년을
              선택해 주세요.
            </p>
          ) : (
            <>
              <div className={s.chips}>
                {timeChoices.map((x) => {
                  const on =
                    x.slotType === slotType && (x.periodNo ?? null) === (periodNo ?? null)
                  const isLunch = x.slotType === 'LUNCH'
                  return (
                    <button
                      key={`${x.slotType}:${x.periodNo ?? 'L'}`}
                      type="button"
                      className={[
                        s.chip,
                        isLunch ? s.chipLunch : '',
                        on ? (isLunch ? s.chipLunchOn : s.chipOn) : '',
                      ]
                        .filter(Boolean)
                        .join(' ')}
                      onClick={() => {
                        setSlotType(x.slotType)
                        setPeriodNo(x.periodNo)
                        setResult(null)
                      }}
                    >
                      {isLunch ? x.label : `${x.periodNo}`}
                    </button>
                  )
                })}
              </div>
              {chosenTime && (
                <p className={s.timeHint}>
                  <strong>
                    {minToHm(chosenTime.startMin)} ~ {minToHm(chosenTime.endMin)}
                  </strong>{' '}
                  이 시각을 기준으로 판단합니다
                  {charge && <InChargeText ic={charge.slot.inCharge} />}
                </p>
              )}
            </>
          )}
        </div>

        <div className={s.searchRow}>
          <Button variant="primary" icon="search" disabled={!canSearch} onClick={() => search.mutate()}>
            {search.isPending ? '찾는 중…' : '보결 가능 교사 조회'}
          </Button>
        </div>

        {/* 이 날 등록된 결근 — 눌러서 그 선생님의 다건 배정으로 바로 간다 */}
        {absences && absences.length > 0 && (
          <div className={s.absenceRow}>
            <span className={s.absenceLabel}>이 날 결근</span>
            <div className={s.absenceList}>
              {absences.map((a) => (
                <span key={a.id} className={s.absenceChip}>
                  <button
                    type="button"
                    className={s.absencePick}
                    onClick={() => setBatch({ teacherId: a.teacherId })}
                    title="이 선생님의 그 날 보결을 한 번에 배정합니다"
                  >
                    {a.teacherName}
                    <span className={s.absenceWhy}>
                      {a.reasonLabel}
                      {a.isAllDay
                        ? ' · 종일'
                        : ` · ${minToHm(a.startMin ?? 0)}~${minToHm(a.endMin ?? 0)}`}
                    </span>
                  </button>
                  <button
                    type="button"
                    className={s.absenceDel}
                    onClick={() => setAbsenceToDelete(a)}
                    title={`${a.teacherName} 선생님의 결근을 삭제합니다`}
                    aria-label={`${a.teacherName} 선생님 결근 삭제`}
                  >
                    ×
                  </button>
                </span>
              ))}
              <span className={s.absenceHint}>선생님을 누르면 바로 다건 배정으로 갑니다</span>
            </div>
          </div>
        )}
        {charge?.notice && !result && <NoticeBox n={charge.notice} />}
      </section>

      {search.error && (
        <Notice tone="danger">
          <div>
            <p>{errorMessage(search.error)}</p>
            {errorDetail(search.error) && (
              <details className={s.detailBox}>
                <summary>자세히</summary>
                <pre className="selectable">{errorDetail(search.error)}</pre>
              </details>
            )}
          </div>
        </Notice>
      )}

      {done && (
        <div className={s.saved} role="status">
          <span className={s.savedIcon} aria-hidden="true">
            <Icon name="check" size={18} />
          </span>
          <div>
            <p className={s.savedTitle}>
              {done.classFullLabel} {done.slotLabel} 보결을 <strong>{done.subTeacherName}</strong>{' '}
              선생님으로 배정했습니다.
            </p>
            <p className={s.savedBody}>
              <span className="num">
                {minToHm(done.startMin)}~{minToHm(done.endMin)}
              </span>
              {' · '}
              배정 내역 화면에서 확인하거나 취소할 수 있습니다.
            </p>
          </div>
          <button type="button" className={s.savedClose} onClick={() => setDone(null)}>
            닫기
          </button>
        </div>
      )}

      {/* ---------- 결과 ---------- */}
      {result && (
        <>
          <div className={s.resultHead}>
            <p className={s.resultTitle}>
              {result.slot.classFullLabel} · {DAY_LABEL[result.slot.dayOfWeek]}요일{' '}
              {result.slot.slotLabel}
              <span className={s.resultTime}>
                {minToHm(result.slot.startMin)}~{minToHm(result.slot.endMin)}
              </span>
            </p>
            <p className={s.resultCount}>
              <strong className={s.okCount}>가능 {result.eligible.length}명</strong>
              <span className={s.sep}>/</span>
              <span>불가 {result.excluded.length}명</span>
            </p>
          </div>

          {result.notice && <NoticeBox n={result.notice} />}

          {result.warnings.map((w) => (
            <Notice key={w} tone="warn">
              {w}
            </Notice>
          ))}

          {result.eligible.length > 0 && (
            <div className={s.tableCard}>
              <table className={s.table}>
                <thead>
                  <tr>
                    <th className={s.rankCol}>순위</th>
                    <th>교사</th>
                    <th>구분</th>
                    <th>담당</th>
                    <th>현재 상태</th>
                    <th className={s.numCol}>오늘</th>
                    <th className={s.numCol}>이번 달</th>
                    <th className={s.numCol}>누적</th>
                    <th>추천 근거</th>
                    <th className={s.assignCol} />
                  </tr>
                </thead>
                <tbody>
                  {result.eligible.map((c) => (
                    <Row
                      key={c.teacherId}
                      c={c}
                      showDebug={showDebug}
                      onAssign={() => setPicked(c)}
                    />
                  ))}
                </tbody>
              </table>
              <p className={s.sortHint}>
                {priorityStamp
                  ? `추천 순서 기준: ${(priority?.rules ?? [])
                      .filter((r) => r.enabled)
                      .map((r, i) => `${i + 1}. ${r.label}`)
                      .join(' → ')}`
                  : '추천 기준이 설정되지 않았습니다.'}
                {'  '}
                프로그램은 추천만 하며, 최종 배정은 담당자가 결정합니다.
              </p>
            </div>
          )}

          <div className={s.foldRow}>
            <button
              type="button"
              className={s.foldBtn}
              onClick={() => setShowExcluded((v) => !v)}
            >
              {showExcluded ? '▾' : '▸'} 배정할 수 없는 선생님 {result.excluded.length}명
            </button>
            <label className={s.debugToggle}>
              <input
                type="checkbox"
                checked={showDebug}
                onChange={(e) => setShowDebug(e.target.checked)}
              />
              <span>개발자 확인</span>
            </label>
          </div>

          {showExcluded && (
            <div className={s.tableCard}>
              <table className={s.table}>
                <thead>
                  <tr>
                    <th>교사</th>
                    <th>구분</th>
                    <th>담당</th>
                    <th>제외 이유</th>
                    <th>겹치는 일정</th>
                  </tr>
                </thead>
                <tbody>
                  {result.excluded.map((c) => (
                    <tr key={c.teacherId} className={s.rowOut}>
                      <td className={s.name}>{c.name}</td>
                      <td>
                        <span className={`${s.roleTag} ${s[`role_${c.roleCode}`]}`}>
                          {c.roleLabel}
                        </span>
                      </td>
                      <td className={s.duty}>{c.duty}</td>
                      <td>
                        <span className={s.outTag}>{c.statusLabel}</span>
                        {showDebug && <code className={s.code}>{c.reasonCode}</code>}
                      </td>
                      <td className={s.detailCell}>{c.detail ?? '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {showDebug && (
            <details className={s.debugBox} open>
              <summary>개발자 확인 — 그 날 교사별 일정</summary>
              <div className={s.debugList}>
                {[...result.eligible, ...result.excluded].map((c) => (
                  <div key={c.teacherId} className={s.debugItem}>
                    <span className={s.debugName}>
                      {c.name} <code className={s.code}>{c.reasonCode}</code>
                    </span>
                    {c.blocks.length === 0 ? (
                      <span className={s.debugNone}>일정 없음</span>
                    ) : (
                      <span className={s.debugBlocks}>
                        {c.blocks.map((b, i) => (
                          <span key={i} className={s.debugBlock}>
                            {minToHm(b.startMin)}~{minToHm(b.endMin)} {BUSY_KIND_LABEL[b.kind]}{' '}
                            {b.label}
                          </span>
                        ))}
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </details>
          )}
        </>
      )}

      <AbsenceModal
        open={absenceOpen}
        date={date}
        teachers={opt.teachers}
        reasons={opt.reasons}
        onClose={() => setAbsenceOpen(false)}
        onSaved={(_tid, savedDate, name) => setAbsenceSaved({ date: savedDate, name })}
      />

      {absenceToDelete && (
        <AbsenceDeleteModal
          row={absenceToDelete}
          onClose={() => setAbsenceToDelete(null)}
          onDeleted={() => {
            setAbsenceToDelete(null)
            setResult(null) // 후보가 달라지므로 조회 결과를 비운다
          }}
        />
      )}

      {batch && (
        <BatchAssignModal
          open
          date={date}
          opt={opt}
          defaultClassId={classId}
          defaultTeacherId={batch.teacherId}
          onClose={() => setBatch(null)}
          onSaved={async () => {
            await qc.invalidateQueries({ queryKey: ['absences', date] })
            setResult(null)
          }}
        />
      )}

      {picked && result && (
        <AssignConfirmModal
          open
          result={result}
          candidate={picked}
          onClose={() => setPicked(null)}
          onSaved={(saved) => {
            setPicked(null)
            setDone(saved)
            search.mutate() // 방금 배정한 결과를 반영해 다시 조회한다
          }}
        />
      )}

      {!result && !search.isPending && (
        <div className={s.empty}>
          <div className={s.emptyIcon} aria-hidden="true">
            <Icon name="search" size={26} />
          </div>
          <p className={s.emptyTitle}>조건을 고르고 조회해 보세요</p>
          <p className={s.emptyDesc}>
            그 시간에 실제로 수업이나 다른 일정이 없는 선생님만 찾아 드립니다. 교시 번호가 아니라
            실제 시작·종료 시각으로 판단하므로 학년마다 시간이 달라도 정확합니다.
          </p>
        </div>
      )}
    </div>
  )
}

/** '이 시간 담당: 김민수 (전담 · 영어)' */
function InChargeText({ ic }: { ic: InCharge }) {
  if (!ic.name) {
    return <span className={s.chargeHint}>이 시간 담당: 담임 미지정</span>
  }
  const paren = [ic.roleLabel, ic.subjectName].filter(Boolean).join(' · ')
  return (
    <span className={ic.coveredByOther ? `${s.chargeHint} ${s.chargeOther}` : s.chargeHint}>
      이 시간 담당: <strong>{ic.name}</strong>
      {paren && ` (${paren})`}
    </span>
  )
}

/** 배정 전에 꼭 확인할 알림. 전담 시간에 헛배정하는 것을 막는다. */
function NoticeBox({ n }: { n: SlotNotice }) {
  return (
    <div className={s.notice} role="alert">
      <span className={s.noticeIcon} aria-hidden="true">
        <Icon name="warning" size={18} />
      </span>
      <div>
        <p className={s.noticeTitle}>{n.title}</p>
        <p className={s.noticeBody}>{n.body}</p>
      </div>
    </div>
  )
}

function Row({
  c,
  showDebug,
  onAssign,
}: {
  c: Candidate
  showDebug: boolean
  onAssign: () => void
}) {
  const rankClass = [s.rank, c.rank === 1 ? s.rank1 : '', c.rank <= 3 ? s.rankTop : '']
    .filter(Boolean)
    .join(' ')

  return (
    <tr className={c.rank === 1 ? `${s.rowOk} ${s.rowTop}` : s.rowOk}>
      <td className={s.rankCol}>
        <span className={rankClass}>{c.rank}</span>
      </td>
      <td className={s.name}>{c.name}</td>
      <td>
        <span className={`${s.roleTag} ${s[`role_${c.roleCode}`]}`}>{c.roleLabel}</span>
      </td>
      <td className={s.duty}>{c.duty}</td>
      <td>
        <span className={s.okTag} title={c.detail ?? undefined}>
          {c.statusLabel}
        </span>
        {showDebug && <code className={s.code}>{c.reasonCode}</code>}
      </td>
      <td className={`${s.numCol} num`}>{c.counts.today}</td>
      <td className={`${s.numCol} num`}>{c.counts.month}</td>
      <td className={`${s.numCol} num`}>{c.counts.total}</td>
      <td
        className={s.reasonCell}
        title={
          c.decidedBy ? `다음 순위와의 차이: ${c.decidedBy}` : undefined
        }
      >
        {c.reason}
        {c.decidedBy && <span className={s.decidedBy}>{c.decidedBy}</span>}
      </td>
      <td className={s.assignCol}>
        <button type="button" className={s.assignBtn} onClick={onAssign}>
          배정
        </button>
      </td>
    </tr>
  )
}
