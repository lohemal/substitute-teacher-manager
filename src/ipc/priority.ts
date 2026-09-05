import { invoke } from './invoke'

export interface RuleView {
  ruleKey: string
  label: string
  hint: string
  enabled: boolean
  sortOrder: number
}

export interface PriorityView {
  rules: RuleView[]
  /** 마지막 동점 처리 기준 (사람 말로) */
  tieBreak: string
  isDefault: boolean
}

export interface RuleInput {
  ruleKey: string
  enabled: boolean
}

export const priorityApi = {
  list: () => invoke<PriorityView>('priority_list'),
  save: (rules: RuleInput[]) => invoke<PriorityView>('priority_save', { rules }),
  reset: () => invoke<PriorityView>('priority_reset'),
  finishStep: () => invoke<string[]>('priority_finish_step'),
}
