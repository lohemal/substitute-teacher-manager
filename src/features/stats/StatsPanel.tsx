import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'

import { Notice } from '@/components/ui'
import { DAY_LABEL, minToHm } from '@/ipc/bell'
import {
  PRESET_LABEL,
  statsApi,
  type Band,
  type DayStat,
  type Preset,
  type StatsView,
  type TeacherStat,
} from '@/ipc/stats'
import { adminApi } from '@/ipc/admin'
import { ExportButton } from '@/features/admin/ExportButton'
import { errorMessage } from '@/ipc/invoke'
import { useRemembered } from '@/lib/remember'
import {
  MAX_SORT,
  sortDir,
  sortRank,
  sortTeachers,
  toggleSort,
  type SortField,
  type SortKey,
} from './sortTeachers'
import s from './StatsPanel.module.css'

function todayStr(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

const PRESETS: Preset[] = ['TODAY', 'WEEK', 'MONTH', 'TERM', 'CUSTOM']

/** 고른 기간에 대응하는 교사별 표의 열 */
const PRESET_COL: Partial<Record<Preset, keyof TeacherStat>> = {
  TODAY: 'today',
  WEEK: 'week',
  MONTH: 'month',
  TERM: 'term',
}

/** 정렬 기준 칩에 쓰는 열 이름 */
const COL_LABEL: Record<SortField, string> = {
  name: '교사',
  role: '구분',
  duty: '담당',
  period: '기간',
  today: '오늘',
  week: '이번 주',
  month: '이번 달',
  term: '이번 학기',
  total: '누적',
  band: '참고',
}

const BAND_CLASS: Record<Band, string> = {
  MORE: 'bandMore',
  TYPICAL: 'bandTypical',
  LESS: 'bandLess',
  NONE: 'bandNone',
}

/**
 * 보결 현황.
 *
 * 모든 숫자는 원본(`substitutions`·`absences`)을 그때그때 다시 세어 만든다.
 * 배정을 취소하거나 고치면 이 화면을 다시 열 때 곧바로 반영된다.
 */
export function StatsPanel() {
  // 자주 보는 기간이 사람마다 다르다 — 마지막에 고른 것으로 열어 준다
  const [preset, setPreset] = useRemembered<Preset>('stats.preset', 'TODAY')
  const [from, setFrom] = useState(todayStr)
  const [to, setTo] = useState(todayStr)
  const [showAll, setShowAll] = useState(false)

  const query = {
    preset,
    from: preset === 'CUSTOM' ? from : null,
    to: preset === 'CUSTOM' ? to : null,
  }
  const { data, isLoading, error } = useQuery({
    queryKey: ['stats', query],
    queryFn: () => statsApi.view(query),
  })

  return (
    <div className={s.wrap}>
      {/* ---------- 기간 ---------- */}
      <section className={s.periodBar}>
        <div className={s.segment}>
          {PRESETS.map((p) => (
            <button
              key={p}
              type="button"
              className={preset === p ? `${s.seg} ${s.segOn}` : s.seg}
              onClick={() => setPreset(p)}
            >
              {PRESET_LABEL[p]}
            </button>
          ))}
        </div>

        {preset === 'CUSTOM' && (
          <div className={s.range}>
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
          </div>
        )}

        {data && <span className={s.rangeLabel}>{data.rangeLabel}</span>}
        <ExportButton run={() => adminApi.exportStats(query)} />
      </section>

      {error && <Notice tone="danger">{errorMessage(error)}</Notice>}
      {isLoading && <p className={s.center}>불러오는 중…</p>}

      {data && (
        <>
          <SummaryCards v={data} />
          <OpenList v={data} />
          <TeacherTable v={data} showAll={showAll} onToggleAll={() => setShowAll((x) => !x)} />
          <Fairness v={data} />
          <AbsenceTable v={data} />
          <DayTable v={data} />
        </>
      )}
    </div>
  )
}

/* ============================================================
   요약 카드
   ============================================================ */

function SummaryCards({ v }: { v: StatsView }) {
  const m = v.summary
  const isToday = v.preset === 'TODAY'
  return (
    <section>
      <h2 className={s.head}>{isToday ? '오늘 현황' : '기간 현황'}</h2>
      <div className={s.cards}>
        <Card
          label="결근 교사"
          value={m.absentTeachers}
          unit="명"
          sub={`결근 기록 ${m.absenceCount}건`}
        />
        <Card
          label="보결 필요"
          value={m.required}
          unit="건"
          sub={m.required > 0 ? `배정 ${m.covered} · 미배정 ${m.unassigned}` : '등록된 결근 없음'}
        />
        <Card
          label="미배정"
          value={m.unassigned}
          unit="건"
          tone={m.unassigned > 0 ? 'alert' : 'ok'}
          sub={
            m.unassigned > 0
              ? `보결 필요 ${m.required}건 가운데`
              : m.required > 0
                ? '모두 배정되었습니다'
                : '남은 것이 없습니다'
          }
          big
        />
        <Card
          label="배정 완료"
          value={m.assigned}
          unit="건"
          tone="ok"
          sub={
            m.assigned > m.covered
              ? `결근 등록 없이 배정한 ${m.assigned - m.covered}건 포함`
              : undefined
          }
        />
        <Card label="취소" value={m.cancelled} unit="건" />
        <Card label="보결 맡은 교사" value={m.subTeachers} unit="명" />
      </div>
      <p className={s.note}>
        <b>보결 필요</b>는 등록된 결근에서 계산한 시간 수이고, <b>배정 완료</b>는 그 기간에
        저장된 보결 건수입니다. 결근을 등록하지 않고 바로 배정하면 두 숫자가 다를 수 있습니다.
      </p>
    </section>
  )
}

function Card({
  label,
  value,
  unit,
  sub,
  tone,
  big,
}: {
  label: string
  value: number
  unit: string
  sub?: string
  tone?: 'ok' | 'alert'
  big?: boolean
}) {
  const cls = [s.card, tone ? s[`card_${tone}`] : '', big ? s.cardBig : ''].filter(Boolean).join(' ')
  return (
    <div className={cls}>
      <p className={s.cardLabel}>{label}</p>
      <p className={s.cardValue}>
        <span className="num">{value}</span>
        <span className={s.cardUnit}>{unit}</span>
      </p>
      {sub && <p className={s.cardSub}>{sub}</p>}
    </div>
  )
}

/* ============================================================
   미배정 목록
   ============================================================ */

function OpenList({ v }: { v: StatsView }) {
  if (v.openSlots.length === 0) return null
  return (
    <section className={s.openBox}>
      <h2 className={s.openHead}>
        아직 배정하지 않은 시간 <span className="num">{v.openSlots.length}</span>건
      </h2>
      <ul className={s.openList}>
        {v.openSlots.slice(0, 40).map((o, i) => (
          <li key={`${o.date}-${o.classId}-${o.startMin}-${i}`} className={s.openItem}>
            <span className={`${s.openDate} num`}>
              {o.date.slice(5)} ({DAY_LABEL[o.dayOfWeek]})
            </span>
            <span className={s.openCls}>{o.classLabel}</span>
            <span className={s.openSlot}>{o.slotLabel}</span>
            <span className={`${s.openTime} num`}>
              {minToHm(o.startMin)}~{minToHm(o.endMin)}
            </span>
            <span className={s.openWho}>{o.absentTeacherName} 선생님 대신</span>
            <span className={s.openKind}>{o.kindLabel}</span>
          </li>
        ))}
      </ul>
      {v.openSlots.length > 40 && (
        <p className={s.openMore}>… 그 밖에 {v.openSlots.length - 40}건이 더 있습니다.</p>
      )}
    </section>
  )
}

/* ============================================================
   교사별 현황
   ============================================================ */

function TeacherTable({
  v,
  showAll,
  onToggleAll,
}: {
  v: StatsView
  showAll: boolean
  onToggleAll: () => void
}) {
  // 머리글을 누른 순서대로 쌓인다. 먼저 누른 열이 우선한다.
  const [sortKeys, setSortKeys] = useState<SortKey[]>([])

  const visible = showAll ? v.teachers : v.teachers.filter((t) => t.active)
  const rows = sortTeachers(visible, sortKeys)
  const hidden = v.teachers.length - v.teachers.filter((t) => t.active).length
  const col = PRESET_COL[v.preset]

  /** 정렬할 수 있는 머리글 */
  const Th = ({
    field,
    label,
    className,
    highlight,
  }: {
    field: SortField
    label: string
    className?: string
    highlight?: boolean
  }) => {
    const dir = sortDir(sortKeys, field)
    const rank = sortRank(sortKeys, field)
    const cls = [s.sortTh, className, highlight ? s.colOn : '', dir ? s.sortOn : '']
      .filter(Boolean)
      .join(' ')
    const next = dir === null ? '오름차순' : dir === 'asc' ? '내림차순' : '정렬 해제'
    return (
      <th className={cls} aria-sort={dir === 'asc' ? 'ascending' : dir === 'desc' ? 'descending' : 'none'}>
        <button
          type="button"
          className={s.sortBtn}
          onClick={() => setSortKeys((k) => toggleSort(k, field))}
          title={`${label} 기준으로 ${next}`}
        >
          <span>{label}</span>
          <span className={s.sortMark} aria-hidden="true">
            {dir === 'asc' ? '▲' : dir === 'desc' ? '▼' : '↕'}
          </span>
          {rank > 0 && sortKeys.length > 1 && <span className={s.sortRank}>{rank}</span>}
        </button>
      </th>
    )
  }

  return (
    <section>
      <div className={s.headRow}>
        <h2 className={s.head}>교사별 보결 현황</h2>
        <span className={s.headNote}>취소한 배정은 세지 않습니다</span>
        {hidden > 0 && (
          <button type="button" className={s.linkBtn} onClick={onToggleAll}>
            {showAll ? '비활성 교사 숨기기' : `비활성 교사 ${hidden}명 보기`}
          </button>
        )}
      </div>

      <div className={s.sortBar}>
        {sortKeys.length === 0 ? (
          <span className={s.sortHint}>
            머리글을 누르면 오름차순 → 내림차순 → 해제 순으로 정렬됩니다. 여러 열을 누르면
            먼저 누른 열이 우선하며 최대 {MAX_SORT}개까지 쌓입니다.
          </span>
        ) : (
          <>
            <span className={s.sortHint}>정렬 기준</span>
            <span className={s.sortChips}>
              {sortKeys.map((k, i) => (
                <span key={k.field} className={s.sortChip}>
                  {i + 1}. {COL_LABEL[k.field]} {k.dir === 'asc' ? '↑' : '↓'}
                </span>
              ))}
            </span>
            <button type="button" className={s.linkBtn} onClick={() => setSortKeys([])}>
              정렬 해제
            </button>
          </>
        )}
      </div>

      <div className={s.tableCard}>
        <table className={s.table}>
          <thead>
            <tr>
              <Th field="name" label="교사" />
              <Th field="role" label="구분" className={s.roleCol} />
              <Th field="duty" label="담당" />
              {v.preset === 'CUSTOM' && (
                <Th field="period" label="기간" className={s.numCol} highlight />
              )}
              <Th field="today" label="오늘" className={s.numCol} highlight={col === 'today'} />
              <Th field="week" label="이번 주" className={s.numCol} highlight={col === 'week'} />
              <Th field="month" label="이번 달" className={s.numCol} highlight={col === 'month'} />
              <Th field="term" label="이번 학기" className={s.numCol} highlight={col === 'term'} />
              <Th field="total" label="누적" className={`${s.numCol} ${s.totalCol}`} />
              <Th field="band" label="참고" className={s.bandCol} />
            </tr>
          </thead>
          <tbody>
            {rows.map((t) => (
              <tr key={t.teacherId} className={t.active ? undefined : s.rowOff}>
                <td className={s.name}>
                  {t.name}
                  {!t.active && <span className={s.tag}>비활성</span>}
                  {t.active && !t.isSubstitutable && <span className={s.tag}>보결 제외</span>}
                </td>
                <td className={s.roleCol}>
                  <span className={`${s.roleTag} ${s[`role_${t.roleCode}`]}`}>{t.roleLabel}</span>
                </td>
                <td className={s.duty}>{t.duty || '—'}</td>
                {v.preset === 'CUSTOM' && (
                  <td className={`${s.numCol} ${s.colOn} num`}>{t.period}</td>
                )}
                <td className={col === 'today' ? `${s.numCol} ${s.colOn} num` : `${s.numCol} num`}>
                  {t.today}
                </td>
                <td className={col === 'week' ? `${s.numCol} ${s.colOn} num` : `${s.numCol} num`}>
                  {t.week}
                </td>
                <td className={col === 'month' ? `${s.numCol} ${s.colOn} num` : `${s.numCol} num`}>
                  {t.month}
                </td>
                <td className={col === 'term' ? `${s.numCol} ${s.colOn} num` : `${s.numCol} num`}>
                  {t.term}
                </td>
                <td className={`${s.numCol} ${s.totalCol} num`}>{t.total}</td>
                <td className={s.bandCol}>
                  {t.band !== 'NONE' && (
                    <span className={`${s.band} ${s[BAND_CLASS[t.band]]}`}>{t.bandLabel}</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

/* ============================================================
   공정성 참고
   ============================================================ */

function Fairness({ v }: { v: StatsView }) {
  if (v.spreads.length === 0) return null
  return (
    <section>
      <h2 className={s.head}>구분별 분포 (참고)</h2>
      <div className={s.spreadRow}>
        {v.spreads.map((sp) => (
          <div key={sp.roleCode} className={s.spread}>
            <p className={s.spreadTitle}>
              {sp.roleLabel} <span className={s.spreadPeople}>{sp.people}명</span>
            </p>
            <dl className={s.spreadList}>
              <dt>합계</dt>
              <dd className="num">{sp.total}회</dd>
              <dt>평균</dt>
              <dd className="num">{sp.avg.toFixed(1)}회</dd>
              <dt>최다</dt>
              <dd>
                <span className="num">{sp.max}회</span>
                {sp.maxName && <span className={s.spreadWho}>{sp.maxName}</span>}
              </dd>
              <dt>최소</dt>
              <dd>
                <span className="num">{sp.min}회</span>
                {sp.minName && <span className={s.spreadWho}>{sp.minName}</span>}
              </dd>
              <dt>차이</dt>
              <dd className="num">{sp.spread}회</dd>
            </dl>
            {!sp.comparable && <p className={s.spreadNone}>견줄 만한 차이가 없습니다</p>}
          </div>
        ))}
      </div>
      <p className={s.note}>{v.fairnessNote}</p>
    </section>
  )
}

/* ============================================================
   결근 현황
   ============================================================ */

function AbsenceTable({ v }: { v: StatsView }) {
  return (
    <section>
      <h2 className={s.head}>결근 현황</h2>
      {v.absences.length === 0 ? (
        <p className={s.empty}>이 기간에 등록된 결근이 없습니다.</p>
      ) : (
        <div className={s.tableCard}>
          <table className={s.table}>
            <thead>
              <tr>
                <th>교사</th>
                <th className={s.roleCol}>구분</th>
                <th className={s.numCol}>결근</th>
                <th className={s.numCol}>종일</th>
                <th className={s.numCol}>일부</th>
                <th>사유</th>
                <th className={s.numCol}>보결 필요</th>
                <th className={s.numCol}>배정</th>
                <th className={s.numCol}>미배정</th>
              </tr>
            </thead>
            <tbody>
              {v.absences.map((a) => (
                <tr key={a.teacherId}>
                  <td className={s.name}>{a.name}</td>
                  <td className={s.roleCol}>{a.roleLabel}</td>
                  <td className={`${s.numCol} num`}>{a.count}</td>
                  <td className={`${s.numCol} num`}>{a.allDay}</td>
                  <td className={`${s.numCol} num`}>{a.partial}</td>
                  <td className={s.duty}>{a.reasons}</td>
                  <td className={`${s.numCol} num`}>{a.required}</td>
                  <td className={`${s.numCol} num`}>{a.assigned}</td>
                  <td className={`${s.numCol} num`}>
                    {a.unassigned > 0 ? (
                      <span className={s.openMark}>{a.unassigned}</span>
                    ) : (
                      a.unassigned
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  )
}

/* ============================================================
   날짜별 현황
   ============================================================ */

function DayTable({ v }: { v: StatsView }) {
  // 아무 일도 없던 날은 접어 둔다 — 학기 전체를 봐도 표가 짧게 유지된다
  const [showEmpty, setShowEmpty] = useState(false)
  const busy = (d: DayStat) =>
    d.absentTeachers > 0 || d.required > 0 || d.assigned > 0 || d.cancelled > 0
  const rows = showEmpty ? v.days : v.days.filter(busy)
  const quiet = v.days.length - v.days.filter(busy).length

  return (
    <section>
      <div className={s.headRow}>
        <h2 className={s.head}>날짜별 현황</h2>
        {quiet > 0 && (
          <button type="button" className={s.linkBtn} onClick={() => setShowEmpty((x) => !x)}>
            {showEmpty ? '기록 없는 날 숨기기' : `기록 없는 날 ${quiet}일 보기`}
          </button>
        )}
      </div>

      {v.daysTruncated && (
        <Notice tone="warn">기간이 길어 앞부분만 보여 줍니다. 기간을 좁혀 주세요.</Notice>
      )}

      {rows.length === 0 ? (
        <p className={s.empty}>이 기간에 결근이나 보결 기록이 없습니다.</p>
      ) : (
        <div className={s.tableCard}>
          <table className={s.table}>
            <thead>
              <tr>
                <th className={s.dateCol}>날짜</th>
                <th className={s.numCol}>결근</th>
                <th>결근 교사</th>
                <th className={s.numCol}>보결 필요</th>
                <th className={s.numCol}>배정 완료</th>
                <th className={s.numCol}>미배정</th>
                <th className={s.numCol}>취소</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((d) => (
                <tr key={d.date} className={d.unassigned > 0 ? s.rowOpen : undefined}>
                  <td className={s.dateCol}>
                    <span className="num">{d.date}</span>
                    <span className={s.day}>{DAY_LABEL[d.dayOfWeek]}</span>
                  </td>
                  <td className={`${s.numCol} num`}>{d.absentTeachers || '—'}</td>
                  <td className={s.duty}>{d.absentNames || '—'}</td>
                  <td className={`${s.numCol} num`}>{d.required || '—'}</td>
                  <td className={`${s.numCol} num`}>{d.assigned || '—'}</td>
                  <td className={s.numCol}>
                    {d.unassigned > 0 ? (
                      <span className={s.openMark}>{d.unassigned}</span>
                    ) : (
                      <span className="num">—</span>
                    )}
                  </td>
                  <td className={`${s.numCol} num`}>{d.cancelled || '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  )
}
