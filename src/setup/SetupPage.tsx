import { useQuery } from '@tanstack/react-query'
import { Navigate, useNavigate, useParams } from 'react-router-dom'

import { Icon } from '@/components/Icon'
import { errorMessage } from '@/ipc/invoke'
import { isStepKey, setupApi, STEP_ORDER, type SetupStepKey } from '@/ipc/setup'
import { Stepper } from './Stepper'
import { BellStep } from './steps/BellStep'
import { DoneStep } from './steps/DoneStep'
import { LessonStep } from './steps/LessonStep'
import { PriorityStep } from './steps/PriorityStep'
import { SchoolStep } from './steps/SchoolStep'
import { TeacherStep } from './steps/TeacherStep'
import type { SetupNav } from './types'
import s from './SetupPage.module.css'

export function SetupPage() {
  const { stepKey } = useParams()
  const navigate = useNavigate()

  const { data: state, isLoading, error } = useQuery({
    queryKey: ['setup-state'],
    queryFn: setupApi.getState,
  })

  if (isLoading) {
    return <div className={s.center}>불러오는 중…</div>
  }
  if (error || !state) {
    return <div className={s.center}>{errorMessage(error)}</div>
  }
  if (!isStepKey(stepKey)) {
    return <Navigate to={`/setup/${state.resumeStep}`} replace />
  }

  const index = STEP_ORDER.indexOf(stepKey)
  const nav: SetupNav = {
    stepKey,
    hasPrev: index > 0,
    goPrev: () => navigate(`/setup/${STEP_ORDER[Math.max(0, index - 1)]}`),
    goNext: () =>
      navigate(`/setup/${STEP_ORDER[Math.min(STEP_ORDER.length - 1, index + 1)]}`),
    goTo: (k: SetupStepKey) => navigate(`/setup/${k}`),
  }

  return (
    <div className={s.wrap}>
      <header className={s.top}>
        <div className={s.topInner}>
          <div className={s.brand}>
            <div className={s.brandMark} aria-hidden="true">
              <Icon name="check" size={16} strokeWidth={2.6} />
            </div>
            <div>
              <p className={s.brandTitle}>초기 설정</p>
              <p className={s.brandSub}>
                {state.completed
                  ? '설정을 다시 확인하고 있습니다'
                  : '처음 한 번만 설정하면 됩니다'}
              </p>
            </div>
          </div>
          <span className={s.counter}>
            <strong>{index + 1}</strong> / {STEP_ORDER.length} 단계
          </span>
        </div>

        <div className={s.stepperWrap}>
          <Stepper steps={state.steps} current={stepKey} onJump={nav.goTo} />
        </div>
      </header>

      <div className={s.content}>
        {stepKey === 'SCHOOL' && <SchoolStep nav={nav} state={state} />}
        {stepKey === 'BELL' && <BellStep nav={nav} state={state} />}
        {stepKey === 'TEACHER' && <TeacherStep nav={nav} state={state} />}
        {stepKey === 'LESSON' && <LessonStep nav={nav} state={state} />}
        {stepKey === 'PRIORITY' && <PriorityStep nav={nav} state={state} />}
        {stepKey === 'DONE' && <DoneStep nav={nav} state={state} />}
      </div>
    </div>
  )
}
