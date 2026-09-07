import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { NumberInput } from '@/components/NumberInput'
import { Button, Card, Notice } from '@/components/ui'
import { adminApi } from '@/ipc/admin'
import { errorMessage } from '@/ipc/invoke'
import { won, type PayPolicy } from '@/ipc/pay'
import s from './admin.module.css'

/** 1회 보결 수당의 상한. Rust 의 `SUB_PAY_MAX` 와 같다. */
const MAX = 1_000_000

/**
 * 보결 수당 설정.
 *
 * ## 여기에는 학교 규정만 담는다
 *
 * 1회 금액과 지급 기준만 저장한다. 계산 결과(누가 얼마)는 저장하지 않고
 * 조회할 때마다 배정 기록에서 다시 센다. 그래서 여기서 금액을 바꾸면
 * 보결 수당 화면의 결과가 곧바로 따라온다.
 *
 * ## 기존 기록은 건드리지 않는다
 *
 * 15,000원을 20,000원으로 바꾸어도 지나간 배정 기록은 하나도 달라지지
 * 않는다. 다시 계산될 뿐이다. '9월 지급자료는 당시 금액으로 확정해 둔다'
 * 같은 마감 기능은 아직 없다 — 필요해지면 계산을 그대로 두고 따로 얹는다.
 */
export function PayCard() {
  const qc = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: ['settings'],
    queryFn: adminApi.settings,
  })

  const [perCase, setPerCase] = useState<number | null>(null)
  const [policy, setPolicy] = useState<PayPolicy | null>(null)
  useEffect(() => {
    setPerCase(data?.subPayPerCase ?? null)
    setPolicy(data?.subPayPolicy ?? null)
  }, [data])

  const save = useMutation({
    mutationFn: () =>
      adminApi.saveSettings({ subPayPerCase: perCase!, subPayPolicy: policy! }),
    onSuccess: async () => {
      await Promise.all([
        qc.invalidateQueries({ queryKey: ['settings'] }),
        // 수당 화면은 이 설정으로 다시 계산한다
        qc.invalidateQueries({ queryKey: ['pay'] }),
        qc.invalidateQueries({ queryKey: ['pay-detail'] }),
      ])
    },
  })

  if (isLoading) {
    return (
      <Card title="보결 수당" subtitle="1회 금액과 지급 기준" collapsible rememberKey="pay">
        <p className={s.muted}>불러오는 중…</p>
      </Card>
    )
  }
  if (error || !data || perCase === null || policy === null) {
    return (
      <Card title="보결 수당" subtitle="1회 금액과 지급 기준" collapsible rememberKey="pay">
        <Notice tone="danger">{errorMessage(error)}</Notice>
      </Card>
    )
  }

  const dirty = perCase !== data.subPayPerCase || policy !== data.subPayPolicy

  return (
    <Card
      title="보결 수당"
      subtitle={`1회 ${won(data.subPayPerCase)}원 · ${
        data.subPayPolicies.find((p) => p.code === data.subPayPolicy)?.label ?? ''
      }`}
      collapsible
      rememberKey="pay"
      footer={
        <div className={s.footRow}>
          <span className={s.footNote}>
            {dirty
              ? '저장하면 보결 수당 화면이 이 기준으로 다시 계산됩니다.'
              : '지나간 배정 기록은 바뀌지 않습니다. 금액만 다시 계산됩니다.'}
          </span>
          <div className={s.footBtns}>
            <Button
              variant="primary"
              icon="check"
              disabled={!dirty || save.isPending}
              onClick={() => save.mutate()}
            >
              {save.isPending ? '저장 중…' : '저장'}
            </Button>
          </div>
        </div>
      }
    >
      {save.error && <Notice tone="danger">{errorMessage(save.error)}</Notice>}

      <div className={s.block} style={{ marginTop: 0, paddingTop: 0, borderTop: 0 }}>
        <div className={s.blockTitle}>1회 보결 수당</div>
        <div className={s.row}>
          <NumberInput value={perCase} onChange={setPerCase} min={0} max={MAX} step={1000} />
          <span className={s.rowLabel} style={{ minWidth: 0 }}>
            원
          </span>
          <span className={s.calc}>{won(perCase)}원</span>
        </div>
        <p className={s.hint}>
          보결 1회에 지급하는 금액입니다. 학교가 정한 금액을 넣어 주세요. 0원으로 두면 횟수는
          세지만 지급액이 0원으로 나옵니다.
        </p>
      </div>

      <div className={s.block}>
        <div className={s.blockTitle}>지급 기준</div>
        <ul className={s.optList}>
          {data.subPayPolicies.map((p) => (
            <li key={p.code} className={s.optItem}>
              <label className={s.optLabel}>
                <input
                  type="radio"
                  name="sub-pay-policy"
                  checked={policy === p.code}
                  onChange={() => setPolicy(p.code)}
                />
                <span>
                  <span className={s.optTitle}>
                    {p.label}
                    {p.code === data.subPayPolicy && <span className={s.optChanged}>지금 기준</span>}
                  </span>
                  <span className={s.optHint}>{p.hint}</span>
                </span>
              </label>
            </li>
          ))}
        </ul>

        <div className={s.fixed}>
          <strong>어느 기준이든 이렇게 셉니다</strong>
          취소한 배정은 횟수에 넣지 않습니다. 결근으로 수업이 비었더라도 아무도 배정되지
          않았다면 차감하지 않습니다. 같은 날 여러 교시는 각각 1회이고, 점심 보결도 1회입니다.
        </div>
      </div>
    </Card>
  )
}
