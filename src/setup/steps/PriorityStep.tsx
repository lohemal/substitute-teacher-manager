import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Button, Notice } from '@/components/ui'
import { PriorityEditor } from '@/features/priority/PriorityEditor'
import { errorMessage } from '@/ipc/invoke'
import { priorityApi } from '@/ipc/priority'
import { StepFrame } from '../StepFrame'
import type { StepProps } from '../types'
import s from './PriorityStep.module.css'

export function PriorityStep({ nav }: StepProps) {
  const qc = useQueryClient()
  const [blockers, setBlockers] = useState<string[] | null>(null)

  const finish = useMutation({
    mutationFn: priorityApi.finishStep,
    onSuccess: async (problems) => {
      setBlockers(problems)
      if (problems.length === 0) {
        await qc.invalidateQueries({ queryKey: ['setup-state'] })
        nav.goNext()
      }
    },
  })

  return (
    <StepFrame
      title="보결 배정 우선순위"
      description="우리 학교의 보결 배정 기준을 위에서부터 차례로 정합니다. 프로그램은 이 기준으로 추천 순서만 제시하고, 최종 배정은 담당자가 결정합니다."
      footerLeft={
        nav.hasPrev ? (
          <Button variant="ghost" onClick={nav.goPrev}>
            ← 이전
          </Button>
        ) : null
      }
      footerRight={
        <Button variant="primary" onClick={() => finish.mutate()} disabled={finish.isPending}>
          {finish.isPending ? '확인 중…' : '다음 단계 →'}
        </Button>
      }
    >
      {finish.error && <Notice tone="danger">{errorMessage(finish.error)}</Notice>}

      {blockers && blockers.length > 0 && (
        <Notice tone="warn">
          <div>
            <strong>다음 단계로 넘어가기 전에 확인해 주세요</strong>
            <ul className={s.list}>
              {blockers.map((b) => (
                <li key={b}>{b}</li>
              ))}
            </ul>
          </div>
        </Notice>
      )}

      <PriorityEditor />
    </StepFrame>
  )
}
