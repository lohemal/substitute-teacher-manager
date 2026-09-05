import { Page } from '@/components/ui'
import { HistoryPanel } from '@/features/assign/HistoryPanel'

export function AssignmentsPage() {
  return (
    <Page
      title="배정 내역"
      description="배정한 보결 기록을 확인하고 취소할 수 있습니다. 취소해도 기록은 지워지지 않고 '취소됨'으로 남습니다."
    >
      <HistoryPanel />
    </Page>
  )
}
