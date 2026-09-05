import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, Notice } from '@/components/ui'
import { assignApi, type AbsenceRow } from '@/ipc/assign'
import { minToHm } from '@/ipc/bell'
import { errorMessage } from '@/ipc/invoke'
import s from './AbsenceDeleteModal.module.css'

interface Props {
  row: AbsenceRow
  onClose: () => void
  onDeleted: () => void
}

function rangeText(a: AbsenceRow): string {
  if (a.isAllDay) return '종일'
  return `${minToHm(a.startMin ?? 0)}~${minToHm(a.endMin ?? 0)}`
}

/**
 * 결근 삭제 확인.
 *
 * 결근하기로 했다가 출근하게 되거나 대체 교사를 구한 경우에 쓴다.
 * 삭제해도 이 결근으로 이미 배정한 보결은 그대로 남는다 — 사람이 이미
 * 그렇게 알고 있을 수 있으므로 프로그램이 마음대로 지우지 않는다.
 */
export function AbsenceDeleteModal({ row, onClose, onDeleted }: Props) {
  const qc = useQueryClient()

  const run = useMutation({
    mutationFn: () => assignApi.absenceCancel(row.id),
    onSuccess: async () => {
      await Promise.all([
        qc.invalidateQueries({ queryKey: ['absences'] }),
        qc.invalidateQueries({ queryKey: ['day-plan'] }),
        qc.invalidateQueries({ queryKey: ['stats'] }),
      ])
      onDeleted()
    },
  })

  return (
    <Modal
      open
      title="결근 삭제"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            닫기
          </Button>
          <Button variant="danger" disabled={run.isPending} onClick={() => run.mutate()}>
            {run.isPending ? '삭제하는 중…' : '삭제'}
          </Button>
        </>
      }
    >
      {run.error && <Notice tone="danger">{errorMessage(run.error)}</Notice>}

      <p className={s.ask}>
        <strong>{row.teacherName}</strong> 선생님의{' '}
        <span className="num">{row.date}</span> 결근
        <span className={s.detail}>
          ({row.reasonLabel} · {rangeText(row)})
        </span>
        을 삭제하시겠습니까?
      </p>

      {row.subCount > 0 && (
        <Notice tone="warn">
          <div>
            이 결근으로 이미 배정한 보결이 <strong>{row.subCount}건</strong> 있습니다.
            <p className={s.warnBody}>
              결근을 삭제해도 그 보결은 그대로 남습니다. 필요하면 <b>배정 내역</b> 화면에서
              따로 취소해 주세요.
            </p>
          </div>
        </Notice>
      )}

      <p className={s.note}>
        삭제하면 보결 조회와 현황에서 곧바로 빠집니다. 기록 자체는 '취소됨'으로 남습니다.
      </p>
    </Modal>
  )
}
