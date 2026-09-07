import { useState } from 'react'
import { useMutation } from '@tanstack/react-query'

import { Button } from '@/components/ui'
import { adminApi, type ExportResult } from '@/ipc/admin'
import { errorMessage } from '@/ipc/invoke'
import s from './admin.module.css'

/**
 * 지금 화면에 걸린 조건 그대로 엑셀 파일(XLSX)로 저장한다.
 *
 * 파일은 자료 폴더 안 `exports`에 만들고, 저장한 뒤 [폴더 열기]를 함께 띄운다.
 * 표가 여러 개인 화면은 시트로 나뉘어 나온다.
 *
 * 예전에는 CSV로 만들었다. 금액과 횟수가 결국 글자가 되어 엑셀에서 바로
 * 합계를 낼 수 없었고, 한글이 깨지지 않게 파일 앞에 BOM을 붙여야 했다.
 * XLSX는 그것들이 형식 안에서 해결된다.
 */
export function ExportButton({ run }: { run: () => Promise<ExportResult> }) {
  const [done, setDone] = useState<ExportResult | null>(null)
  const save = useMutation({ mutationFn: run, onSuccess: setDone })

  return (
    <span className={s.exportWrap}>
      <Button variant="ghost" disabled={save.isPending} onClick={() => save.mutate()}>
        {save.isPending ? '만드는 중…' : '엑셀 파일로 저장'}
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
