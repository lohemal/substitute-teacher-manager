import { classFull, classOnly, classShort, normalizeClassName } from '../src/lib/classLabel.ts'
import { parseRoster } from '../src/features/teacher/parseRoster.ts'

let fails = 0
function check(name: string, cond: boolean, extra?: unknown) {
  if (cond) console.log(`  ok   ${name}`)
  else {
    fails++
    console.log(`  FAIL ${name}`, extra ?? '')
  }
}

const T = '\t'

// ---------- 표시 이름 (Rust label.rs 와 같은 규칙) ----------
{
  check('이름 없으면 숫자 짧게', classShort(5, 1, null) === '5-1')
  check('이름 없으면 숫자 길게', classFull(5, 1, null) === '5학년 1반')
  check('이름 있으면 이름 짧게', classShort(5, 1, '가람') === '5-가람')
  check('이름 있으면 이름 길게', classFull(5, 1, '가람') === '5학년 가람반')
  check('반만 짧게 (숫자)', classOnly(2, null) === '2반')
  check('반만 짧게 (이름)', classOnly(2, '나리') === '나리반')
  check('빈 이름은 숫자로 되돌림', classShort(3, 2, '   ') === '3-2')
  check("'반'을 붙여 입력해도 한 번만", normalizeClassName('가람반') === '가람')
  check('앞뒤 공백 정리', normalizeClassName('  나리반  ') === '나리')
  check("이름 자체가 '반'이면 그대로", normalizeClassName('반') === '반')
  check('이름 안쪽 반은 안 지움', normalizeClassName('반딧불') === '반딧불')
  check(
    '정리한 이름으로 만들면 반이 한 번만 붙는다',
    classFull(1, 1, normalizeClassName('가람반')) === '1학년 가람반',
  )
}

// ---------- 명단에서 이름 반 읽기 ----------
{
  const r = parseRoster(
    [
      `김민수${T}담임${T}1-가람`,
      `이영희${T}담임${T}1학년 나리반`,
      `박서준${T}담임${T}2-다솜반`,
      `최다은${T}담임${T}3 - 라온`,
    ].join('\n'),
  )
  check('1-가람 을 읽는다', r[0].grade === 1 && r[0].className === '가람', r[0])
  check('1학년 나리반 을 읽는다', r[1].grade === 1 && r[1].className === '나리', r[1])
  check('2-다솜반 을 읽는다', r[2].grade === 2 && r[2].className === '다솜', r[2])
  check('공백 섞인 3 - 라온 을 읽는다', r[3].grade === 3 && r[3].className === '라온', r[3])
  check('이름 반이면 반 번호는 비운다', r.every((x) => x.classNo === undefined), r.map((x) => x.classNo))
  check('모두 담임으로 본다', r.every((x) => x.roleCode === 'HOMEROOM'))
}

// ---------- 숫자 반과 헷갈리지 않는다 ----------
{
  const r = parseRoster(`김민수${T}담임${T}1-2`)
  check('1-2 는 반 번호로 읽는다', r[0].classNo === 2 && r[0].className === undefined, r[0])

  const r2 = parseRoster(`이영희${T}담임${T}1학년 2반`)
  check('1학년 2반 도 반 번호', r2[0].classNo === 2 && r2[0].className === undefined, r2[0])
}

// ---------- 구분 없이 이름 반만 있어도 담임으로 짐작 ----------
{
  const r = parseRoster(`김민수${T}4-가람`)
  check('이름 반만 있으면 담임으로 짐작', r[0].roleCode === 'HOMEROOM', r[0])
  check('학년과 이름을 함께 읽는다', r[0].grade === 4 && r[0].className === '가람', r[0])
}

// ---------- 과목과 헷갈리지 않는다 ----------
{
  const r = parseRoster(`박지훈${T}전담${T}체육`)
  check('학년 없는 낱말은 학급으로 읽지 않는다', r[0].grade === undefined, r[0])
  check('과목으로 읽는다', (r[0].subjectNames ?? [])[0] === '체육', r[0])
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
if (fails > 0) process.exit(1)
