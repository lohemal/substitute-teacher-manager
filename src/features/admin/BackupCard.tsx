import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, Card, Notice } from '@/components/ui'
import {
  adminApi,
  BACKUP_MODE_LABEL,
  type BackupFile,
  type BackupMode,
} from '@/ipc/admin'
import { errorDetail, errorMessage } from '@/ipc/invoke'
import s from './admin.module.css'

/**
 * 자료 백업과 복원.
 *
 * 복원은 지금 자료를 덮어쓰는 일이므로 **되돌릴 길을 먼저 만들어 둔다.**
 * 프로그램이 복원 직전에 지금 자료를 자동으로 백업하고, 그 파일 이름을
 * 화면에 알려 준다. 잘못 복원했으면 그 파일로 다시 되돌리면 된다.
 */
export function BackupCard() {
  const qc = useQueryClient()
  const { data: files, isLoading } = useQuery({
    queryKey: ['backups'],
    queryFn: adminApi.backupList,
  })

  const [made, setMade] = useState<string | null>(null)
  const [restoring, setRestoring] = useState<BackupFile | null>(null)
  const [restored, setRestored] = useState<string | null>(null)
  const [removing, setRemoving] = useState<BackupFile | null>(null)
  const [otherPath, setOtherPath] = useState('')

  const refresh = () => qc.invalidateQueries({ queryKey: ['backups'] })

  const create = useMutation({
    mutationFn: adminApi.backupCreate,
    onSuccess: async (r) => {
      setMade(r.name)
      await refresh()
    },
  })

  /** 다른 곳의 파일이 우리 자료인지 미리 확인한다 */
  const check = useMutation({ mutationFn: (path: string) => adminApi.backupInspect(path) })

  const remove = useMutation({
    mutationFn: (name: string) => adminApi.backupDelete(name),
    onSuccess: async () => {
      setRemoving(null)
      await refresh()
    },
  })

  return (
    <Card
      title="자료 백업 · 복원"
      subtitle="자료를 파일로 저장해 두고, 필요하면 그 시점으로 되돌립니다"
      collapsible
      rememberKey="backup"
      footer={
        <div className={s.footRow}>
          <span className={s.footNote}>
            백업 파일은 프로그램 자료 폴더 안 <code>backups</code> 에 저장됩니다.
          </span>
          <div className={s.footBtns}>
            <Button variant="ghost" onClick={() => adminApi.openBackupFolder()}>
              폴더 열기
            </Button>
            <Button
              variant="primary"
              icon="plus"
              disabled={create.isPending}
              onClick={() => create.mutate()}
            >
              {create.isPending ? '만드는 중…' : '백업 만들기'}
            </Button>
          </div>
        </div>
      }
    >
      {create.error && <Notice tone="danger">{errorMessage(create.error)}</Notice>}
      {made && (
        <Notice tone="info">
          <div>
            <strong>{made}</strong> 로 저장했습니다.
            <button type="button" className={s.linkBtn} onClick={() => setMade(null)}>
              닫기
            </button>
          </div>
        </Notice>
      )}
      {restored && (
        <Notice tone="info">
          <div>
            <strong>복원했습니다.</strong> 되돌리기 직전 자료는{' '}
            <code>{restored}</code> 로 남겨 두었습니다. 잘못 복원했다면 그 파일로 다시
            되돌릴 수 있습니다.
            <button type="button" className={s.linkBtn} onClick={() => setRestored(null)}>
              닫기
            </button>
          </div>
        </Notice>
      )}

      {isLoading && <p className={s.muted}>불러오는 중…</p>}

      {files && files.length === 0 && (
        <p className={s.muted}>아직 백업이 없습니다. [백업 만들기]를 눌러 하나 만들어 두세요.</p>
      )}

      {files && files.length > 0 && (
        <div className={s.tableWrap}>
          <table className={s.table}>
            <thead>
              <tr>
                <th>만든 시각</th>
                <th>파일</th>
                <th className={s.kindCol}>종류</th>
                <th className={s.numCol}>크기</th>
                <th className={s.actCol} />
              </tr>
            </thead>
            <tbody>
              {files.map((f) => (
                <tr key={f.name} className={f.restorable ? undefined : s.rowBad}>
                  <td className="num">{f.madeAt}</td>
                  <td className={s.fileName}>
                    {f.name}
                    {f.problem && <span className={s.problem}>{f.problem}</span>}
                  </td>
                  <td className={s.kindCol}>
                    <span className={`${s.kindTag} ${s[`kind_${f.kind}`]}`}>{f.kindLabel}</span>
                  </td>
                  <td className={`${s.numCol} num`}>{f.sizeKb.toLocaleString()} KB</td>
                  <td className={s.actCol}>
                    <button
                      type="button"
                      className={s.smallBtn}
                      disabled={!f.restorable}
                      onClick={() => setRestoring(f)}
                    >
                      복원
                    </button>
                    <button
                      type="button"
                      className={`${s.smallBtn} ${s.smallDanger}`}
                      onClick={() => setRemoving(f)}
                    >
                      삭제
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* ---------- 자동 백업 ---------- */}
      <AutoBackup />

      {/* ---------- 다른 곳의 파일 ---------- */}
      <div className={s.block}>
        <h3 className={s.blockTitle}>다른 곳에 있는 백업으로 복원</h3>
        <p className={s.hint}>
          USB나 다른 컴퓨터에서 가져온 백업 파일의 <b>전체 경로</b>를 붙여 넣으세요.
          (탐색기에서 파일을 <kbd>Shift</kbd>+오른쪽 클릭 → '경로로 복사')
        </p>
        <div className={s.pathRow}>
          <input
            type="text"
            className={s.pathInput}
            value={otherPath}
            placeholder="D:\\보결자료-20260905-090000.db"
            onChange={(e) => setOtherPath(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && otherPath.trim()) check.mutate(otherPath.trim())
            }}
          />
          <Button
            variant="ghost"
            disabled={!otherPath.trim() || check.isPending}
            onClick={() => check.mutate(otherPath.trim())}
          >
            확인
          </Button>
        </div>
        {check.error && <Notice tone="danger">{errorMessage(check.error)}</Notice>}
        {check.data && !check.data.ok && <Notice tone="danger">{check.data.problem}</Notice>}
        {check.data?.ok && (
          <Notice tone="info">
            <div>
              복원할 수 있는 자료 파일입니다 (자료 구조 v{check.data.schemaVersion}).
              <Button
                variant="danger"
                className={s.inlineBtn}
                onClick={() =>
                  setRestoring({
                    name: otherPath.trim(),
                    path: otherPath.trim(),
                    kind: 'MANUAL',
                    kindLabel: '다른 곳의 파일',
                    madeAt: '',
                    sizeKb: 0,
                    schemaVersion: check.data!.schemaVersion,
                    restorable: true,
                    problem: null,
                  })
                }
              >
                이 파일로 복원
              </Button>
            </div>
          </Notice>
        )}
      </div>

      {restoring && (
        <RestoreModal
          file={restoring}
          external={restoring.madeAt === ''}
          onClose={() => setRestoring(null)}
          onDone={async (safety) => {
            setRestoring(null)
            setRestored(safety)
            setOtherPath('')
            check.reset()
            // 자료가 통째로 바뀌었다 — 화면 전체를 새로 읽는다
            await qc.invalidateQueries()
          }}
        />
      )}

      {removing && (
        <Modal
          open
          title="백업 삭제"
          onClose={() => setRemoving(null)}
          footer={
            <>
              <Button variant="ghost" onClick={() => setRemoving(null)}>
                닫기
              </Button>
              <Button
                variant="danger"
                disabled={remove.isPending}
                onClick={() => remove.mutate(removing.name)}
              >
                삭제
              </Button>
            </>
          }
        >
          {remove.error && <Notice tone="danger">{errorMessage(remove.error)}</Notice>}
          <p className={s.ask}>
            <strong>{removing.name}</strong> 을 지우시겠습니까?
          </p>
          <p className={s.hint}>백업 파일만 지웁니다. 지금 쓰고 있는 자료는 그대로입니다.</p>
        </Modal>
      )}
    </Card>
  )
}

/**
 * 자동 백업 설정.
 *
 * 백업과 한자리에 두어야 '언제 자동으로 남는지'와 '지금 무엇이 쌓여 있는지'를
 * 함께 볼 수 있다. 누르면 바로 저장된다 — 따로 저장 단추를 둘 만큼 무거운
 * 설정이 아니다.
 */
function AutoBackup() {
  const qc = useQueryClient()
  const { data } = useQuery({ queryKey: ['settings'], queryFn: adminApi.settings })

  const save = useMutation({
    mutationFn: (v: { mode?: BackupMode; keep?: number }) =>
      adminApi.saveSettings({ autoBackupMode: v.mode ?? null, autoBackupKeep: v.keep ?? null }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['settings'] }),
  })

  const runNow = useMutation({
    mutationFn: adminApi.backupAutoNow,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['backups'] }),
  })

  if (!data) return null
  const off = data.autoBackupMode === 'OFF'

  return (
    <div className={s.block}>
      <h3 className={s.blockTitle}>자동 백업</h3>

      {save.error && <Notice tone="danger">{errorMessage(save.error)}</Notice>}

      <div className={s.row}>
        <span className={s.rowLabel}>방식</span>
        <div className={s.segment}>
          {(['OFF', 'DAILY', 'ON_EXIT'] as BackupMode[]).map((m) => (
            <button
              key={m}
              type="button"
              className={data.autoBackupMode === m ? `${s.seg} ${s.segOn}` : s.seg}
              disabled={save.isPending}
              onClick={() => save.mutate({ mode: m })}
            >
              {BACKUP_MODE_LABEL[m]}
            </button>
          ))}
        </div>
      </div>

      <div className={s.row}>
        <span className={s.rowLabel}>보관 개수</span>
        <div className={s.segment}>
          {[5, 10, 20, 30].map((k) => (
            <button
              key={k}
              type="button"
              className={data.autoBackupKeep === k ? `${s.seg} ${s.segOn}` : s.seg}
              disabled={off || save.isPending}
              onClick={() => save.mutate({ keep: k })}
            >
              {k}개
            </button>
          ))}
        </div>
        {!off && (
          <button
            type="button"
            className={s.linkBtn}
            disabled={runNow.isPending}
            onClick={() => runNow.mutate()}
          >
            지금 한 번 남기기
          </button>
        )}
      </div>

      <p className={s.hint}>
        자동 백업만 개수를 지켜 오래된 것부터 지웁니다. <b>직접 만든 백업과 복원 전 안전
        백업은 지우지 않습니다.</b> '하루 1회'는 그날 처음 프로그램을 켤 때 한 번만 남깁니다.
      </p>
    </div>
  )
}

