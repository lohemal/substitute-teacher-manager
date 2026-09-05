import {
  MAX_SORT,
  sortDir,
  sortRank,
  sortTeachers,
  toggleSort,
  type SortKey,
} from '../src/features/stats/sortTeachers.ts'
import type { TeacherStat } from '../src/ipc/stats.ts'

let fails = 0
function check(name: string, cond: boolean, extra?: unknown) {
  if (cond) console.log(`  ok   ${name}`)
  else {
    fails++
    console.log(`  FAIL ${name}`, extra ?? '')
  }
}

function t(
  name: string,
  roleCode: TeacherStat['roleCode'],
  counts: Partial<TeacherStat> = {},
): TeacherStat {
  return {
    teacherId: name.charCodeAt(0),
    name,
    roleCode,
    roleLabel: roleCode === 'HOMEROOM' ? '담임' : roleCode === 'SPECIAL' ? '전담' : '기타',
    duty: '',
    active: true,
    isSubstitutable: true,
    period: 0,
    today: 0,
    week: 0,
    month: 0,
    term: 0,
    total: 0,
    band: 'NONE',
    bandLabel: '—',
    lastDate: null,
    ...counts,
  }
}

const names = (rows: TeacherStat[]) => rows.map((r) => r.name).join(',')

// ---------- 머리글을 누르면 오름 → 내림 → 해제 ----------
{
  let k: SortKey[] = []
  k = toggleSort(k, 'total')
  check('처음 누르면 오름차순', k.length === 1 && k[0].dir === 'asc', k)
  k = toggleSort(k, 'total')
  check('한 번 더 누르면 내림차순', k.length === 1 && k[0].dir === 'desc', k)
  k = toggleSort(k, 'total')
  check('세 번째에 해제된다', k.length === 0, k)
}

// ---------- 먼저 누른 열이 우선 ----------
{
  let k: SortKey[] = []
  k = toggleSort(k, 'role')
  k = toggleSort(k, 'total')
  check('누른 순서대로 쌓인다', k[0].field === 'role' && k[1].field === 'total', k)
  check('첫 번째 기준은 1순위', sortRank(k, 'role') === 1)
  check('두 번째 기준은 2순위', sortRank(k, 'total') === 2)
  check('기준이 아니면 0', sortRank(k, 'name') === 0)
  check('방향을 알 수 있다', sortDir(k, 'total') === 'asc' && sortDir(k, 'name') === null)

  // 앞 기준의 방향만 바꿔도 순서는 그대로
  k = toggleSort(k, 'role')
  check('방향을 바꿔도 우선순위는 그대로', k[0].field === 'role' && k[0].dir === 'desc', k)

  // 가운데 것을 해제하면 뒤가 앞으로 당겨진다
  k = toggleSort(k, 'role')
  check('해제하면 뒤 기준이 1순위가 된다', sortRank(k, 'total') === 1, k)
}

// ---------- 최대 5개 ----------
{
  let k: SortKey[] = []
  for (const f of ['role', 'today', 'week', 'month', 'term'] as const) k = toggleSort(k, f)
  check('다섯 개까지 쌓인다', k.length === MAX_SORT, k)

  k = toggleSort(k, 'total')
  check('여섯 번째에도 다섯 개를 넘지 않는다', k.length === MAX_SORT, k)
  check('앞 네 기준은 그대로', k.slice(0, 4).map((x) => x.field).join(',') === 'role,today,week,month', k)
  check('가장 낮은 기준이 새 기준으로 바뀐다', k[4].field === 'total', k)
}

// ---------- 실제 정렬 ----------
const rows: TeacherStat[] = [
  t('김담임', 'HOMEROOM', { total: 5, today: 1, duty: '1-가람', band: 'MORE' }),
  t('박전담', 'SPECIAL', { total: 5, today: 0, duty: '영어', band: 'TYPICAL' }),
  t('이담임', 'HOMEROOM', { total: 2, today: 3, band: 'LESS' }),
  t('최기타', 'OTHER', { total: 9, today: 0, duty: '교감' }),
]

{
  check('기준이 없으면 받은 순서 그대로', names(sortTeachers(rows, [])) === '김담임,박전담,이담임,최기타')
}

{
  const asc = sortTeachers(rows, [{ field: 'total', dir: 'asc' }])
  check('숫자 오름차순', names(asc) === '이담임,김담임,박전담,최기타', names(asc))
  const desc = sortTeachers(rows, [{ field: 'total', dir: 'desc' }])
  check('숫자 내림차순', names(desc) === '최기타,김담임,박전담,이담임', names(desc))
}

{
  // 누적이 같은 두 사람은 이름 순으로 갈린다
  const r = sortTeachers(rows, [{ field: 'total', dir: 'asc' }])
  check('동점이면 이름 순', r[1].name === '김담임' && r[2].name === '박전담', names(r))
}

{
  // 구분 우선 + 누적 내림차순
  const r = sortTeachers(rows, [
    { field: 'role', dir: 'asc' },
    { field: 'total', dir: 'desc' },
  ])
  check('구분은 담임 → 전담 → 기타 순', names(r) === '김담임,이담임,박전담,최기타', names(r))
}

{
  const r = sortTeachers(rows, [
    { field: 'role', dir: 'desc' },
    { field: 'total', dir: 'asc' },
  ])
  check('구분 내림차순', names(r) === '최기타,박전담,이담임,김담임', names(r))
}

{
  const r = sortTeachers(rows, [{ field: 'name', dir: 'asc' }])
  check('이름 오름차순 (한글)', names(r) === '김담임,박전담,이담임,최기타', names(r))
}

// ---------- 값이 없으면 늘 끝으로 ----------
{
  const asc = sortTeachers(rows, [{ field: 'duty', dir: 'asc' }])
  check('담당이 빈 사람은 오름차순에서 끝', asc[asc.length - 1].name === '이담임', names(asc))
  const desc = sortTeachers(rows, [{ field: 'duty', dir: 'desc' }])
  check('내림차순에서도 끝', desc[desc.length - 1].name === '이담임', names(desc))
}

{
  const asc = sortTeachers(rows, [{ field: 'band', dir: 'asc' }])
  check('참고 표시는 적음 → 평균 → 많음', names(asc).startsWith('이담임,박전담,김담임'), names(asc))
  check('표시 없는 사람은 끝', asc[3].name === '최기타', names(asc))
  const desc = sortTeachers(rows, [{ field: 'band', dir: 'desc' }])
  check('내림차순에서도 표시 없는 사람은 끝', desc[3].name === '최기타', names(desc))
}

// ---------- 원본을 건드리지 않는다 ----------
{
  const before = names(rows)
  sortTeachers(rows, [{ field: 'total', dir: 'desc' }])
  check('원본 배열은 그대로', names(rows) === before)
}

// ---------- 같은 자료면 늘 같은 순서 ----------
{
  const flat = [t('가', 'HOMEROOM'), t('나', 'HOMEROOM'), t('다', 'HOMEROOM')]
  const a = names(sortTeachers(flat, [{ field: 'total', dir: 'asc' }]))
  const b = names(sortTeachers(flat.slice().reverse(), [{ field: 'total', dir: 'asc' }]))
  check('모두 같은 값이어도 결과가 같다', a === b && a === '가,나,다', `${a} / ${b}`)
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
process.exit(fails === 0 ? 0 : 1)
