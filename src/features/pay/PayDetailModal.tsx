import { useQuery } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, Notice } from '@/components/ui'
import { DAY_LABEL, minToHm } from '@/ipc/bell'
import { errorMessage } from '@/ipc/invoke'
import { payApi, won, type PayCase, type PayQuery } from '@/ipc/pay'
import s from './PayPanel.module.css'

/**
 * 계산 근거.
 *
 * 수당은 실제 지급으로 이어지므로 **왜 이 금액인지** 사람이 확인할 수 있어야
 * 한다. 세어진 보결을 하나하나 보여 주고, 정책이 만든 계산식을 그대로 적는다.
 *
 * 학급 표기는 기록 당시 스냅샷(`class_label`)이다. 반 이름을 바꿔도 과거
 * 기록의 표기는 달라지지 않는다.
 */
export function PayDetailModal({
  teacherId,
  query,
  onClose,
}: {
  teacherId: number
  query: PayQuery
  onClose: () => void
}) {
  const { data, isLoading, error } = useQuery({
    queryKey: ['pay-detail', teacherId, query],
    queryFn: () => payApi.detail(teacherId, query),
  })

  return (
    <Modal
      open
      wide
      title={data ? `${data.name} 선생님 — 계산 근거` : '계산 근거'}
      description={data ? `${data.rangeLabel} · ${data.policyLabel}` : undefined}
      onClose={onClose}
      footer={
        <Button variant="primary" onClick={onClose}>
          닫기
        </Button>
      }
    >
      {isLoading && <p className={s.center}>불러오는 중…</p>}
      {error && <Notice tone="danger">{errorMessage(error)}</Notice>}

      {data && (
        <>
          <section className={s.dSection}>
            <h3 className={s.dTitle}>
              보결한 내역
              <span className={s.dCount}>{data.substituted.length}회</span>
            </h3>
            <p className={s.dHint}>
              다른 선생님을 대신해 실제로 들어간 기록입니다. 취소한 배정은 들어 있지 않습니다.
            </p>
            <CaseList
              rows={data.substituted}
              whoLabel="결근"
              empty="이 기간에 대신 들어간 기록이 없습니다."
            />
          </section>

          <section className={s.dSection}>
            <h3 className={s.dTitle}>
              본인으로 인해 발생한 보결
              <span className={s.dCount}>{data.ownCaused.length}회</span>
            </h3>
            <p className={s.dHint}>
              이 선생님이 결근해 <b>다른 선생님이 실제로 들어간</b> 기록입니다. 결근으로 수업이
              비었더라도 아무도 배정되지 않았다면 여기에 들어가지 않습니다.
              {!data.deducts && ' 지금 지급 기준에서는 차감하지 않고 참고로만 보여 줍니다.'}
            </p>
            <CaseList
              rows={data.ownCaused}
              whoLabel="보결"
              empty="이 기간에 본인 결근으로 생긴 보결이 없습니다."
            />
          </section>

          <div className={s.calcBox}>
            <div className={s.calcTitle}>계산</div>
            <ul className={s.calcSteps}>
              {data.steps.map((line, i) => (
                <li key={i}>{line}</li>
              ))}
            </ul>
            <p className={s.calcNote}>
              지금 설정({data.policyLabel} · 1회 {won(data.perCase)}원) 으로 계산한 값입니다.
              설정을 바꾸면 이 화면의 금액도 함께 바뀝니다.
            </p>
          </div>
        </>
      )}
    </Modal>
  )
}

function CaseList({
  rows,
  whoLabel,
  empty,
}: {
  rows: PayCase[]
  whoLabel: string
  empty: string
}) {
  if (rows.length === 0) {
    return <p className={s.dEmpty}>{empty}</p>
  }
  return (
    <div className={s.dList}>
      {rows.map((c, i) => (
        <div key={`${c.date}-${c.startMin}-${i}`} className={s.dItem}>
          <span className={s.dDate}>
            {c.date.slice(5).replace('-', '/')} ({DAY_LABEL[c.dayOfWeek]})
          </span>
          <span className={s.dCls}>{c.classLabel}</span>
          <span className={s.dSlot}>{c.slotLabel}</span>
          <span className={s.dTime}>
            {minToHm(c.startMin)}~{minToHm(c.endMin)}
          </span>
          {c.counterpart && (
            <span className={s.dWho}>
              {whoLabel} <b>{c.counterpart}</b>
            </span>
          )}
        </div>
      ))}
    </div>
  )
}
