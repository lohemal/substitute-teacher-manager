import { useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

import { Button, Card, Notice } from '@/components/ui'
import { appApi } from '@/ipc/app'
import s from './admin.module.css'

type Phase =
  | { kind: 'IDLE' }
  | { kind: 'CHECKING' }
  | { kind: 'LATEST' }
  | { kind: 'FOUND'; update: Update }
  | { kind: 'DOWNLOADING'; got: number; total: number | null }
  | { kind: 'READY' }
  | { kind: 'FAILED'; message: string }

/** 사용자가 읽을 수 있는 말로 바꾼다. 원문은 접기 안에 둔다. */
function explain(e: unknown): string {
  const raw = e instanceof Error ? e.message : String(e)
  const low = raw.toLowerCase()

  if (low.includes('network') || low.includes('dns') || low.includes('connect')) {
    return '인터넷에 연결하지 못했습니다. 연결 상태를 확인한 뒤 다시 시도해 주세요.'
  }
  if (low.includes('signature') || low.includes('verify')) {
    return '내려받은 파일의 서명이 맞지 않아 설치하지 않았습니다. 잠시 뒤 다시 시도해 주세요.'
  }
  if (low.includes('404') || low.includes('not found')) {
    return '업데이트 정보를 찾지 못했습니다. 아직 배포된 새 버전이 없을 수 있습니다.'
  }
  return '업데이트를 확인하지 못했습니다. 잠시 뒤 다시 시도해 주세요.'
}

/**
 * 프로그램 업데이트.
 *
 * ## 서명을 확인한 것만 설치한다
 *
 * 내려받은 파일은 우리 개인 키로 서명된 것인지 확인한 뒤에만 설치된다.
 * 확인은 Tauri Updater가 하고, 공개 키는 프로그램 안에 들어 있다. 그래서
 * 중간에 파일이 바뀌었거나 다른 곳에서 받은 파일이면 설치되지 않는다.
 *
 * ## 사용자가 눌러야 설치된다
 *
 * 몰래 받아 두었다가 다음 실행에 바뀌어 있으면 곤란하다. 확인 → 내려받기 →
 * 다시 시작을 모두 사람이 누르게 한다. 학교 업무 중에 프로그램이 갑자기
 * 다시 켜지는 일은 없어야 한다.
 */
export function UpdateCard() {
  const { data: info } = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })
  const [phase, setPhase] = useState<Phase>({ kind: 'IDLE' })
  const [detail, setDetail] = useState<string | null>(null)

  const look = useMutation({
    mutationFn: async () => {
      setDetail(null)
      setPhase({ kind: 'CHECKING' })
      return await check()
    },
    onSuccess: (update) => {
      setPhase(update ? { kind: 'FOUND', update } : { kind: 'LATEST' })
    },
    onError: (e) => {
      setDetail(e instanceof Error ? e.message : String(e))
      setPhase({ kind: 'FAILED', message: explain(e) })
    },
  })

  const install = useMutation({
    mutationFn: async (update: Update) => {
      let got = 0
      let total: number | null = null
      setPhase({ kind: 'DOWNLOADING', got: 0, total: null })

      await update.downloadAndInstall((ev) => {
        if (ev.event === 'Started') {
          total = ev.data.contentLength ?? null
          setPhase({ kind: 'DOWNLOADING', got: 0, total })
        } else if (ev.event === 'Progress') {
          got += ev.data.chunkLength
          setPhase({ kind: 'DOWNLOADING', got, total })
        } else if (ev.event === 'Finished') {
          setPhase({ kind: 'READY' })
        }
      })
      setPhase({ kind: 'READY' })
    },
    onError: (e) => {
      setDetail(e instanceof Error ? e.message : String(e))
      setPhase({ kind: 'FAILED', message: explain(e) })
    },
  })

  const busy = phase.kind === 'CHECKING' || phase.kind === 'DOWNLOADING'

  return (
    <Card
      title="프로그램 업데이트"
      subtitle={info ? `지금 v${info.appVersion}` : '새 버전이 있는지 확인합니다'}
      collapsible
      rememberKey="update"
      footer={
        <div className={s.footRow}>
          <span className={s.footNote}>
            새 버전은 만든 사람의 서명을 확인한 뒤에만 설치됩니다.
          </span>
          <div className={s.footBtns}>
            <Button
              variant="primary"
              icon="check"
              disabled={busy}
              onClick={() => look.mutate()}
            >
              {phase.kind === 'CHECKING' ? '확인하는 중…' : '업데이트 확인'}
            </Button>
          </div>
        </div>
      }
    >
      {phase.kind === 'IDLE' && (
        <p className={s.hint}>
          [업데이트 확인]을 누르면 새 버전이 있는지 알아봅니다. 확인만 하고 설치하지는
          않으므로, 새 버전이 있어도 직접 누르기 전에는 바뀌지 않습니다.
        </p>
      )}

      {phase.kind === 'LATEST' && (
        <Notice tone="info">
          최신 버전을 쓰고 계십니다{info && ` (v${info.appVersion})`}.
        </Notice>
      )}

      {phase.kind === 'FOUND' && (
        <div>
          <Notice tone="info">
            <div>
              <strong>새 버전 v{phase.update.version}</strong> 이 나왔습니다.
              {phase.update.date && <span className={s.sub}>{phase.update.date}</span>}
            </div>
          </Notice>
          {phase.update.body && <pre className={s.notes}>{phase.update.body}</pre>}
          <p className={s.hint}>
            내려받아 설치하면 프로그램을 다시 시작합니다. <b>자료는 그대로 남습니다.</b>
            하던 일이 있으면 마친 뒤에 눌러 주세요.
          </p>
          <div className={s.dangerRow}>
            <Button
              variant="primary"
              onClick={() => install.mutate(phase.update)}
              disabled={install.isPending}
            >
              내려받아 설치
            </Button>
          </div>
        </div>
      )}

      {phase.kind === 'DOWNLOADING' && (
        <div>
          <p className={s.hint}>새 버전을 내려받는 중입니다. 창을 닫지 말아 주세요.</p>
          <div className={s.bar}>
            <div
              className={s.barFill}
              style={{
                width: phase.total
                  ? `${Math.min(100, Math.round((phase.got / phase.total) * 100))}%`
                  : '30%',
              }}
            />
          </div>
          <p className={s.hint}>
            {phase.total
              ? `${Math.round(phase.got / 1024)} / ${Math.round(phase.total / 1024)} KB`
              : `${Math.round(phase.got / 1024)} KB`}
          </p>
        </div>
      )}

      {phase.kind === 'READY' && (
        <div>
          <Notice tone="info">
            <div>
              <strong>설치 준비가 끝났습니다.</strong>
              <p className={s.warnBody}>
                다시 시작하면 새 버전으로 열립니다. 저장하지 않은 것이 없는지 확인해 주세요.
              </p>
            </div>
          </Notice>
          <div className={s.dangerRow}>
            <Button variant="primary" onClick={() => relaunch()}>
              지금 다시 시작
            </Button>
          </div>
        </div>
      )}

      {phase.kind === 'FAILED' && (
        <Notice tone="danger">
          <div>
            <p>{phase.message}</p>
            {detail && (
              <details className={s.detail}>
                <summary>자세히</summary>
                <pre className="selectable">{detail}</pre>
              </details>
            )}
          </div>
        </Notice>
      )}
    </Card>
  )
}
