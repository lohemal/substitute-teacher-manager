import { parseLessonGrid } from '../src/features/lesson/parseLessonGrid.ts'
import type { ClassOption, SubjectOption } from '../src/ipc/lesson.ts'

let fails = 0
function check(name: string, cond: boolean, extra?: unknown) {
  if (cond) console.log(`  ok   ${name}`)
  else {
    fails++
    console.log(`  FAIL ${name}`, extra ?? '')
  }
}

const T = '\t'

// 3~6학년 각 2반. 3학년만 이름 반(가람/나리)
const classes: ClassOption[] = []
let cid = 1
for (const g of [1, 2, 3, 4, 5, 6]) {
  for (const n of [1, 2]) {
    const named = g === 3
    classes.push({
      classId: cid++,
      grade: g,
      classNo: n,
      name: named ? (n === 1 ? '가람' : '나리') : null,
      label: named ? `${g}-${n === 1 ? '가람' : '나리'}` : `${g}-${n}`,
      fullLabel: '',
      homeroomName: null,
    })
  }
}
const subjects: SubjectOption[] = [
  { id: 1, name: '국어' },
  { id: 2, name: '영어' },
  { id: 3, name: '체육' },
  { id: 4, name: '창의적 체험활동' },
]
const look = { classes, subjects }
const find = (r: ReturnType<typeof parseLessonGrid>, d: number, p: number) =>
  r.cells.find((c) => c.dayOfWeek === d && c.periodNo === p)

// 1) 열 = 요일 (가장 흔한 형태)
{
  const text = [
    `${T}월${T}화${T}수${T}목${T}금`,
    `1교시${T}4-1 영어${T}${T}5-1 영어${T}${T}`,
    `2교시${T}4-2 영어${T}5-1 영어${T}${T}${T}`,
    `3교시${T}${T}5-2 영어${T}4-1 영어${T}${T}`,
  ].join('\n')
  const r = parseLessonGrid(text, look)

  check('열이 요일인 표를 알아본다', r.layout === 'DAY_COLUMNS', r.layout)
  check('빈 칸은 건너뛴다', r.cells.length === 6, r.cells.length)
  check('월 1교시 4-1 영어', find(r, 1, 1)?.classLabel === '4-1' && find(r, 1, 1)?.subjectName === '영어', find(r, 1, 1))
  check('수 1교시 5-1 영어', find(r, 3, 1)?.classLabel === '5-1', find(r, 3, 1))
  check('화 3교시 5-2 영어', find(r, 2, 3)?.classLabel === '5-2', find(r, 2, 3))
  check('문제 없는 칸', r.cells.every((c) => !c.problem), r.cells.filter((c) => c.problem))
}

// 2) 열 = 교시
{
  const text = [
    `${T}1교시${T}2교시${T}3교시`,
    `월${T}4-1 체육${T}4-2 체육${T}`,
    `화${T}${T}5-1 체육${T}5-2 체육`,
  ].join('\n')
  const r = parseLessonGrid(text, look)

  check('열이 교시인 표를 알아본다', r.layout === 'PERIOD_COLUMNS', r.layout)
  check('월 1교시', find(r, 1, 1)?.classLabel === '4-1', find(r, 1, 1))
  check('화 3교시', find(r, 2, 3)?.classLabel === '5-2', find(r, 2, 3))
  check('칸 4개', r.cells.length === 4, r.cells.length)
}

// 3) 칸 표기 여러 형태
{
  const text = [
    `${T}월${T}화${T}수${T}목`,
    `1교시${T}4-1영어${T}영어 4-2${T}4학년 1반 체육${T}5-1`,
  ].join('\n')
  const r = parseLessonGrid(text, { ...look, defaultSubjectId: 2, defaultSubjectName: '영어' })

  check('붙여 쓴 4-1영어', find(r, 1, 1)?.classLabel === '4-1' && find(r, 1, 1)?.subjectName === '영어', find(r, 1, 1))
  check('과목이 앞에 온 경우', find(r, 2, 1)?.classLabel === '4-2' && find(r, 2, 1)?.subjectName === '영어', find(r, 2, 1))
  check('4학년 1반 체육', find(r, 3, 1)?.classLabel === '4-1' && find(r, 3, 1)?.subjectName === '체육', find(r, 3, 1))
  check('학급만 있으면 기본 과목', find(r, 4, 1)?.subjectName === '영어', find(r, 4, 1))
}

