import { useCallback, useState } from 'react'

/**
 * 마지막에 고른 값을 기억한다.
 *
 * 자주 쓰는 화면에서 같은 값을 매번 다시 고르는 것은 성가시다. 다만
 * **기억해서 위험한 값은 넣지 않는다** — 날짜는 늘 오늘로 시작해야 하고,
 * 결근·배정 같은 기록은 기억할 것이 아니다. 화면을 보기 편하게 하는
 * 값(학년·반·기간 등)만 담는다.
 *
 * 값은 이 컴퓨터의 브라우저 저장소에만 남으며 자료 파일과는 무관하다.
 */
export function useRemembered<T>(key: string, fallback: T): [T, (v: T) => void] {
  const full = `bogyeol:${key}`

  const [value, setValue] = useState<T>(() => {
    try {
      const raw = localStorage.getItem(full)
      return raw === null ? fallback : (JSON.parse(raw) as T)
    } catch {
      return fallback
    }
  })

  const put = useCallback(
    (v: T) => {
      setValue(v)
      try {
        localStorage.setItem(full, JSON.stringify(v))
      } catch {
        // 저장에 실패해도 화면은 그대로 쓸 수 있어야 한다
      }
    },
    [full],
  )

  return [value, put]
}
