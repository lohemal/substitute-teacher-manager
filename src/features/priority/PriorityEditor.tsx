import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Icon } from '@/components/Icon'
import { Button, Notice } from '@/components/ui'
import { errorMessage } from '@/ipc/invoke'
import { priorityApi, type RuleView } from '@/ipc/priority'
import s from './PriorityEditor.module.css'

export function PriorityEditor() {
  const qc = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: ['priority-rules'],
    queryFn: priorityApi.list,
  })

  const [rules, setRules] = useState<RuleView[] | null>(null)
  const [dirty, setDirty] = useState(false)
  const [dragKey, setDragKey] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  // 저장 중이 아니면 서버 값을 그대로 보여 준다
  useEffect(() => {
    if (data && !dirty) setRules(data.rules)
  }, [data, dirty])

  const afterSave = (v: Awaited<ReturnType<typeof priorityApi.list>>) => {
    qc.setQueryData(['priority-rules'], v)
    // 조회 화면이 새 기준으로 다시 정렬되도록 알린다
    qc.invalidateQueries({ queryKey: ['setup-state'] })
    setRules(v.rules)
    setDirty(false)
    setSaved(true)
    setTimeout(() => setSaved(false), 2500)
  }

  const save = useMutation({
    mutationFn: () =>
      priorityApi.save(
        (rules ?? []).map((r) => ({ ruleKey: r.ruleKey, enabled: r.enabled })),
      ),
    onSuccess: afterSave,
  })

  const reset = useMutation({
    mutationFn: priorityApi.reset,
    onSuccess: afterSave,
  })

  if (isLoading) return <div className={s.center}>불러오는 중…</div>
  if (error || !data || !rules) return <Notice tone="danger">{errorMessage(error)}</Notice>

  const move = (index: number, delta: number) => {
    const next = [...rules]
    const to = index + delta
    if (to < 0 || to >= next.length) return
    ;[next[index], next[to]] = [next[to], next[index]]
    setRules(next)
    setDirty(true)
  }

  const moveTo = (fromKey: string, toIndex: number) => {
    const from = rules.findIndex((r) => r.ruleKey === fromKey)
    if (from < 0 || from === toIndex) return
    const next = [...rules]
    const [item] = next.splice(from, 1)
    next.splice(toIndex, 0, item)
    setRules(next)
    setDirty(true)
  }

  const toggle = (key: string) => {
    setRules(rules.map((r) => (r.ruleKey === key ? { ...r, enabled: !r.enabled } : r)))
    setDirty(true)
  }

  const enabled = rules.filter((r) => r.enabled)
  const noneOn = enabled.length === 0

  return (
    <div className={s.wrap}>
      {/* ---------- 현재 기준 한눈에 ---------- */}
      <div className={s.summary}>
        <span className={s.summaryLabel}>지금 추천 순서</span>
        {noneOn ? (
          <span className={s.summaryEmpty}>켜 둔 기준이 없습니다</span>
        ) : (
          <span className={s.summaryList}>
            {enabled.map((r, i) => (
              <span key={r.ruleKey} className={s.summaryItem}>
                <span className={s.summaryNo}>{i + 1}</span>
                {r.label}
              </span>
            ))}
          </span>
        )}
      </div>

      {save.error && <Notice tone="danger">{errorMessage(save.error)}</Notice>}
      {reset.error && <Notice tone="danger">{errorMessage(reset.error)}</Notice>}
      {noneOn && (
        <Notice tone="warn">
          기준을 하나 이상 켜 주세요. 모두 끄면 추천 순서를 정할 수 없습니다.
        </Notice>
      )}

      {/* ---------- 기준 목록 ---------- */}
      <ol className={s.list}>
        {rules.map((r, i) => {
          const activeNo = r.enabled ? enabled.findIndex((x) => x.ruleKey === r.ruleKey) + 1 : null
          return (
            <li
              key={r.ruleKey}
              className={[s.item, r.enabled ? s.itemOn : s.itemOff, dragKey === r.ruleKey ? s.itemDrag : '']
                .filter(Boolean)
                .join(' ')}
              draggable
              onDragStart={() => setDragKey(r.ruleKey)}
              onDragEnd={() => setDragKey(null)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => {
                e.preventDefault()
                if (dragKey) moveTo(dragKey, i)
                setDragKey(null)
              }}
            >
              <span className={s.handle} title="끌어서 순서 바꾸기" aria-hidden="true">
                ⋮⋮
              </span>

              <span className={activeNo ? s.no : `${s.no} ${s.noOff}`}>
                {activeNo ? `${activeNo}순위` : '사용 안 함'}
              </span>

              <label className={s.main}>
                <input
                  type="checkbox"
                  className={s.check}
                  checked={r.enabled}
                  onChange={() => toggle(r.ruleKey)}
                />
                <span>
                  <span className={s.label}>{r.label}</span>
                  <span className={s.hint}>{r.hint}</span>
                </span>
              </label>

              <span className={s.arrows}>
                <button
                  type="button"
                  className={s.arrow}
                  disabled={i === 0}
                  onClick={() => move(i, -1)}
                  aria-label={`${r.label} 위로`}
                >
                  ▲
                </button>
                <button
                  type="button"
                  className={s.arrow}
                  disabled={i === rules.length - 1}
                  onClick={() => move(i, 1)}
                  aria-label={`${r.label} 아래로`}
                >
                  ▼
                </button>
              </span>
            </li>
          )
        })}
      </ol>

      <p className={s.tieBreak}>
        <strong>모든 기준이 같을 때</strong> {data.tieBreak} 순서로 정합니다. 같은 자료라면 언제
        조회해도 순서가 바뀌지 않습니다.
      </p>

      <div className={s.foot}>
        <div className={s.footLeft}>
          {saved && !dirty && (
            <span className={s.savedMark}>
              <Icon name="check" size={15} strokeWidth={3} /> 저장했습니다. 다음 조회부터 적용됩니다.
            </span>
          )}
          {dirty && <span className={s.dirtyMark}>저장하지 않은 변경 내용이 있습니다</span>}
          {!dirty && !saved && data.isDefault && (
            <span className={s.defaultMark}>기본 설정을 쓰고 있습니다</span>
          )}
        </div>
        <div className={s.footRight}>
          <Button
            variant="ghost"
            onClick={() => reset.mutate()}
            disabled={reset.isPending || save.isPending}
          >
            기본값으로 되돌리기
          </Button>
          {dirty && (
            <Button
              variant="ghost"
              onClick={() => {
                setRules(data.rules)
                setDirty(false)
              }}
            >
              되돌리기
            </Button>
          )}
          <Button
            variant="primary"
            disabled={!dirty || noneOn || save.isPending}
            onClick={() => save.mutate()}
          >
            {save.isPending ? '저장 중…' : '변경 내용 저장'}
          </Button>
        </div>
      </div>
    </div>
  )
}
