/**
 * 다건 배정에서 각 칸의 선생님을 고르고, 겹치는 곳을 표시하는 규칙.
 * (순수 계산 — 화면을 모른다)
 *
 * ## 같은 사람이 여러 번 들어가도 된다
 *
 * 한 반의 1교시와 4교시에 같은 선생님이 들어가는 일은 현장에서 흔하다.
 * 그래서 자동으로 다음 순위로 넘기지 않는다. 대신 **같은 분이 두 곳 이상에
 * 골라졌다는 사실을 표시**해서, 담당자가 보고 그대로 둘지 바꿀지 정하게 한다.
 *
 * ## 다만 시각이 겹치면 저장할 수 없다
 *
 * 같은 시각에 두 반을 동시에 맡을 수는 없다. 이것은 표시로 끝나지 않고
 * 저장 자체가 막히므로 따로 구분한다.
 */

export interface PickSlot {
  key: string
  startMin: number
  endMin: number
  /** 추천 순서대로 정렬된 후보의 교사 id */
  candidateIds: number[]
  /** 이미 배정되어 있으면 true — 고르지 않는다 */
  taken?: boolean
}

export type Picks = Record<string, number | null>

/** NONE 문제 없음 · DUPLICATE 같은 분이 다른 시간에도 · OVERLAP 시각이 겹침 */
export type PickFlag = 'NONE' | 'DUPLICATE' | 'OVERLAP'

function overlaps(a: PickSlot, b: PickSlot): boolean {
  return a.startMin < b.endMin && b.startMin < a.endMin
}

/** 각 칸에 추천 1순위를 골라 둔다. 이미 배정된 칸은 비운다. */
export function initialPicks(slots: PickSlot[]): Picks {
  const out: Picks = {}
  for (const x of slots) {
    out[x.key] = x.taken ? null : (x.candidateIds[0] ?? null)
  }
  return out
}

/**
 * 같은 선생님이 두 곳 이상 골라진 칸을 찾는다.
 *
 * 시각이 겹치면 `OVERLAP`, 겹치지 않으면 `DUPLICATE`. 겹치는 칸은 양쪽 모두
 * `OVERLAP`으로 표시한다 — 어느 쪽을 고쳐도 되기 때문이다.
 */
export function flagPicks(slots: PickSlot[], picks: Picks): Record<string, PickFlag> {
  const flags: Record<string, PickFlag> = {}
  for (const x of slots) flags[x.key] = 'NONE'

  // 선생님별로 어느 칸에 들어갔는지 모은다
  const byTeacher = new Map<number, PickSlot[]>()
  for (const x of slots) {
    if (x.taken) continue
    const tid = picks[x.key]
    if (tid == null) continue
    const list = byTeacher.get(tid) ?? []
    list.push(x)
    byTeacher.set(tid, list)
  }

  for (const list of byTeacher.values()) {
    if (list.length < 2) continue
    for (const x of list) flags[x.key] = 'DUPLICATE'
    for (let i = 0; i < list.length; i++) {
      for (let j = i + 1; j < list.length; j++) {
        if (overlaps(list[i], list[j])) {
          flags[list[i].key] = 'OVERLAP'
          flags[list[j].key] = 'OVERLAP'
        }
      }
    }
  }
  return flags
}

/** 이 선생님이 이 배정 묶음에서 몇 번째로 들어가는지 (1부터). 표시용. */
export function repeatIndex(slots: PickSlot[], picks: Picks, key: string): number {
  const tid = picks[key]
  if (tid == null) return 0
  let n = 0
  for (const x of slots) {
    if (x.taken) continue
    if (picks[x.key] === tid) {
      n++
      if (x.key === key) return n
    }
  }
  return n
}
