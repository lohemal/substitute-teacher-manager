/**
 * 표의 '작업' 열이 눌려 한글 단추가 세로로 쪼개지는 일을 막는다.
 *
 * ## 왜 생기는가
 *
 * 작업 열의 머리글은 비어 있다 — `<th className={s.actCol} />`. 다른 열은
 * 머리글이 `white-space: nowrap` 이라 그 글자 폭이 열의 바닥이 되는데,
 * 작업 열만 바닥이 0 이다. 그래서 표가 좁아지면 이 열이 가장 먼저 눌린다.
 *
 * 그리고 **한글은 글자 사이 어디서나 줄이 바뀔 수 있다.** 열이 눌리면
 * '수정' 이 '수' / '정' 두 줄로 쪼개지고, 단추가 높아지면서 표의 행까지
 * 함께 늘어난다. 영어 'Edit' 라면 생기지 않을 일이다.
 *
 * ## 규칙
 *
 * 머리글이 빈 작업 열 클래스는 CSS 에서 `white-space: nowrap` 을 가져야
 * 한다. 그러면 칸의 최소 너비가 단추 너비가 되어 표가 그 아래로 줄이지
 * 못하고, 옆 열을 침범하지도 않는다.
 *
 *     node --experimental-strip-types scripts/check-action-buttons.ts
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'

const RULE = 'white-space: nowrap'

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) walk(p, out)
    else out.push(p)
  }
  return out
}

let fails = 0
const ok = (name: string, cond: boolean, extra?: string) => {
  console.log(`  ${cond ? 'ok  ' : 'FAIL'}  ${name}${extra ? `\n          ${extra}` : ''}`)
  if (!cond) fails++
}

console.log('\n=== 표의 작업 열 ===')

const files = walk('src')
const tsx = files.filter((f) => f.endsWith('.tsx'))

/** 머리글이 비어 있는 열: `<th className={s.foo} />` */
const EMPTY_TH = /<th\s+className=\{s\.([A-Za-z0-9_]+)\}\s*\/>/g

type Found = { file: string; cls: string }
const found: Found[] = []
for (const f of tsx) {
  const src = readFileSync(f, 'utf8')
  for (const m of src.matchAll(EMPTY_TH)) found.push({ file: f, cls: m[1] })
}

ok('머리글이 빈 작업 열을 찾았다', found.length > 0, `${found.length}개`)

for (const { file, cls } of found) {
  const short = file.replace(/\\/g, '/')
  // 같은 폴더의 CSS Module 가운데 그 클래스를 정의한 파일
  const dir = short.slice(0, short.lastIndexOf('/'))
  const sheets = files.filter(
    (f) => f.endsWith('.module.css') && f.replace(/\\/g, '/').startsWith(dir + '/'),
  )

  let block: string | null = null
  let where = ''
  for (const sheet of sheets) {
    const css = readFileSync(sheet, 'utf8')
    // '.cls {' 부터 닫는 중괄호까지
    const i = css.indexOf(`.${cls} {`)
    if (i < 0) continue
    const j = css.indexOf('}', i)
    block = css.slice(i, j)
    where = sheet.replace(/\\/g, '/')
    break
  }

  if (block === null) {
    ok(`${short} 의 .${cls} 규칙을 찾았다`, false, '같은 폴더의 CSS Module 에 없다')
    continue
  }
  ok(
    `${where} 의 .${cls} 이 ${RULE}`,
    block.includes(RULE),
    block.includes(RULE) ? undefined : '한글 단추가 세로로 쪼개질 수 있다',
  )
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
process.exit(fails === 0 ? 0 : 1)
