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

// 1) 이름만 있는 줄
{
  const r = parseRoster('김민수\n이영희')
  check('이름만 있어도 읽는다', r.length === 2 && r[0].name === '김민수')
  check('구분이 없으면 기타로 본다', r[0].roleCode === 'OTHER', r[0])
}

// 2) 담임 + 학급 (여러 표기)
{
  const r = parseRoster(
    [
      `김민수${T}담임${T}1-1`,
      `이영희${T}담임${T}1학년 2반`,
      `박서준${T}담임${T}5 - 3`,
      `최다은${T}담임${T}6-2반`,
    ].join('\n'),
  )
  check('1-1 을 읽는다', r[0].grade === 1 && r[0].classNo === 1, r[0])
  check('1학년 2반 을 읽는다', r[1].grade === 1 && r[1].classNo === 2, r[1])
  check('공백이 섞인 5 - 3 을 읽는다', r[2].grade === 5 && r[2].classNo === 3, r[2])
  check('6-2반 을 읽는다', r[3].grade === 6 && r[3].classNo === 2, r[3])
  check('모두 담임으로 본다', r.every((x) => x.roleCode === 'HOMEROOM'))
}

// 3) 전담 + 과목
{
  const r = parseRoster(`박지훈${T}전담${T}체육, 음악`)
  check('전담으로 읽는다', r[0].roleCode === 'SPECIAL', r[0])
  check('과목 두 개를 읽는다', (r[0].subjectNames ?? []).join() === '체육,음악', r[0].subjectNames)
}

// 4) 슬래시·가운뎃점 구분자
{
  const r = parseRoster(`박지훈${T}전담${T}체육/음악`)
  check('슬래시로 나눈 과목', (r[0].subjectNames ?? []).length === 2, r[0].subjectNames)
  const r2 = parseRoster(`김하늘${T}전담${T}영어·과학`)
  check('가운뎃점으로 나눈 과목', (r2[0].subjectNames ?? []).length === 2, r2[0].subjectNames)
}

// 5) 기타 + 담당 업무
{
  const r = parseRoster(`최수정${T}기타${T}보건교사`)
  check('기타로 읽는다', r[0].roleCode === 'OTHER', r[0])
  check('담당 업무를 메모로 남긴다', r[0].memo === '보건교사', r[0])
}

// 6) 구분 칸에 업무명이 바로 있는 경우
{
  const r = parseRoster(`강호준${T}보건`)
  check('보건 -> 기타로 본다', r[0].roleCode === 'OTHER', r[0])
  check('보건을 메모로도 남긴다', r[0].memo === '보건', r[0])
}

// 7) 열 순서가 뒤바뀐 경우
{
  const r = parseRoster(`김민수${T}1-1${T}담임`)
  check('학급이 앞에 와도 읽는다', r[0].grade === 1 && r[0].classNo === 1, r[0])
  check('구분이 뒤에 와도 읽는다', r[0].roleCode === 'HOMEROOM', r[0])
}

// 8) CSV
{
  const r = parseRoster('김민수,담임,3-1\n박지훈,전담,과학')
  check('쉼표로 나눈 줄도 읽는다', r.length === 2 && r[0].grade === 3, r[0])
  check('CSV 과목도 읽는다', (r[1].subjectNames ?? [])[0] === '과학', r[1])
}

// 9) 머리글·빈 줄 건너뛰기
{
  const r = parseRoster(`이름${T}구분${T}담당\n\n김민수${T}담임${T}1-1\n   \n`)
  check('머리글과 빈 줄을 건너뛴다', r.length === 1 && r[0].name === '김민수', r)
}

// 10) 구분 없이 학급만 / 과목만
{
  const a = parseRoster(`김민수${T}2-1`)
  check('학급만 있으면 담임으로 짐작', a[0].roleCode === 'HOMEROOM', a[0])
  const b = parseRoster(`박지훈${T}체육`)
  check('과목만 있으면 전담으로 짐작', b[0].roleCode === 'SPECIAL', b[0])
  check('짐작한 경우 표시를 남긴다', a[0].note !== undefined && b[0].note !== undefined)
}

// 11) 담임인데 과목처럼 보이는 칸이 있으면 메모로 옮긴다
{
  const r = parseRoster(`김민수${T}담임${T}방송부`)
  check('담임의 학급 없는 낱말은 메모로', r[0].memo === '방송부', r[0])
  check('담임에게 과목을 붙이지 않는다', (r[0].subjectNames ?? []).length === 0, r[0])
}

// 12) 없는 학급 표기는 학급으로 읽지 않는다
{
  const r = parseRoster(`김민수${T}담임${T}1학년`)
  check('1학년만 있으면 학급으로 읽지 않는다', r[0].grade === undefined, r[0])
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
if (fails > 0) process.exit(1)
