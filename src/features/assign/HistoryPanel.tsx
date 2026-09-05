import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, EmptyState, Notice } from '@/components/ui'
import {
  assignApi,
  CANCEL_REASONS,
  type HistoryFilter,
  type HistoryRow,
} from '@/ipc/assign'
import { adminApi } from '@/ipc/admin'
import { ExportButton } from '@/features/admin/ExportButton'
import { DAY_LABEL, minToHm } from '@/ipc/bell'
import { errorMessage } from '@/ipc/invoke'
import s from './HistoryPanel.module.css'

function todayStr(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

function monthAgo(): string {
  const d = new Date()
  d.setMonth(d.getMonth() - 1)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

type StatusPick = 'ALL' | 'ASSIGNED' | 'CANCELLED'

/** 배정 내역 — 날짜 범위·이름·상태로 거를 수 있다. 취소된 기록도 함께 남는다. */
export function HistoryPanel() {
  const qc = useQueryClient()
  const [from, setFrom] = useState(monthAgo)
  const [to, setTo] = useState(todayStr)
  const [keyword, setKeyword] = useState('')
  const [status, setStatus] = useState<StatusPick>('ALL')
  const [cancelling, setCancelling] = useState<HistoryRow | null>(null)

  const filter: HistoryFilter = { from, to, keyword: keyword.trim() || null, status }
  const { data, isLoading, error } = useQuery({
    queryKey: ['assign-history', filter],
    queryFn: () => assignApi.history(filter),
  })

  const rows = data?.rows ?? []

  return (
    <div className={s.wrap}>
      {/* ---------- 거르기 ---------- */}
      <section className={s.filters}>
        <label className={s.field}>
          <span className={s.label}>기간</span>
          <div className={s.range}>
            <input
              type="date"
              className={s.date}
              value={from}
              onChange={(e) => setFrom(e.target.value)}
            />
            <span className={s.tilde}>~</span>
            <input
              type="date"
              className={s.date}
              value={to}
              onChange={(e) => setTo(e.target.value)}
            />
          </div>
        </label>

        <div className={s.quick}>
          <button
            type="button"
            className={s.quickBtn}
            onClick={() => {
              setFrom(todayStr())
              setTo(todayStr())
            }}
          >
            오늘
          </button>
          <button
            type="button"
            className={s.quickBtn}
            onClick={() => {
              setFrom(monthAgo())
              setTo(todayStr())
            }}
          >
            최근 한 달
          </button>
        </div>

        <label className={s.field}>
          <span className={s.label}>교사 이름</span>
          <input
            type="search"
            className={s.text}
            value={keyword}
            placeholder="결근·배정 교사 모두 검색"
            onChange={(e) => setKeyword(e.target.value)}
          />
        </label>

        <div className={s.field}>
          <span className={s.label}>상태</span>
          <div className={s.segment}>
            {(
              [
                ['ALL', '전체'],
                ['ASSIGNED', '배정'],
                ['CANCELLED', '취소'],
              ] as [StatusPick, string][]
            ).map(([v, t]) => (
              <button
                key={v}
                type="button"
                className={status === v ? `${s.seg} ${s.segOn}` : s.seg}
                onClick={() => setStatus(v)}
              >
                {t}
              </button>
            ))}
          </div>
        </div>
      </section>

      {error && <Notice tone="danger">{errorMessage(error)}</Notice>}
      {isLoading && <p className={s.center}>불러오는 중…</p>}

      {data && (
        <p className={s.summary}>
          <strong className="num">{rows.length}</strong>건
          <span className={s.sep}>·</span>
          배정 <strong className={`${s.ok} num`}>{data.assignedCount}</strong>
          <span className={s.sep}>/</span>
          취소 <span className="num">{data.cancelledCount}</span>
          {data.truncated && <span className={s.more}>결과가 많아 일부만 보여 줍니다</span>}
          <span className={s.exportSlot}>
            <ExportButton run={() => adminApi.exportHistory(filter)} />
          </span>
        </p>
      )}

      {data && rows.length === 0 && (
        <div className={s.emptyCard}>
          <EmptyState
            icon="list"
            title="조건에 맞는 기록이 없습니다"
            description="기간을 넓히거나 검색어를 지워 보세요. 보결 조회 화면에서 배정하면 이곳에 기록이 남습니다."
          />
        </div>
      )}

      {rows.length > 0 && (
        <div className={s.tableCard}>
          <table className={s.table}>
            <thead>
              <tr>
                <th className={s.dateCol}>날짜</th>
                <th className={s.clsCol}>학년/반</th>
                <th className={s.slotCol}>교시</th>
                <th className={s.timeCol}>실제 시각</th>
                <th>결근 교사</th>
                <th>배정 교사</th>
                <th className={s.stateCol}>상태</th>
                <th className={s.actCol} />
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => {
                const dead = r.status === 'CANCELLED'
                return (
                  <tr key={r.id} className={dead ? s.rowDead : undefined}>
                    <td className={s.dateCol}>
                      <span className="num">{r.date}</span>
                      <span className={s.day}>{DAY_LABEL[r.dayOfWeek]}</span>
                    </td>
                    <td className={s.clsCol}>{r.classLabel}</td>
                    <td className={s.slotCol}>
                      {r.slotLabel}
                      {r.subjectName && <span className={s.subject}>{r.subjectName}</span>}
                    </td>
                    <td className={`${s.timeCol} num`}>
                      {minToHm(r.startMin)}~{minToHm(r.endMin)}
                    </td>
                    <td>
                      {r.absentTeacherName ?? <span className={s.none}>—</span>}
                      {r.reasonLabel && <span className={s.reason}>{r.reasonLabel}</span>}
                    </td>
                    <td>
                      <span className={s.subName}>{r.subTeacherName}</span>
                      {r.recommendRank != null && (
                        <span className={s.rank}>{r.recommendRank}순위</span>
                      )}
                    </td>
                    <td className={s.stateCol}>
                      <span className={dead ? `${s.state} ${s.stateDead}` : s.state}>
                        {dead ? '취소됨' : '배정'}
                      </span>
                      {dead && r.cancelReason && (
                        <span className={s.cancelWhy} title={r.cancelReason}>
                          {r.cancelReason}
                        </span>
                      )}
                    </td>
                    <td className={s.actCol}>
                      {!dead && (
                        <button
                          type="button"
                          className={s.cancelBtn}
                          onClick={() => setCancelling(r)}
                        >
                          취소
                        </button>
                      )}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}

      {cancelling && (
        <CancelModal
          row={cancelling}
          onClose={() => setCancelling(null)}
          onDone={async () => {
            await qc.invalidateQueries({ queryKey: ['assign-history'] })
            setCancelling(null)
          }}
        />
      )}
    </div>
  )
}

/** 배정 취소. 기록은 지우지 않고 상태만 바꾼다. */
function CancelModal({
  row,
  onClose,
  onDone,
}: {
  row: HistoryRow
  onClose: () => void
  onDone: () => void
}) {
  const [reason, setReason] = useState<string>(CANCEL_REASONS[0])
  const [custom, setCustom] = useState('')
  const useCustom = reason === '__custom__'

  const run = useMutation({
    mutationFn: () => assignApi.cancel(row.id, useCustom ? custom.trim() : reason),
    onSuccess: onDone,
  })

  return (
    <Modal
      open
      title="보결 배정 취소"
      description="기록을 지우지 않고 '취소됨'으로 표시합니다. 보결 횟수 집계에서는 빠지고, 같은 시간에 다시 배정할 수 있습니다."
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            닫기
          </Button>
          <Button
            variant="danger"
            disabled={run.isPending || (useCustom && !custom.trim())}
            onClick={() => run.mutate()}
          >
            {run.isPending ? '취소하는 중…' : '배정 취소'}
          </Button>
        </>
      }
    >
      {run.error && <Notice tone="danger">{errorMessage(run.error)}</Notice>}

      <dl className={s.sheet}>
        <dt>날짜</dt>
        <dd className="num">
          {row.date} ({DAY_LABEL[row.dayOfWeek]})
        </dd>
        <dt>대상</dt>
        <dd>
          {row.classLabel} {row.slotLabel}
          <span className={`${s.sub} num`}>
            {minToHm(row.startMin)}~{minToHm(row.endMin)}
          </span>
        </dd>
        <dt>배정 교사</dt>
        <dd>
          <strong>{row.subTeacherName}</strong>
        </dd>
      </dl>

      <div className={s.field}>
        <span className={s.label}>
          취소 사유 <span className={s.optional}>(선택)</span>
        </span>
        <select
          className={`${s.text} ${s.full}`}
          value={reason}
          onChange={(e) => setReason(e.target.value)}
        >
          {CANCEL_REASONS.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
          <option value="__custom__">직접 입력</option>
        </select>
        {useCustom && (
          <input
            type="text"
            className={`${s.text} ${s.full} ${s.gapTop}`}
            value={custom}
            maxLength={100}
            placeholder="취소 사유를 적어 주세요"
            onChange={(e) => setCustom(e.target.value)}
          />
        )}
      </div>
    </Modal>
  )
}
