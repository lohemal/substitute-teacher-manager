import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Button, Notice } from '@/components/ui'
import { bellApi } from '@/ipc/bell'
import { errorMessage } from '@/ipc/invoke'
import { BellEditor } from '@/features/bell/BellEditor'
import { StepFrame } from '../StepFrame'
import type { StepProps } from '../types'
import s from './BellStep.module.css'

export function BellStep({ nav }: StepProps) {
  const qc = useQueryClient()
  const [blockers, setBlockers] = useState<string[] | null>(null)

  const finish = useMutation({
    mutationFn: bellApi.finishStep,
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
      title="학년별 시정표 · 점심시간"
      description="학년마다 교시 시각과 점심시간이 다를 수 있습니다. 보결 가능 여부는 교시 번호가 아니라 여기 입력한 실제 시각으로 판단합니다."
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

      <BellEditor />
    </StepFrame>
  )
}
