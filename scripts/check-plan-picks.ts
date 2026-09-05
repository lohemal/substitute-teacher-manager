import {
  flagPicks,
  initialPicks,
  repeatIndex,
  type PickSlot,
} from '../src/features/assign/planPicks.ts'

let fails = 0
function check(name: string, cond: boolean, extra?: unknown) {
  if (cond) console.log(`  ok   ${name}`)
  else {
    fails++
    console.log(`  FAIL ${name}`, extra ?? '')
  }
}

const hm = (h: number, m: number) => h * 60 + m

/** 한 반의 1~n교시. 교시끼리는 시간이 겹치지 않는다. */
function periods(cands: number[][]): PickSlot[] {
  return cands.map((ids, i) => ({
    key: `p${i + 1}`,
    startMin: hm(9, 0) + i * 45,
    endMin: hm(9, 40) + i * 45,
    candidateIds: ids,
  }))
}

// ---------- 처음에는 언제나 1순위 ----------
{
  const slots = periods([
    [10, 11],
    [10, 11],
    [12, 10],
  ])
  const p = initialPicks(slots)
  check('모든 칸에 1순위를 고른다', p.p1 === 10 && p.p2 === 10 && p.p3 === 12, p)
}

{
  const slots = periods([[10], [], [11]])
  const p = initialPicks(slots)
  check('후보가 없으면 미배정', p.p2 === null)
}

{
  const slots = periods([[10, 11], [10, 11]])
  slots[0].taken = true
  const p = initialPicks(slots)
  check('이미 배정된 칸은 고르지 않는다', p.p1 === null && p.p2 === 10, p)
}

// ---------- 같은 사람이 여러 번이면 '중복' ----------
{
  const slots = periods([
    [10, 11],
    [11, 10],
    [10, 12],
  ])
  const p = initialPicks(slots) // 10 / 11 / 10
  const f = flagPicks(slots, p)
  check('1교시와 3교시가 같은 사람이면 둘 다 중복', f.p1 === 'DUPLICATE' && f.p3 === 'DUPLICATE', f)
  check('한 번만 들어간 칸은 표시 없음', f.p2 === 'NONE', f)
  check('같은 사람은 자동으로 바뀌지 않는다', p.p3 === 10, p)
}

{
  const slots = periods([
    [10, 11],
    [11, 10],
    [12, 10],
  ])
  const f = flagPicks(slots, initialPicks(slots)) // 10 / 11 / 12
  check('모두 다른 사람이면 표시 없음', Object.values(f).every((v) => v === 'NONE'), f)
}

{
  const slots = periods([[10], [10], [10]])
  const f = flagPicks(slots, initialPicks(slots))
  check('세 칸 모두 같은 사람이면 셋 다 중복', Object.values(f).every((v) => v === 'DUPLICATE'), f)
}

// ---------- 미배정은 중복이 아니다 ----------
{
  const slots = periods([[], [], [10]])
  const f = flagPicks(slots, initialPicks(slots))
  check('빈 칸끼리는 중복이 아니다', f.p1 === 'NONE' && f.p2 === 'NONE', f)
}

// ---------- 시각이 겹치면 '겹침' ----------
{
  // 복식학급처럼 같은 시각의 칸이 둘일 때
  const slots: PickSlot[] = [
    { key: 'a', startMin: hm(9, 0), endMin: hm(9, 40), candidateIds: [10, 11] },
    { key: 'b', startMin: hm(9, 0), endMin: hm(9, 40), candidateIds: [10, 11] },
  ]
  const f = flagPicks(slots, initialPicks(slots)) // 둘 다 10
  check('같은 시각 두 칸은 겹침으로 표시', f.a === 'OVERLAP' && f.b === 'OVERLAP', f)
}

{
  // 한쪽만 바꾸면 풀린다
  const slots: PickSlot[] = [
    { key: 'a', startMin: hm(9, 0), endMin: hm(9, 40), candidateIds: [10, 11] },
    { key: 'b', startMin: hm(9, 0), endMin: hm(9, 40), candidateIds: [10, 11] },
  ]
  const f = flagPicks(slots, { a: 10, b: 11 })
  check('서로 다른 사람이면 겹침이 풀린다', f.a === 'NONE' && f.b === 'NONE', f)
}

{
  // 세 칸 중 두 칸만 시각이 겹치는 경우
  const slots: PickSlot[] = [
    { key: 'a', startMin: hm(9, 0), endMin: hm(9, 40), candidateIds: [10] },
    { key: 'b', startMin: hm(9, 0), endMin: hm(9, 40), candidateIds: [10] },
    { key: 'c', startMin: hm(11, 0), endMin: hm(11, 40), candidateIds: [10] },
  ]
  const f = flagPicks(slots, initialPicks(slots))
  check('겹치는 두 칸은 겹침', f.a === 'OVERLAP' && f.b === 'OVERLAP', f)
  check('겹치지 않는 칸은 중복까지만', f.c === 'DUPLICATE', f)
}

// ---------- 몇 번째로 들어가는지 ----------
{
  const slots = periods([
    [10, 11],
    [11, 10],
    [10, 12],
    [10, 12],
  ])
  const p = { p1: 10, p2: 11, p3: 10, p4: 10 }
  check('첫 번째는 1', repeatIndex(slots, p, 'p1') === 1)
  check('한 번뿐인 사람도 1', repeatIndex(slots, p, 'p2') === 1)
  check('두 번째는 2', repeatIndex(slots, p, 'p3') === 2)
  check('세 번째는 3', repeatIndex(slots, p, 'p4') === 3)
  check('미배정은 0', repeatIndex(slots, { ...p, p1: null }, 'p1') === 0)
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
process.exit(fails === 0 ? 0 : 1)
