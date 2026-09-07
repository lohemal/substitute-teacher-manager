import { Page } from '@/components/ui'
import { PayPanel } from '@/features/pay/PayPanel'

export function PayPage() {
  return (
    <Page
      title="보결 수당"
      description="배정 기록으로 기간별·교사별 수당을 계산합니다. 금액을 따로 저장하지 않고 지금 설정으로 그때그때 계산하므로, 설정을 바꾸면 결과도 함께 바뀝니다."
    >
      <PayPanel />
    </Page>
  )
}
