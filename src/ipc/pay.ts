import { invoke } from './invoke'

/** 지급 기준 정책 코드. 새 정책은 Rust 쪽 `domain::pay` 에만 추가한다. */
export type PayPolicy = 'ALL_ASSIGNED' | 'DEDUCT_OWN_CAUSED'

export type PayMode = 'MONTH' | 'CUSTOM'

export interface PayQuery {
  mode?: PayMode
  /** mode='MONTH' 일 때 'YYYY-MM' */
  month?: string | null
  from?: string | null
  to?: string | null
}

export interface PayRow {
  teacherId: number
  name: string
  roleCode: 'HOMEROOM' | 'SPECIAL' | 'OTHER'
  roleLabel: string
  duty: string
  active: boolean
  /** 다른 선생님을 대신해 들어간 횟수 */
  substituted: number
  /** 본인 결근으로 다른 선생님이 들어간 횟수 (참고정보) */
  ownCaused: number
  payable: number
  perCase: number
  amount: number
}

export interface PaySummary {
  paidTeachers: number
  listedTeachers: number
  totalSubstituted: number
  totalOwnCaused: number
  totalPayable: number
  totalAmount: number
}

export interface PayView {
  mode: PayMode
  /** 'YYYY-MM' */
  month: string
  prevMonth: string
  nextMonth: string
  from: string
  to: string
  rangeLabel: string

  policy: PayPolicy
  policyLabel: string
  policyHint: string
  perCase: number
  /** 1회 수당을 아직 정하지 않았다 */
  perCaseUnset: boolean

  summary: PaySummary
  rows: PayRow[]
  /** 고른 기간이 지금 학기를 벗어났을 때의 안내 */
  termNote: string | null
}

export interface PayCase {
  date: string
  dayOfWeek: number
  /** 기록 당시 표기 그대로 */
  classLabel: string
  slotLabel: string
  startMin: number
  endMin: number
  /** 보결한 내역이면 결근한 선생님, 본인 발생분이면 대신 들어간 선생님 */
  counterpart: string
  reasonLabel: string | null
}

export interface PayDetail {
  teacherId: number
  name: string
  roleLabel: string
  from: string
  to: string
  rangeLabel: string
  policy: PayPolicy
  policyLabel: string
  perCase: number
  substituted: PayCase[]
  ownCaused: PayCase[]
  payable: number
  amount: number
  /** 계산 설명. 정책이 만든 문장을 그대로 보여 준다 */
  steps: string[]
  /** 이 정책이 본인 발생분을 차감하는가 */
  deducts: boolean
}

/** 천 단위 쉼표. Rust 의 `domain::pay::won` 과 같은 모양이다. */
export function won(v: number): string {
  return v.toLocaleString('ko-KR')
}

export const payApi = {
  view: (query: PayQuery) => invoke<PayView>('pay_view', { query }),
  detail: (teacherId: number, query: PayQuery) =>
    invoke<PayDetail>('pay_detail', { teacherId, query }),
}
