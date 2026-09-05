import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { NumberInput } from '@/components/NumberInput'
import { Button, Card, Notice } from '@/components/ui'
import { adminApi, type NewTermResult, type TermRow } from '@/ipc/admin'
import { errorMessage } from '@/ipc/invoke'
import s from './admin.module.css'

/**
 * 학기 관리.
 *
 * 학기 시작·종료일은 통계의 '이번 학기'를 정하는 기준이다. 비워 두면
 * 우리나라 학사 일정(1학기 3~8월, 2학기 9~2월)으로 계산한다.
 * 날짜를 고쳐도 보결 기록은 조금도 바뀌지 않는다 — 기록은 날짜를 스스로 갖고 있다.
 */
export function TermCard() {
  const qc = useQueryClient()
  const { data: terms, isLoading, error } = useQuery({
    queryKey: ['terms'],
    queryFn: adminApi.termList,
  })

  const [editing, setEditing] = useState<TermRow | null>(null)
  const [newOpen, setNewOpen] = useState(false)
  const [made, setMade] = useState<NewTermResult | null>(null)

  const refresh = async () => {
    // 학기가 바뀌면 거의 모든 화면이 달라진다
    await qc.invalidateQueries()
  }

  const setCurrent = useMutation({
    mutationFn: (id: number) => adminApi.termSetCurrent(id),
    onSuccess: refresh,
  })

  return (
    <Card
      title="학년도 · 학기"
      subtitle="학기 기간을 정하고 새 학기를 시작합니다"
      collapsible
      rememberKey="term"
      footer={
        <div className={s.footRow}>
          <span className={s.footNote}>
            새 학기를 시작해도 지난 학기의 학급·시간표·보결 기록은 그대로 남습니다.
          </span>
          <div className={s.footBtns}>
            <Button variant="primary" icon="plus" onClick={() => setNewOpen(true)}>
              새 학기 시작
            </Button>
          </div>
        </div>
      }
    >
      {error && <Notice tone="danger">{errorMessage(error)}</Notice>}
      {setCurrent.error && <Notice tone="danger">{errorMessage(setCurrent.error)}</Notice>}
      {isLoading && <p className={s.muted}>불러오는 중…</p>}

      {made && (
        <Notice tone="info">
          <div>
            <strong>{made.name}</strong>를 시작했습니다. (학급 {made.copiedClasses} · 시정표{' '}
            {made.copiedBells} · 전담 수업 {made.copiedLessons} 가져옴)
            <ul className={s.stepList}>
              {made.nextSteps.map((x) => (
                <li key={x}>{x}</li>
              ))}
            </ul>
            <button type="button" className={s.linkBtn} onClick={() => setMade(null)}>
              닫기
            </button>
          </div>
        </Notice>
      )}

      {terms && (
        <div className={s.tableWrap}>
          <table className={s.table}>
            <thead>
              <tr>
                <th>학기</th>
                <th>기간</th>
                <th className={s.numCol}>학급</th>
                <th className={s.numCol}>전담 수업</th>
                <th className={s.numCol}>보결</th>
                <th className={s.actCol} />
              </tr>
            </thead>
            <tbody>
              {terms.map((t) => (
                <tr key={t.id} className={t.isCurrent ? s.rowNow : undefined}>
                  <td className={s.fileName}>
                    {t.name}
                    {t.isCurrent && <span className={s.nowTag}>현재</span>}
                  </td>
                  <td>
                    <span className="num">
                      {t.effectiveFrom} ~ {t.effectiveTo}
                    </span>
                    {!t.explicit && <span className={s.calc}>학사 일정으로 계산</span>}
                  </td>
                  <td className={`${s.numCol} num`}>{t.classCount}</td>
                  <td className={`${s.numCol} num`}>{t.lessonCount}</td>
                  <td className={`${s.numCol} num`}>{t.subCount}</td>
                  <td className={s.actCol}>
                    {!t.isCurrent && (
                      <button
                        type="button"
                        className={s.smallBtn}
                        onClick={() => setCurrent.mutate(t.id)}
                      >
                        현재로
                      </button>
                    )}
                    <button type="button" className={s.smallBtn} onClick={() => setEditing(t)}>
                      날짜 수정
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {editing && (
        <DatesModal term={editing} onClose={() => setEditing(null)} onSaved={refresh} />
      )}
      {newOpen && (
        <NewTermModal
          terms={terms ?? []}
          onClose={() => setNewOpen(false)}
          onDone={async (r) => {
            setNewOpen(false)
            setMade(r)
            await refresh()
          }}
        />
      )}
    </Card>
  )
}

/* ============================================================
   학기 날짜
   ============================================================ */

function DatesModal({
  term,
  onClose,
  onSaved,
}: {
  term: TermRow
  onClose: () => void
  onSaved: () => Promise<void>
}) {
  const [name, setName] = useState(term.name)
  const [from, setFrom] = useState(term.startDate ?? '')
  const [to, setTo] = useState(term.endDate ?? '')

  const save = useMutation({
    mutationFn: () =>
      adminApi.termSaveDates({
        id: term.id,
        name,
        startDate: from || null,
        endDate: to || null,
      }),
    onSuccess: async () => {
      await onSaved()
      onClose()
    },
  })

  return (
    <Modal
      open
      title="학기 기간"
      description="통계의 '이번 학기'를 이 기간으로 셉니다. 보결 기록은 바뀌지 않습니다."
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            닫기
          </Button>
          <Button
            variant="primary"
            icon="check"
            disabled={save.isPending}
            onClick={() => save.mutate()}
          >
            저장
          </Button>
        </>
      }
    >
      {save.error && <Notice tone="danger">{errorMessage(save.error)}</Notice>}

      <label className={s.field}>
        <span className={s.fieldLabel}>학기 이름</span>
        <input
          type="text"
          className={s.text}
          value={name}
          maxLength={40}
          onChange={(e) => setName(e.target.value)}
        />
      </label>

      <div className={s.field}>
        <span className={s.fieldLabel}>기간</span>
        <div className={s.pathRow}>
          <input
            type="date"
            className={s.date}
            value={from}
            onChange={(e) => setFrom(e.target.value)}
          />
          <span className={s.tilde}>~</span>
          <input
            type="date"
            className={s.date}
            value={to}
            onChange={(e) => setTo(e.target.value)}
          />
          {(from || to) && (
            <button
              type="button"
              className={s.linkBtn}
              onClick={() => {
                setFrom('')
                setTo('')
              }}
            >
              비우기
            </button>
          )}
        </div>
        <p className={s.hint}>
          비워 두면 학사 일정으로 계산합니다 (1학기 3/1~8/31, 2학기 9/1~다음해 2월 말).
          지금 계산값은 <span className="num">{term.effectiveFrom} ~ {term.effectiveTo}</span>{' '}
          입니다.
        </p>
      </div>
    </Modal>
  )
}

/* ============================================================
   새 학기 시작
   ============================================================ */

function NewTermModal({
  terms,
  onClose,
  onDone,
}: {
  terms: TermRow[]
  onClose: () => void
  onDone: (r: NewTermResult) => Promise<void>
}) {
  const now = terms.find((t) => t.isCurrent) ?? terms[0]
  // 1학기 다음은 같은 해 2학기, 2학기 다음은 다음 해 1학기
  const nextYear = now ? (now.semester === 1 ? now.schoolYear : now.schoolYear + 1) : 2026
  const nextSem = now ? (now.semester === 1 ? 2 : 1) : 1

  const [year, setYear] = useState(nextYear)
  const [sem, setSem] = useState(nextSem)
  const [copyClasses, setCopyClasses] = useState(true)
  const [copyHomerooms, setCopyHomerooms] = useState(false)
  const [copyBells, setCopyBells] = useState(true)
  const [copyLessons, setCopyLessons] = useState(false)
  const [makeCurrent, setMakeCurrent] = useState(true)

  useEffect(() => {
    if (!copyClasses) {
      setCopyHomerooms(false)
      setCopyLessons(false)
    }
  }, [copyClasses])
  useEffect(() => {
    if (!copyBells) setCopyLessons(false)
  }, [copyBells])

  const run = useMutation({
    mutationFn: () =>
      adminApi.termStartNew({
        schoolYear: year,
        semester: sem,
        copyClasses,
        copyHomerooms,
        copyBells,
        copyLessons,
        setCurrent: makeCurrent,
      }),
    onSuccess: onDone,
  })

  return (
    <Modal
      open
      title="새 학기 시작"
      description="새 학기 자료를 만듭니다. 지난 학기 자료는 그대로 남습니다."
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            닫기
          </Button>
          <Button
            variant="primary"
            icon="check"
            disabled={run.isPending}
            onClick={() => run.mutate()}
          >
            {run.isPending ? '만드는 중…' : `${year}학년도 ${sem}학기 시작`}
          </Button>
        </>
      }
    >
      {run.error && <Notice tone="danger">{errorMessage(run.error)}</Notice>}

      <div className={s.field}>
        <span className={s.fieldLabel}>새 학기</span>
        <div className={s.pathRow}>
          <NumberInput value={year} min={2000} max={2100} onChange={setYear} />
          <span className={s.tilde}>학년도</span>
          <div className={s.segment}>
            {[1, 2].map((v) => (
              <button
                key={v}
                type="button"
                className={sem === v ? `${s.seg} ${s.segOn}` : s.seg}
                onClick={() => setSem(v)}
              >
                {v}학기
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className={s.field}>
        <span className={s.fieldLabel}>가져올 자료</span>
        <ul className={s.optList}>
          <li className={s.optItem}>
            <label className={s.optLabel}>
              <input
                type="checkbox"
                checked={copyClasses}
                onChange={(e) => setCopyClasses(e.target.checked)}
              />
              <span>
                <span className={s.optTitle}>학급 구성 (학년 · 반 수 · 반 이름)</span>
                <span className={s.optHint}>
                  끄면 학급을 처음부터 만들어야 합니다. 반 이름이 바뀌면 가져온 뒤 고치는 편이
                  빠릅니다.
                </span>
              </span>
            </label>
          </li>
          <li className={s.optItem}>
            <label className={s.optLabel}>
              <input
                type="checkbox"
                checked={copyHomerooms}
                disabled={!copyClasses}
                onChange={(e) => setCopyHomerooms(e.target.checked)}
              />
              <span>
                <span className={s.optTitle}>담임 배정까지 그대로</span>
                <span className={s.optHint}>
                  보통 새 학기에는 담임이 바뀌므로 꺼 두었습니다. 그대로 가는 학교만 켜세요.
                </span>
              </span>
            </label>
          </li>
          <li className={s.optItem}>
            <label className={s.optLabel}>
              <input
                type="checkbox"
                checked={copyBells}
                onChange={(e) => setCopyBells(e.target.checked)}
              />
              <span>
                <span className={s.optTitle}>시정표 (교시 시각 · 점심시간)</span>
                <span className={s.optHint}>
                  시정표는 학기가 바뀌어도 거의 같으므로 켜 두었습니다.
                </span>
              </span>
            </label>
          </li>
          <li className={s.optItem}>
            <label className={s.optLabel}>
              <input
                type="checkbox"
                checked={copyLessons}
                disabled={!copyClasses || !copyBells}
                onChange={(e) => setCopyLessons(e.target.checked)}
              />
              <span>
                <span className={s.optTitle}>전담 시간표</span>
                <span className={s.optHint}>
                  전담 시간표는 학기마다 새로 짜는 것이 보통이라 꺼 두었습니다. 뼈대를 두고
                  고치실 거면 켜세요. (학급·시정표를 함께 가져올 때만 쓸 수 있습니다)
                </span>
              </span>
            </label>
          </li>
          <li className={s.optItem}>
            <label className={s.optLabel}>
              <input
                type="checkbox"
                checked={makeCurrent}
                onChange={(e) => setMakeCurrent(e.target.checked)}
              />
              <span>
                <span className={s.optTitle}>바로 현재 학기로 삼기</span>
                <span className={s.optHint}>
                  켜면 모든 화면이 새 학기 기준으로 바뀝니다. 미리 만들어만 두려면 끄세요.
                </span>
              </span>
            </label>
          </li>
        </ul>
      </div>

      <Notice tone="info">
        <div>
          <strong>교사와 과목은 복사하지 않습니다.</strong>
          <p className={s.warnBody}>
            두 자료는 학기가 아니라 학교에 속해서 그대로 이어집니다. 전출·전입만 교사 관리에서
            정리해 주세요. 새 학기를 만들기 전에 지금 자료를 자동으로 백업해 둡니다.
          </p>
        </div>
      </Notice>
    </Modal>
  )
}
