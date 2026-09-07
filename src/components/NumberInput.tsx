import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { maxDigits } from '@/lib/numberField'
import s from './NumberInput.module.css'

/**
 * 숫자 입력칸.
 *
 * 글자를 칠 때마다 곧바로 최솟값·최댓값으로 잘라 버리면
 * '50'을 넣으려고 5를 치는 순간 10으로 바뀌어 입력을 못 한다.
 * 그래서 **입력하는 동안에는 그대로 두고, 칸을 벗어날 때 한 번만** 범위를 맞춘다.
 *
 * 위·아래 화살표로 1씩(Shift와 함께 누르면 10씩) 조절할 수 있다.
 *
 * **몇 자리까지 넣을 수 있는지는 `max` 가 정한다.** 예전에는 네 자리로 못
 * 박아 두어서, 1회 보결 수당처럼 다섯 자리 이상인 값을 넣을 수 없었다.
 * 칸 너비도 자릿수에 맞춰 늘어난다.
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

  // 넣을 수 있는 가장 긴 값의 자릿수. 규칙은 lib/numberField 한 곳에 있다.
  const digits = maxDigits(min, max)

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
      // 사용자 지정 속성은 문자열로 넘긴다 — 단위가 붙지 않는 것이 확실하다
      style={{ '--digits': String(digits) } as CSSProperties}
      value={text}
      maxLength={digits}
      onFocus={(e) => {
        editing.current = true
        e.currentTarget.select()
      }}
      onChange={(e) =>
        setText(
          min < 0
            ? e.target.value.replace(/(?!^-)[^0-9]/g, '')
            : e.target.value.replace(/[^0-9]/g, ''),
        )
      }
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
