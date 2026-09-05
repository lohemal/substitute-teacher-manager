import type { SetupState, SetupStepKey } from '@/ipc/setup'

export interface SetupNav {
  stepKey: SetupStepKey
  hasPrev: boolean
  goPrev: () => void
  goNext: () => void
  goTo: (key: SetupStepKey) => void
}

export interface StepProps {
  nav: SetupNav
  state: SetupState
}
