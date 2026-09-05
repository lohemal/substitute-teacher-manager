import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Button, Notice } from '@/components/ui'
import { TeacherManager } from '@/features/teacher/TeacherManager'
import { errorMessage } from '@/ipc/invoke'
import { teacherApi } from '@/ipc/teacher'
import { StepFrame } from '../StepFrame'
import type { StepProps } from '../types'
import s from './TeacherStep.module.css'

export function TeacherStep({ nav }: StepProps) {
  const qc = useQueryClient()
  const [blockers, setBlockers] = useState<string[] | null>(null)

  const finish = useMutation({
    mutationFn: teacherApi.finishStep,
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
      title="교사 등록"
      description="선생님 명단을 등록하고 담임 학급과 담당 과목을 지정합니다. 담임 선생님의 수업 시간은 따로 입력하지 않아도 전담 시간표에서 자동으로 계산됩니다."
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

      <TeacherManager />
    </StepFrame>
  )
}
