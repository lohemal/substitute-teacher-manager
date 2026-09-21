import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Notice } from '@/components/ui'
import { DAY_LABEL } from '@/ipc/bell'
import { errorMessage } from '@/ipc/invoke'
import { mealApi, type MealDay, type MealPattern, type MealSource } from '@/ipc/meal'
import s from './MealCard.module.css'

/**
 * 전담교사 식사시간.
 *
 * 사용자가 알아야 하는 것은 두 줄이다.
 *
 *   전담 선생님이 일반 보결을 맡지 않는 식사시간을 정합니다.
 *   점심 보결에는 이 제한이 적용되지 않습니다.
 *
 * 그래서 화면도 두 줄이다 — 위는 학교 기본 식사시간, 아래는 이 선생님의
 * 요일별 식사시간. 시각을 직접 치게 하지 않고 **시정표에 있는 점심시간**
 * 중에서 고르게 한다.
 *
 * 자동으로 정해진 것과 직접 지정한 것을 구분해 보여 준다.
 */
export function MealCard({ teacherId }: { teacherId: number | null }) {
  const qc = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: ['meal', teacherId],
    queryFn: () => mealApi.view(teacherId),
  })

  const after = (next: typeof data) => {
    qc.setQueryData(['meal', teacherId], next)
    // 식사시간이 바뀌면 보결 후보도 달라진다
    qc.invalidateQueries({ queryKey: ['find-candidates'] })
    qc.invalidateQueries({ queryKey: ['day-plan'] })
  }

  const setDefault = useMutation({
    mutationFn: (bellId: number | null) => mealApi.setDefault(bellId, teacherId),
    onSuccess: after,
  })
  const setOverride = useMutation({
    mutationFn: (v: { dayOfWeek: number; startMin: number | null; endMin: number | null }) =>
      mealApi.setOverride({ teacherId: teacherId!, ...v }),
    onSuccess: after,
  })

  if (isLoading) return null
  if (error) {
    return (
      <section className={s.card}>
        <Notice tone="danger">{errorMessage(error)}</Notice>
      </section>
    )
  }
  if (!data) return null

  const busy = setDefault.isPending || setOverride.isPending
  const saveError = setDefault.error ?? setOverride.error

  return (
    <section className={s.card}>
      <div className={s.head}>
        <h3 className={s.title}>전담교사 식사시간</h3>
        <p className={s.lead}>
          이 시간에는 <b>일반 수업 보결</b>을 맡기지 않습니다. 점심 보결은 그대로 맡을 수
          있습니다.
        </p>
      </div>

      {saveError && <Notice tone="danger">{errorMessage(saveError)}</Notice>}

      {data.patterns.length === 0 ? (
        <Notice tone="warn">
          시정표에 점심시간이 없어 식사시간을 정할 수 없습니다. 먼저 [시정표 · 점심시간]에서
          점심시간을 넣어 주세요.
        </Notice>
      ) : (
        <>
          {/* ---------- 학교 기본 식사시간 ---------- */}
          <div className={s.row}>
            <span className={s.rowLabel}>학교 기본</span>
            <div className={s.chips}>
              {data.patterns.map((p) => (
                <PatternChip
                  key={p.bellScheduleId}
                  p={p}
                  on={data.defaultBellId === p.bellScheduleId}
                  disabled={busy}
                  onPick={() =>
                    setDefault.mutate(
                      data.defaultBellId === p.bellScheduleId ? null : p.bellScheduleId,
                    )
                  }
                />
              ))}
            </div>
          </div>
          <p className={s.hint}>
            그 날 수업과 겹치지 않는 시간이 여러 개일 때 이 시간을 씁니다. 하나뿐이면 자동으로
            정해집니다.
          </p>

          {data.needsDefault && (
            <Notice tone="warn">
              학교 기본 식사시간을 아직 정하지 않았습니다. 정하기 전까지는 식사시간을 알 수 없는
              선생님을 <b>후보에서 빼지 않습니다</b> — 잘못 빼는 것보다 낫기 때문입니다.
            </Notice>
          )}

          {/* ---------- 이 선생님의 요일별 ---------- */}
          {teacherId != null && data.isSpecial && (
            <div className={s.days}>
              {data.days.map((d) => (
                <DayCell
                  key={d.dayOfWeek}
                  d={d}
                  patterns={data.patterns}
                  disabled={busy}
                  onPick={(startMin, endMin) =>
                    setOverride.mutate({ dayOfWeek: d.dayOfWeek, startMin, endMin })
                  }
                />
              ))}
            </div>
          )}

          {teacherId != null && !data.isSpecial && (
            <p className={s.hint}>
              전담 선생님에게만 적용됩니다. 담임 선생님의 급식 지도는 [설정 → 보결 판단 동작
              옵션]에서 따로 정합니다.
            </p>
          )}
        </>
      )}
    </section>
  )
}

function PatternChip({
  p,
  on,
  disabled,
  onPick,
}: {
  p: MealPattern
  on: boolean
  disabled: boolean
  onPick: () => void
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      className={on ? `${s.chip} ${s.chipOn}` : s.chip}
      onClick={onPick}
      title={on ? '다시 누르면 정하지 않은 상태로 되돌립니다' : undefined}
    >
      <span className={s.chipGrades}>{p.gradeLabel}</span>
      <span className={s.chipTime}>{p.timeLabel}</span>
    </button>
  )
}

const SOURCE_CLASS: Record<MealSource, string> = {
  MANUAL: 'srcManual',
  AUTO: 'srcAuto',
  DEFAULT: 'srcDefault',
  UNKNOWN: 'srcUnknown',
}

function DayCell({
  d,
  patterns,
  disabled,
  onPick,
}: {
  d: MealDay
  patterns: MealPattern[]
  disabled: boolean
  onPick: (startMin: number | null, endMin: number | null) => void
}) {
  // 그 요일에 고를 수 있는 시간들 (시정표에서 그대로)
  const options = patterns
    .map((p) => p.byDay.find((w) => w.dayOfWeek === d.dayOfWeek))
    .filter((w): w is NonNullable<typeof w> => !!w)
  const seen = new Set<string>()
  const uniq = options.filter((w) => {
    const k = `${w.startMin}-${w.endMin}`
    if (seen.has(k)) return false
    seen.add(k)
    return true
  })

  const value = d.source === 'MANUAL' ? `${d.startMin}-${d.endMin}` : ''

  return (
    <label className={s.day}>
      <span className={s.dayName}>{DAY_LABEL[d.dayOfWeek]}</span>

      <span className={`${s.badge} ${s[SOURCE_CLASS[d.source]]}`}>{d.sourceLabel}</span>
      <span className={s.dayTime}>{d.timeLabel || '—'}</span>

      <select
        className={s.select}
        value={value}
        disabled={disabled}
        aria-label={`${DAY_LABEL[d.dayOfWeek]}요일 식사시간`}
        onChange={(e) => {
          if (!e.target.value) return onPick(null, null)
          const [a, b] = e.target.value.split('-').map(Number)
          onPick(a, b)
        }}
      >
        <option value="">자동으로</option>
        {uniq.map((w) => (
          <option key={`${w.startMin}-${w.endMin}`} value={`${w.startMin}-${w.endMin}`}>
            {hm(w.startMin)}~{hm(w.endMin)} 로 지정
          </option>
        ))}
      </select>

      {d.note && <span className={s.dayNote}>{d.note}</span>}
    </label>
  )
}

function hm(v: number): string {
  return `${String(Math.floor(v / 60)).padStart(2, '0')}:${String(v % 60).padStart(2, '0')}`
}
