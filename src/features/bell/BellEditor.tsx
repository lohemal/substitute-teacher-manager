import { useEffect, useMemo, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Icon } from '@/components/Icon'
import { Button, Notice } from '@/components/ui'
import { NumberInput } from '@/components/NumberInput'
import { TimeInput } from '@/components/TimeInput'
import {
  bellApi,
  DAY_LABEL,
  minToHm,
  type BellOverview,
  type BellTemplate,
  type ReflowParams,
  type Slot,
} from '@/ipc/bell'
import { errorMessage } from '@/ipc/invoke'
import {
  copyDay,
  derive,
  DEFAULT_LENGTHS,
  emptyState,
  expand,
  maxPeriod,
  mergeLengths,
  richestDayOf,
  toCommon,
  toPerDay,
  type EditorState,
  type Lengths,
} from './bellModel'
import { useRemembered } from '@/lib/remember'
import { SlotRows } from './SlotRows'
import s from './BellEditor.module.css'

const TEMPLATES: { key: BellTemplate; title: string; desc: string; detail: string }[] = [
  {
    key: 'SAME',
    title: '전 학년 같음',
    desc: '모든 학년의 교시 시각과 점심시간이 같습니다',
    detail: '시정표 1개',
  },
  {
    key: 'TWO',
    title: '저학년 · 고학년',
    desc: '저학년이 먼저 점심을 먹는 등 두 가지로 나뉩니다',
    detail: '시정표 2개',
  },
  {
    key: 'THREE',
    title: '저 · 중 · 고학년',
    desc: '학년군마다 시각이 다릅니다',
    detail: '시정표 3개',
  },
]

