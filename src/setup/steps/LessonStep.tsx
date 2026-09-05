import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Button, Notice } from '@/components/ui'
import { LessonEditor } from '@/features/lesson/LessonEditor'
import { errorMessage } from '@/ipc/invoke'
import { lessonApi } from '@/ipc/lesson'
import { StepFrame } from '../StepFrame'
import type { StepProps } from '../types'
import s from './LessonStep.module.css'

export function LessonStep({ nav }: StepProps) {
  const qc = useQueryClient()
  const [blockers, setBlockers] = useState<string[] | null>(null)

  const { data: warnings } = useQuery({
    queryKey: ['lesson-warnings'],
    queryFn: lessonApi.warnings,
  })

  const finish = useMutation({
    mutationFn: lessonApi.finishStep,
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
      title="전담교사 시간표"
      description="전담교사의 주간 시간표만 넣으면 됩니다. 담임교사가 언제 수업하는지는 여기서 자동으로 계산되므로 따로 입력하지 않습니다."
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
        <Notice tone="danger">
          <div>
            <strong>다음 단계로 넘어가기 전에 고쳐 주세요</strong>
            <ul className={s.list}>
              {blockers.map((b) => (
                <li key={b}>{b}</li>
              ))}
            </ul>
          </div>
        </Notice>
      )}

      {warnings && warnings.length > 0 && (
        <Notice tone="warn">
          <div>
            <strong>확인해 주세요</strong>
            <ul className={s.list}>
              {warnings.map((w) => (
                <li key={w}>{w}</li>
              ))}
            </ul>
            이대로 넘어갈 수는 있습니다.
          </div>
        </Notice>
      )}

      <LessonEditor />
    </StepFrame>
  )
}
