import {
  CURRENT_SCHOOL_TYPE,
  isCurrentSchoolType,
  SCHOOL_TYPE_LABEL,
  DEFAULT_GRADE_RANGE,
  type SchoolType,
} from '../src/lib/schoolType.ts'

let fails = 0
function check(name: string, cond: boolean, extra?: unknown) {
  if (cond) console.log(`  ok   ${name}`)
  else {
    console.log(`  FAIL ${name}`)
    if (extra !== undefined) console.log('       ', extra)
    fails++
  }
}

const ALL: SchoolType[] = ['ELEMENTARY', 'MIDDLE', 'HIGH']

// 1) 새로 설정하면 초등학교로 저장된다 — 사용자가 고르는 단계는 없다
{
  check('이 버전이 다루는 학교급은 초등학교', CURRENT_SCHOOL_TYPE === 'ELEMENTARY', CURRENT_SCHOOL_TYPE)
  check('초등학교 자료는 판정한다', isCurrentSchoolType('ELEMENTARY'))
  check('중학교 자료는 판정하지 않는다', !isCurrentSchoolType('MIDDLE'))
  check('고등학교 자료는 판정하지 않는다', !isCurrentSchoolType('HIGH'))
}

// 2) 내부 구조는 그대로 살아 있다 — 지운 것이 아니라 화면에서 묻지 않는 것이다
{
  check('세 학교급 타입이 모두 남아 있다', ALL.every((t) => !!SCHOOL_TYPE_LABEL[t]), SCHOOL_TYPE_LABEL)
  check('중학교 이름이 남아 있다', SCHOOL_TYPE_LABEL.MIDDLE === '중학교')
  check('고등학교 이름이 남아 있다', SCHOOL_TYPE_LABEL.HIGH === '고등학교')
  check(
    '학교급별 기본 학년 범위가 모두 남아 있다',
    ALL.every((t) => Array.isArray(DEFAULT_GRADE_RANGE[t]) && DEFAULT_GRADE_RANGE[t].length === 2),
    DEFAULT_GRADE_RANGE,
  )
  check('초등학교 기본 학년은 1~6', DEFAULT_GRADE_RANGE.ELEMENTARY.join('-') === '1-6')
  check('중학교 기본 학년은 1~3', DEFAULT_GRADE_RANGE.MIDDLE.join('-') === '1-3')
  check('고등학교 기본 학년은 1~3', DEFAULT_GRADE_RANGE.HIGH.join('-') === '1-3')
}

// 3) 예전에 임시로 두었던 UI 제한 코드는 남아 있지 않다 (dead code 방지)
{
  const src = await import('node:fs').then((fs) =>
    fs.readFileSync('src/lib/schoolType.ts', 'utf8'),
  )
  for (const gone of [
    'SUPPORTED_SCHOOL_TYPES',
    'isSupportedSchoolType',
    'canPickSchoolType',
    'SCHOOL_TYPE_PENDING',
    'SCHOOL_TYPE_NOTE',
  ]) {
    check(`${gone} 는 지워졌다`, !src.includes(gone))
  }
}

// 4) 화면에서 학교급을 묻지 않는다
{
  const fs = await import('node:fs')
  const step = fs.readFileSync('src/setup/steps/SchoolStep.tsx', 'utf8')
  check('학교 기본 설정에 학교급 선택 칸이 없다', !step.includes('학교 구분'), '학교 구분')
  check('중학교·고등학교를 화면에 그리지 않는다', !/중학교|고등학교/.test(step))
  check('추후 지원 예정 배지가 없다', !step.includes('추후 지원 예정'))
  check(
    '새 설정은 CURRENT_SCHOOL_TYPE 으로 저장한다',
    step.includes('schoolType: CURRENT_SCHOOL_TYPE'),
  )
  check(
    '저장되어 있던 학교급은 그대로 이어받는다',
    step.includes('schoolType: saved.schoolType'),
  )

  const css = fs.readFileSync('src/setup/steps/SchoolStep.module.css', 'utf8')
  for (const gone of ['.chipOff', '.chipBadge', '.typeNote']) {
    check(`${gone} 규칙이 지워졌다`, !css.includes(gone))
  }
  for (const kept of ['.chips', '.chip ', '.chipOn']) {
    check(`${kept} 는 요일·학기 칩이 쓰므로 남는다`, css.includes(kept))
  }
}

// 5) 초등학교가 아닌 자료는 보결 조회를 막는다 (값은 바꾸지 않는다)
{
  const fs = await import('node:fs')
  const page = fs.readFileSync('src/pages/FindPage.tsx', 'utf8')
  check('보결 조회가 학교급을 확인한다', page.includes('isCurrentSchoolType'))
  check('초등학교가 아니면 조회 화면을 그리지 않는다', /other \?/.test(page))
}

console.log(fails === 0 ? '\nALL OK' : `\n${fails} FAILED`)
if (fails > 0) process.exit(1)
