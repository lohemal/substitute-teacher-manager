/**
 * 보결 수당 교사별 계산 표의 정렬 규칙 확인.
 *
 *     node --experimental-strip-types scripts/check-sort-pay.ts
 */
import {
  paySortDir,
  sortPayRows,
  togglePaySort,
  type PaySort,
} from '../src/features/pay/sortPayRows.ts'
import type { PayRow } from '../src/ipc/pay.ts'

let fails = 0
function check(name: string, cond: boolean, extra?: unknown) {
  if (cond) console.log(`  ok   ${name}`)
  else {
    fails++
    console.log(`  FAIL ${name}`, extra ?? '')
  }
}

let nextId = 1
function r(
  name: string,
  roleCode: PayRow['roleCode'],
  substituted: number,
  amount: number,
  extra: Partial<PayRow> = {},
): PayRow {
  return {
    teacherId: nextId++,
    name,
    roleCode,
    roleLabel: roleCode === 'HOMEROOM' ? '담임' : roleCode === 'SPECIAL' ? '전담' : '기타',
    duty: '',
    active: true,
    substituted,
    ownCaused: 0,
    payable: substituted,
    perCase: 15000,
    amount,
    ...extra,
  }
}

const names = (rows: PayRow[]) => rows.map((x) => x.name).join(',')

// 서버가 주는 기본 순서 — 지급액 많은 사람부터
const BASE: PayRow[] = [
  r('김민수', 'SPECIAL', 3, 45000),
  r('박도현', 'HOMEROOM', 2, 30000),
  r('이도현', 'HOMEROOM', 2, 30000),
  r('최가람', 'OTHER', 10, 15000),
  r('강나리', 'HOMEROOM', 100, 15000),
]
const BASE_ORDER = names(BASE)

console.log('\n=== 보결 수당 표 정렬 ===')

// ---------- 기본 상태 ----------
check('기준이 없으면 받은 순서 그대로', names(sortPayRows(BASE, null)) === BASE_ORDER)
check('기준이 없으면 같은 배열을 돌려준다', sortPayRows(BASE, null) === BASE)

// ---------- 세 번 눌러 한 바퀴 ----------
{
  let s: PaySort = null
  s = togglePaySort(s, 'amount')
  check('1번 누르면 오름차순', s?.field === 'amount' && s.dir === 'asc', s)
  s = togglePaySort(s, 'amount')
  check('2번 누르면 내림차순', s?.field === 'amount' && s.dir === 'desc', s)
  s = togglePaySort(s, 'amount')
  check('3번 누르면 해제', s === null, s)
  s = togglePaySort(s, 'amount')
  check('4번 누르면 다시 오름차순', s?.field === 'amount' && s.dir === 'asc', s)
}

// ---------- 다른 열을 누르면 앞 기준이 풀린다 ----------
{
  const s = togglePaySort({ field: 'amount', dir: 'desc' }, 'name')
  check('다른 열을 누르면 그 열의 오름차순', s?.field === 'name' && s.dir === 'asc', s)
  check('앞 열은 기준이 아니다', paySortDir(s, 'amount') === null)
  check('누른 열만 기준이다', paySortDir(s, 'name') === 'asc')
}

// ---------- 교사명 (한국어 사전 순) ----------
{
  const asc = sortPayRows(BASE, { field: 'name', dir: 'asc' })
  check('교사명 오름차순은 가나다순', names(asc) === '강나리,김민수,박도현,이도현,최가람', names(asc))
  const desc = sortPayRows(BASE, { field: 'name', dir: 'desc' })
  check('교사명 내림차순은 역순', names(desc) === '최가람,이도현,박도현,김민수,강나리', names(desc))
}

// ---------- 구분 ----------
{
  const asc = sortPayRows(BASE, { field: 'role', dir: 'asc' })
  check(
    '구분 오름차순은 담임 → 전담 → 기타',
    asc.map((x) => x.roleCode).join(',') === 'HOMEROOM,HOMEROOM,HOMEROOM,SPECIAL,OTHER',
    asc.map((x) => x.roleCode),
  )
  const desc = sortPayRows(BASE, { field: 'role', dir: 'desc' })
  check(
    '구분 내림차순은 그 반대',
    desc.map((x) => x.roleCode).join(',') === 'OTHER,SPECIAL,HOMEROOM,HOMEROOM,HOMEROOM',
    desc.map((x) => x.roleCode),
  )
}

