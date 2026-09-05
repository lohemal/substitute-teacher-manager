import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'

import { Modal } from '@/components/Modal'
import { Button, Card, Notice } from '@/components/ui'
import { adminApi, RESET_PHRASE } from '@/ipc/admin'
import { errorMessage } from '@/ipc/invoke'
import s from './admin.module.css'

/** 자료 저장 위치 안내 */
export function LocationCard() {
  const { data } = useQuery({ queryKey: ['settings'], queryFn: adminApi.settings })
  if (!data) return null

  return (
    <Card
      title="자료 저장 위치"
      subtitle="자료 파일 · 백업 · 내보낸 파일이 있는 곳"
      collapsible
      rememberKey="location"
    >
      <dl className={s.dl}>
        <dt>자료 파일</dt>
        <dd>
          <span className={`${s.path} selectable`}>{data.dbPath}</span>
          <button type="button" className={s.linkBtn} onClick={() => adminApi.openDataFolder()}>
            폴더 열기
          </button>
        </dd>

        <dt>백업</dt>
        <dd>
          <span className={`${s.path} selectable`}>{data.backupDir}</span>
          <button type="button" className={s.linkBtn} onClick={() => adminApi.openBackupFolder()}>
            폴더 열기
          </button>
        </dd>

        <dt>내보낸 파일</dt>
        <dd>
          <span className={`${s.path} selectable`}>{data.exportDir}</span>
          <button type="button" className={s.linkBtn} onClick={() => adminApi.openExportFolder()}>
            폴더 열기
          </button>
        </dd>
      </dl>
      <p className={s.hint}>
        이 폴더째로 복사해 두면 다른 컴퓨터에서도 그대로 쓸 수 있습니다. 자료는 이 컴퓨터
        안에만 있고 어디로도 보내지 않습니다.
      </p>
    </Card>
  )
}

/**
 * 자료 전체 초기화.
 *
 * 되돌릴 수 없는 유일한 기능이므로 **문구를 직접 입력**하게 하고,
 * 지우기 직전에 반드시 백업을 남긴다. 동작 옵션 초기화와는 완전히 떼어 놓았다.
 */
export function DangerCard() {
  const qc = useQueryClient()
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)
  const [phrase, setPhrase] = useState('')
  const [done, setDone] = useState<string | null>(null)

  const run = useMutation({
    mutationFn: () => adminApi.dataReset(phrase),
    onSuccess: async (r) => {
      setOpen(false)
      setPhrase('')
      setDone(r.backup)
      await qc.invalidateQueries()
      navigate('/welcome')
    },
  })

  return (
    <Card
      title="자료 전체 초기화"
      subtitle="모든 자료를 지우고 처음 상태로 (되돌릴 수 없음)"
      collapsible
      rememberKey="danger"
    >
      {done && (
        <Notice tone="info">
          <div>
            자료를 모두 지웠습니다. 지우기 직전 자료는 <code>{done}</code> 로 백업해 두었습니다.
            <p className={s.warnBody}>
              되돌리려면 [자료 백업 · 복원]에서 그 파일로 복원하세요.
            </p>
          </div>
        </Notice>
      )}

      <Notice tone="danger">
        <div>
          <strong>이 기능은 되돌릴 수 없습니다.</strong>
          <p className={s.warnBody}>
            학교 설정 · 학급 · 교사 · 시간표 · 결근 · 보결 기록을 <b>모두</b> 지우고 처음
            상태로 돌아갑니다. 다음 학교로 옮기며 프로그램을 넘겨줄 때처럼 정말 필요한
            경우에만 쓰세요.
          </p>
          <p className={s.warnBody}>
            동작 옵션만 처음으로 되돌리려면 위 [보결 판단 동작 옵션]의{' '}
            <b>[기본값으로 되돌리기]</b>를 쓰세요. 그것은 자료를 지우지 않습니다.
          </p>
        </div>
      </Notice>

      <div className={s.dangerRow}>
        <Button variant="danger" onClick={() => setOpen(true)}>
          자료 전체 초기화…
        </Button>
      </div>

      {open && (
        <Modal
          open
          title="정말 모든 자료를 지우시겠습니까?"
          onClose={() => {
            setOpen(false)
            setPhrase('')
          }}
          footer={
            <>
              <Button
                variant="ghost"
                onClick={() => {
                  setOpen(false)
                  setPhrase('')
                }}
              >
                닫기
              </Button>
              <Button
                variant="danger"
                disabled={phrase.trim() !== RESET_PHRASE || run.isPending}
                onClick={() => run.mutate()}
              >
                {run.isPending ? '지우는 중…' : '모두 지우기'}
              </Button>
            </>
          }
        >
          {run.error && <Notice tone="danger">{errorMessage(run.error)}</Notice>}

          <Notice tone="danger">
            <div>
              지금까지 쌓인 <strong>보결 기록과 통계가 모두 사라집니다.</strong>
              <p className={s.warnBody}>
                지우기 직전에 자동으로 백업을 남기므로, 실수했다면 [자료 백업 · 복원]에서
                되돌릴 수 있습니다.
              </p>
            </div>
          </Notice>

          <label className={s.field}>
            <span className={s.fieldLabel}>
              계속하려면 <code className={s.phrase}>{RESET_PHRASE}</code> 를 그대로 입력하세요
            </span>
            <input
              type="text"
              className={s.text}
              value={phrase}
              autoFocus
              placeholder={RESET_PHRASE}
              onChange={(e) => setPhrase(e.target.value)}
            />
          </label>
        </Modal>
      )}
    </Card>
  )
}
