import { useEffect, useRef, useState } from 'react'
import { minToHm, parseTimeText } from '@/lib/time'
import s from './TimeInput.module.css'

/**
 * 24시간 형식 시각 입력칸.
 *
 * 브라우저 기본 `<input type="time">`은 한국어 환경에서 '오전 09:40'처럼
 * 표시돼 칸이 좁으면 분이 잘린다. 그래서 직접 만든다.
 *
 * 입력 요령 (모두 09:40으로 인식)
 *   0940 / 940 / 9:40 / 09:40
 * 위·아래 화살표로 5분씩(Shift와 함께 누르면 30분씩) 조절할 수 있다.
 */

interface Props {
  value: number
  onChange: (minutes: number) => void
  'aria-label'?: string
  className?: string
  disabled?: boolean
}

export function TimeInput({ value, onChange, className, disabled, ...rest }: Props) {
  const [text, setText] = useState(() => minToHm(value))
  const editing = useRef(false)

  useEffect(() => {
    if (!editing.current) setText(minToHm(value))
  }, [value])

  const commit = (raw: string) => {
    const min = parseTimeText(raw)
    if (min === null) {
      setText(minToHm(value)) // 못 읽으면 원래 값으로 되돌린다
      return
    }
    setText(minToHm(min))
    if (min !== value) onChange(min)
  }

  const step = (delta: number) => {
    const next = Math.max(0, Math.min(1440, value + delta))
    setText(minToHm(next))
    onChange(next)
  }

  return (
    <input
      {...rest}
      type="text"
      inputMode="numeric"
      autoComplete="off"
      disabled={disabled}
      className={[s.input, className].filter(Boolean).join(' ')}
      value={text}
      placeholder="09:40"
      maxLength={5}
      onFocus={(e) => {
        editing.current = true
        e.currentTarget.select()
      }}
      onChange={(e) => setText(e.target.value)}
      onBlur={(e) => {
        editing.current = false
        commit(e.target.value)
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          commit(e.currentTarget.value)
          e.currentTarget.select()
        } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
          e.preventDefault()
          const unit = e.shiftKey ? 30 : 5
          step(e.key === 'ArrowUp' ? unit : -unit)
        }
      }}
    />
  )
}
