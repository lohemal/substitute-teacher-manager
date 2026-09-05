import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'

import { Notice, Page } from '@/components/ui'
import { BellEditor } from '@/features/bell/BellEditor'
import { LessonEditor } from '@/features/lesson/LessonEditor'
import { bellApi } from '@/ipc/bell'
import { lessonApi } from '@/ipc/lesson'
import s from './TimetablePage.module.css'

type Tab = 'BELL' | 'LESSON'

export function TimetablePage() {
  const [tab, setTab] = useState<Tab>('BELL')

  const { data: problems } = useQuery({
    queryKey: ['bell-readiness'],
    queryFn: bellApi.readiness,
  })
  const { data: lessonWarnings } = useQuery({
    queryKey: ['lesson-warnings'],
    queryFn: lessonApi.warnings,
  })

  return (
    <Page
      title="시간표 관리"
      description="학년별 시정표와 전담교사 시간표를 관리합니다. 여기서 고친 내용은 앞으로의 보결 조회에만 적용되며, 이미 배정된 기록은 그대로 남습니다."
    >
      <div className={s.tabs}>
        <button
          type="button"
          className={tab === 'BELL' ? `${s.tab} ${s.tabOn}` : s.tab}
          onClick={() => setTab('BELL')}
        >
          시정표 · 점심시간
        </button>
        <button
          type="button"
          className={tab === 'LESSON' ? `${s.tab} ${s.tabOn}` : s.tab}
          onClick={() => setTab('LESSON')}
        >
          전담교사 시간표
        </button>
      </div>

      {tab === 'BELL' && (
        <>
          {problems && problems.length > 0 && (
            <Notice tone="warn">
              <div>
                <strong>아직 남은 설정이 있습니다</strong>
                <ul className={s.list}>
                  {problems.map((p) => (
                    <li key={p}>{p}</li>
                  ))}
                </ul>
              </div>
            </Notice>
          )}
          <BellEditor />
        </>
      )}

      {tab === 'LESSON' && (
        <>
          {lessonWarnings && lessonWarnings.length > 0 && (
            <Notice tone="warn">
              <div>
                <strong>확인해 주세요</strong>
                <ul className={s.list}>
                  {lessonWarnings.map((w) => (
                    <li key={w}>{w}</li>
                  ))}
                </ul>
              </div>
            </Notice>
          )}
          <LessonEditor />
        </>
      )}
    </Page>
  )
}
