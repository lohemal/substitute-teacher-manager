import { useState } from 'react'
import { useMutation } from '@tanstack/react-query'

import { Button } from '@/components/ui'
import { adminApi, type ExportResult } from '@/ipc/admin'
import { errorMessage } from '@/ipc/invoke'
import s from './admin.module.css'

/**
 * 지금 화면에 걸린 조건 그대로 CSV로 저장한다.
 *
 * 파일은 자료 폴더 안 `exports`에 만들고, 저장한 뒤 [폴더 열기]를 함께 띄운다.
 * UTF-8 BOM을 붙이므로 Excel에서 바로 열어도 한글이 깨지지 않는다.
 */
export function ExportButton({ run }: { run: () => Promise<ExportResult> }) {
  const [done, setDone] = useState<ExportResult | null>(null)
  const save = useMutation({ mutationFn: run, onSuccess: setDone })

  return (
    <span className={s.exportWrap}>
      <Button variant="ghost" disabled={save.isPending} onClick={() => save.mutate()}>
        {save.isPending ? '만드는 중…' : 'CSV 내보내기'}
      </Button>

      {save.error && <span className={s.exportBad}>{errorMessage(save.error)}</span>}

      {done && (
        <span className={s.exportOk}>
          <b>{done.name}</b> 저장
          <button
            type="button"
            className={s.linkBtn}
            onClick={() => adminApi.openExportFolder()}
          >
            폴더 열기
          </button>
          <button type="button" className={s.linkBtn} onClick={() => setDone(null)}>
            닫기
          </button>
        </span>
      )}
    </span>
  )
}
