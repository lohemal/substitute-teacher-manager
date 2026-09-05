import { useEffect, useRef, useState } from 'react'
import s from './NumberInput.module.css'

/**
 * 숫자 입력칸.
 *
 * 글자를 칠 때마다 곧바로 최솟값·최댓값으로 잘라 버리면
 * '50'을 넣으려고 5를 치는 순간 10으로 바뀌어 입력을 못 한다.
 * 그래서 **입력하는 동안에는 그대로 두고, 칸을 벗어날 때 한 번만** 범위를 맞춘다.
 *
 * 위·아래 화살표로 1씩(Shift와 함께 누르면 10씩) 조절할 수 있다.
 */

interface Props {
  value: number
  onChange: (value: number) => void
  min: number
  max: number
  /** 화살표 한 번에 움직이는 크기 */
  step?: number
  'aria-label'?: string
  className?: string
  disabled?: boolean
}

export function NumberInput({
  value,
  onChange,
  min,
  max,
  step = 1,
  className,
  disabled,
  ...rest
}: Props) {
  const [text, setText] = useState(() => String(value))
  const editing = useRef(false)

  useEffect(() => {
    if (!editing.current) setText(String(value))
  }, [value])

  const commit = (raw: string) => {
    const trimmed = raw.trim()
    if (trimmed === '') {
      setText(String(value))
      return
    }
    const n = Number(trimmed)
    if (Number.isNaN(n)) {
      setText(String(value))
      return
    }
    const clamped = Math.max(min, Math.min(max, Math.round(n)))
    setText(String(clamped))
    if (clamped !== value) onChange(clamped)
  }

  const bump = (delta: number) => {
    const next = Math.max(min, Math.min(max, value + delta))
    setText(String(next))
    if (next !== value) onChange(next)
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
      maxLength={4}
      onFocus={(e) => {
        editing.current = true
        e.currentTarget.select()
      }}
      onChange={(e) => setText(e.target.value.replace(/[^0-9]/g, ''))}
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
          const unit = (e.shiftKey ? 10 : 1) * step
          bump(e.key === 'ArrowUp' ? unit : -unit)
        }
      }}
    />
  )
}
