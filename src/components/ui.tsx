import { useState, type ButtonHTMLAttributes, type ReactNode } from 'react'
import { Icon, type IconName } from './Icon'
import { useRemembered } from '@/lib/remember'
import s from './ui.module.css'

/* ---------- Page ---------- */

export function Page({
  title,
  description,
  actions,
  children,
}: {
  title: string
  description?: string
  actions?: ReactNode
  children: ReactNode
}) {
  return (
    <div className={s.page}>
      <header className={s.pageHead}>
        <div>
          <h1 className={s.pageTitle}>{title}</h1>
          {description && <p className={s.pageDesc}>{description}</p>}
        </div>
        {actions && <div className={s.pageActions}>{actions}</div>}
      </header>
      {children}
    </div>
  )
}

/* ---------- Card ---------- */

export function Card({
  title,
  subtitle,
  footer,
  children,
  padded = true,
  collapsible = false,
  rememberKey,
  defaultOpen = false,
}: {
  title?: string
  /** 접혀 있을 때도 무엇인지 알 수 있게 한 줄 설명 */
  subtitle?: string
  footer?: ReactNode
  children: ReactNode
  padded?: boolean
  /** 제목을 눌러 접었다 펼 수 있게 한다 */
  collapsible?: boolean
  /** 접힘 상태를 기억할 이름. 주면 다음에 열 때도 그대로다 */
  rememberKey?: string
  defaultOpen?: boolean
}) {
  const [open, setOpen] = useRemembered(
    rememberKey ? `card.${rememberKey}` : 'card.__tmp',
    defaultOpen,
  )
  // 기억하지 않는 경우를 위한 자리
  const [local, setLocal] = useState(defaultOpen)
  const isOpen = !collapsible || (rememberKey ? open : local)
  const toggle = () => (rememberKey ? setOpen(!open) : setLocal(!local))

  if (!collapsible) {
    return (
      <section className={s.card}>
        {title && <h2 className={s.cardTitle}>{title}</h2>}
        <div className={padded ? s.cardBody : undefined}>{children}</div>
        {footer && <div className={s.cardFoot}>{footer}</div>}
      </section>
    )
  }

  return (
    <section className={isOpen ? `${s.card} ${s.cardOpen}` : s.card}>
      <h2 className={s.cardHead}>
        <button
          type="button"
          className={s.cardToggle}
          aria-expanded={isOpen}
          onClick={toggle}
        >
          <span className={isOpen ? `${s.cardMark} ${s.cardMarkOpen}` : s.cardMark} aria-hidden="true">
            <Icon name="chevronRight" size={16} />
          </span>
          <span className={s.cardHeadText}>
            <span className={s.cardHeadTitle}>{title}</span>
            {subtitle && <span className={s.cardHeadSub}>{subtitle}</span>}
          </span>
        </button>
      </h2>
      {isOpen && (
        <>
          <div className={padded ? s.cardBody : undefined}>{children}</div>
          {footer && <div className={s.cardFoot}>{footer}</div>}
        </>
      )}
    </section>
  )
}

/* ---------- Button ---------- */

type ButtonVariant = 'primary' | 'accent' | 'ghost' | 'danger'

export function Button({
  variant = 'ghost',
  icon,
  children,
  ...rest
}: {
  variant?: ButtonVariant
  icon?: IconName
} & ButtonHTMLAttributes<HTMLButtonElement>) {
  const cls = [s.btn, s[`btn_${variant}`], rest.className].filter(Boolean).join(' ')
  return (
    <button type="button" {...rest} className={cls}>
      {icon && <Icon name={icon} size={17} />}
      {children}
    </button>
  )
}

/* ---------- EmptyState ---------- */

export function EmptyState({
  icon = 'search',
  title,
  description,
  action,
}: {
  icon?: IconName
  title: string
  description?: string
  action?: ReactNode
}) {
  return (
    <div className={s.empty}>
      <div className={s.emptyIcon} aria-hidden="true">
        <Icon name={icon} size={26} />
      </div>
      <p className={s.emptyTitle}>{title}</p>
      {description && <p className={s.emptyDesc}>{description}</p>}
      {action && <div className={s.emptyAction}>{action}</div>}
    </div>
  )
}

/* ---------- Notice ---------- */

export function Notice({
  tone = 'info',
  children,
}: {
  tone?: 'info' | 'warn' | 'danger'
  children: ReactNode
}) {
  return <div className={`${s.notice} ${s[`notice_${tone}`]}`}>{children}</div>
}
