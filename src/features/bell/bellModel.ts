import type { Slot } from '@/ipc/bell'

/** 중간놀이처럼 사용자가 이름을 정하는 구간의 기본 이름 */
export const RECESS_LABEL = '중간놀이'

/**
 * 편집 화면이 다루는 형태.
 *
 * 실제 저장 구조는 '요일 × 교시'로 펼쳐진 목록이지만,
 * 대부분의 학교는 요일마다 시각이 같고 교시 수만 다르다.
 * 그래서 기본은 **교시별 시각 한 벌 + 요일별 교시 수**로 다루고,
 * 요일마다 시각이 다른 학교만 요일별 편집으로 전환한다.
 */

export interface Row {
  /** React 목록 키 (저장에는 쓰이지 않음) */
  key: string
  kind: 'PERIOD' | 'LUNCH' | 'OTHER'
  periodNo: number | null
  /** kind='OTHER' 일 때 사용자가 정한 이름 (중간놀이 등) */
  name?: string
  startMin: number
  endMin: number
}

export interface EditorState {
  /** true면 요일마다 시각을 따로 관리 */
  perDay: boolean
  /** perDay=false 일 때: 모든 요일이 공유하는 교시별 시각 */
  base: Row[]
  /** perDay=false 일 때: 요일별 교시 수 */
  periodsByDay: Record<number, number>
  /** perDay=true 일 때: 요일별 시각 */
  byDay: Record<number, Row[]>
}

let keySeq = 0
const nextKey = () => `r${++keySeq}`

export const DEFAULT_LESSON = 40
export const DEFAULT_BREAK = 10

export function rowLabel(r: Row): string {
  if (r.kind === 'LUNCH') return '점심'
  if (r.kind === 'OTHER') return (r.name ?? '').trim() || RECESS_LABEL
  return `${r.periodNo}교시`
}

function toRow(s: Slot): Row {
  return {
    key: nextKey(),
    kind: s.slotType,
    periodNo: s.periodNo,
    name: s.slotType === 'OTHER' ? s.label : undefined,
    startMin: s.startMin,
    endMin: s.endMin,
  }
}

function toSlot(r: Row, day: number): Slot {
  return {
    dayOfWeek: day,
    slotType: r.kind,
    periodNo: r.kind === 'PERIOD' ? r.periodNo : null,
    label: rowLabel(r),
    startMin: r.startMin,
    endMin: r.endMin,
  }
}

const byStart = (a: Row, b: Row) => a.startMin - b.startMin

/* ------------------------------------------------------------------ */
/*  펼치기 (편집 상태 -> 저장 형태)                                      */
/* ------------------------------------------------------------------ */

export function expand(st: EditorState, schoolDays: number[]): Slot[] {
  const out: Slot[] = []

  if (st.perDay) {
    for (const day of schoolDays) {
      for (const r of st.byDay[day] ?? []) out.push(toSlot(r, day))
    }
    return out
  }

  for (const day of schoolDays) {
    const count = st.periodsByDay[day] ?? 0
    if (count <= 0) continue
    for (const r of st.base) {
      if (r.kind !== 'PERIOD' || (r.periodNo ?? 0) <= count) out.push(toSlot(r, day))
    }
  }
  return out
}

/* ------------------------------------------------------------------ */
/*  불러오기 (저장 형태 -> 편집 상태)                                    */
/* ------------------------------------------------------------------ */

function sameSlots(a: Slot[], b: Slot[]): boolean {
  if (a.length !== b.length) return false
  const norm = (list: Slot[]) =>
    list
      .map(
        (s) =>
          `${s.dayOfWeek}|${s.slotType}|${s.periodNo ?? '-'}|${s.label}|${s.startMin}|${s.endMin}`,
      )
      .sort()
  const [x, y] = [norm(a), norm(b)]
  return x.every((v, i) => v === y[i])
}

/**
 * 저장된 시각을 보고 '요일 공통'으로 표현할 수 있는지 스스로 판단한다.
 * 공통으로 펼쳤을 때 원본과 완전히 같으면 공통 모드, 아니면 요일별 모드.
 */
export function derive(slots: Slot[], schoolDays: number[]): EditorState {
  const byDay: Record<number, Row[]> = {}
  for (const day of schoolDays) byDay[day] = []
  for (const s of slots) {
    if (!byDay[s.dayOfWeek]) byDay[s.dayOfWeek] = []
    byDay[s.dayOfWeek].push(toRow(s))
  }
  for (const day of Object.keys(byDay)) byDay[Number(day)].sort(byStart)

  const periodsByDay: Record<number, number> = {}
  for (const day of schoolDays) {
    periodsByDay[day] = (byDay[day] ?? []).reduce((max, r) => Math.max(max, r.periodNo ?? 0), 0)
  }

  // 행이 가장 많은 요일을 대표로 삼는다
  const richest = schoolDays
    .map((d) => byDay[d] ?? [])
    .reduce<Row[]>((best, cur) => (cur.length > best.length ? cur : best), [])

  const base = richest.map((r) => ({ ...r, key: nextKey() }))
  const candidate: EditorState = { perDay: false, base, periodsByDay, byDay }

  if (slots.length > 0 && sameSlots(expand(candidate, schoolDays), slots)) {
    return candidate
  }
  return { ...candidate, perDay: slots.length > 0 }
}

