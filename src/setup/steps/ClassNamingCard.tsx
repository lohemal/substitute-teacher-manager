import { NumberInput } from '@/components/NumberInput'
import { classFull, normalizeClassName } from '@/lib/classLabel'
import type { ClassNaming } from '@/ipc/school'
import s from './ClassNamingCard.module.css'

interface Props {
  naming: ClassNaming
  grades: number[]
  /** 학년 -> 반 수 */
  counts: Record<number, number>
  onChange: (naming: ClassNaming) => void
}

const MAX_SHOWN = 12

/**
 * 반을 '1반, 2반'으로 부를지 '가람반, 나리반'으로 부를지 정하는 카드.
 *
 * 대부분의 학교는 학년마다 같은 이름을 쓰므로 이름 몇 개만 적으면 끝나고,
 * 학년마다 다른 드문 경우에만 학년별 입력으로 펼친다.
 */
export function ClassNamingCard({ naming, grades, counts, onChange }: Props) {
  const maxCount = Math.max(1, ...grades.map((g) => counts[g] ?? 1))
  const isName = naming.mode === 'NAME'

  const setMode = (mode: 'NUMBER' | 'NAME') => {
    if (mode === 'NUMBER') {
      onChange({ ...naming, mode })
      return
    }
    // 처음 켤 때 빈 칸을 반 수만큼 준비한다
    const shared = [...naming.sharedNames]
    while (shared.length < maxCount) shared.push('')
    onChange({ ...naming, mode, sharedNames: shared.slice(0, Math.max(maxCount, shared.length)) })
  }

  const setShared = (index: number, value: string) => {
    const next = [...naming.sharedNames]
    while (next.length <= index) next.push('')
    next[index] = value
    onChange({ ...naming, sharedNames: next })
  }

  const gradeNamesOf = (grade: number): string[] =>
    naming.gradeNames.find((g) => g.grade === grade)?.names ?? []

  const setGradeName = (grade: number, index: number, value: string) => {
    const list = [...naming.gradeNames]
    let entry = list.find((g) => g.grade === grade)
    if (!entry) {
      entry = { grade, names: [] }
      list.push(entry)
    }
    const names = [...entry.names]
    while (names.length <= index) names.push('')
    names[index] = value
    const nextList = list
      .map((g) => (g.grade === grade ? { grade, names } : g))
      .sort((a, b) => a.grade - b.grade)
    onChange({ ...naming, gradeNames: nextList })
  }

  const setPerGrade = (on: boolean) => {
    if (on) {
      // 지금 공통 이름을 학년별로 펼쳐 준다 (그대로 시작해서 필요한 학년만 고침)
      const gradeNames = grades.map((g) => ({
        grade: g,
        names: Array.from({ length: counts[g] ?? 1 }, (_, i) => naming.sharedNames[i] ?? ''),
      }))
      onChange({ ...naming, perGrade: true, gradeNames })
    } else {
      // 가장 반이 많은 학년의 이름을 공통으로 삼는다
      const richest = grades.reduce(
        (best, g) => ((counts[g] ?? 0) > (counts[best] ?? 0) ? g : best),
        grades[0] ?? 1,
      )
      onChange({ ...naming, perGrade: false, sharedNames: gradeNamesOf(richest) })
    }
  }

  return (
    <div className={s.card}>
      <div className={s.head}>
        <div>
          <h2 className={s.title}>반 이름</h2>
          <p className={s.desc}>
            우리 학교가 반을 어떻게 부르는지 고르세요. 보결 조회와 배정 내역에 이 이름으로
            표시됩니다.
          </p>
        </div>
        <div className={s.modeChips}>
          <button
            type="button"
            className={!isName ? `${s.modeChip} ${s.modeChipOn}` : s.modeChip}
            onClick={() => setMode('NUMBER')}
          >
            <span className={s.modeName}>숫자로</span>
            <span className={s.modeEx}>1반, 2반</span>
          </button>
          <button
            type="button"
            className={isName ? `${s.modeChip} ${s.modeChipOn}` : s.modeChip}
            onClick={() => setMode('NAME')}
          >
            <span className={s.modeName}>이름으로</span>
            <span className={s.modeEx}>가람반, 나리반</span>
          </button>
        </div>
      </div>

      {isName && (
        <div className={s.body}>
          {!naming.perGrade ? (
            <div className={s.nameRow}>
              {Array.from({ length: Math.min(maxCount, MAX_SHOWN) }, (_, i) => (
                <label key={i} className={s.nameCell}>
                  <span className={s.ordinal}>{i + 1}번째</span>
                  <input
                    className={s.nameInput}
                    value={naming.sharedNames[i] ?? ''}
                    maxLength={10}
                    placeholder="이름"
                    onChange={(e) => setShared(i, e.target.value)}
                  />
                </label>
              ))}
            </div>
          ) : (
            <div className={s.perGradeList}>
              {grades.map((g) => (
                <div key={g} className={s.perGradeRow}>
                  <span className={s.gradeName}>{g}학년</span>
                  <div className={s.nameRow}>
                    {Array.from({ length: Math.min(counts[g] ?? 1, MAX_SHOWN) }, (_, i) => (
                      <input
                        key={i}
                        className={s.nameInput}
                        value={gradeNamesOf(g)[i] ?? ''}
                        maxLength={10}
                        placeholder={`${i + 1}번째`}
                        onChange={(e) => setGradeName(g, i, e.target.value)}
                      />
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}

          <label className={s.switch}>
            <input
              type="checkbox"
              checked={naming.perGrade}
              onChange={(e) => setPerGrade(e.target.checked)}
            />
            <span>학년마다 반 이름이 다릅니다</span>
          </label>

          <div className={s.preview}>
            <span className={s.previewLabel}>미리보기</span>
            <div className={s.previewList}>
              {grades.slice(0, 3).map((g) => {
                const list = naming.perGrade ? gradeNamesOf(g) : naming.sharedNames
                const count = counts[g] ?? 1
                return (
                  <span key={g} className={s.previewRow}>
                    {Array.from({ length: count }, (_, i) => {
                      const raw = list[i] ?? ''
                      const name = raw.trim() ? normalizeClassName(raw) : null
                      return (
                        <span key={i} className={name ? s.previewChip : s.previewChipEmpty}>
                          {name ? classFull(g, i + 1, name) : `${g}학년 ${i + 1}번째 (미입력)`}
                        </span>
                      )
                    })}
                  </span>
                )
              })}
              {grades.length > 3 && <span className={s.previewMore}>…</span>}
            </div>
          </div>

          <p className={s.hint}>
            &lsquo;반&rsquo;은 자동으로 붙으니 <strong>가람</strong>까지만 적으면 됩니다. 표에서는{' '}
            <strong>5-가람</strong>처럼 짧게 표시됩니다.
          </p>
        </div>
      )}
    </div>
  )
}

/** 학년별 반 수 입력 (반 이름 카드와 같은 곳에서 쓰인다) */
export function ClassCountRow({
  grade,
  value,
  onChange,
}: {
  grade: number
  value: number
  onChange: (v: number) => void
}) {
  return (
    <div className={s.countCell}>
      <span className={s.countGrade}>{grade}학년</span>
      <NumberInput value={value} onChange={onChange} min={1} max={30} aria-label={`${grade}학년 반 수`} />
      <span className={s.countUnit}>개 반</span>
    </div>
  )
}
