import { useQuery } from '@tanstack/react-query'
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom'

import { AppShell } from '@/components/AppShell'
import { appApi } from '@/ipc/app'
import { errorDetail, errorMessage } from '@/ipc/invoke'
import { schoolApi } from '@/ipc/school'
import { setupApi } from '@/ipc/setup'
import { FindPage } from '@/pages/FindPage'
import { SettingsPage } from '@/pages/SettingsPage'
import { AssignmentsPage } from '@/pages/AssignmentsPage'
import { PayPage } from '@/pages/PayPage'
import { StatsPage } from '@/pages/StatsPage'
import { TeachersPage } from '@/pages/TeachersPage'
import { TimetablePage } from '@/pages/TimetablePage'
import { SetupPage } from '@/setup/SetupPage'
import { WelcomePage } from '@/setup/WelcomePage'
import s from './App.module.css'

export function App() {
  const info = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })
  const setup = useQuery({ queryKey: ['setup-state'], queryFn: setupApi.getState })
  const school = useQuery({ queryKey: ['school'], queryFn: schoolApi.get })

  if (info.isLoading || setup.isLoading) {
    return (
      <div className={s.splash}>
        <div className={s.splashMark} />
        <p className={s.splashText}>보결 배정 시스템을 준비하는 중…</p>
      </div>
    )
  }

  const err = info.error ?? setup.error
  if (err || !info.data || !setup.data) {
    return (
      <div className={s.splash}>
        <h1 className={s.fatalTitle}>프로그램을 시작하지 못했습니다</h1>
        <p className={s.fatalMsg}>{errorMessage(err)}</p>
        {errorDetail(err) && (
          <details className={s.fatalDetail}>
            <summary>자세히</summary>
            <pre className="selectable">{errorDetail(err)}</pre>
          </details>
        )}
      </div>
    )
  }

  const completed = setup.data.completed

  return (
    <HashRouter>
      <Routes>
        {/* 시작 화면과 설정 마법사는 사이드바 없이 단독으로 보여준다 */}
        <Route path="/welcome" element={<WelcomePage />} />
        <Route path="/setup/:stepKey" element={<SetupPage />} />

        {/*
          업무 화면은 초기 설정을 마친 뒤에만 열린다.
          아직이라면 시작 화면으로 보내 마지막 단계부터 이어갈 수 있게 한다.
        */}
        <Route
          element={
            completed ? (
              <AppShell
                appVersion={info.data.appVersion}
                schoolName={school.data?.name}
                termName={school.data?.termName}
              />
            ) : (
              <Navigate to="/welcome" replace />
            )
          }
        >
          <Route path="/find" element={<FindPage />} />
          <Route path="/assignments" element={<AssignmentsPage />} />
          <Route path="/stats" element={<StatsPage />} />
          <Route path="/pay" element={<PayPage />} />
          <Route path="/teachers" element={<TeachersPage />} />
          <Route path="/timetable" element={<TimetablePage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>

        <Route path="*" element={<Navigate to={completed ? '/find' : '/welcome'} replace />} />
      </Routes>
    </HashRouter>
  )
}
