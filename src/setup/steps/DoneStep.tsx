import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'

import { Icon } from '@/components/Icon'
import { Button, Notice } from '@/components/ui'
import { errorMessage } from '@/ipc/invoke'
import { schoolApi } from '@/ipc/school'
import { setupApi, STEP_META, STEP_ORDER, type SetupStepKey } from '@/ipc/setup'
import { StepFrame } from '../StepFrame'
import type { StepProps } from '../types'
import s from './DoneStep.module.css'

export function DoneStep({ nav, state }: StepProps) {
  const qc = useQueryClient()
  const navigate = useNavigate()

  const { data: school } = useQuery({ queryKey: ['school'], queryFn: schoolApi.get })

  const complete = useMutation({
    mutationFn: setupApi.complete,
    onSuccess: (fresh) => {
      // 서버가 돌려준 상태를 **곧바로** 캐시에 넣는다.
      //
      // 다시 물어보고(invalidate) 기다리면, 답이 화면에 반영되기 전에
      // 아래 navigate 가 먼저 일어날 수 있다. 그러면 라우터가 아직
      // '설정 미완료'로 보고 시작 화면으로 되돌려 버린다.
      qc.setQueryData(['setup-state'], fresh)
      // 버전 정보는 화면 진입을 막지 않으므로 기다리지 않는다
      void qc.invalidateQueries({ queryKey: ['app-info'] })
      navigate('/find', { replace: true })
    },
  })

  const contentSteps = STEP_ORDER.filter((k) => k !== 'DONE')
  const statusOf = (k: SetupStepKey) => state.steps.find((x) => x.key === k)?.status ?? 'PENDING'
  const unfinished = contentSteps.filter((k) => statusOf(k) !== 'DONE')

  const totalClasses = school?.classCounts.reduce((a, c) => a + c.count, 0) ?? 0

  return (
    <StepFrame
      title="설정 완료"
      description="입력한 내용을 확인하고 마치면 보결 조회 화면으로 이동합니다."
      footerLeft={
        <Button variant="ghost" onClick={nav.goPrev}>
          ← 이전
        </Button>
      }
      footerRight={
        <Button
          variant="accent"
          onClick={() => complete.mutate()}
          disabled={complete.isPending}
          icon="check"
        >
          {complete.isPending ? '마무리하는 중…' : '설정 마치고 시작하기'}
        </Button>
      }
    >
      {complete.error && <Notice tone="danger">{errorMessage(complete.error)}</Notice>}

      <div className={s.card}>
        <h2 className={s.cardTitle}>입력한 내용</h2>
        {school ? (
          <dl className={s.summary}>
            <dt>학교</dt>
            <dd>{school.name}</dd>
            <dt>학기</dt>
            <dd>{school.termName}</dd>
            <dt>학년 · 학급</dt>
            <dd>
              {school.minGrade}~{school.maxGrade}학년 · 전체 {totalClasses}개 학급
            </dd>
          </dl>
        ) : (
          <p className={s.muted}>학교 정보가 아직 저장되지 않았습니다.</p>
        )}
      </div>

      <div className={s.card}>
        <h2 className={s.cardTitle}>설정 단계</h2>
        <ul className={s.steps}>
          {contentSteps.map((k) => {
            const st = statusOf(k)
            const done = st === 'DONE'
            return (
              <li key={k} className={done ? `${s.step} ${s.stepDone}` : s.step}>
                <span className={s.stepIcon}>
                  {done ? <Icon name="check" size={14} strokeWidth={3} /> : '—'}
                </span>
                <span className={s.stepName}>{STEP_META[k].title}</span>
                <span className={s.stepStatus}>
                  {done ? '완료' : st === 'SKIPPED' ? '건너뜀' : '아직 안 함'}
                </span>
                {!done && (
                  <button type="button" className={s.stepGo} onClick={() => nav.goTo(k)}>
                    설정하기 →
                  </button>
                )}
              </li>
            )
          })}
        </ul>
      </div>

      {unfinished.length > 0 && (
        <Notice tone="warn">
          아직 완료하지 않은 단계가 {unfinished.length}개 있습니다. 지금 마쳐도 되지만,
          <strong> 시정표와 교사 정보가 있어야 보결 조회를 사용할 수 있습니다.</strong> 왼쪽 메뉴의
          시간표 관리·교사 관리에서 언제든 이어서 설정할 수 있습니다.
        </Notice>
      )}
    </StepFrame>
  )
}
