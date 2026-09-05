import { useEffect, useRef, useState } from 'react'

export type AutoSaveStatus = 'idle' | 'pending' | 'saved' | 'error'

/**
 * 값이 바뀌면 잠깐 기다렸다가 자동으로 저장한다.
 * 사용자가 [저장] 버튼을 누르지 않아도 입력 내용이 남도록 하기 위한 것.
 */
export function useAutoSave<T>(
  value: T,
  save: (value: T) => Promise<unknown>,
  options: { enabled: boolean; delay?: number },
): AutoSaveStatus {
  const { enabled, delay = 700 } = options
  const [status, setStatus] = useState<AutoSaveStatus>('idle')

  const saveRef = useRef(save)
  saveRef.current = save

  // 처음 불러온 값은 저장할 필요가 없다
  const skipFirst = useRef(true)

  useEffect(() => {
    if (!enabled) return
    if (skipFirst.current) {
      skipFirst.current = false
      return
    }

    setStatus('pending')
    let cancelled = false
    const timer = setTimeout(() => {
      saveRef
        .current(value)
        .then(() => !cancelled && setStatus('saved'))
        .catch(() => !cancelled && setStatus('error'))
    }, delay)

    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [value, enabled, delay])

  return status
}