export function emptyState(schoolDays: number[]): EditorState {
  const periodsByDay: Record<number, number> = {}
  const byDay: Record<number, Row[]> = {}
  for (const d of schoolDays) {
    periodsByDay[d] = 0
    byDay[d] = []
  }
  return { perDay: false, base: [], periodsByDay, byDay }
}

/* ------------------------------------------------------------------ */
/*  편집 동작                                                           */
/* ------------------------------------------------------------------ */

export function maxPeriod(rows: Row[]): number {
  return rows.reduce((m, r) => Math.max(m, r.periodNo ?? 0), 0)
}

export function hasLunch(rows: Row[]): boolean {
  return rows.some((r) => r.kind === 'LUNCH')
}

/** 마지막 행 뒤에 새 교시를 붙인다. */
export function addPeriod(rows: Row[]): Row[] {
  const sorted = [...rows].sort(byStart)
  const last = sorted[sorted.length - 1]
  const lastPeriod = sorted.filter((r) => r.kind === 'PERIOD').pop()
  const length = lastPeriod ? lastPeriod.endMin - lastPeriod.startMin : DEFAULT_LESSON
  const start = last ? last.endMin + (last.kind === 'LUNCH' ? 0 : DEFAULT_BREAK) : 9 * 60
  return [
    ...rows,
    {
      key: nextKey(),
      kind: 'PERIOD',
      periodNo: maxPeriod(rows) + 1,
      startMin: Math.min(start, 1380),
      endMin: Math.min(start + length, 1440),
    },
  ]
}

/** 점심을 넣는다. 끼워 넣은 만큼 뒤 교시를 밀어낸다. */
export function addLunch(rows: Row[], afterPeriod: number, minutes = 50): Row[] {
  return insertBreakRow(rows, { kind: 'LUNCH', afterPeriod, minutes, fallbackStart: 12 * 60 })
}

/** 중간놀이 등 기타 시간 구간을 넣는다. 끼워 넣은 만큼 뒤 교시를 밀어낸다. */
export function addRecess(rows: Row[], afterPeriod: number, minutes = 30): Row[] {
  return insertBreakRow(rows, {
    kind: 'OTHER',
    afterPeriod,
    minutes,
    name: RECESS_LABEL,
    fallbackStart: 10 * 60 + 30,
  })
}

/** 수업과 수업 사이의 쉬는 시간을 지금 자료에서 읽어 낸다. */
export function detectBreak(rows: Row[]): number {
  const sorted = [...rows].sort(byStart)
  for (let i = 0; i + 1 < sorted.length; i++) {
    if (sorted[i].kind === 'PERIOD' && sorted[i + 1].kind === 'PERIOD') {
      return Math.max(0, sorted[i + 1].startMin - sorted[i].endMin)
    }
  }
  return DEFAULT_BREAK
}

/**
 * 수업이 아닌 구간(점심·중간놀이)을 지정한 교시 바로 뒤에 끼워 넣는다.
 *
 * 앞 교시가 끝나자마자 시작하고, 뒤 교시는 이 구간이 끝나자마자 시작하도록
 * 뒤쪽 전체를 밀어낸다. 원래 그 자리에 있던 쉬는 시간은 이 구간이 대신한다.
 */
function insertBreakRow(
  rows: Row[],
  opts: {
    kind: 'LUNCH' | 'OTHER'
    afterPeriod: number
    minutes: number
    name?: string
    fallbackStart: number
  },
): Row[] {
  const sorted = [...rows].sort(byStart)
  const anchor = sorted
    .filter((r) => r.kind === 'PERIOD' && (r.periodNo ?? 0) <= opts.afterPeriod)
    .pop()
  const start = anchor ? anchor.endMin : opts.fallbackStart
  const end = start + opts.minutes

  // 끼워 넣은 구간 바로 다음 칸이 그 구간이 끝나는 시각에 시작하도록 맞춘다
  const next = sorted.find((r) => r.startMin >= start)
  const shift = next ? end - next.startMin : 0

  const moved =
    shift === 0
      ? rows
      : rows.map((r) =>
          r.startMin >= start
            ? { ...r, startMin: r.startMin + shift, endMin: r.endMin + shift }
            : r,
        )

  return [
    ...moved,
    {
      key: nextKey(),
      kind: opts.kind,
      periodNo: null,
      name: opts.name,
      startMin: start,
      endMin: end,
    },
  ]
}

/**
 * 점심·중간놀이를 앞뒤 칸과 자리바꿈한다.
 *
 * 두 칸이 차지하던 전체 구간은 그대로 두고 안에서만 순서를 바꾸므로,
 * 앞뒤 다른 교시의 시각은 전혀 달라지지 않는다.
 */
