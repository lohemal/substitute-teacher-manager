import { useEffect, useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { NumberInput } from '@/components/NumberInput'
import { Button, Notice } from '@/components/ui'
import { errorMessage } from '@/ipc/invoke'
import {
  DEFAULT_GRADE_RANGE,
  DEFAULT_NAMING,
  SCHOOL_TYPE_LABEL,
  schoolApi,
  type ClassNaming,
  type SchoolType,
} from '@/ipc/school'
import { setupApi } from '@/ipc/setup'
import { useAutoSave } from '@/lib/useAutoSave'
import { Field, SaveHint, StepFrame } from '../StepFrame'
import { ClassNamingCard } from './ClassNamingCard'
import type { StepProps } from '../types'
import s from './SchoolStep.module.css'

const DAY_LABEL = ['', '월', '화', '수', '목', '금', '토', '일']

interface Form {
  name: string
  schoolType: SchoolType
  minGrade: number
  maxGrade: number
  schoolDays: number[]
  schoolYear: number
  semester: number
  /** 학년 -> 반 수 */
  classCounts: Record<number, number>
  naming: ClassNaming
}

/** 3월 이전이면 아직 지난 학년도. 8월 이전이면 1학기. */
function defaultTerm(): { schoolYear: number; semester: number } {
  const now = new Date()
  const y = now.getFullYear()
  const m = now.getMonth() + 1
  if (m < 3) return { schoolYear: y - 1, semester: 2 }
  if (m <= 7) return { schoolYear: y, semester: 1 }
  return { schoolYear: y, semester: 2 }
}

function emptyForm(): Form {
  const [min, max] = DEFAULT_GRADE_RANGE.ELEMENTARY
  const counts: Record<number, number> = {}
  for (let g = min; g <= max; g++) counts[g] = 1
  return {
    name: '',
    schoolType: 'ELEMENTARY',
    minGrade: min,
    maxGrade: max,
    schoolDays: [1, 2, 3, 4, 5],
    ...defaultTerm(),
    classCounts: counts,
    naming: DEFAULT_NAMING,
  }
}

export function SchoolStep({ nav }: StepProps) {
  const qc = useQueryClient()
  const [form, setForm] = useState<Form | null>(null)

  // 자동 저장된 입력 -> 이미 저장된 학교 정보 -> 기본값 순으로 불러온다
  const { data: initial, isLoading } = useQuery({
    queryKey: ['setup-initial', 'SCHOOL'],
    queryFn: async (): Promise<Form> => {
      const draft = await setupApi.getDraft<Form>('SCHOOL')
      if (draft) return { ...emptyForm(), ...draft }

      const saved = await schoolApi.get()
      if (saved) {
        const counts: Record<number, number> = {}
        for (const c of saved.classCounts) counts[c.grade] = c.count
        for (let g = saved.minGrade; g <= saved.maxGrade; g++) counts[g] ??= 1
        return {
          name: saved.name,
          schoolType: saved.schoolType,
          minGrade: saved.minGrade,
          maxGrade: saved.maxGrade,
          schoolDays: saved.schoolDays,
          schoolYear: saved.schoolYear,
          semester: saved.semester,
          classCounts: counts,
          naming: saved.naming ?? DEFAULT_NAMING,
        }
      }
      return emptyForm()
    },
    staleTime: Infinity,
    gcTime: 0,
  })

  useEffect(() => {
    if (initial && !form) setForm(initial)
  }, [initial, form])

  const saveStatus = useAutoSave(
    form,
    async (v) => {
      if (v) await setupApi.saveDraft('SCHOOL', v)
    },
    { enabled: !!form },
  )

  const grades = (f: Form) => {
    const out: number[] = []
    for (let g = f.minGrade; g <= f.maxGrade; g++) out.push(g)
    return out
  }

  const save = useMutation({
    mutationFn: async (f: Form) =>
      schoolApi.save({
        name: f.name.trim(),
        schoolType: f.schoolType,
        minGrade: f.minGrade,
        maxGrade: f.maxGrade,
        schoolDays: f.schoolDays,
        schoolYear: f.schoolYear,
        semester: f.semester,
        classCounts: grades(f).map((g) => ({ grade: g, count: f.classCounts[g] ?? 1 })),
        naming: f.naming,
      }),
    onSuccess: async () => {
      await Promise.all([
        qc.invalidateQueries({ queryKey: ['setup-state'] }),
        qc.invalidateQueries({ queryKey: ['school'] }),
      ])
      nav.goNext()
    },
  })

  const gradeList = useMemo(() => (form ? grades(form) : []), [form])

  if (isLoading || !form) {
    return (
      <StepFrame title="학교 기본 설정" description="불러오는 중…">
        <div className={s.loading}>잠시만 기다려 주세요…</div>
      </StepFrame>
    )
  }

  const patch = (p: Partial<Form>) => setForm((f) => (f ? { ...f, ...p } : f))

  const changeSchoolType = (t: SchoolType) => {
    const [min, max] = DEFAULT_GRADE_RANGE[t]
    const counts: Record<number, number> = { ...form.classCounts }
    for (let g = min; g <= max; g++) counts[g] ??= 1
    patch({ schoolType: t, minGrade: min, maxGrade: max, classCounts: counts })
  }

  const changeMaxGrade = (max: number) => {
    const counts = { ...form.classCounts }
    for (let g = form.minGrade; g <= max; g++) counts[g] ??= 1
    patch({ maxGrade: max, classCounts: counts })
  }

  const toggleDay = (d: number) => {
    const has = form.schoolDays.includes(d)
    patch({
      schoolDays: has
        ? form.schoolDays.filter((x) => x !== d)
        : [...form.schoolDays, d].sort((a, b) => a - b),
    })
  }

  const setCount = (grade: number, count: number) => {
    const n = Math.max(1, Math.min(30, count))
    patch({ classCounts: { ...form.classCounts, [grade]: n } })
  }

  const nameEmpty = form.name.trim().length === 0
  const noDays = form.schoolDays.length === 0
  const canSubmit = !nameEmpty && !noDays && !save.isPending

  const totalClasses = gradeList.reduce((sum, g) => sum + (form.classCounts[g] ?? 0), 0)

  return (
    <StepFrame
      title="학교 기본 설정"
      description="우리 학교 정보를 입력해 주세요. 여기서 만든 학급을 기준으로 시정표와 시간표를 설정하게 됩니다."
      footerLeft={<SaveHint status={saveStatus} />}
      footerRight={
        <Button variant="primary" disabled={!canSubmit} onClick={() => save.mutate(form)}>
          {save.isPending ? '저장 중…' : '저장하고 다음 단계 →'}
        </Button>
      }
    >
      {save.error && <Notice tone="danger">{errorMessage(save.error)}</Notice>}

      <div className={s.card}>
        <Field label="학교 이름" hint="예) 한빛초등학교">
          <input
            className={s.input}
            value={form.name}
            maxLength={40}
            placeholder="학교 이름을 입력하세요"
            onChange={(e) => patch({ name: e.target.value })}
            autoFocus
          />
          {nameEmpty && <p className={s.warnText}>학교 이름을 입력해야 다음 단계로 넘어갑니다.</p>}
        </Field>

        <Field label="학교 구분">
          <div className={s.chips}>
            {(Object.keys(SCHOOL_TYPE_LABEL) as SchoolType[]).map((t) => (
              <button
                key={t}
                type="button"
                className={form.schoolType === t ? `${s.chip} ${s.chipOn}` : s.chip}
                onClick={() => changeSchoolType(t)}
              >
                {SCHOOL_TYPE_LABEL[t]}
              </button>
            ))}
          </div>
        </Field>

        <Field label="학년" hint="마지막 학년까지 표시됩니다">
          <div className={s.inline}>
            <select
              className={s.select}
              value={form.minGrade}
              onChange={(e) => patch({ minGrade: Number(e.target.value) })}
            >
              {[1, 2, 3, 4, 5, 6].map((g) => (
                <option key={g} value={g} disabled={g > form.maxGrade}>
                  {g}학년
                </option>
              ))}
            </select>
            <span className={s.tilde}>~</span>
            <select
              className={s.select}
              value={form.maxGrade}
              onChange={(e) => changeMaxGrade(Number(e.target.value))}
            >
              {[1, 2, 3, 4, 5, 6].map((g) => (
                <option key={g} value={g} disabled={g < form.minGrade}>
                  {g}학년
                </option>
              ))}
            </select>
          </div>
        </Field>

        <Field label="수업 요일" hint="수업이 있는 요일만 선택합니다">
          <div className={s.chips}>
            {[1, 2, 3, 4, 5, 6, 7].map((d) => (
              <button
                key={d}
                type="button"
                className={form.schoolDays.includes(d) ? `${s.chip} ${s.chipOn}` : s.chip}
                onClick={() => toggleDay(d)}
              >
                {DAY_LABEL[d]}
              </button>
            ))}
          </div>
          {noDays && <p className={s.warnText}>수업하는 요일을 하나 이상 선택해 주세요.</p>}
        </Field>

        <Field label="학년도 · 학기" hint="지금 사용할 학기입니다. 나중에 학기를 바꿔도 기록은 남습니다.">
          <div className={s.inline}>
            <NumberInput
              className={s.inputYear}
              min={2000}
              max={2100}
              value={form.schoolYear}
              onChange={(v) => patch({ schoolYear: v })}
              aria-label="학년도"
            />
            <span className={s.unit}>학년도</span>
            <div className={s.chips}>
              {[1, 2].map((n) => (
                <button
                  key={n}
                  type="button"
                  className={form.semester === n ? `${s.chip} ${s.chipOn}` : s.chip}
                  onClick={() => patch({ semester: n })}
                >
                  {n}학기
                </button>
              ))}
            </div>
          </div>
        </Field>
      </div>

      <div className={s.card}>
        <div className={s.classHead}>
          <div>
            <h2 className={s.classTitle}>학년별 반 수</h2>
            <p className={s.classDesc}>
              각 학년에 몇 개 반이 있는지 입력하세요. 전체 {totalClasses}개 학급이 만들어집니다.
            </p>
          </div>
        </div>

        <div className={s.gradeGrid}>
          {gradeList.map((g) => (
            <div key={g} className={s.gradeCell}>
              <span className={s.gradeName}>{g}학년</span>
              <div className={s.counter}>
                <button
                  type="button"
                  className={s.counterBtn}
                  onClick={() => setCount(g, (form.classCounts[g] ?? 1) - 1)}
                  disabled={(form.classCounts[g] ?? 1) <= 1}
                  aria-label={`${g}학년 반 수 줄이기`}
                >
                  −
                </button>
                <NumberInput
                  className={s.counterInput}
                  min={1}
                  max={30}
                  value={form.classCounts[g] ?? 1}
                  onChange={(v) => setCount(g, v)}
                  aria-label={`${g}학년 반 수`}
                />
                <button
                  type="button"
                  className={s.counterBtn}
                  onClick={() => setCount(g, (form.classCounts[g] ?? 1) + 1)}
                  disabled={(form.classCounts[g] ?? 1) >= 30}
                  aria-label={`${g}학년 반 수 늘리기`}
                >
                  +
                </button>
              </div>
              <span className={s.gradeUnit}>개 반</span>
            </div>
          ))}
        </div>
      </div>

      <ClassNamingCard
        naming={form.naming}
        grades={gradeList}
        counts={form.classCounts}
        onChange={(naming) => patch({ naming })}
      />
    </StepFrame>
  )
}
