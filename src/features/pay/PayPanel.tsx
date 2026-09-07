import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'

import { Icon } from '@/components/Icon'
import { EmptyState, Notice } from '@/components/ui'
import { ExportButton } from '@/features/admin/ExportButton'
import { adminApi } from '@/ipc/admin'
import { errorMessage } from '@/ipc/invoke'
import { payApi, won, type PayMode, type PayQuery, type PayRow } from '@/ipc/pay'
import { useRemembered } from '@/lib/remember'
import { PayDetailModal } from './PayDetailModal'
import s from './PayPanel.module.css'

function thisMonth(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`
}

function todayStr(): string {
  const d = new Date()
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

const MODE_LABEL: Record<PayMode, string> = {
  MONTH: '월별',
  CUSTOM: '기간 지정',
}

/**
 * 보결 수당.
 *
 * ## 지금 설정으로 그때그때 계산한다
 *
 * 화면에 보이는 금액은 **남아 있는 배정 기록 × 지금 설정**이다. 1회 수당이나
 * 지급 기준을 바꾸면 이 화면을 다시 열 때 곧바로 반영된다. 계산 결과를 따로
 * 저장하지 않으므로, 기록과 금액이 어긋나는 일이 없다.
 *
 * ## 왜 이 금액인지 확인할 수 있다
 *
 * 수당은 실제 지급으로 이어지는 자료다. 그래서 교사 행을 누르면 그 기간에
 * 세어진 보결이 하나하나 보이고, 계산식도 그대로 보여 준다.
 */
export function PayPanel() {
  const navigate = useNavigate()

  // 월별이 기본. 마지막에 고른 방식으로 열어 준다.
  const [mode, setMode] = useRemembered<PayMode>('pay.mode', 'MONTH')
  const [month, setMonth] = useState(thisMonth)
  const [from, setFrom] = useState(todayStr)
  const [to, setTo] = useState(todayStr)
  const [openId, setOpenId] = useState<number | null>(null)

  const query: PayQuery =
    mode === 'CUSTOM' ? { mode, from, to } : { mode, month }

  const { data, isLoading, error } = useQuery({
    queryKey: ['pay', query],
    queryFn: () => payApi.view(query),
  })

  const [y, m] = month.split('-')

  return (
    <div className={s.wrap}>
      {/* ---------- 기간 ---------- */}
      <section className={s.periodBar}>
        <div className={s.segment}>
          {(['MONTH', 'CUSTOM'] as PayMode[]).map((k) => (
            <button
              key={k}
              type="button"
              className={mode === k ? `${s.seg} ${s.segOn}` : s.seg}
              onClick={() => setMode(k)}
            >
              {MODE_LABEL[k]}
            </button>
          ))}
        </div>

        {mode === 'MONTH' ? (
          <>
            <div className={s.monthNav}>
              <button
                type="button"
                className={`${s.navBtn} ${s.navPrev}`}
                onClick={() => data && setMonth(data.prevMonth)}
                aria-label="이전 달"
                title="이전 달"
              >
                <Icon name="chevronRight" size={17} />
              </button>
              <span className={s.monthNow}>
                {y}년 {Number(m)}월
              </span>
              <button
                type="button"
                className={s.navBtn}
                onClick={() => data && setMonth(data.nextMonth)}
                aria-label="다음 달"
                title="다음 달"
              >
                <Icon name="chevronRight" size={17} />
              </button>
            </div>
            {month !== thisMonth() && (
              <button type="button" className={s.thisMonth} onClick={() => setMonth(thisMonth())}>
                이번 달
              </button>
            )}
          </>
        ) : (
          <div className={s.range}>
            <input
              type="date"
              className={s.date}
              value={from}
              max={to}
              onChange={(e) => setFrom(e.target.value)}
            />
            <span className={s.tilde}>~</span>
            <input
              type="date"
              className={s.date}
              value={to}
              min={from}
              onChange={(e) => setTo(e.target.value)}
            />
          </div>
        )}

        <span className={s.spacer} />
        <ExportButton run={() => adminApi.exportPay(query)} />
      </section>

      {error && <Notice tone="danger">{errorMessage(error)}</Notice>}
      {isLoading && <p className={s.center}>계산하는 중…</p>}

      {data && (
        <>
          {/* ---------- 지금 기준 ---------- */}
          <div className={s.basis}>
            <span className={s.basisItem}>
              기간 <b>{data.rangeLabel}</b>
            </span>
            <span className={s.basisItem}>
              지급 기준 <b>{data.policyLabel}</b>
            </span>
            <span className={s.basisItem}>
              1회 보결 수당 <b>{won(data.perCase)}원</b>
            </span>
            <button
              type="button"
              className={s.basisLink}
              onClick={() => navigate('/settings')}
            >
              설정에서 바꾸기
            </button>
          </div>

          {data.perCaseUnset && (
            <Notice tone="warn">
              <div>
                <strong>1회 보결 수당이 아직 0원입니다.</strong>
                <p>
                  횟수는 그대로 세지만 지급액이 모두 0원으로 나옵니다.{' '}
                  <b>설정 → 보결 수당</b> 에서 학교가 정한 금액을 넣어 주세요.
                </p>
              </div>
            </Notice>
          )}

          {data.termNote && <Notice tone="info">{data.termNote}</Notice>}

          {/* ---------- 요약 ---------- */}
          <div className={s.cards}>
            <div className={s.card}>
              <div className={s.cardLabel}>지급 대상 교사</div>
              <div className={s.cardValue}>
                {data.summary.paidTeachers}
                <span className={s.cardUnit}>명</span>
              </div>
              <div className={s.cardSub}>표에 오른 {data.summary.listedTeachers}명 가운데</div>
            </div>
            <div className={s.card}>
              <div className={s.cardLabel}>총 보결 횟수</div>
              <div className={s.cardValue}>
                {data.summary.totalSubstituted}
                <span className={s.cardUnit}>회</span>
              </div>
              <div className={s.cardSub}>취소한 배정은 빠짐</div>
            </div>
            <div className={s.card}>
              <div className={s.cardLabel}>총 지급 인정 횟수</div>
              <div className={s.cardValue}>
                {data.summary.totalPayable}
                <span className={s.cardUnit}>회</span>
              </div>
              <div className={s.cardSub}>
                {data.policy === 'DEDUCT_OWN_CAUSED'
                  ? `본인 발생 ${data.summary.totalOwnCaused}회 차감 후`
                  : '실제 보결 횟수 전체'}
              </div>
            </div>
            <div className={`${s.card} ${s.cardTotal}`}>
              <div className={s.cardLabel}>총 지급액</div>
              <div className={s.cardValue}>{won(data.summary.totalAmount)}원</div>
              <div className={s.cardSub}>
                {data.summary.totalPayable}회 × {won(data.perCase)}원
              </div>
            </div>
          </div>

          {/* ---------- 교사별 ---------- */}
          <section className={s.tableCard}>
            <div className={s.tableHead}>
              <span className={s.tableTitle}>교사별 계산</span>
              <span className={s.tableNote}>
                교사를 누르면 어떤 보결이 세어졌는지 볼 수 있습니다
              </span>
            </div>

            {data.rows.length === 0 ? (
              <EmptyState
                icon="won"
                title="이 기간에 보결 기록이 없습니다"
                description="보결을 배정하면 여기에 계산이 나타납니다."
              />
            ) : (
              <div className={s.tableWrap}>
                <table className={s.table}>
                  <thead>
                    <tr>
                      <th>교사</th>
                      <th className={s.roleCol}>구분</th>
                      <th className={s.numCol}>보결한 횟수</th>
                      <th className={s.numCol}>본인 발생 보결</th>
                      <th className={s.numCol}>지급 인정 횟수</th>
                      <th className={s.moneyCol}>1회 수당</th>
                      <th className={s.moneyCol}>지급액</th>
                      <th className={s.actCol} />
                    </tr>
                  </thead>
                  <tbody>
                    {data.rows.map((r) => (
                      <Row key={r.teacherId} r={r} onOpen={() => setOpenId(r.teacherId)} />
                    ))}
                    <tr className={s.totalRow}>
                      <td colSpan={2}>합계</td>
                      <td className={s.numCol}>{data.summary.totalSubstituted}</td>
                      <td className={s.numCol}>{data.summary.totalOwnCaused}</td>
                      <td className={s.numCol}>{data.summary.totalPayable}</td>
                      <td className={s.moneyCol} />
                      <td className={s.moneyCol}>{won(data.summary.totalAmount)}원</td>
                      <td className={s.actCol} />
                    </tr>
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {data.policy === 'ALL_ASSIGNED' && data.summary.totalOwnCaused > 0 && (
            <Notice tone="info">
              지금 지급 기준은 <b>실제 보결 횟수 전체 지급</b> 입니다. &lsquo;본인 발생
              보결&rsquo; 은 참고로만 보여 주며 지급 인정 횟수에서 차감하지 않습니다.
            </Notice>
          )}

          {openId !== null && (
            <PayDetailModal teacherId={openId} query={query} onClose={() => setOpenId(null)} />
          )}
        </>
      )}
    </div>
  )
}

function Row({ r, onOpen }: { r: PayRow; onOpen: () => void }) {
  const cls = [s.rowLink, r.active ? '' : s.rowOff].filter(Boolean).join(' ')
  const num = (v: number) => (v === 0 ? <span className={s.zero}>0</span> : v)

  return (
    <tr className={cls} onClick={onOpen}>
      <td>
        <span className={s.name}>{r.name}</span>
        {r.duty && <span className={s.duty}>{r.duty}</span>}
        {!r.active && <span className={s.offTag}>비활성</span>}
      </td>
      <td className={s.roleCol}>{r.roleLabel}</td>
      <td className={s.numCol}>{num(r.substituted)}</td>
      <td className={s.numCol}>{num(r.ownCaused)}</td>
      <td className={s.numCol}>{num(r.payable)}</td>
      <td className={s.moneyCol}>{won(r.perCase)}원</td>
      <td className={`${s.moneyCol} ${s.amountCol}`}>{won(r.amount)}원</td>
      <td className={s.actCol}>
        <button
          type="button"
          className={s.detailBtn}
          onClick={(e) => {
            e.stopPropagation()
            onOpen()
          }}
        >
          상세
        </button>
      </td>
    </tr>
  )
}