export function moveRow(rows: Row[], key: string, dir: -1 | 1): Row[] {
  const sorted = [...rows].sort(byStart)
  const i = sorted.findIndex((r) => r.key === key)
  const j = i + dir
  if (i < 0 || j < 0 || j >= sorted.length) return rows

  const [first, second] = dir === 1 ? [sorted[i], sorted[j]] : [sorted[j], sorted[i]]
  const gap = second.startMin - first.endMin
  const durFirst = first.endMin - first.startMin
  const durSecond = second.endMin - second.startMin
  const blockStart = first.startMin

  // 순서를 뒤집어 다시 배치한다
  const newSecondStart = blockStart
  const newSecondEnd = newSecondStart + durSecond
  const newFirstStart = newSecondEnd + gap
  const newFirstEnd = newFirstStart + durFirst

  const next = rows.map((r) => {
    if (r.key === first.key) return { ...r, startMin: newFirstStart, endMin: newFirstEnd }
    if (r.key === second.key) return { ...r, startMin: newSecondStart, endMin: newSecondEnd }
    return r
  })

  // 교시 번호는 항상 시간 순서를 따른다
  let n = 0
  return [...next]
    .sort(byStart)
    .map((r) => (r.kind === 'PERIOD' ? { ...r, periodNo: ++n } : r))
}

/** 이 칸을 위/아래로 옮길 수 있는가 (수업이 아닌 구간만 옮긴다) */
export function canMove(rows: Row[], key: string, dir: -1 | 1): boolean {
  const sorted = [...rows].sort(byStart)
  const i = sorted.findIndex((r) => r.key === key)
  if (i < 0 || sorted[i].kind === 'PERIOD') return false
  const j = i + dir
  return j >= 0 && j < sorted.length
}

export function hasRecess(rows: Row[]): boolean {
  return rows.some((r) => r.kind === 'OTHER')
}

export function removeRow(rows: Row[], key: string): Row[] {
  const sorted = [...rows].sort(byStart)
  const i = sorted.findIndex((r) => r.key === key)
  const target = i >= 0 ? sorted[i] : undefined
  let left = rows.filter((r) => r.key !== key)

  // 점심·중간놀이를 지우면 뒤 칸을 앞으로 당긴다.
  // 수업과 수업이 맞닿게 되면 원래 쉬는 시간만큼은 남긴다.
  if (target && target.kind !== 'PERIOD') {
    const prev = sorted[i - 1]
    const next = sorted[i + 1]
    if (next) {
      const newNextStart = prev
        ? prev.endMin +
          (prev.kind === 'PERIOD' && next.kind === 'PERIOD' ? detectBreak(rows) : 0)
        : next.startMin
      const delta = newNextStart - next.startMin
      if (delta !== 0) {
        const from = next.startMin
        left = left.map((r) =>
          r.startMin >= from ? { ...r, startMin: r.startMin + delta, endMin: r.endMin + delta } : r,
        )
      }
    }
  }

  // 교시 번호를 시간 순서대로 다시 매긴다
  let n = 0
  return [...left]
    .sort(byStart)
    .map((r) => (r.kind === 'PERIOD' ? { ...r, periodNo: ++n } : r))
}

export function patchRow(rows: Row[], key: string, patch: Partial<Row>): Row[] {
  return rows.map((r) => (r.key === key ? { ...r, ...patch } : r))
}

/** 한 요일의 시각을 다른 요일들에 그대로 복사한다. */
export function copyDay(st: EditorState, from: number, to: number[]): EditorState {
  const src = st.byDay[from] ?? []
  const byDay = { ...st.byDay }
  for (const d of to) {
    if (d === from) continue
    byDay[d] = src.map((r) => ({ ...r, key: nextKey() }))
  }
  return { ...st, byDay }
}

/** 공통 -> 요일별 (내용 그대로 펼친다) */
export function toPerDay(st: EditorState, schoolDays: number[]): EditorState {
  const byDay: Record<number, Row[]> = {}
  for (const day of schoolDays) {
    const count = st.periodsByDay[day] ?? 0
    byDay[day] =
      count <= 0
        ? []
        : st.base
            .filter((r) => r.kind !== 'PERIOD' || (r.periodNo ?? 0) <= count)
            .map((r) => ({ ...r, key: nextKey() }))
  }
  return { ...st, perDay: true, byDay }
}

/** 요일별 -> 공통 (행이 가장 많은 요일을 기준으로 통일한다) */
export function toCommon(st: EditorState, schoolDays: number[]): EditorState {
  const richestDay = schoolDays.reduce(
    (best, d) => ((st.byDay[d]?.length ?? 0) > (st.byDay[best]?.length ?? 0) ? d : best),
    schoolDays[0] ?? 1,
  )
  const base = (st.byDay[richestDay] ?? []).map((r) => ({ ...r, key: nextKey() }))
  const periodsByDay: Record<number, number> = {}
  for (const d of schoolDays) periodsByDay[d] = maxPeriod(st.byDay[d] ?? [])
  return { ...st, perDay: false, base, periodsByDay }
}

export function richestDayOf(st: EditorState, schoolDays: number[]): number {
  return schoolDays.reduce(
    (best, d) => ((st.byDay[d]?.length ?? 0) > (st.byDay[best]?.length ?? 0) ? d : best),
    schoolDays[0] ?? 1,
  )
}
