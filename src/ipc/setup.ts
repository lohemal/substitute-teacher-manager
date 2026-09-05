import { invoke } from './invoke'

export type SetupStepKey = 'SCHOOL' | 'BELL' | 'TEACHER' | 'LESSON' | 'PRIORITY' | 'DONE'
export type SetupStepStatus = 'PENDING' | 'IN_PROGRESS' | 'DONE' | 'SKIPPED'

export interface SetupStep {
  key: SetupStepKey
  order: number
  status: SetupStepStatus
}

export interface SetupState {
  steps: SetupStep[]
  resumeStep: SetupStepKey
  hasProgress: boolean
  completed: boolean
  lastWorkedStep: SetupStepKey | null
}

/** 화면에 보여줄 단계 이름과 안내 문구 (사용자 언어) */
export const STEP_META: Record<
  SetupStepKey,
  { short: string; title: string; description: string }
> = {
  SCHOOL: {
    short: '학교 정보',
    title: '학교 기본 설정',
    description: '학교 이름과 학년·반 편성을 입력합니다. 나중에 언제든 바꿀 수 있습니다.',
  },
  BELL: {
    short: '시정표',
    title: '학년별 시정표 · 점심시간',
    description: '학년마다 교시 시간과 점심시간이 다를 수 있습니다. 실제 시각을 기준으로 보결 가능 여부를 판단합니다.',
  },
  TEACHER: {
    short: '교사 등록',
    title: '교사 등록',
    description: '선생님 명단과 담임 학급, 보결 배정 대상 여부를 등록합니다.',
  },
  LESSON: {
    short: '전담 시간표',
    title: '전담교사 시간표',
    description: '전담교사의 주간 시간표만 입력하면 담임교사의 수업 시간은 자동으로 계산됩니다.',
  },
  PRIORITY: {
    short: '배정 기준',
    title: '보결 배정 우선순위',
    description: '우리 학교의 보결 배정 기준을 순서대로 정합니다. 프로그램은 이 기준으로 추천만 하고, 최종 결정은 담당자가 합니다.',
  },
  DONE: {
    short: '완료',
    title: '설정 완료',
    description: '이제 보결 조회를 시작할 수 있습니다.',
  },
}

export const STEP_ORDER: SetupStepKey[] = ['SCHOOL', 'BELL', 'TEACHER', 'LESSON', 'PRIORITY', 'DONE']

export function isStepKey(v: string | undefined): v is SetupStepKey {
  return !!v && (STEP_ORDER as string[]).includes(v)
}

export const setupApi = {
  getState: () => invoke<SetupState>('setup_get_state'),

  saveDraft: (stepKey: SetupStepKey, value: unknown) =>
    invoke<void>('setup_save_draft', { stepKey, valueJson: JSON.stringify(value) }),

  getDraft: async <T>(stepKey: SetupStepKey): Promise<T | null> => {
    const raw = await invoke<string | null>('setup_get_draft', { stepKey })
    if (!raw) return null
    try {
      return JSON.parse(raw) as T
    } catch {
      return null
    }
  },

  skipStep: (stepKey: SetupStepKey) => invoke<SetupState>('setup_skip_step', { stepKey }),
  complete: () => invoke<SetupState>('setup_complete'),
  restart: () => invoke<SetupState>('setup_restart'),
}
