/**
 * 학교급. **DB도 Tauri도 모르는 순수 모듈.**
 *
 * ## 이 프로그램은 초등학교용이다
 *
 * 중·고등학교는 보결 운영 구조가 상당히 다르다 — 교사마다 담당 교과가 있고,
 * 대부분이 개인 시간표를 가지며, 같은 교과 교사의 보강 우선순위나 교환수업과
 * 보강의 관계, 결강 누계·주간 시수 같은 학교별 규정이 순서에 영향을 준다.
 * 초등학교용 판정을 그대로 적용하면 **맞지 않는 결과를 조용히 내놓는다.**
 *
 * 그래서 화면에서는 학교급을 아예 묻지 않고 초등학교로 저장한다.
 *
 * ## 그래도 구조는 남겨 둔다
 *
 * `SchoolType` 의 세 값과 학교급별 기본 학년 범위는 그대로 둔다. DB의
 * `school_type` 도 `ELEMENTARY | MIDDLE | HIGH` 를 그대로 받는다. 나중에
 * 중·고등학교 운영 방식을 조사해 지원하기로 하면 여기서부터 넓히면 된다.
 */

export type SchoolType = 'ELEMENTARY' | 'MIDDLE' | 'HIGH'

export const SCHOOL_TYPE_LABEL: Record<SchoolType, string> = {
  ELEMENTARY: '초등학교',
  MIDDLE: '중학교',
  HIGH: '고등학교',
}

/** 학교급별 기본 학년 범위 */
export const DEFAULT_GRADE_RANGE: Record<SchoolType, [number, number]> = {
  ELEMENTARY: [1, 6],
  MIDDLE: [1, 3],
  HIGH: [1, 3],
}

/** 이 버전이 다루는 학교급. 새로 설정하면 이 값으로 저장된다. */
export const CURRENT_SCHOOL_TYPE: SchoolType = 'ELEMENTARY'

/**
 * 이 버전이 판정할 수 있는 자료인가.
 *
 * 예전 버전이나 시험용으로 만들어 둔 중·고등학교 자료를 열었을 때 쓴다.
 * **값을 바꾸지는 않는다.** 초등학교 기준으로 조용히 계산해 틀린 답을
 * 내놓는 것이 자료를 그대로 두는 것보다 나쁘기 때문에, 보결 조회만 막고
 * 나머지(기록 열람·백업)는 그대로 쓸 수 있게 둔다.
 */
export function isCurrentSchoolType(t: SchoolType): boolean {
  return t === CURRENT_SCHOOL_TYPE
}