export function BellEditor() {
  const qc = useQueryClient()
  const { data: ov, isLoading, error } = useQuery({
    queryKey: ['bell-overview'],
    queryFn: bellApi.overview,
  })

  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [state, setState] = useState<EditorState | null>(null)
  const [grades, setGrades] = useState<number[]>([])
  const [dirty, setDirty] = useState(false)
  const [problems, setProblems] = useState<string[]>([])
  const [saved, setSaved] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)

  /**
   * 시각 길이 — 화면 전체가 이 하나만 쓴다.
   *
   * 사용자가 마지막에 정한 값을 브라우저 저장소에 기억해 둔다. 그래서 설정
   * 단계를 옮기거나 앱을 다시 켜도 같은 값이 남는다 (자료 파일과는 무관하다).
   */
  const [rememberedLengths, setRememberedLengths] = useRemembered<Lengths>(
    'bell.lengths',
    DEFAULT_LENGTHS,
  )

  const schoolDays = ov?.schoolDays ?? [1, 2, 3, 4, 5]
  const profile = ov?.profiles.find((p) => p.id === selectedId) ?? null

  // 처음 열리거나 유형을 바꿀 때 편집 상태를 다시 만든다
  const loadedFor = useRef<number | null>(null)
  useEffect(() => {
    if (!ov) return
    const target =
      ov.profiles.find((p) => p.id === selectedId) ?? ov.profiles[0] ?? null
    if (!target) {
      loadedFor.current = null
      setState(null)
      return
    }
    if (loadedFor.current === target.id && dirty) return
    loadedFor.current = target.id
    setSelectedId(target.id)
    setState(
      target.slots.length > 0
        ? derive(target.slots, ov.schoolDays)
        : emptyState(ov.schoolDays),
    )
    setGrades(target.grades)
    setDirty(false)
  }, [ov, selectedId, dirty])

  /**
   * 화면 전체가 쓰는 길이는 이 값 하나다.
   *
   * 시정표를 새로 불러올 때 **그 자료에 들어 있는 길이를 한 번 받아들이고**,
   * 그 뒤 사용자가 고친 값은 자료가 덮어쓰지 않는다. 그래서 일괄 조정에서
   * 중간놀이를 20분으로 바꾸면 [중간놀이 추가]도 20분을 넣는다.
   */
  const lengths = rememberedLengths
  const adoptedFor = useRef<number | null>(null)
  useEffect(() => {
    if (!state || !profile) return
    if (adoptedFor.current === profile.id) return
    adoptedFor.current = profile.id
    const rows = state.perDay
      ? (state.byDay[richestDayOf(state, schoolDays)] ?? [])
      : state.base
    setRememberedLengths(mergeLengths(rows, rememberedLengths))
  }, [state, profile, schoolDays, rememberedLengths, setRememberedLengths])

  const slots: Slot[] = useMemo(
    () => (state ? expand(state, schoolDays) : []),
    [state, schoolDays],
  )

  // 저장 전에 문제점을 미리 보여준다 (검증 규칙은 Rust 한 곳에만 있다)
  useEffect(() => {
    if (!state) return
    const t = setTimeout(() => {
      bellApi.checkSlots(slots).then(setProblems).catch(() => setProblems([]))
    }, 350)
    return () => clearTimeout(t)
  }, [slots, state])

  const refresh = (next: BellOverview) => {
    qc.setQueryData(['bell-overview'], next)
    qc.invalidateQueries({ queryKey: ['setup-state'] })
  }

  const applyTemplate = useMutation({
    mutationFn: bellApi.applyTemplate,
    onSuccess: (next) => {
      loadedFor.current = null
      setDirty(false)
      setSelectedId(next.profiles[0]?.id ?? null)
      refresh(next)
    },
  })

  const createProfile = useMutation({
    mutationFn: () => bellApi.createProfile(`새 시정표 ${(ov?.profiles.length ?? 0) + 1}`),
    onSuccess: (next) => {
      const added = next.profiles[next.profiles.length - 1]
      loadedFor.current = null
      setDirty(false)
      setSelectedId(added?.id ?? null)
      refresh(next)
    },
  })

  const renameProfile = useMutation({
    mutationFn: (name: string) => bellApi.renameProfile(selectedId!, name),
    onSuccess: refresh,
  })

  const deleteProfile = useMutation({
    mutationFn: () => bellApi.deleteProfile(selectedId!),
    onSuccess: (next) => {
      loadedFor.current = null
      setDirty(false)
      setSelectedId(next.profiles[0]?.id ?? null)
      refresh(next)
    },
  })

  const save = useMutation({
    mutationFn: () => bellApi.saveProfile(selectedId!, grades, slots),
    onSuccess: (next) => {
      setDirty(false)
      setSaved(true)
      setTimeout(() => setSaved(false), 2500)
      refresh(next)
    },
  })

  const reflow = useMutation({
    mutationFn: (v: { ids: number[]; params: Parameters<typeof bellApi.reflow>[1] }) =>
      bellApi.reflow(v.ids, v.params),
    onSuccess: (next) => {
      loadedFor.current = null
      setDirty(false)
      setSaved(true)
      setTimeout(() => setSaved(false), 2500)
      refresh(next)
    },
  })

  const generate = useMutation({
    mutationFn: bellApi.generate,
    onSuccess: (made) => {
      if (!ov) return
      setState(derive(made, ov.schoolDays))
      setDirty(true)
    },
  })

  /* ---------------- 화면 ---------------- */

  if (isLoading) return <div className={s.center}>불러오는 중…</div>
  if (error || !ov) return <Notice tone="danger">{errorMessage(error)}</Notice>

  // 아직 시정표가 하나도 없으면 템플릿부터 고르게 한다
  if (ov.profiles.length === 0) {
    return (
      <div className={s.wrap}>
        <Notice tone="info">
          먼저 우리 학교가 <strong>몇 가지 시정표를 쓰는지</strong> 골라 주세요. 기본 시각을 자동으로
          채워 드리고, 그다음 필요한 부분만 고치면 됩니다.
        </Notice>
        {applyTemplate.error && <Notice tone="danger">{errorMessage(applyTemplate.error)}</Notice>}

        <div className={s.templateGrid}>
          {TEMPLATES.map((t) => (
            <button
              key={t.key}
              type="button"
              className={s.templateCard}
              disabled={applyTemplate.isPending}
              onClick={() => applyTemplate.mutate(t.key)}
            >
              <span className={s.templateTitle}>{t.title}</span>
              <span className={s.templateDesc}>{t.desc}</span>
              <span className={s.templateDetail}>{t.detail}</span>
            </button>
          ))}
        </div>
        <p className={s.templateHint}>
          나중에 유형을 더 만들거나 지울 수 있습니다. 지금 정확히 몰라도 괜찮습니다.
        </p>
      </div>
    )
  }

  const setRows = (updater: (st: EditorState) => EditorState) => {
    setState((st) => (st ? updater(st) : st))
    setDirty(true)
  }

  const switchProfile = (id: number) => {
    if (dirty) return
    loadedFor.current = null
    setSelectedId(id)
  }

  const toggleGrade = (g: number) => {
    setGrades((gs) => (gs.includes(g) ? gs.filter((x) => x !== g) : [...gs, g].sort((a, b) => a - b)))
    setDirty(true)
  }

  const ownerOf = (g: number) => ov.profiles.find((p) => p.grades.includes(g))

  return (
    <div className={s.wrap}>
      {ov.unassignedGrades.length > 0 && (
        <Notice tone="warn">
          {ov.unassignedGrades.map((g) => `${g}학년`).join(', ')}에 사용할 시정표가 아직 정해지지
          않았습니다. 아래 <strong>적용 학년</strong>에서 선택해 주세요.
        </Notice>
      )}

      {/* ---- 유형 목록 ---- */}
      <div className={s.profileBar}>
        {ov.profiles.map((p) => (
          <button
            key={p.id}
            type="button"
            className={p.id === selectedId ? `${s.profileChip} ${s.profileChipOn}` : s.profileChip}
            onClick={() => switchProfile(p.id)}
            disabled={dirty && p.id !== selectedId}
            title={dirty && p.id !== selectedId ? '먼저 변경 내용을 저장하거나 되돌려 주세요' : undefined}
          >
            <span className={s.profileName}>{p.name}</span>
            <span className={s.profileGrades}>
              {p.grades.length > 0 ? p.grades.map((g) => `${g}학년`).join(' ') : '학년 미지정'}
            </span>
          </button>
        ))}
        <button
          type="button"
          className={s.profileAdd}
          onClick={() => createProfile.mutate()}
          disabled={dirty || createProfile.isPending}
        >
          <Icon name="plus" size={16} /> 유형 추가
        </button>
      </div>

      {dirty && (
        <Notice tone="info">
          저장하지 않은 변경 내용이 있습니다. 다른 유형으로 옮기려면 먼저 저장하거나 되돌려 주세요.
        </Notice>
      )}

      {state && profile && (
        <>
          {/* ---- 이름 · 적용 학년 ---- */}
          <section className={s.card}>
            <div className={s.cardHead}>
              <input
                key={profile.id}
                className={s.nameInput}
                defaultValue={profile.name}
                onBlur={(e) => {
                  const v = e.target.value.trim()
                  if (v && v !== profile.name) renameProfile.mutate(v)
                  else e.target.value = profile.name
                }}
                aria-label="시정표 유형 이름"
                maxLength={30}
              />
              {confirmDelete ? (
                <span className={s.confirmBar}>
                  <span className={s.confirmText}>정말 지울까요?</span>
                  <button
                    type="button"
                    className={s.confirmYes}
                    onClick={() => {
                      setConfirmDelete(false)
                      deleteProfile.mutate()
                    }}
                    disabled={deleteProfile.isPending}
                  >
                    지우기
                  </button>
                  <button
                    type="button"
                    className={s.confirmNo}
                    onClick={() => setConfirmDelete(false)}
                  >
                    취소
                  </button>
                </span>
              ) : (
                <button
                  type="button"
                  className={s.deleteLink}
                  onClick={() => setConfirmDelete(true)}
                >
                  이 유형 지우기
                </button>
              )}
            </div>
            {deleteProfile.error && <Notice tone="danger">{errorMessage(deleteProfile.error)}</Notice>}
            {renameProfile.error && <Notice tone="danger">{errorMessage(renameProfile.error)}</Notice>}

            <div className={s.fieldRow}>
              <span className={s.fieldLabel}>적용 학년</span>
              <div className={s.gradeChips}>
                {Array.from({ length: ov.maxGrade - ov.minGrade + 1 }, (_, i) => ov.minGrade + i).map(
                  (g) => {
                    const on = grades.includes(g)
                    const owner = ownerOf(g)
                    const takenByOther = !on && owner && owner.id !== profile.id
                    return (
                      <button
                        key={g}
                        type="button"
                        className={on ? `${s.gradeChip} ${s.gradeChipOn}` : s.gradeChip}
                        onClick={() => toggleGrade(g)}
                        title={takenByOther ? `지금은 '${owner!.name}'를 쓰고 있습니다` : undefined}
                      >
                        {g}학년
                        {takenByOther && <span className={s.gradeTaken}>{owner!.name}</span>}
                      </button>
                    )
                  },
                )}
              </div>
            </div>
            <p className={s.fieldHint}>
              한 학년은 하나의 시정표만 씁니다. 다른 유형을 쓰던 학년을 고르면 이쪽으로 옮겨집니다.
            </p>
          </section>

          {/* ---- 시간 일괄 조정 ---- */}
          <BulkAdjust
            profiles={ov.profiles}
            currentId={profile.id}
            lengths={lengths}
            onLengths={setRememberedLengths}
            dirty={dirty}
            pending={reflow.isPending}
            error={reflow.error ? errorMessage(reflow.error) : null}
            onApply={(ids, params) => reflow.mutate({ ids, params })}
          />

          {/* ---- 처음부터 다시 만들기 ---- */}
          <QuickFill
            schoolDays={schoolDays}
            lengths={lengths}
            onLengths={setRememberedLengths}
            pending={generate.isPending}
            error={generate.error ? errorMessage(generate.error) : null}
            onGenerate={(params) => generate.mutate(params)}
          />

          {/* ---- 시각 편집 ---- */}
          <section className={s.card}>
            <div className={s.modeBar}>
              <h3 className={s.cardTitle}>
                {state.perDay ? '요일별 시각' : '교시별 시각'}
              </h3>
              <label className={s.switch}>
                <input
                  type="checkbox"
                  checked={state.perDay}
                  onChange={(e) =>
                    setRows((st) =>
                      e.target.checked ? toPerDay(st, schoolDays) : toCommon(st, schoolDays),
                    )
                  }
                />
                <span>요일마다 시각이 다릅니다</span>
              </label>
            </div>

            {state.perDay ? (
              <PerDayEditor
                state={state}
                schoolDays={schoolDays}
                lengths={lengths}
                setRows={setRows}
              />
            ) : (
              <div className={s.commonGrid}>
                <div>
                  <SlotRows
                    lengths={lengths}
                    rows={state.base}
                    onChange={(rows) =>
                      setRows((st) => {
                        const max = maxPeriod(rows)
                        const periodsByDay = { ...st.periodsByDay }
                        for (const d of schoolDays) {
                          periodsByDay[d] = Math.min(periodsByDay[d] ?? max, max)
                          if ((periodsByDay[d] ?? 0) === 0) periodsByDay[d] = max
                        }
                        return { ...st, base: rows, periodsByDay }
                      })
                    }
                  />
                </div>

                <div className={s.dayCounts}>
                  <h4 className={s.subTitle}>요일별 교시 수</h4>
                  <p className={s.subHint}>수업이 일찍 끝나는 요일은 숫자를 줄이세요.</p>
                  {schoolDays.map((d) => {
                    const max = maxPeriod(state.base)
                    const v = Math.min(state.periodsByDay[d] ?? 0, max)
                    return (
                      <div key={d} className={s.dayCountRow}>
                        <span className={s.dayName}>{DAY_LABEL[d]}요일</span>
                        <div className={s.counter}>
                          <button
                            type="button"
                            className={s.counterBtn}
                            disabled={v <= 0}
                            onClick={() =>
                              setRows((st) => ({
                                ...st,
                                periodsByDay: { ...st.periodsByDay, [d]: v - 1 },
                              }))
                            }
                            aria-label={`${DAY_LABEL[d]}요일 교시 수 줄이기`}
                          >
                            −
                          </button>
                          <span className={`${s.counterValue} num`}>{v}</span>
                          <button
                            type="button"
                            className={s.counterBtn}
                            disabled={v >= max}
                            onClick={() =>
                              setRows((st) => ({
                                ...st,
                                periodsByDay: { ...st.periodsByDay, [d]: v + 1 },
                              }))
                            }
                            aria-label={`${DAY_LABEL[d]}요일 교시 수 늘리기`}
                          >
                            +
                          </button>
                        </div>
                        <span className={s.dayUnit}>교시</span>
                      </div>
                    )
                  })}
                </div>
              </div>
            )}
          </section>

          {/* ---- 미리보기 ---- */}
          <Preview slots={slots} schoolDays={schoolDays} />

          {/* ---- 문제점 · 저장 ---- */}
          {problems.length > 0 && (
            <Notice tone="danger">
              <div>
                <strong>고쳐야 할 부분이 있습니다</strong>
                <ul className={s.problemList}>
                  {problems.map((p) => (
                    <li key={p}>{p}</li>
                  ))}
                </ul>
              </div>
            </Notice>
          )}
          {save.error && <Notice tone="danger">{errorMessage(save.error)}</Notice>}

          <div className={s.saveBar}>
            <div className={s.saveHint}>
              {saved && !dirty && (
                <span className={s.savedMark}>
                  <Icon name="check" size={15} strokeWidth={3} /> 저장했습니다
                </span>
              )}
              {dirty && problems.length === 0 && <span>저장하면 바로 적용됩니다.</span>}
            </div>
            <div className={s.saveActions}>
              {dirty && (
                <Button
                  variant="ghost"
                  onClick={() => {
                    loadedFor.current = null
                    setDirty(false)
                  }}
                >
                  변경 내용 되돌리기
                </Button>
              )}
              <Button
                variant="primary"
                disabled={!dirty || problems.length > 0 || save.isPending}
                onClick={() => save.mutate()}
              >
                {save.isPending ? '저장 중…' : '변경 내용 저장'}
              </Button>
            </div>
          </div>
        </>
      )}
    </div>
  )
}

