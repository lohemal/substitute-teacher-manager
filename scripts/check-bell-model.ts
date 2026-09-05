import {
  derive, expand, emptyState, toPerDay, toCommon, copyDay,
  addPeriod, addLunch, addRecess, moveRow, canMove, removeRow, rowLabel,
} from '../src/features/bell/bellModel.ts'
import { parseTimeText, minToHm } from '../src/lib/time.ts'
import type { Slot } from '../src/ipc/bell.ts'

const DAYS = [1, 2, 3, 4, 5]
let fails = 0
function check(name: string, cond: boolean, extra?: unknown) {
  if (cond) console.log(`  ok   ${name}`)
  else {
    fails++
    console.log(`  FAIL ${name}`, extra ?? '')
  }
}

function p(day: number, no: number, start: number, end: number): Slot {
  return { dayOfWeek: day, slotType: 'PERIOD', periodNo: no, label: `${no}교시`, startMin: start, endMin: end }
}
function lunch(day: number, start: number, end: number): Slot {
  return { dayOfWeek: day, slotType: 'LUNCH', periodNo: null, label: '점심', startMin: start, endMin: end }
}

const key = (s: Slot) => `${s.dayOfWeek}|${s.slotType}|${s.periodNo ?? '-'}|${s.startMin}|${s.endMin}`
const same = (a: Slot[], b: Slot[]) =>
  a.length === b.length && a.map(key).sort().join(',') === b.map(key).sort().join(',')

// 1) 모든 요일 같은 시각 -> 공통 모드로 인식하고 그대로 복원
{
  const slots: Slot[] = []
  for (const d of DAYS) {
    slots.push(p(d, 1, 540, 580), p(d, 2, 590, 630), p(d, 3, 640, 680), p(d, 4, 690, 730))
    slots.push(lunch(d, 730, 780))
    slots.push(p(d, 5, 780, 820))
  }
  const st = derive(slots, DAYS)
  check('모든 요일 동일 -> 공통 모드', st.perDay === false)
  check('공통 모드 왕복 변환이 원본과 같다', same(expand(st, DAYS), slots))
  check('요일별 교시 수를 5로 읽는다', DAYS.every((d) => st.periodsByDay[d] === 5))
}

// 2) 요일마다 교시 수만 다름 -> 여전히 공통 모드
{
  const counts: Record<number, number> = { 1: 6, 2: 6, 3: 4, 4: 6, 5: 5 }
  const slots: Slot[] = []
  const times: [number, number][] = [[540, 580], [590, 630], [640, 680], [690, 730], [780, 820], [830, 870]]
  for (const d of DAYS) {
    for (let n = 1; n <= counts[d]; n++) slots.push(p(d, n, times[n - 1][0], times[n - 1][1]))
    slots.push(lunch(d, 730, 780))
  }
  const st = derive(slots, DAYS)
  check('교시 수만 다르면 공통 모드', st.perDay === false)
  check('수요일 4교시로 읽는다', st.periodsByDay[3] === 4)
  check('교시 수만 다를 때 왕복 변환 일치', same(expand(st, DAYS), slots), expand(st, DAYS).length + ' vs ' + slots.length)
}

// 3) 요일마다 시각이 다름 -> 요일별 모드로 전환하고 원본 보존
{
  const slots: Slot[] = []
  for (const d of DAYS) {
    const shift = d === 3 ? 10 : 0 // 수요일만 10분 늦게 시작
    slots.push(p(d, 1, 540 + shift, 580 + shift), p(d, 2, 590 + shift, 630 + shift))
  }
  const st = derive(slots, DAYS)
  check('요일마다 시각이 다르면 요일별 모드', st.perDay === true)
  check('요일별 모드 왕복 변환이 원본과 같다', same(expand(st, DAYS), slots))
}

// 4) 공통 <-> 요일별 전환이 내용을 잃지 않는다
{
  const slots: Slot[] = []
  for (const d of DAYS) {
    slots.push(p(d, 1, 540, 580), p(d, 2, 590, 630), lunch(d, 630, 680), p(d, 3, 680, 720))
  }
  const st = derive(slots, DAYS)
  const perDay = toPerDay(st, DAYS)
  check('공통 -> 요일별 전환 시 내용 보존', same(expand(perDay, DAYS), slots))
  const back = toCommon(perDay, DAYS)
  check('요일별 -> 공통 전환 시 내용 보존', same(expand(back, DAYS), slots))
}

// 5) 요일 복사
{
  const slots: Slot[] = [p(1, 1, 540, 580), p(1, 2, 590, 630)]
  const st = toPerDay(derive(slots, DAYS), DAYS)
  const copied = copyDay(st, 1, [2, 3])
  const out = expand(copied, DAYS)
  check('복사한 요일에 같은 시각이 생긴다', out.filter((s) => s.dayOfWeek === 3).length === 2)
  check('복사해도 원본 요일은 그대로', out.filter((s) => s.dayOfWeek === 1).length === 2)
  const tue = out.filter((s) => s.dayOfWeek === 2).map((s) => `${s.startMin}-${s.endMin}`).sort()
  check('복사된 시각이 원본과 동일', tue.join() === '540-580,590-630', tue)
}

// 6) 교시 추가 / 삭제
{
  let st = emptyState(DAYS)
  check('빈 상태는 공통 모드', st.perDay === false && st.base.length === 0)
  let rows = addPeriod(st.base)
  rows = addPeriod(rows)
  check('교시를 추가하면 번호가 1,2로 매겨진다', rows.map((r) => r.periodNo).join() === '1,2')
  check('두 번째 교시는 쉬는시간 뒤에 붙는다', rows[1].startMin === rows[0].endMin + 10, rows)
  const after = removeRow(rows, rows[0].key)
  check('중간 교시를 지우면 번호를 다시 매긴다', after.map((r) => r.periodNo).join() === '1')
}

