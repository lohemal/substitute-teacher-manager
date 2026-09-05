/**
 * 시각 변환. 프로그램 안에서 시각은 항상 '자정 기준 분' 정수로 다룬다.
 * (09:00 -> 540). 문자열로 비교하면 겹침 계산에서 실수가 나기 때문이다.
 */

export function minToHm(min: number): string {
  const m = Math.max(0, Math.min(1440, Math.round(min)))
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
}

/**
 * 사람이 입력한 여러 형태를 분 단위 숫자로 바꾼다. 못 읽으면 null.
 *
 * 모두 09:40으로 인식한다:  0940 / 940 / 9:40 / 09:40 / 9.40
 */
export function parseTimeText(raw: string): number | null {
  const t = raw.trim().replace(/[.\s]/g, ':')
  if (!t) return null

  let h: number
  let m: number

  if (t.includes(':')) {
    const [hs, ms = '0'] = t.split(':')
    if (!/^\d{1,2}$/.test(hs) || !/^\d{1,2}$/.test(ms)) return null
    h = Number(hs)
    m = Number(ms)
  } else {
    if (!/^\d{1,4}$/.test(t)) return null
    if (t.length <= 2) {
      h = Number(t)
      m = 0
    } else {
      h = Number(t.slice(0, t.length - 2))
      m = Number(t.slice(-2))
    }
  }

  if (m > 59) return null
  if (h > 24 || (h === 24 && m > 0)) return null
  return h * 60 + m
}

/** `parseTimeText`의 별칭. 'HH:MM' 형태를 분으로 바꿀 때 쓴다. */
export const hmToMin = parseTimeText