/* ------------------------------------------------------------------ */

function QuickFill({
  schoolDays,
  lengths,
  onLengths,
  pending,
  error,
  onGenerate,
}: {
  schoolDays: number[]
  lengths: Lengths
  onLengths: (v: Lengths) => void
  pending: boolean
  error: string | null
  onGenerate: (p: {
    firstStartMin: number
    lessonMinutes: number
    breakMinutes: number
    lunchAfterPeriod: number
    lunchMinutes: number
    recessAfterPeriod: number
    recessMinutes: number
    recessLabel: string
    periodsByDay: [number, number][]
  }) => void
}) {
  // 처음 들어왔을 때 펼쳐 둔다 — 시정표를 처음 만드는 사람이 먼저 볼 곳이다
  const [open, setOpen] = useState(true)
  const [lunchAfter, setLunchAfter] = useState(4)
  const [recessAfter, setRecessAfter] = useState(0)
  const [counts, setCounts] = useState<Record<number, number>>(() =>
    Object.fromEntries(schoolDays.map((d) => [d, 5])),
  )

  // 길이는 화면 전체가 함께 쓰는 값이다 (여기에 따로 두지 않는다)
  const put = (patch: Partial<Lengths>) => onLengths({ ...lengths, ...patch })

  return (
    <section className={s.card}>
      <button type="button" className={s.disclosure} onClick={() => setOpen((o) => !o)}>
        <span className={s.cardTitle}>처음부터 다시 만들기</span>
        <span className={s.disclosureHint}>
          {open ? '접기' : '교시 수까지 규칙으로 새로 만들기'}
        </span>
      </button>

      {open && (
        <div className={s.quickBody}>
          <p className={s.subHint}>
            아래 규칙대로 요일별 교시와 점심시간을 계산해서 채웁니다. 채운 뒤에 필요한 칸만 고치면
            됩니다. <strong>기존에 입력한 시각은 지워집니다.</strong>
          </p>

          <div className={s.quickRow}>
            <label className={s.quickField}>
              <span>1교시 시작</span>
              <TimeInput value={lengths.firstStart} onChange={(v) => put({ firstStart: v })} />
            </label>
            <label className={s.quickField}>
              <span>수업</span>
              <NumBox
                value={lengths.period}
                onChange={(v) => put({ period: v })}
                min={5}
                max={180}
                unit="분"
              />
            </label>
            <label className={s.quickField}>
              <span>쉬는 시간</span>
              <NumBox
                value={lengths.break}
                onChange={(v) => put({ break: v })}
                min={0}
                max={60}
                unit="분"
              />
            </label>
            <label className={s.quickField}>
              <span>점심</span>
              <NumBox value={lunchAfter} onChange={setLunchAfter} min={0} max={10} unit="교시 뒤" />
            </label>
            <label className={s.quickField}>
              <span>점심 길이</span>
              <NumBox
                value={lengths.lunch}
                onChange={(v) => put({ lunch: v })}
                min={10}
                max={120}
                unit="분"
              />
            </label>
            <label className={s.quickField}>
              <span>중간놀이</span>
              <NumBox value={recessAfter} onChange={setRecessAfter} min={0} max={10} unit="교시 뒤" />
            </label>
            <label className={s.quickField}>
              <span>중간놀이 길이</span>
              <NumBox
                value={lengths.recess}
                onChange={(v) => put({ recess: v })}
                min={5}
                max={120}
                unit="분"
              />
            </label>
          </div>
          <p className={s.subHint}>중간놀이가 없으면 &lsquo;0교시 뒤&rsquo;로 두세요.</p>

          <div className={s.quickRow}>
            <span className={s.quickLabel}>요일별 교시 수</span>
            {schoolDays.map((d) => (
              <label key={d} className={s.quickDay}>
                <span>{DAY_LABEL[d]}</span>
                <NumBox
                  value={counts[d] ?? 5}
                  onChange={(v) => setCounts((c) => ({ ...c, [d]: v }))}
                  min={0}
                  max={15}
                />
              </label>
            ))}
          </div>

          {error && <Notice tone="danger">{error}</Notice>}

          <Button
            variant="ghost"
            disabled={pending}
            onClick={() =>
              onGenerate({
                firstStartMin: lengths.firstStart,
                lessonMinutes: lengths.period,
                breakMinutes: lengths.break,
                lunchAfterPeriod: lunchAfter,
                lunchMinutes: lengths.lunch,
                recessAfterPeriod: recessAfter,
                recessMinutes: lengths.recess,
                recessLabel: '',
                periodsByDay: schoolDays.map((d) => [d, counts[d] ?? 5] as [number, number]),
              })
            }
          >
            이 규칙으로 시각 채우기
          </Button>
        </div>
      )}
    </section>
  )
}