// 7) 교시 수 0인 요일은 아예 빠진다
{
  const slots: Slot[] = []
  for (const d of DAYS) slots.push(p(d, 1, 540, 580))
  const st = derive(slots, DAYS)
  st.periodsByDay[5] = 0
  const out = expand(st, DAYS)
  check('교시 수 0이면 그 요일 슬롯이 없다', out.filter((s) => s.dayOfWeek === 5).length === 0)
  check('다른 요일은 유지', out.length === 4)
}

const times = (rows: { startMin: number; endMin: number }[]) =>
  [...rows]
    .sort((a, b) => a.startMin - b.startMin)
    .map((r) => `${r.startMin}-${r.endMin}`)
    .join()

const labels = (rows: Parameters<typeof rowLabel>[0][]) =>
  [...rows].sort((a, b) => a.startMin - b.startMin).map(rowLabel).join()

// 8) 중간놀이를 넣으면 뒤 교시가 밀리고, 지우면 다시 당겨진다
{
  let rows = addPeriod(addPeriod(addPeriod([]))) // 09:00-09:40, 09:50-10:30, 10:40-11:20
  const before = times(rows)
  check('세 교시가 10분 간격으로 만들어진다', before === '540-580,590-630,640-680', before)

  rows = addRecess(rows, 2, 30)
  const st = [...rows].sort((a, b) => a.startMin - b.startMin)
  check('중간놀이가 2교시 바로 뒤에 붙는다', st[2].startMin === 630 && st[2].endMin === 660, st[2])
  check('중간놀이 뒤 3교시가 바로 이어진다', st[3].startMin === 660 && st[3].endMin === 700, st[3])
  check('중간놀이 앞 교시는 그대로', st[0].startMin === 540 && st[1].startMin === 590)

  const removed = removeRow(rows, st[2].key)
  check('중간놀이를 지우면 뒤 교시가 원래대로 당겨진다', times(removed) === before, times(removed))
}

// 9) 점심을 넣어도 뒤가 밀린다
{
  let rows = addPeriod(addPeriod(addPeriod(addPeriod([]))))
  rows = addLunch(rows, 3, 50)
  const st = [...rows].sort((a, b) => a.startMin - b.startMin)
  check('점심이 3교시 뒤에 들어간다', rowLabel(st[3]) === '점심' && st[3].startMin === 680, st[3])
  check('점심 뒤 4교시가 50분 밀린다', st[4].startMin === 730 && st[4].endMin === 770, st[4])
}

// 10) 중간놀이 위치 이동 — 바깥 교시 시각은 전혀 바뀌지 않는다
{
  let rows = addPeriod(addPeriod(addPeriod(addPeriod([]))))
  rows = addRecess(rows, 2, 30)
  let st = [...rows].sort((a, b) => a.startMin - b.startMin)
  const recessKey = st.find((r) => r.kind === 'OTHER')!.key
  const firstStart = st[0].startMin
  const lastEnd = st[st.length - 1].endMin

  check('처음에는 2·3교시 사이', labels(rows) === '1교시,2교시,중간놀이,3교시,4교시', labels(rows))

  rows = moveRow(rows, recessKey, 1)
  st = [...rows].sort((a, b) => a.startMin - b.startMin)
  check('아래로 옮기면 3·4교시 사이', labels(rows) === '1교시,2교시,3교시,중간놀이,4교시', labels(rows))
  check('옮겨도 하루 시작 시각은 그대로', st[0].startMin === firstStart)
  check('옮겨도 하루 종료 시각은 그대로', st[st.length - 1].endMin === lastEnd)
  check(
    '교시 번호가 시간 순서를 유지한다',
    st.filter((r) => r.kind === 'PERIOD').map((r) => r.periodNo).join() === '1,2,3,4',
  )

  rows = moveRow(rows, recessKey, -1)
  check('위로 옮기면 되돌아온다', labels(rows) === '1교시,2교시,중간놀이,3교시,4교시', labels(rows))
}

// 11) 옮길 수 있는 칸 판별
{
  let rows = addPeriod(addPeriod([]))
  rows = addRecess(rows, 1, 20)
  const st = [...rows].sort((a, b) => a.startMin - b.startMin)
  const recess = st.find((r) => r.kind === 'OTHER')!
  const period1 = st.find((r) => r.periodNo === 1)!
  check('중간놀이는 위아래로 옮길 수 있다', canMove(rows, recess.key, -1) && canMove(rows, recess.key, 1))
  check('교시 자체는 옮길 수 없다', !canMove(rows, period1.key, 1))
  check('맨 앞 칸은 위로 못 옮긴다', !canMove(rows, st[0].key, -1))
}

// 12) 시각 입력 해석
{
  check('0940 -> 09:40', parseTimeText('0940') === 580)
  check('940 -> 09:40', parseTimeText('940') === 580)
  check('9:40 -> 09:40', parseTimeText('9:40') === 580)
  check('09:40 -> 09:40', parseTimeText('09:40') === 580)
  check('9 -> 09:00', parseTimeText('9') === 540)
  check('9.40 -> 09:40', parseTimeText('9.40') === 580)
  check('24:00 허용', parseTimeText('24:00') === 1440)
  check('25:00 거부', parseTimeText('25:00') === null)
  check('09:70 거부', parseTimeText('09:70') === null)
  check('빈 값 거부', parseTimeText('   ') === null)
  check('글자 거부', parseTimeText('아홉시') === null)
  check('분이 그대로 표시된다', minToHm(580) === '09:40' && minToHm(600) === '10:00')
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
if (fails > 0) process.exit(1)
