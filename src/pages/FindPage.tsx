import { useQuery } from '@tanstack/react-query'

import { Notice, Page } from '@/components/ui'
import { FindPanel } from '@/features/find/FindPanel'
import { isCurrentSchoolType, schoolApi, SCHOOL_TYPE_LABEL } from '@/ipc/school'

/**
 * 보결 조회.
 *
 * 보결을 만들어 내는 길(단일 배정·다건 배정·결근 등록)은 모두 `FindPanel`
 * 안에 있다. 그래서 이 프로그램이 판정할 수 없는 자료라면 여기 한 곳만
 * 막으면 된다.
 *
 * 초등학교가 아닌 자료는 **값을 바꾸지 않고** 조회만 막는다. 중·고등학교는
 * 교과·개인 시간표 중심이라 초등학교 기준으로 계산하면 그럴듯하지만 틀린
 * 답이 나오는데, 그것이 자료를 그대로 두는 것보다 나쁘다. 기록 열람과
 * 백업은 그대로 쓸 수 있다.
 */
export function FindPage() {
  const { data: school } = useQuery({ queryKey: ['school'], queryFn: schoolApi.get })
  const other = school && !isCurrentSchoolType(school.schoolType) ? school.schoolType : null

  return (
    <Page
      title="보결 조회"
      description="날짜와 시간을 고르면 그 시간에 실제로 수업이나 다른 일정이 없는 선생님을 찾아 드립니다."
    >
      {other ? (
        <Notice tone="warn">
          <div>
            <strong>{SCHOOL_TYPE_LABEL[other]} 자료라서 보결을 찾지 않습니다.</strong>
            <p>
              이 프로그램은 초등학교 보결만 판정합니다. 저장된 자료는 그대로 있으며, 배정 내역과
              현황·수당 조회, 백업은 계속 쓰실 수 있습니다.
            </p>
          </div>
        </Notice>
      ) : (
        <FindPanel />
      )}
    </Page>
  )
}