// ---------- 숫자는 글자가 아니라 숫자로 ----------
{
  const asc = sortPayRows(BASE, { field: 'substituted', dir: 'asc' })
  check(
    '횟수 오름차순 2 → 3 → 10 → 100',
    asc.map((x) => x.substituted).join(',') === '2,2,3,10,100',
    asc.map((x) => x.substituted),
  )
  const desc = sortPayRows(BASE, { field: 'substituted', dir: 'desc' })
  check(
    '횟수 내림차순 100 → 10 → 3 → 2',
    desc.map((x) => x.substituted).join(',') === '100,10,3,2,2',
    desc.map((x) => x.substituted),
  )
}

// ---------- 금액 ----------
{
  const asc = sortPayRows(BASE, { field: 'amount', dir: 'asc' })
  check(
    '지급액 오름차순 15,000 → 30,000 → 45,000',
    asc.map((x) => x.amount).join(',') === '15000,15000,30000,30000,45000',
    asc.map((x) => x.amount),
  )
  const desc = sortPayRows(BASE, { field: 'amount', dir: 'desc' })
  check(
    '지급액 내림차순 45,000 → 30,000 → 15,000',
    desc.map((x) => x.amount).join(',') === '45000,30000,30000,15000,15000',
    desc.map((x) => x.amount),
  )
}

// ---------- 같은 값은 받은 순서를 지킨다 (안정 정렬) ----------
{
  const asc = sortPayRows(BASE, { field: 'amount', dir: 'asc' })
  check(
    '15,000원 둘은 받은 순서대로 최가람 → 강나리',
    names(asc).startsWith('최가람,강나리'),
    names(asc),
  )
  check(
    '30,000원 둘도 받은 순서대로 박도현 → 이도현',
    names(asc).includes('박도현,이도현'),
    names(asc),
  )
  const desc = sortPayRows(BASE, { field: 'amount', dir: 'desc' })
  check(
    '내림차순에서도 같은 값끼리는 받은 순서 그대로',
    names(desc) === '김민수,박도현,이도현,최가람,강나리',
    names(desc),
  )
}

// ---------- 원본을 건드리지 않는다 ----------
{
  const before = names(BASE)
  sortPayRows(BASE, { field: 'amount', dir: 'asc' })
  sortPayRows(BASE, { field: 'name', dir: 'desc' })
  check('정렬해도 받은 배열은 그대로', names(BASE) === before, names(BASE))
  check('해제하면 원래 순서로 돌아온다', names(sortPayRows(BASE, null)) === BASE_ORDER)
}

// ---------- 교사를 가리키는 것은 번호이지 자리가 아니다 ----------
{
  const desc = sortPayRows(BASE, { field: 'amount', dir: 'desc' })
  const first = desc[0]
  const same = BASE.find((x) => x.teacherId === first.teacherId)
  check('정렬해도 행과 교사 번호가 함께 움직인다', same?.name === first.name, first)
  check('교사 번호가 겹치지 않는다', new Set(desc.map((x) => x.teacherId)).size === desc.length)
}

// ---------- 값은 하나도 달라지지 않는다 ----------
{
  const sum = (rows: PayRow[]) => rows.reduce((n, x) => n + x.amount, 0)
  const cnt = (rows: PayRow[]) => rows.reduce((n, x) => n + x.payable, 0)
  const base = { amount: sum(BASE), payable: cnt(BASE), len: BASE.length }
  for (const f of ['name', 'role', 'substituted', 'ownCaused', 'payable', 'perCase', 'amount'] as const) {
    for (const d of ['asc', 'desc'] as const) {
      const out = sortPayRows(BASE, { field: f, dir: d })
      if (sum(out) !== base.amount || cnt(out) !== base.payable || out.length !== base.len) {
        check(`${f} ${d} 에서 합계가 그대로`, false, { sum: sum(out), cnt: cnt(out) })
      }
    }
  }
  check('어떤 정렬에서도 합계·인정 횟수·행 수가 같다', true)
}

// ---------- 가장자리 ----------
{
  check('빈 목록도 오류가 없다', sortPayRows([], { field: 'amount', dir: 'asc' }).length === 0)
  const one = [r('혼자', 'HOMEROOM', 1, 15000)]
  check('한 명이어도 그대로', names(sortPayRows(one, { field: 'name', dir: 'desc' })) === '혼자')
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
process.exit(fails === 0 ? 0 : 1)
