import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'

import { Button, Card, Notice, Page } from '@/components/ui'
import { BackupCard } from '@/features/admin/BackupCard'
import { DangerCard, LocationCard } from '@/features/admin/DangerCard'
import { OptionsCard } from '@/features/admin/OptionsCard'
import { TermCard } from '@/features/admin/TermCard'
import { UpdateCard } from '@/features/admin/UpdateCard'
import { PriorityEditor } from '@/features/priority/PriorityEditor'
import { appApi } from '@/ipc/app'
import { errorMessage } from '@/ipc/invoke'
import s from './SettingsPage.module.css'

export function SettingsPage() {
  const navigate = useNavigate()
  const { data, isLoading, error } = useQuery({
    queryKey: ['app-info'],
    queryFn: appApi.info,
  })

  return (
    <Page
      title="설정"
      description="프로그램 동작 방식과 자료를 관리합니다."
      actions={
        <Button variant="ghost" onClick={() => navigate('/setup/SCHOOL')}>
          초기 설정 다시 열기
        </Button>
      }
    >
      <Card
        title="보결 배정 우선순위"
        subtitle="추천 순서를 정하는 기준"
        collapsible
        rememberKey="priority"
      >
        <PriorityEditor />
      </Card>

      <OptionsCard />
      <TermCard />
      <BackupCard />
      <LocationCard />

      <UpdateCard />

      <Card
        title="프로그램 정보"
        subtitle="버전과 자료 구조"
        collapsible
        rememberKey="about"
      >
        {isLoading && <p className={s.muted}>불러오는 중…</p>}
        {error && <Notice tone="danger">{errorMessage(error)}</Notice>}
        {data && (
          <dl className={s.dl}>
            <dt>프로그램 버전</dt>
            <dd className="num">v{data.appVersion}</dd>

            <dt>자료 구조 버전</dt>
            <dd className="num">
              v{data.schemaVersion}
              {data.schemaVersion !== data.latestSchemaVersion && (
                <span className={s.warn}> (최신 v{data.latestSchemaVersion})</span>
              )}
            </dd>

            <dt>초기 설정</dt>
            <dd>{data.setupCompleted ? '완료' : '진행 전'}</dd>
          </dl>
        )}
      </Card>

      <DangerCard />
    </Page>
  )
}
