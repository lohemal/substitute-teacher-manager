/**
 * 보결 수당 '교사별 계산' 표의 정렬. (순수 계산 — 화면을 모른다)
 *
 * ## 현황 표와 다르게 한 곳
 *
 * 보결 현황(`sortTeachers`)은 기준을 여러 개 쌓을 수 있지만, 여기는 **한 번에
 * 한 열만** 본다. 수당 표는 '지급액 많은 사람부터' 처럼 한 가지로 훑어보는
 * 자리라, 기준이 쌓이면 지금 무엇으로 줄 세운 것인지 알기 어려워진다.
 *
 * 머리글을 누르면 **오름차순 → 내림차순 → 해제** 순으로 돈다. 해제는 교사명
 * 순으로 바꾸는 것이 아니라 **서버가 보내 준 그대로**(지급액 많은 순)로
 * 돌아가는 것이다.
 *
 * ## 받은 배열을 건드리지 않는다
 *
 * 늘 복사본을 정렬해 돌려준다. 원본이 바뀌면 '해제' 로 돌아갈 자리가 사라진다.
 *
 * ## 같은 값은 받은 순서를 지킨다
 *
 * 지급액이 같은 사람이 여럿일 때 누를 때마다 자리가 뒤바뀌면 읽기 어렵다.
 * 그래서 번호를 붙여 비교해 **안정 정렬**로 만든다. (`Array#sort` 는 요즘
 * 모두 안정이지만, 뜻을 코드에 드러내 두는 편이 낫다)
 */

import type { PayRow } from '@/ipc/pay'

export type PaySortField =
  | 'name'
  | 'role'
  | 'substituted'
  | 'ownCaused'
  | 'payable'
  | 'perCase'
  | 'amount'

export type PaySortDir = 'asc' | 'desc'

/** 정렬하지 않는 상태는 `null` */
export type PaySort = { field: PaySortField; dir: PaySortDir } | null

/** 구분은 글자 순이 아니라 담임 → 전담 → 기타 순으로 본다 */
const ROLE_ORDER: Record<string, number> = { HOMEROOM: 0, SPECIAL: 1, OTHER: 2 }

/** 한국어 이름을 사전 순으로 (ㄱㄴㄷ…) */
const collator = new Intl.Collator('ko-KR', { numeric: true, sensitivity: 'base' })

function valueOf(r: PayRow, field: PaySortField): number | string {
  switch (field) {
    case 'name':
      return r.name
    case 'role':
      return ROLE_ORDER[r.roleCode] ?? 9
    default:
      // 횟수와 금액은 **숫자 그대로** 본다. 화면의 '15,000원' 같은 글자를
      // 비교하면 100 이 2 보다 앞에 오게 된다.
      return r[field]
  }
}

/**
 * 머리글을 한 번 눌렀을 때의 다음 상태.
 *
 * 같은 열: 오름차순 → 내림차순 → 해제 → (다시 오름차순)
 * 다른 열: 그 열의 오름차순으로 옮겨 간다 (앞의 기준은 풀린다)
 */
export function togglePaySort(cur: PaySort, field: PaySortField): PaySort {
  if (cur?.field !== field) return { field, dir: 'asc' }
  if (cur.dir === 'asc') return { field, dir: 'desc' }
  return null
}

/** 정렬한 **새 배열**. 기준이 없으면 받은 순서 그대로 돌려준다. */
export function sortPayRows(rows: PayRow[], sort: PaySort): PayRow[] {
  if (!sort) return rows

  const dir = sort.dir === 'asc' ? 1 : -1
  return rows
    .map((row, at) => ({ row, at }))
    .sort((x, y) => {
      const a = valueOf(x.row, sort.field)
      const b = valueOf(y.row, sort.field)
      let d: number
      if (typeof a === 'number' && typeof b === 'number') d = a - b
      else d = collator.compare(String(a), String(b))
      // 값이 같으면 받은 순서를 지킨다
      return d === 0 ? x.at - y.at : d * dir
    })
    .map((w) => w.row)
}

/** 이 열이 지금 어떤 방향으로 정렬되어 있는가. 아니면 `null` */
export function paySortDir(cur: PaySort, field: PaySortField): PaySortDir | null {
  return cur?.field === field ? cur.dir : null
}
