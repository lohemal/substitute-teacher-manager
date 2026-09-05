import type { ReactNode } from 'react'
import { Icon } from '@/components/Icon'
import type { AutoSaveStatus } from '@/lib/useAutoSave'
import s from './StepFrame.module.css'

export function StepFrame({
  title,
  description,
  children,
  footerLeft,
  footerRight,
}: {
  title: string
  description: string
  children: ReactNode
  footerLeft?: ReactNode
  footerRight?: ReactNode
}) {
  return (
    <div className={s.frame}>
      <header className={s.head}>
        <h1 className={s.title}>{title}</h1>
        <p className={s.desc}>{description}</p>
      </header>

      <div className={s.body}>{children}</div>

      <footer className={s.foot}>
        <div className={s.footLeft}>{footerLeft}</div>
        <div className={s.footRight}>{footerRight}</div>
      </footer>
    </div>
  )
}

/** 입력 자동 저장 상태 표시 */
export function SaveHint({ status }: { status: AutoSaveStatus }) {
  if (status === 'idle') return <span className={s.hintMuted}>입력하면 자동으로 저장됩니다</span>
  if (status === 'pending') return <span className={s.hintMuted}>저장 중…</span>
  if (status === 'error')
    return <span className={s.hintError}>자동 저장에 실패했습니다. 입력을 다시 확인해 주세요.</span>
  return (
    <span className={s.hintSaved}>
      <Icon name="check" size={14} strokeWidth={3} />
      자동 저장됨
    </span>
  )
}

/** 폼 한 줄 */
export function Field({
  label,
  hint,
  children,
  wide = false,
}: {
  label: string
  hint?: string
  children: ReactNode
  wide?: boolean
}) {
  return (
    <div className={wide ? `${s.field} ${s.fieldWide}` : s.field}>
      <div className={s.fieldLabel}>
        <span>{label}</span>
        {hint && <span className={s.fieldHint}>{hint}</span>}
      </div>
      <div className={s.fieldControl}>{children}</div>
    </div>
  )
}
