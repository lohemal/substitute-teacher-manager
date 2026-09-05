import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Button, Card, Notice } from '@/components/ui'
import { adminApi, type OptionRow } from '@/ipc/admin'
import { errorMessage } from '@/ipc/invoke'
import s from './admin.module.css'

/**
 * 프로그램 동작 옵션.
 *
 * 학교마다 다른 **운영 방침**만 여기에 둔다. 시간이 겹치는 선생님을 후보에
 * 넣는 설정은 만들지 않는다 — 그것은 방침이 아니라 사실의 문제다.
 */
export function OptionsCard() {
  const qc = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: ['settings'],
    queryFn: adminApi.settings,
  })

  /** 저장 전 화면 상태 (동작 옵션만 담는다) */
  const [draft, setDraft] = useState<OptionRow[] | null>(null)
  useEffect(() => setDraft(data?.engine ?? null), [data])

  const invalidateAll = async () => {
    await Promise.all([
      qc.invalidateQueries({ queryKey: ['settings'] }),
      // 옵션이 바뀌면 후보 조회와 현황도 달라진다
      qc.invalidateQueries({ queryKey: ['find-options'] }),
      qc.invalidateQueries({ queryKey: ['find-in-charge'] }),
      qc.invalidateQueries({ queryKey: ['day-plan'] }),
      qc.invalidateQueries({ queryKey: ['stats'] }),
    ])
  }

  const save = useMutation({
    mutationFn: () =>
      adminApi.saveSettings({
        engine: draft!.map((o) => ({ key: o.key, value: o.value })),
      }),
    onSuccess: invalidateAll,
  })

  const reset = useMutation({
    mutationFn: adminApi.resetSettings,
    onSuccess: invalidateAll,
  })

  if (isLoading) return <Card title="보결 판단 동작 옵션">불러오는 중…</Card>
  if (error || !draft || !data) {
    return (
      <Card title="보결 판단 동작 옵션">
        <Notice tone="danger">{errorMessage(error)}</Notice>
      </Card>
    )
  }

  const dirty = JSON.stringify(draft) !== JSON.stringify(data.engine)
  const set = (key: string, value: boolean) =>
    setDraft(draft.map((o) => (o.key === key ? { ...o, value } : o)))

  return (
    <Card
      title="보결 판단 동작 옵션"
      subtitle="학교 운영 방침에 맞게 후보 판단 규칙을 조정합니다"
      collapsible
      rememberKey="options"
      footer={
        <div className={s.footRow}>
          <span className={s.footNote}>
            {dirty ? '바꾼 내용이 아직 저장되지 않았습니다.' : '저장된 설정입니다.'}
          </span>
          <div className={s.footBtns}>
            <Button
              variant="ghost"
              disabled={reset.isPending}
              onClick={() => reset.mutate()}
              title="자료는 그대로 두고 동작 옵션만 처음 상태로"
            >
              기본값으로 되돌리기
            </Button>
            <Button
              variant="primary"
              icon="check"
              disabled={!dirty || save.isPending}
              onClick={() => save.mutate()}
            >
              {save.isPending ? '저장 중…' : '변경 내용 저장'}
            </Button>
          </div>
        </div>
      }
    >
      {(save.error || reset.error) && (
        <Notice tone="danger">{errorMessage(save.error ?? reset.error)}</Notice>
      )}

      <ul className={s.optList}>
        {draft.map((o) => (
          <li key={o.key} className={s.optItem}>
            <label className={s.optLabel}>
              <input
                type="checkbox"
                checked={o.value}
                onChange={(e) => set(o.key, e.target.checked)}
              />
              <span>
                <span className={s.optTitle}>
                  {o.label}
                  {!o.isDefault && <span className={s.optChanged}>기본값 아님</span>}
                </span>
                <span className={s.optHint}>{o.hint}</span>
              </span>
            </label>
          </li>
        ))}
      </ul>

      <div className={s.fixed}>
        <strong>설정으로 끌 수 없는 것</strong>
        <p>{data.fixedNote}</p>
      </div>

    </Card>
  )
}
