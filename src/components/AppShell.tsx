import { NavLink, Outlet } from 'react-router-dom'
import { Icon, type IconName } from './Icon'
import s from './AppShell.module.css'

interface MenuItem {
  to: string
  label: string
  icon: IconName
}

const MENU: MenuItem[] = [
  { to: '/find', label: '보결 조회', icon: 'search' },
  { to: '/assignments', label: '배정 내역', icon: 'list' },
  { to: '/stats', label: '보결 현황', icon: 'chart' },
  { to: '/pay', label: '보결 수당', icon: 'won' },
  { to: '/teachers', label: '교사 관리', icon: 'people' },
  { to: '/timetable', label: '시간표 관리', icon: 'calendar' },
  { to: '/settings', label: '설정', icon: 'settings' },
]

interface Props {
  schoolName?: string
  termName?: string
  appVersion?: string
}

export function AppShell({ schoolName, termName, appVersion }: Props) {
  return (
    <div className={s.shell}>
      <aside className={s.sidebar}>
        <div className={s.brand}>
          <div className={s.brandMark} aria-hidden="true">
            <Icon name="check" size={18} strokeWidth={2.6} />
          </div>
          <div className={s.brandText}>
            <span className={s.brandTitle}>보결 배정</span>
            <span className={s.brandSub}>{schoolName ?? '학교 미설정'}</span>
          </div>
        </div>

        <nav className={s.nav}>
          {MENU.map((m) => (
            <NavLink
              key={m.to}
              to={m.to}
              className={({ isActive }) => (isActive ? `${s.navItem} ${s.navItemActive}` : s.navItem)}
            >
              <Icon name={m.icon} size={19} />
              <span>{m.label}</span>
            </NavLink>
          ))}
        </nav>

        <div className={s.sidebarFoot}>
          {termName && <span className={s.termChip}>{termName}</span>}
          <span className={s.version}>v{appVersion ?? '0.0.0'}</span>
        </div>
      </aside>

      <main className={s.main}>
        <Outlet />
      </main>
    </div>
  )
}
