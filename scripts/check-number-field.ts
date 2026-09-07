import { maxDigits } from '../src/lib/numberField.ts'

let fails = 0
function check(name: string, cond: boolean, extra?: unknown) {
  if (cond) console.log(`  ok   ${name}`)
  else {
    console.log(`  FAIL ${name}`)
    if (extra !== undefined) console.log('       ', extra)
    fails++
  }
}

// 1) 실제로 쓰고 있는 칸들이 필요한 만큼 받는가
{
  const fields: [string, number, number, string][] = [
    ['1회 보결 수당', 0, 1_000_000, '1000000'],
    ['1회 보결 수당 — 15,000원', 0, 1_000_000, '15000'],
    ['학년도', 2000, 2100, '2026'],
    ['반 수', 1, 30, '30'],
    ['수업 길이(분)', 5, 180, '180'],
    ['쉬는 시간(분)', 0, 120, '120'],
    ['점심 길이(분)', 10, 180, '180'],
    ['교시 뒤', 0, 10, '10'],
    ['요일별 교시 수', 0, 15, '15'],
  ]
  for (const [label, min, max, typed] of fields) {
    const d = maxDigits(min, max)
    check(`${label} — '${typed}' 를 넣을 수 있다 (${d}자리)`, typed.length <= d, {
      min,
      max,
      digits: d,
    })
  }
}

// 2) max 를 넘는 자릿수까지 열어 주지는 않는다 (오타 방어)
{
  check('0~15 칸은 두 자리까지', maxDigits(0, 15) === 2)
  check('1~30 칸은 두 자리까지', maxDigits(1, 30) === 2)
  check('0~180 칸은 세 자리까지', maxDigits(0, 180) === 3)
  check('2000~2100 칸은 네 자리까지', maxDigits(2000, 2100) === 4)
  check('0~1,000,000 칸은 일곱 자리까지', maxDigits(0, 1_000_000) === 7)
}

// 3) 경계
{
  check('0~0 은 한 자리', maxDigits(0, 0) === 1)
  check('0~9 는 한 자리', maxDigits(0, 9) === 1)
  check('0~10 은 두 자리', maxDigits(0, 10) === 2)
  check('0~99 는 두 자리', maxDigits(0, 99) === 2)
  check('0~100 은 세 자리', maxDigits(0, 100) === 3)
}

// 4) 음수 칸은 부호 자리를 더 받는다
{
  check('-30~30 은 세 자리 (부호 포함)', maxDigits(-30, 30) === 3)
  check('-5~5 는 두 자리', maxDigits(-5, 5) === 2)
  check('-120~0 은 네 자리', maxDigits(-120, 0) === 4)
}

// 5) 어떤 범위에서도 최댓값 문자열이 반드시 들어간다 — 이 성질이 깨지면 입력이 막힌다
{
  let ok = true
  const bad: unknown[] = []
  for (const max of [1, 7, 10, 42, 99, 100, 999, 1000, 15_000, 99_999, 1_000_000]) {
    for (const min of [0, 1, max]) {
      if (min > max) continue
      const d = maxDigits(min, max)
      if (String(max).length > d || String(min).length > d) {
        ok = false
        bad.push({ min, max, digits: d })
      }
    }
  }
  check('모든 범위에서 최솟값·최댓값을 그대로 칠 수 있다', ok, bad)
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
if (fails > 0) process.exit(1)
