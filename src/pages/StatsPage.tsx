import { Page } from '@/components/ui'
import { StatsPanel } from '@/features/stats/StatsPanel'

export function StatsPage() {
  return (
    <Page
      title="보결 현황"
      description="결근과 보결 기록을 그때그때 다시 세어 보여 줍니다. 취소한 배정은 횟수에서 빠집니다."
    >
      <StatsPanel />
    </Page>
  )
}
