import { Icon } from '@/components/Icon'
import type { SetupStep, SetupStepKey } from '@/ipc/setup'
import { STEP_META } from '@/ipc/setup'
import s from './Stepper.module.css'

interface Props {
  steps: SetupStep[]
  current: SetupStepKey
  onJump: (key: SetupStepKey) => void
}

export function Stepper({ steps, current, onJump }: Props) {
  const currentIndex = steps.findIndex((x) => x.key === current)

  return (
    <ol className={s.list}>
      {steps.map((step, i) => {
        const isCurrent = step.key === current
        const finished = step.status === 'DONE'
        const skipped = step.status === 'SKIPPED'
        // 이미 지나온 단계는 자유롭게 되돌아갈 수 있다
        const reachable = i <= currentIndex || finished || skipped

        const cls = [
          s.item,
          isCurrent ? s.current : '',
          finished ? s.done : '',
          skipped ? s.skipped : '',
          reachable ? s.reachable : '',
        ]
          .filter(Boolean)
          .join(' ')

        return (
          <li key={step.key} className={cls}>
            {i > 0 && <span className={s.line} aria-hidden="true" />}
            <button
              type="button"
              className={s.node}
              disabled={!reachable || isCurrent}
              onClick={() => onJump(step.key)}
              aria-current={isCurrent ? 'step' : undefined}
              title={STEP_META[step.key].title}
            >
              <span className={s.circle}>
                {finished ? <Icon name="check" size={14} strokeWidth={3} /> : i + 1}
              </span>
              <span className={s.label}>{STEP_META[step.key].short}</span>
            </button>
          </li>
        )
      })}
    </ol>
  )
}
