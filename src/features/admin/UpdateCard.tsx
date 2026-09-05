import { useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { check, type Update } from '@tauri-apps/plugin-updater'

import { Button, Card, Notice } from '@/components/ui'
import { appApi } from '@/ipc/app'
import s from './admin.module.css'

type Phase =
  | { kind: 'IDLE' }
  | { kind: 'CHECKING' }
  | { kind: 'LATEST' }
  | { kind: 'FOUND'; update: Update }
  | { kind: 'DOWNLOADING'; update: Update; got: number; total: number | null }
  | { kind: 'INSTALLING' }
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
  return '업데이트를 하지 못했습니다. 잠시 뒤 다시 시도해 주세요.'
}

function kb(n: number): string {
  return Math.round(n / 1024).toLocaleString()
}

/**
 * 프로그램 업데이트.
 *
 * ## 한 번에 끝난다
 *
 * `check()` 로 찾은 `Update` 객체 하나로 `download()` → `install()` 까지 이어서
 * 한다. 확인을 두 번 하거나 앱을 다시 켜서 처음부터 하는 일은 없다.
 *
 * ## `relaunch()` 를 부르지 않는다
 *
 * Windows에서 `install()` 은 **돌아오지 않는다.** 설치 프로그램을 띄우고
 * `std::process::exit(0)` 로 앱을 스스로 끝내며, 설치가 끝나면 NSIS가 앱을
 * 다시 켠다(`restart_after_install` 기본값 `true`). 여기서 `relaunch()` 를
 * 부르면 방금 뜬 설치 프로그램과 경쟁해서, 앱이 먼저 다시 켜지는 바람에
 * 파일을 바꾸지 못하고 **구 버전 그대로 남는다.** 그래서 부르지 않는다.
 *
 * ## 사람이 눌러야 시작한다
 *
 * 확인만으로 설치하지 않는다. 수업 중에 프로그램이 갑자기 닫히면 안 된다.
 */
export function UpdateCard() {
  const { data: info } = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })
  const [phase, setPhase] = useState<Phase>({ kind: 'IDLE' })
  const [detail, setDetail] = useState<string | null>(null)

  const fail = (e: unknown) => {
    setDetail(e instanceof Error ? e.message : String(e))
    setPhase({ kind: 'FAILED', message: explain(e) })
  }

  const look = useMutation({
    mutationFn: async () => {
      setDetail(null)
      setPhase({ kind: 'CHECKING' })
      return await check()
    },
    onSuccess: (update) => setPhase(update ? { kind: 'FOUND', update } : { kind: 'LATEST' }),
    onError: fail,
  })

  const run = useMutation({
    mutationFn: async (update: Update) => {
      // 1) 내려받기 — 진행률을 보여 준다
      let got = 0
      let total: number | null = null
      setPhase({ kind: 'DOWNLOADING', update, got: 0, total: null })

      await update.download((ev) => {
        if (ev.event === 'Started') {
          total = ev.data.contentLength ?? null
          setPhase({ kind: 'DOWNLOADING', update, got: 0, total })
        } else if (ev.event === 'Progress') {
          got += ev.data.chunkLength
          setPhase({ kind: 'DOWNLOADING', update, got, total })
        }
      })

      // 2) 설치 — 여기서 앱이 스스로 닫히고, 설치가 끝나면 다시 켜진다.
      //    아래 줄 다음은 실행되지 않는다 (Windows).
      setPhase({ kind: 'INSTALLING' })
      await update.install()
    },
    onError: fail,
  })

  const busy =
    phase.kind === 'CHECKING' || phase.kind === 'DOWNLOADING' || phase.kind === 'INSTALLING'

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
            <Button variant="primary" icon="check" disabled={busy} onClick={() => look.mutate()}>
              {phase.kind === 'CHECKING' ? '확인하는 중…' : '업데이트 확인'}
            </Button>
          </div>
        </div>
      }
    >
      {phase.kind === 'IDLE' && (
        <p className={s.hint}>
          [업데이트 확인]을 누르면 새 버전이 있는지 알아봅니다. 확인만 하고 설치하지는
          않으므로, 새 버전이 있어도 직접 [업데이트 시작]을 누르기 전에는 바뀌지 않습니다.
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
              {info && <span className={s.sub}>지금 v{info.appVersion}</span>}
            </div>
          </Notice>
          {phase.update.body && <pre className={s.notes}>{phase.update.body}</pre>}

          <p className={s.hint}>
            [업데이트 시작]을 누르면 내려받아 설치하고 <b>프로그램이 저절로 다시
            시작됩니다.</b> 자료는 그대로 남습니다. 하던 일이 있으면 마친 뒤에 눌러 주세요.
          </p>

          <div className={s.dangerRow}>
            <Button variant="primary" onClick={() => run.mutate(phase.update)}>
              업데이트 시작
            </Button>
          </div>
        </div>
      )}

      {phase.kind === 'DOWNLOADING' && (
        <div>
          <p className={s.stepText}>업데이트 파일을 내려받고 있습니다.</p>
          <div className={s.bar}>
            <div
              className={s.barFill}
              style={{
                width: phase.total
                  ? `${Math.min(100, Math.round((phase.got / phase.total) * 100))}%`
                  : '35%',
              }}
            />
          </div>
          <p className={s.hint}>
            {phase.total
              ? `${Math.round((phase.got / phase.total) * 100)}%  (${kb(phase.got)} / ${kb(phase.total)} KB)`
              : `${kb(phase.got)} KB`}
            {' · '}창을 닫지 말아 주세요.
          </p>
        </div>
      )}

      {phase.kind === 'INSTALLING' && (
        <div>
          <p className={s.stepText}>업데이트를 설치합니다.</p>
          <div className={s.bar}>
            <div className={`${s.barFill} ${s.barDone}`} style={{ width: '100%' }} />
          </div>
          <Notice tone="warn">
            <div>
              <strong>프로그램이 잠시 뒤 저절로 닫혔다가 다시 켜집니다.</strong>
              <p className={s.warnBody}>
                설치가 끝나면 새 버전으로 열립니다. 그동안 아무것도 누르지 말아 주세요.
              </p>
            </div>
          </Notice>
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
