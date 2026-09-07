import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Navigate, useNavigate } from 'react-router-dom'

import { Icon } from '@/components/Icon'
import { Button } from '@/components/ui'
import { setupApi, STEP_META } from '@/ipc/setup'
import { errorMessage } from '@/ipc/invoke'
import s from './WelcomePage.module.css'

export function WelcomePage() {
  const navigate = useNavigate()
  const qc = useQueryClient()

  const { data: state, isLoading } = useQuery({
    queryKey: ['setup-state'],
    queryFn: setupApi.getState,
  })

  const restart = useMutation({
    mutationFn: setupApi.restart,
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ['setup-state'] })
      navigate('/setup/SCHOOL')
    },
  })

  // 설정을 이미 마쳤다면 이 화면에 머물 이유가 없다.
  //
  // 모든 단계가 DONE 이어도 hasProgress 는 참이므로, 이 가드가 없으면
  // 설정을 마친 뒤에도 '설정을 이어서 진행할까요?' 가 뜬다.
  if (state?.completed) {
    return <Navigate to="/find" replace />
  }

  const resuming = !!state?.hasProgress
  const lastStep = state?.lastWorkedStep

  return (
    <div className={s.wrap}>
      <div className={s.panel}>
        <div className={s.mark} aria-hidden="true">
          <Icon name="check" size={30} strokeWidth={2.6} />
        </div>

        <h1 className={s.title}>보결 배정 시스템</h1>
        <p className={s.lead}>
          우리 학교의 보결 업무를
          <br />
          쉽고 공정하게 관리하세요.
        </p>

        {isLoading ? (
          <p className={s.loading}>준비하는 중…</p>
        ) : resuming ? (
          <div className={s.resumeBox}>
            <p className={s.resumeQ}>설정을 이어서 진행할까요?</p>
            {lastStep && (
              <p className={s.resumeLast}>
                마지막 작업: <strong>{STEP_META[lastStep].title}</strong>
              </p>
            )}
            <div className={s.actions}>
              <Button
                variant="primary"
                onClick={() => navigate(`/setup/${state.resumeStep}`)}
                className={s.wide}
              >
                이어서 설정하기
              </Button>
              <Button
                variant="ghost"
                onClick={() => restart.mutate()}
                disabled={restart.isPending}
                className={s.wide}
              >
                처음 단계부터 다시 보기
              </Button>
            </div>
            <p className={s.hint}>
              처음부터 다시 보더라도 지금까지 입력한 내용은 지워지지 않습니다.
            </p>
            {restart.error && <p className={s.error}>{errorMessage(restart.error)}</p>}
          </div>
        ) : (
          <div className={s.actions}>
            <Button variant="primary" onClick={() => navigate('/setup/SCHOOL')} className={s.wide}>
              시작하기
            </Button>
            <p className={s.hint}>
              처음 한 번만 설정하면 됩니다. 5단계, 약 10~20분 걸립니다.
            </p>
          </div>
        )}
      </div>

      <ul className={s.features}>
        <li>
          <span className={s.dot} />
          실제 시작·종료 시각을 기준으로 보결 가능 교사를 정확히 찾습니다
        </li>
        <li>
          <span className={s.dot} />
          학교가 정한 기준에 따라 공정한 추천 순서를 제시합니다
        </li>
        <li>
          <span className={s.dot} />
          모든 자료는 이 컴퓨터에만 저장됩니다
        </li>
      </ul>
    </div>
  )
}
