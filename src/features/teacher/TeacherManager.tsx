import { useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { Icon } from '@/components/Icon'
import { Button, Notice } from '@/components/ui'
import { errorMessage } from '@/ipc/invoke'
import { teacherApi, type RoleCode, type TeacherList, type TeacherView } from '@/ipc/teacher'
import { PasteRosterModal } from './PasteRosterModal'
import { QuickHomeroomModal } from './QuickHomeroomModal'
import { TeacherEditor } from './TeacherEditor'
import s from './TeacherManager.module.css'

type RoleFilter = 'ALL' | RoleCode
type SubFilter = 'ALL' | 'ON' | 'OFF'

export function TeacherManager() {
  const qc = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: ['teacher-list'],
    queryFn: teacherApi.list,
  })

  const [keyword, setKeyword] = useState('')
  const [roleFilter, setRoleFilter] = useState<RoleFilter>('ALL')
  const [subFilter, setSubFilter] = useState<SubFilter>('ALL')
  const [showInactive, setShowInactive] = useState(false)
  const [selected, setSelected] = useState<Set<number>>(new Set())

  const [editing, setEditing] = useState<TeacherView | null>(null)
  const [editorOpen, setEditorOpen] = useState(false)
  const [quickOpen, setQuickOpen] = useState(false)
  const [pasteOpen, setPasteOpen] = useState(false)
  const [freedNotice, setFreedNotice] = useState<string[] | null>(null)

  const afterChange = (list: TeacherList) => {
    qc.setQueryData(['teacher-list'], list)
    qc.invalidateQueries({ queryKey: ['setup-state'] })
    qc.invalidateQueries({ queryKey: ['teacher-readiness'] })
    setSelected(new Set())
  }

  const bulkSub = useMutation({
    mutationFn: (v: boolean) => teacherApi.setSubstitutableBulk([...selected], v),
    onSuccess: afterChange,
  })

  const bulkActive = useMutation({
    mutationFn: (active: boolean) => teacherApi.setActiveBulk([...selected], active),
    onSuccess: (res) => {
      afterChange(res.list)
      setFreedNotice(res.unassignedClasses.length > 0 ? res.unassignedClasses : null)
    },
  })

  const rows = useMemo(() => {
    if (!data) return []
    const kw = keyword.trim()
    return data.teachers.filter((t) => {
      if (!showInactive && !t.active) return false
      if (roleFilter !== 'ALL' && t.roleCode !== roleFilter) return false
      if (subFilter === 'ON' && !t.isSubstitutable) return false
      if (subFilter === 'OFF' && t.isSubstitutable) return false
      if (kw) {
        const hay = [t.name, t.memo ?? '', ...t.homerooms.map((h) => h.label), ...t.subjects.map((x) => x.name)]
          .join(' ')
          .toLowerCase()
        if (!hay.includes(kw.toLowerCase())) return false
      }
      return true
    })
  }, [data, keyword, roleFilter, subFilter, showInactive])

  if (isLoading) return <div className={s.center}>불러오는 중…</div>
  if (error || !data) return <Notice tone="danger">{errorMessage(error)}</Notice>

  const c = data.counts
  const allChecked = rows.length > 0 && rows.every((t) => selected.has(t.id))
  const missingHomeroom = data.classes.filter((x) => x.homeroomTeacherId === null)

  const toggleAll = () => {
    setSelected(allChecked ? new Set() : new Set(rows.map((t) => t.id)))
  }

  const toggleOne = (id: number) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const openEditor = (t: TeacherView | null) => {
    setEditing(t)
    setEditorOpen(true)
  }

  return (
    <div className={s.wrap}>
      {/* ---- 요약 · 만들기 ---- */}
      <div className={s.topBar}>
        <div className={s.summary}>
          <span className={s.sumMain}>전체 {c.total}명</span>
          <span className={s.sumSep}>·</span>
          <span>담임 {c.homeroom}</span>
          <span className={s.sumSep}>·</span>
          <span>전담 {c.special}</span>
          <span className={s.sumSep}>·</span>
          <span>기타 {c.other}</span>
          <span className={s.sumSep}>·</span>
          <span className={s.sumAccent}>보결 대상 {c.substitutable}명</span>
          {c.inactive > 0 && (
            <>
              <span className={s.sumSep}>·</span>
              <span className={s.sumMuted}>비활성 {c.inactive}</span>
            </>
          )}
        </div>
        <div className={s.topActions}>
          <Button variant="ghost" onClick={() => setPasteOpen(true)}>
            명단 붙여넣기
          </Button>
          <Button variant="ghost" onClick={() => setQuickOpen(true)}>
            담임 빠른 등록
            {missingHomeroom.length > 0 && (
              <span className={s.badge}>{missingHomeroom.length}</span>
            )}
          </Button>
          <Button variant="primary" icon="plus" onClick={() => openEditor(null)}>
            교사 추가
          </Button>
        </div>
      </div>

      {freedNotice && (
        <Notice tone="warn">
          {freedNotice.join(', ')}반의 담임이 비었습니다. 새 담임을 지정해 주세요.
        </Notice>
      )}
      {bulkSub.error && <Notice tone="danger">{errorMessage(bulkSub.error)}</Notice>}
      {bulkActive.error && <Notice tone="danger">{errorMessage(bulkActive.error)}</Notice>}

      {/* ---- 검색 · 필터 ---- */}
      <div className={s.filterBar}>
        <div className={s.search}>
          <Icon name="search" size={17} />
          <input
            className={s.searchInput}
            value={keyword}
            placeholder="이름 · 학급 · 과목 검색"
            onChange={(e) => setKeyword(e.target.value)}
          />
          {keyword && (
            <button type="button" className={s.clear} onClick={() => setKeyword('')} aria-label="검색어 지우기">
              ×
            </button>
          )}
        </div>

        <div className={s.filterGroup}>
          <span className={s.filterLabel}>구분</span>
          {(['ALL', 'HOMEROOM', 'SPECIAL', 'OTHER'] as RoleFilter[]).map((v) => (
            <button
              key={v}
              type="button"
              className={roleFilter === v ? `${s.fChip} ${s.fChipOn}` : s.fChip}
              onClick={() => setRoleFilter(v)}
            >
              {v === 'ALL' ? '전체' : data.roles.find((r) => r.code === v)?.label}
            </button>
          ))}
        </div>

        <div className={s.filterGroup}>
          <span className={s.filterLabel}>보결</span>
          {([['ALL', '전체'], ['ON', '대상'], ['OFF', '제외']] as [SubFilter, string][]).map(
            ([v, label]) => (
              <button
                key={v}
                type="button"
                className={subFilter === v ? `${s.fChip} ${s.fChipOn}` : s.fChip}
                onClick={() => setSubFilter(v)}
              >
                {label}
              </button>
            ),
          )}
        </div>

        <label className={s.inactiveToggle}>
          <input
            type="checkbox"
            checked={showInactive}
            onChange={(e) => setShowInactive(e.target.checked)}
          />
          <span>비활성 포함</span>
        </label>
      </div>

      {/* ---- 목록 ---- */}
      {data.teachers.length === 0 ? (
        <div className={s.empty}>
          <div className={s.emptyIcon} aria-hidden="true">
            <Icon name="people" size={26} />
          </div>
          <p className={s.emptyTitle}>등록된 교사가 없습니다</p>
          <p className={s.emptyDesc}>
            학급이 {data.classes.length}개 만들어져 있습니다.{' '}
            <strong>담임 빠른 등록</strong>으로 이름만 적으면 한 번에 등록됩니다.
          </p>
          <div className={s.emptyActions}>
            <Button variant="primary" onClick={() => setQuickOpen(true)}>
              담임 빠른 등록
            </Button>
            <Button variant="ghost" onClick={() => setPasteOpen(true)}>
              명단 붙여넣기
            </Button>
          </div>
        </div>
      ) : (
        <div className={s.tableCard}>
          <table className={s.table}>
            <thead>
              <tr>
                <th className={s.checkCol}>
                  <input
                    type="checkbox"
                    checked={allChecked}
                    onChange={toggleAll}
                    aria-label="모두 선택"
                  />
                </th>
                <th>이름</th>
                <th>구분</th>
                <th>담당</th>
                <th className={s.centerCol}>보결 대상</th>
                <th className={s.numCol}>수업</th>
                <th className={s.numCol}>보결</th>
                <th className={s.actionCol} />
              </tr>
            </thead>
            <tbody>
              {rows.length === 0 && (
                <tr>
                  <td colSpan={8} className={s.noMatch}>
                    조건에 맞는 교사가 없습니다. 검색어나 필터를 바꿔 보세요.
                  </td>
                </tr>
              )}
              {rows.map((t) => (
                <tr
                  key={t.id}
                  className={[
                    s.row,
                    !t.active ? s.rowInactive : '',
                    selected.has(t.id) ? s.rowSelected : '',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                  onDoubleClick={() => openEditor(t)}
                >
                  <td className={s.checkCol}>
                    <input
                      type="checkbox"
                      checked={selected.has(t.id)}
                      onChange={() => toggleOne(t.id)}
                      aria-label={`${t.name} 선택`}
                    />
                  </td>
                  <td>
                    <span className={s.name}>{t.name}</span>
                    {!t.active && <span className={s.inactiveTag}>비활성</span>}
                  </td>
                  <td>
                    <span className={`${s.roleTag} ${s[`role_${t.roleCode}`]}`}>{t.roleLabel}</span>
                  </td>
                  <td className={s.dutyCell}>
                    {t.homerooms.length > 0 && (
                      <span className={s.duty}>{t.homerooms.map((h) => h.label).join(', ')}</span>
                    )}
                    {t.subjects.length > 0 && (
                      <span className={s.duty}>{t.subjects.map((x) => x.name).join(', ')}</span>
                    )}
                    {t.memo && <span className={s.memo}>{t.memo}</span>}
                    {t.homerooms.length === 0 && t.subjects.length === 0 && !t.memo && (
                      <span className={s.none}>—</span>
                    )}
                  </td>
                  <td className={s.centerCol}>
                    {t.isSubstitutable ? (
                      <span className={s.subOn}>
                        <Icon name="check" size={13} strokeWidth={3} /> 대상
                      </span>
                    ) : (
                      <span className={s.subOff}>제외</span>
                    )}
                  </td>
                  <td className={`${s.numCol} num`}>
                    {t.lessonCount > 0 ? t.lessonCount : <span className={s.none}>—</span>}
                  </td>
                  <td className={`${s.numCol} num`}>
                    {t.subCountTotal > 0 ? t.subCountTotal : <span className={s.none}>—</span>}
                  </td>
                  <td className={s.actionCol}>
                    <button type="button" className={s.editBtn} onClick={() => openEditor(t)}>
                      수정
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* ---- 일괄 작업 ---- */}
      {selected.size > 0 && (
        <div className={s.bulkBar}>
          <span className={s.bulkCount}>{selected.size}명 선택</span>
          <div className={s.bulkActions}>
            <Button variant="accent" onClick={() => bulkSub.mutate(true)} disabled={bulkSub.isPending}>
              보결 대상으로
            </Button>
            <Button variant="ghost" onClick={() => bulkSub.mutate(false)} disabled={bulkSub.isPending}>
              보결에서 제외
            </Button>
            <Button
              variant="ghost"
              onClick={() => bulkActive.mutate(false)}
              disabled={bulkActive.isPending}
            >
              비활성으로
            </Button>
            <Button
              variant="ghost"
              onClick={() => bulkActive.mutate(true)}
              disabled={bulkActive.isPending}
            >
              활성으로
            </Button>
            <button type="button" className={s.bulkClear} onClick={() => setSelected(new Set())}>
              선택 해제
            </button>
          </div>
        </div>
      )}

      <TeacherEditor
        open={editorOpen}
        teacher={editing}
        data={data}
        onClose={() => setEditorOpen(false)}
        onSaved={afterChange}
      />
      <QuickHomeroomModal
        open={quickOpen}
        data={data}
        onClose={() => setQuickOpen(false)}
        onSaved={afterChange}
      />
      <PasteRosterModal
        open={pasteOpen}
        onClose={() => setPasteOpen(false)}
        onSaved={afterChange}
      />
    </div>
  )
}