/** 복원 확인 */
function RestoreModal({
  file,
  external,
  onClose,
  onDone,
}: {
  file: BackupFile
  external: boolean
  onClose: () => void
  onDone: (safetyBackup: string) => void
}) {
  const run = useMutation({
    mutationFn: () =>
      adminApi.backupRestore(external ? { path: file.path } : { name: file.name }),
    onSuccess: (r) => onDone(r.safetyBackup),
  })

  return (
    <Modal
      open
      title="백업으로 되돌리기"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            닫기
          </Button>
          <Button variant="danger" disabled={run.isPending} onClick={() => run.mutate()}>
            {run.isPending ? '되돌리는 중…' : '복원'}
          </Button>
        </>
      }
    >
      {run.error && (
        <Notice tone="danger">
          <div>
            <p>{errorMessage(run.error)}</p>
            {errorDetail(run.error) && (
              <details className={s.detail}>
                <summary>자세히</summary>
                <pre className="selectable">{errorDetail(run.error)}</pre>
              </details>
            )}
          </div>
        </Notice>
      )}

      <p className={s.ask}>
        <strong>{file.name}</strong>
        {file.madeAt && <span className={s.sub}>({file.madeAt}에 만든 백업)</span>}
        <br />이 백업으로 되돌리시겠습니까?
      </p>

      <Notice tone="warn">
        <div>
          지금 자료가 이 백업 시점으로 <strong>모두 바뀝니다.</strong> 그 뒤에 등록한 결근·보결
          기록은 사라집니다.
          <p className={s.warnBody}>
            되돌리기 직전에 지금 자료를 자동으로 백업해 둡니다. 잘못 복원했다면 그 파일로 다시
            되돌릴 수 있습니다.
          </p>
        </div>
      </Notice>
    </Modal>
  )
}