// 4) 이름 반
{
  const text = [`${T}월${T}화`, `1교시${T}3-가람 영어${T}3학년 나리반 영어`].join('\n')
  const r = parseLessonGrid(text, look)
  check('3-가람 을 읽는다', find(r, 1, 1)?.classLabel === '3-가람', find(r, 1, 1))
  check('3학년 나리반 을 읽는다', find(r, 2, 1)?.classLabel === '3-나리', find(r, 2, 1))
}

// 5) 여러 낱말 과목
{
  const text = [`${T}월`, `1교시${T}4-1 창의적 체험활동`].join('\n')
  const r = parseLessonGrid(text, look)
  check('긴 과목 이름', find(r, 1, 1)?.subjectName === '창의적 체험활동', find(r, 1, 1))
}

// 6) 빈 칸 표기
{
  const text = [`${T}월${T}화${T}수${T}목`, `1교시${T}-${T}—${T}공강${T}4-1 영어`].join('\n')
  const r = parseLessonGrid(text, look)
  check('-, —, 공강 은 빈 칸으로 본다', r.cells.length === 1, r.cells)
}

// 7) 없는 학급 / 못 읽은 칸
{
  const text = [`${T}월${T}화${T}수`, `1교시${T}9-1 영어${T}그냥 글자${T}4-1 로봇`].join('\n')
  const r = parseLessonGrid(text, look)

  check('없는 학급을 알려준다', find(r, 1, 1)?.problem?.includes('9학년 1반') === true, find(r, 1, 1))
  check('학급을 못 찾으면 알려준다', find(r, 2, 1)?.problem?.includes('학급') === true, find(r, 2, 1))
  check('모르는 과목을 알려준다', find(r, 3, 1)?.problem?.includes('로봇') === true, find(r, 3, 1))
  check('학급은 찾았으므로 남겨둔다', find(r, 3, 1)?.classLabel === '4-1', find(r, 3, 1))
}

// 8) 머리글 없는 표
{
  const r = parseLessonGrid(`4-1 영어${T}4-2 영어\n5-1 영어${T}5-2 영어`, look)
  check('머리글이 없으면 알려준다', r.fatal !== undefined, r)
  check('요일/교시를 찾지 못했다고 안내', r.fatal?.includes('요일') === true, r.fatal)
}

// 9) 수업이 하나도 없는 표
{
  const r = parseLessonGrid(`${T}월${T}화\n1교시${T}${T}`, look)
  check('수업이 없으면 알려준다', r.fatal?.includes('찾지 못했습니다') === true, r.fatal)
}

// 10) 소계 줄 등 잡음 무시
{
  const text = [
    `${T}월${T}화`,
    `1교시${T}4-1 영어${T}`,
    `합계${T}1${T}0`,
    `2교시${T}${T}4-2 영어`,
  ].join('\n')
  const r = parseLessonGrid(text, look)
  check('합계 줄은 건너뛴다', r.cells.length === 2, r.cells)
  check('그 뒤 줄도 계속 읽는다', find(r, 2, 2)?.classLabel === '4-2', find(r, 2, 2))
}

// 11) CSV
{
  const r = parseLessonGrid(',월,화\n1교시,4-1 영어,5-1 영어', look)
  check('쉼표로 나눈 표도 읽는다', r.cells.length === 2, r.cells)
}

// 12) 요일 전체 이름
{
  const r = parseLessonGrid(
    [`${T}월요일${T}화요일`, `1교시${T}4-1 영어${T}5-1 영어`].join('\n'),
    look,
  )
  check('월요일 표기도 머리글로 인정', r.layout === 'DAY_COLUMNS' && r.cells.length === 2, r)
  check('월요일 -> 1, 화요일 -> 2', find(r, 1, 1)?.classLabel === '4-1' && find(r, 2, 1)?.classLabel === '5-1', r.cells)
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
if (fails > 0) process.exit(1)
