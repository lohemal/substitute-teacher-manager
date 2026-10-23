import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'

import { Icon } from '@/components/Icon'
import { EmptyState, Notice } from '@/components/ui'
import { ExportButton } from '@/features/admin/ExportButton'
import { adminApi } from '@/ipc/admin'
import { errorMessage } from '@/ipc/invoke'
import { payApi, won, type PayMode, type PayQuery, type PayRow } from '@/ipc/pay'
import { PayDetailModal } from './PayDetailModal'
import {
  paySortDir,
  sortPayRows,
  togglePaySort,
  type PaySort,
  type PaySortField,
} from './sortPayRows'
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
  TERM: '이번 학기',
  MONTH: '월별',
  CUSTOM: '기간 지정',
}

/** 2026-09-01 -> 2026.09.01. */
function dotted(ymd: string): string {
  const [y, m, d] = ymd.split('-')
  return y && m && d ? `${y}.${m}.${d}.` : ymd
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

  // **이번 학기가 기본이다.** 학교에서 수당을 정산할 때 보는 것은 대개
  // 학기 전체라, 메뉴를 열 때마다 달을 고르지 않아도 되게 했다.
  //
  // 마지막에 고른 방식을 기억하지 않는다 — 기억하면 한 번 월별을 본 사람은
  // 그다음부터 늘 월별로 열리게 되어, '들어오면 이번 학기' 라는 약속이
  // 깨진다. 모드를 바꾸는 것은 누르면 끝이라 번거롭지 않다.
  const [mode, setMode] = useState<PayMode>('TERM')
  const [month, setMonth] = useState(thisMonth)
  const [from, setFrom] = useState(todayStr)
  const [to, setTo] = useState(todayStr)
  const [openId, setOpenId] = useState<number | null>(null)

  // 표를 어떻게 줄 세웠는가. 메뉴를 나갔다 들어오면 기본 순서로 시작한다
  // — 어딘가에 적어 두지 않는다.
  const [sort, setSort] = useState<PaySort>(null)

  // 학기 날짜는 보내지 않는다 — Rust 쪽이 현재 학기를 보고 정한다.
  // 화면에 날짜를 베껴 두면 학기를 바꿨을 때 어긋난다.
  const query: PayQuery =
    mode === 'CUSTOM' ? { mode, from, to } : mode === 'TERM' ? { mode } : { mode, month }

  const { data, isLoading, error } = useQuery({
    queryKey: ['pay', query],
    queryFn: () => payApi.view(query),
  })

  // 서버가 준 순서를 건드리지 않고, 보여 줄 순서만 따로 만든다.
  // 정렬을 풀면 이 줄이 원래 배열을 그대로 돌려준다.
  const rows = useMemo(() => sortPayRows(data?.rows ?? [], sort), [data?.rows, sort])

  const [y, m] = month.split('-')

  return (
    <div className={s.wrap}>
      {/* ---------- 기간 ---------- */}
      <section className={s.periodBar}>
        <div className={s.segment}>
          {(['TERM', 'MONTH', 'CUSTOM'] as PayMode[]).map((k) => (
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

        {mode === 'TERM' ? (
          <div className={s.termNow}>
            {data?.termMissing ? (
              <span className={s.termNone}>학기 정보 없음</span>
            ) : data ? (
              <>
                <span className={s.termName}>{data.termLabel ?? data.rangeLabel}</span>
                <span className={`${s.termDates} num`}>
                  {dotted(data.from)} ~ {dotted(data.to)}
                </span>
              </>
            ) : null}
          </div>
        ) : mode === 'MONTH' ? (
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

          {data.termMissing && (
            <Notice tone="warn">
              <div>
                <strong>현재 학기 정보를 확인할 수 없습니다.</strong>
                <p>
                  이번 학기로 계산하려면 학기의 시작일과 종료일이 있어야 합니다.{' '}
                  <b>설정 → 학기</b> 에서 확인해 주세요. 그 동안에는 위에서{' '}
                  <b>월별</b> 이나 <b>기간 지정</b> 으로 조회할 수 있습니다.
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
                      <SortTh field="name" label="교사" sort={sort} onSort={setSort} />
                      <SortTh
                        field="role"
                        label="구분"
                        className={s.roleCol}
                        sort={sort}
                        onSort={setSort}
                      />
                      <SortTh
                        field="substituted"
                        label="보결한 횟수"
                        className={s.numCol}
                        sort={sort}
                        onSort={setSort}
                      />
                      <SortTh
                        field="ownCaused"
                        label="본인 발생 보결"
                        className={s.numCol}
                        sort={sort}
                        onSort={setSort}
                      />
                      <SortTh
                        field="payable"
                        label="지급 인정 횟수"
                        className={s.numCol}
                        sort={sort}
                        onSort={setSort}
                      />
                      <SortTh
                        field="perCase"
                        label="1회 수당"
                        className={s.moneyCol}
                        sort={sort}
                        onSort={setSort}
                      />
                      <SortTh
                        field="amount"
                        label="지급액"
                        className={s.moneyCol}
                        sort={sort}
                        onSort={setSort}
                      />
                      <th className={s.actCol} />
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((r) => (
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

/**
 * 눌러서 줄 세울 수 있는 머리글.
 *
 * 글자와 화살표를 **한 덩어리로 가운데**에 둔다. 본문의 정렬(교사명은 왼쪽,
 * 숫자·금액은 오른쪽)은 그대로 둔다 — 숫자는 자릿수를 맞춰 읽는 것이라
 * 가운데로 옮기면 읽기 나빠진다.
 *
 * 진짜 `button` 이라 키보드로도 누를 수 있고, 읽어 주는 기계를 위해
 * `aria-sort` 를 붙인다.
 */
function SortTh({
  field,
  label,
  className,
  sort,
  onSort,
}: {
  field: PaySortField
  label: string
  className?: string
  sort: PaySort
  onSort: (next: PaySort) => void
}) {
  const dir = paySortDir(sort, field)
  const next = dir === null ? '오름차순' : dir === 'asc' ? '내림차순' : '정렬 해제'
  return (
    <th
      className={[s.sortTh, className, dir ? s.sortOn : ''].filter(Boolean).join(' ')}
      aria-sort={dir === 'asc' ? 'ascending' : dir === 'desc' ? 'descending' : 'none'}
    >
      <button
        type="button"
        className={s.sortBtn}
        onClick={() => onSort(togglePaySort(sort, field))}
        title={`${label} 기준으로 ${next}`}
      >
        <span>{label}</span>
        <span className={s.sortMark} aria-hidden="true">
          {dir === 'asc' ? '▲' : dir === 'desc' ? '▼' : '↕'}
        </span>
      </button>
    </th>
  )
}

function Row({ r, onOpen }: { r: PayRow; onOpen: () => void }) {
  const cls = [s.rowLink, r.active ? '' : s.rowOff].filter(Boolean).join(' ')
  const num = (v: number) => (v === 0 ? <span className={s.zero}>0</span> : v)

  return (
    <tr className={cls} onClick={onOpen}>
      <td className={s.nameCol}>
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
