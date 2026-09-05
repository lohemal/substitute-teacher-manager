import { useEffect, type ReactNode } from 'react'
import s from './Modal.module.css'

interface Props {
  open: boolean
  title: string
  description?: string
  onClose: () => void
  footer?: ReactNode
  children: ReactNode
  /** 넓은 내용(표 등)에 사용 */
  wide?: boolean
}

export function Modal({ open, title, description, onClose, footer, children, wide }: Props) {
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, onClose])

  if (!open) return null

  return (
    <div className={s.backdrop} onMouseDown={onClose} role="presentation">
      <div
        className={wide ? `${s.panel} ${s.panelWide}` : s.panel}
        onMouseDown={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <header className={s.head}>
          <div>
            <h2 className={s.title}>{title}</h2>
            {description && <p className={s.desc}>{description}</p>}
          </div>
          <button type="button" className={s.close} onClick={onClose} aria-label="닫기">
            ×
          </button>
        </header>

        <div className={s.body}>{children}</div>

        {footer && <footer className={s.foot}>{footer}</footer>}
      </div>
    </div>
  )
}
