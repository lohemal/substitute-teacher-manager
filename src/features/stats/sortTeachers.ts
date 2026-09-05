/**
 * 교사별 현황 표의 정렬. (순수 계산 — 화면을 모른다)
 *
 * 머리글을 누르면 **오름차순 → 내림차순 → 해제** 순으로 돈다.
 * 여러 열을 눌러 기준을 쌓을 수 있고, **먼저 누른 열이 우선**한다.
 * 기준은 최대 5개까지 두며, 그보다 많이 누르면 가장 나중에 넣은 기준을
 * 새 기준으로 바꾼다 (먼저 정한 우선순위는 지킨다).
 */

import type { TeacherStat } from '@/ipc/stats'

export type SortField =
  | 'name'
  | 'role'
  | 'duty'
  | 'period'
  | 'today'
  | 'week'
  | 'month'
  | 'term'
  | 'total'
  | 'band'

export type Dir = 'asc' | 'desc'

export interface SortKey {
  field: SortField
  dir: Dir
}

export const MAX_SORT = 5

/** 구분은 이름 순이 아니라 담임 → 전담 → 기타 순으로 본다 */
const ROLE_ORDER: Record<string, number> = { HOMEROOM: 0, SPECIAL: 1, OTHER: 2 }

/** 참고 표시는 적음 → 평균 → 많음 순. 표시가 없는 사람은 언제나 끝으로 */
const BAND_ORDER: Record<string, number> = { LESS: 0, TYPICAL: 1, MORE: 2 }

/** 값이 없으면 `null` — 오름·내림과 상관없이 늘 끝으로 보낸다 */
function valueOf(t: TeacherStat, field: SortField): number | string | null {
  switch (field) {
    case 'name':
      return t.name
    case 'role':
      return ROLE_ORDER[t.roleCode] ?? 9
    case 'duty':
      return t.duty || null
    case 'band':
      return BAND_ORDER[t.band] ?? null
    default:
      return t[field]
  }
}

function compare(a: number | string | null, b: number | string | null, dir: Dir): number {
  // 값이 없는 쪽은 방향과 상관없이 뒤로
  if (a === null && b === null) return 0
  if (a === null) return 1
  if (b === null) return -1

  let d: number
  if (typeof a === 'number' && typeof b === 'number') d = a - b
  else d = String(a).localeCompare(String(b), 'ko')

  return dir === 'asc' ? d : -d
}

/** 머리글을 한 번 눌렀을 때의 다음 상태. */
export function toggleSort(keys: SortKey[], field: SortField): SortKey[] {
  const at = keys.findIndex((k) => k.field === field)

  // 이미 기준인 열 — 오름차순이면 내림차순으로, 내림차순이면 해제
  if (at >= 0) {
    if (keys[at].dir === 'asc') {
      const next = keys.slice()
      next[at] = { field, dir: 'desc' }
      return next
    }
    return keys.filter((k) => k.field !== field)
  }

  // 새 기준 — 뒤에 붙인다 (먼저 누른 것이 우선)
  if (keys.length < MAX_SORT) return [...keys, { field, dir: 'asc' }]

  // 가득 찼으면 맨 뒤(우선순위가 가장 낮은 것)를 바꾼다
  return [...keys.slice(0, MAX_SORT - 1), { field, dir: 'asc' }]
}

/**
 * 기준 순서대로 정렬한다. 기준이 없으면 받은 순서를 그대로 둔다
 * (서버가 구분 → 이름 순으로 보내 준다).
 */
export function sortTeachers(rows: TeacherStat[], keys: SortKey[]): TeacherStat[] {
  if (keys.length === 0) return rows

  return rows.slice().sort((x, y) => {
    for (const k of keys) {
      const d = compare(valueOf(x, k.field), valueOf(y, k.field), k.dir)
      if (d !== 0) return d
    }
    // 끝까지 같으면 이름 순 — 같은 자료면 늘 같은 순서로 보이게 한다
    return x.name.localeCompare(y.name, 'ko')
  })
}

/** 이 열이 몇 번째 기준인지 (1부터). 기준이 아니면 0 */
export function sortRank(keys: SortKey[], field: SortField): number {
  const at = keys.findIndex((k) => k.field === field)
  return at < 0 ? 0 : at + 1
}

export function sortDir(keys: SortKey[], field: SortField): Dir | null {
  return keys.find((k) => k.field === field)?.dir ?? null
}
