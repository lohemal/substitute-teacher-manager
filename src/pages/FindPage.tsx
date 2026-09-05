import { Page } from '@/components/ui'
import { FindPanel } from '@/features/find/FindPanel'

export function FindPage() {
  return (
    <Page
      title="보결 조회"
      description="날짜와 시간을 고르면 그 시간에 실제로 수업이나 다른 일정이 없는 선생님을 찾아 드립니다."
    >
      <FindPanel />
    </Page>
  )
}
