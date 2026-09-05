import { useQuery } from '@tanstack/react-query'

import { Notice, Page } from '@/components/ui'
import { TeacherManager } from '@/features/teacher/TeacherManager'
import { teacherApi } from '@/ipc/teacher'
import s from './TeachersPage.module.css'

export function TeachersPage() {
  const { data: problems } = useQuery({
    queryKey: ['teacher-readiness'],
    queryFn: teacherApi.readiness,
  })

  return (
    <Page
      title="교사 관리"
      description="선생님 명단과 담임 학급, 담당 과목, 보결 배정 대상 여부를 관리합니다. 지난 보결 기록은 여기서 무엇을 고쳐도 그대로 남습니다."
    >
      {problems && problems.length > 0 && (
        <Notice tone="warn">
          <div>
            <strong>확인이 필요합니다</strong>
            <ul className={s.list}>
              {problems.map((p) => (
                <li key={p}>{p}</li>
              ))}
            </ul>
          </div>
        </Notice>
      )}

      <TeacherManager />
    </Page>
  )
}