/** 숫자 + 단위를 나란히 보여 주는 작은 래퍼 */
function NumBox({
  value,
  onChange,
  min,
  max,
  unit,
}: {
  value: number
  onChange: (v: number) => void
  min: number
  max: number
  unit?: string
}) {
  return (
    <span className={s.numBox}>
      <NumberInput value={value} onChange={onChange} min={min} max={max} />
      {unit && <span className={s.numUnit}>{unit}</span>}
    </span>
  )
}

/* ------------------------------------------------------------------ */

function PerDayEditor({
  state,
  schoolDays,
  lengths,
  setRows,
}: {
  state: EditorState
  schoolDays: number[]
  lengths: Lengths
  setRows: (u: (st: EditorState) => EditorState) => void
}) {
  const [day, setDay] = useState(schoolDays[0] ?? 1)
  const active = schoolDays.includes(day) ? day : (schoolDays[0] ?? 1)

  return (
    <div>
      <div className={s.dayTabs}>
        {schoolDays.map((d) => (
          <button
            key={d}
            type="button"
            className={d === active ? `${s.dayTab} ${s.dayTabOn}` : s.dayTab}
            onClick={() => setDay(d)}
          >
            {DAY_LABEL[d]}
            <span className={s.dayTabCount}>{(state.byDay[d] ?? []).length}</span>
          </button>
        ))}
      </div>

      <SlotRows
        lengths={lengths}
        rows={state.byDay[active] ?? []}
        onChange={(rows) => setRows((st) => ({ ...st, byDay: { ...st.byDay, [active]: rows } }))}
      />

      <div className={s.copyBar}>
        <span className={s.copyLabel}>{DAY_LABEL[active]}요일 시각을 복사</span>
        {schoolDays
          .filter((d) => d !== active)
          .map((d) => (
            <button
              key={d}
              type="button"
              className={s.copyBtn}
              onClick={() => setRows((st) => copyDay(st, active, [d]))}
            >
              → {DAY_LABEL[d]}
            </button>
          ))}
        <button
          type="button"
          className={`${s.copyBtn} ${s.copyAll}`}
          onClick={() =>
            setRows((st) => copyDay(st, active, schoolDays.filter((d) => d !== active)))
          }
        >
          → 모든 요일
        </button>
      </div>
    </div>
  )
}

