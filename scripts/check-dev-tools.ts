/**
 * 개발용 진단 화면이 배포본에 새어 나가지 않는지 확인한다.
 *
 * `개발자 확인` 같은 진단 UI 는 만드는 동안 아주 쓸모가 있지만, 학교에서
 * 쓰는 분들께는 뜻이 없는 영어 코드일 뿐이다. 한 곳을 숨기고 다른 곳에
 * 남겨 두면 그것이 더 나쁘다.
 *
 * 그래서 **표시를 가르는 곳은 `src/lib/devTools.ts` 하나뿐**이라는 것을
 * 여기서 못 박는다. 진단용 낱말이 들어 있는 화면 파일은 반드시
 * `DEV_TOOLS` 를 들여와 그것으로 가려야 한다.
 *
 *     node --experimental-strip-types scripts/check-dev-tools.ts
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'

/** 이 낱말이 화면 파일에 있으면 개발용 진단 UI 로 본다 */
// 'reasonCode' 만으로는 넓다 — 결근 사유·보결 불필요 사유에도 같은 이름이
// 쓰인다. 후보 판정 코드를 화면에 찍는 'c.reasonCode' 만 본다.
const MARKERS = ['개발자 확인', 'c.reasonCode', 'debugBox', 'debugToggle']

const GATE = 'DEV_TOOLS'
const GATE_FILE = 'src/lib/devTools.ts'

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) walk(p, out)
    else if (/\.(tsx|ts)$/.test(name)) out.push(p)
  }
  return out
}

let fails = 0
const ok = (name: string, cond: boolean, extra?: string) => {
  console.log(`  ${cond ? 'ok  ' : 'FAIL'}  ${name}${extra ? `\n          ${extra}` : ''}`)
  if (!cond) fails++
}

console.log('\n=== 개발용 진단 화면 ===')

// 1) 가르는 곳이 하나뿐인가
const gate = readFileSync(GATE_FILE, 'utf8')
ok(`${GATE_FILE} 이 import.meta.env.DEV 로 가른다`, gate.includes('import.meta.env.DEV'))

// 2) 진단 낱말이 있는 화면 파일은 모두 그 문을 지나는가
const files = walk('src').filter((f) => f.replace(/\\/g, '/') !== GATE_FILE)
const suspects = files.filter((f) => {
  const src = readFileSync(f, 'utf8')
  // 타입 선언과 주석만 있는 파일(ipc 등)은 화면을 그리지 않는다
  if (!f.endsWith('.tsx')) return false
  return MARKERS.some((m) => src.includes(m))
})

ok('진단 낱말이 든 화면 파일을 찾았다', suspects.length > 0, suspects.join(', ') || '(없음)')

for (const f of suspects) {
  const src = readFileSync(f, 'utf8')
  const short = f.replace(/\\/g, '/')
  ok(`${short} 이 ${GATE} 를 들여온다`, src.includes(`import { ${GATE} }`))

  // 체크 상자 자체가 가려져 있는가
  if (src.includes('개발자 확인')) {
    ok(
      `${short} 의 [개발자 확인] 체크 상자가 ${GATE} 안에 있다`,
      /\{DEV_TOOLS && \(/.test(src),
    )
  }

  // 켜짐 상태가 DEV_TOOLS 와 함께 묶여 있는가 — 상자가 없어도 켜지지 않게
  if (src.includes('showDebug')) {
    ok(
      `${short} 의 showDebug 가 ${GATE} 와 묶여 있다`,
      /const showDebug = DEV_TOOLS && /.test(src),
    )
  }
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
process.exit(fails === 0 ? 0 : 1)