/* ------------------------------------------------------------------ */

function Preview({ slots, schoolDays }: { slots: Slot[]; schoolDays: number[] }) {
  // 행 = 교시/점심, 열 = 요일
  const rowKeys: string[] = []
  const labelOf = new Map<string, string>()
  const orderOf = new Map<string, number>()

  for (const s of [...slots].sort((a, b) => a.startMin - b.startMin)) {
    const key = s.slotType === 'LUNCH' ? 'LUNCH' : `P${s.periodNo}`
    if (!labelOf.has(key)) {
      labelOf.set(key, s.label)
      orderOf.set(key, s.startMin)
      rowKeys.push(key)
    }
  }
  rowKeys.sort((a, b) => (orderOf.get(a) ?? 0) - (orderOf.get(b) ?? 0))

  const cell = (day: number, key: string) =>
    slots.find(
      (s) => s.dayOfWeek === day && (s.slotType === 'LUNCH' ? 'LUNCH' : `P${s.periodNo}`) === key,
    )

  return (
    <section className={s.card}>
      <h3 className={s.cardTitle}>미리보기</h3>
      <p className={s.subHint}>
        이 시각을 기준으로 보결 가능 여부를 판단합니다. 비어 있는 칸은 그 요일에 수업이 없다는
        뜻입니다.
      </p>

      {rowKeys.length === 0 ? (
        <p className={s.previewEmpty}>아직 만들어진 시각이 없습니다.</p>
      ) : (
        <div className={s.previewScroll}>
          <table className={s.preview}>
            <thead>
              <tr>
                <th className={s.previewCorner}>구분</th>
                {schoolDays.map((d) => (
                  <th key={d}>{DAY_LABEL[d]}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rowKeys.map((key) => (
                <tr key={key} className={key === 'LUNCH' ? s.previewLunchRow : undefined}>
                  <th className={s.previewRowHead}>{labelOf.get(key)}</th>
                  {schoolDays.map((d) => {
                    const c = cell(d, key)
                    return (
                      <td key={d} className={c ? undefined : s.previewNone}>
                        {c ? `${minToHm(c.startMin)}~${minToHm(c.endMin)}` : '—'}
                      </td>
                    )
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  )
}

/* ------------------------------------------------------------------ */
/*  시간 일괄 조정 — 짜 놓은 구조는 그대로 두고 길이만 다시 계산         */
/* ------------------------------------------------------------------ */

function BulkAdjust({
  profiles,
  currentId,
  lengths,
  onLengths,
  dirty,
  pending,
  error,
  onApply,
}: {
  profiles: { id: number; name: string }[]
  currentId: number
  lengths: Lengths
  onLengths: (v: Lengths) => void
  dirty: boolean
  pending: boolean
  error: string | null
  onApply: (ids: number[], params: ReflowParams) => void
}) {
  // 처음 들어왔을 때는 접어 둔다 — 이미 만들어 둔 시정표를 손볼 때 쓰는 곳이다
  const [open, setOpen] = useState(false)
  const [allProfiles, setAllProfiles] = useState(true)

  // 길이는 화면 전체가 함께 쓰는 값이다 (여기에 따로 두지 않는다)
  const put = (patch: Partial<Lengths>) => onLengths({ ...lengths, ...patch })
  const targets = allProfiles ? profiles.map((p) => p.id) : [currentId]

  return (
    <section className={s.card}>
      <button type="button" className={s.disclosure} onClick={() => setOpen((o) => !o)}>
        <span className={s.cardTitle}>시간 일괄 조정</span>
        <span className={s.disclosureHint}>
          {open ? '접기' : '교시 수는 그대로 두고 길이만 다시 계산'}
        </span>
      </button>

      {open && (
        <div className={s.quickBody}>
          <p className={s.subHint}>
            수업·쉬는 시간·점심·중간놀이 길이를 입력하면{' '}
            <strong>교시 수와 점심·중간놀이 위치는 그대로 둔 채</strong> 시각만 앞에서부터 다시
            계산합니다.
          </p>

          <div className={s.quickRow}>
            <label className={s.quickField}>
              <span>1교시 시작</span>
              <TimeInput value={lengths.firstStart} onChange={(v) => put({ firstStart: v })} />
            </label>
            <label className={s.quickField}>
              <span>수업</span>
              <NumBox
                value={lengths.period}
                onChange={(v) => put({ period: v })}
                min={5}
                max={180}
                unit="분"
              />
            </label>
            <label className={s.quickField}>
              <span>쉬는 시간</span>
              <NumBox
                value={lengths.break}
                onChange={(v) => put({ break: v })}
                min={0}
                max={120}
                unit="분"
              />
            </label>
            <label className={s.quickField}>
              <span>중간놀이</span>
              <NumBox
                value={lengths.recess}
                onChange={(v) => put({ recess: v })}
                min={5}
                max={120}
                unit="분"
              />
            </label>
            <label className={s.quickField}>
              <span>점심</span>
              <NumBox
                value={lengths.lunch}
                onChange={(v) => put({ lunch: v })}
                min={10}
                max={180}
                unit="분"
              />
            </label>
          </div>

          {profiles.length > 1 && (
            <label className={s.switch}>
              <input
                type="checkbox"
                checked={allProfiles}
                onChange={(e) => setAllProfiles(e.target.checked)}
              />
              <span>모든 시정표 유형 {profiles.length}개에 함께 반영</span>
            </label>
          )}

          {error && <Notice tone="danger">{error}</Notice>}

          <div className={s.bulkFoot}>
            {dirty && (
              <span className={s.bulkWarn}>
                저장하지 않은 변경 내용이 있어 일괄 조정을 할 수 없습니다. 먼저 저장하거나 되돌려
                주세요.
              </span>
            )}
            <Button
              variant="ghost"
              disabled={pending || dirty}
              onClick={() =>
                onApply(targets, {
                  firstStartMin: lengths.firstStart,
                  lessonMinutes: lengths.period,
                  breakMinutes: lengths.break,
                  lunchMinutes: lengths.lunch,
                  otherMinutes: lengths.recess,
                })
              }
            >
              {pending
                ? '반영 중…'
                : allProfiles && profiles.length > 1
                  ? `${profiles.length}개 유형 전체에 반영`
                  : '이 유형에 반영'}
            </Button>
          </div>
        </div>
      )}
    </section>
  )
}
