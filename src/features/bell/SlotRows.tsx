import { Icon } from '@/components/Icon'
import { TimeInput } from '@/components/TimeInput'
import {
  addLunch,
  addPeriod,
  addRecess,
  canMove,
  hasLunch,
  maxPeriod,
  moveRow,
  patchRow,
  removeRow,
  rowLabel,
  type Lengths,
  type Row,
} from './bellModel'
import s from './SlotRows.module.css'

interface Props {
  rows: Row[]
  onChange: (rows: Row[]) => void
  /**
   * 점심·중간놀이를 새로 넣을 때 쓸 길이.
   *
   * 위쪽 '시간 일괄 조정'에서 정한 값과 같은 것을 받는다. 여기서 따로
   * 기본값을 두면 사용자가 20분으로 바꿔도 30분이 들어가게 된다.
   */
  lengths: Lengths
  /** 이 표에서 지울 수 있는 교시 번호(마지막 교시만) */
  allowDeletePeriod?: boolean
}

/** 교시·점심·중간놀이 시각을 한 줄씩 편집하는 표. */
export function SlotRows({ rows, onChange, lengths, allowDeletePeriod = true }: Props) {
  const sorted = [...rows].sort((a, b) => a.startMin - b.startMin)
  const lastPeriodKey = sorted.filter((r) => r.kind === 'PERIOD').pop()?.key

  return (
    <div className={s.wrap}>
      {sorted.length === 0 && (
        <p className={s.empty}>아직 교시가 없습니다. 아래 [교시 추가]를 눌러 시작하세요.</p>
      )}

      {sorted.map((r) => {
        const isLunch = r.kind === 'LUNCH'
        const isOther = r.kind === 'OTHER'
        const isBreak = isLunch || isOther
        const canDelete = isBreak || (allowDeletePeriod && r.key === lastPeriodKey)
        const rowClass = [s.row, isLunch ? s.rowLunch : '', isOther ? s.rowOther : '']
          .filter(Boolean)
          .join(' ')

        return (
          <div key={r.key} className={rowClass}>
            {/* 점심·중간놀이는 순서를 바꿀 수 있다 */}
            {isBreak ? (
              <span className={s.moveCol}>
                <button
                  type="button"
                  className={s.moveBtn}
                  disabled={!canMove(rows, r.key, -1)}
                  onClick={() => onChange(moveRow(rows, r.key, -1))}
                  aria-label={`${rowLabel(r)} 앞으로 옮기기`}
                  title="앞 교시와 자리 바꾸기"
                >
                  ▲
                </button>
                <button
                  type="button"
                  className={s.moveBtn}
                  disabled={!canMove(rows, r.key, 1)}
                  onClick={() => onChange(moveRow(rows, r.key, 1))}
                  aria-label={`${rowLabel(r)} 뒤로 옮기기`}
                  title="뒤 교시와 자리 바꾸기"
                >
                  ▼
                </button>
              </span>
            ) : (
              <span className={s.moveCol} />
            )}

            {isOther ? (
              <input
                className={s.nameInput}
                value={r.name ?? ''}
                placeholder="중간놀이"
                maxLength={12}
                onChange={(e) => onChange(patchRow(rows, r.key, { name: e.target.value }))}
                aria-label="시간 구간 이름"
              />
            ) : (
              <span className={s.label}>
                {isLunch && <span className={s.lunchDot} aria-hidden="true" />}
                {rowLabel(r)}
              </span>
            )}

            <TimeInput
              value={r.startMin}
              onChange={(v) => onChange(patchRow(rows, r.key, { startMin: v }))}
              aria-label={`${rowLabel(r)} 시작 시각`}
            />
            <span className={s.tilde}>~</span>
            <TimeInput
              value={r.endMin}
              onChange={(v) => onChange(patchRow(rows, r.key, { endMin: v }))}
              aria-label={`${rowLabel(r)} 종료 시각`}
            />

            <span className={s.duration}>
              {r.endMin > r.startMin ? `${r.endMin - r.startMin}분` : '—'}
            </span>

            {canDelete ? (
              <button
                type="button"
                className={s.del}
                onClick={() => onChange(removeRow(rows, r.key))}
                aria-label={`${rowLabel(r)} 지우기`}
                title={isBreak ? '지우고 뒤 교시를 앞으로 당기기' : '마지막 교시 지우기'}
              >
                ×
              </button>
            ) : (
              <span className={s.delSpacer} />
            )}
          </div>
        )
      })}

      <div className={s.actions}>
        <button type="button" className={s.addBtn} onClick={() => onChange(addPeriod(rows, lengths))}>
          <Icon name="plus" size={15} /> 교시 추가
        </button>
        {!hasLunch(rows) && (
          <button
            type="button"
            className={s.addBtn}
            onClick={() => onChange(addLunch(rows, Math.max(1, maxPeriod(rows) - 1), lengths.lunch))}
          >
            <Icon name="plus" size={15} /> 점심시간 추가 ({lengths.lunch}분)
          </button>
        )}
        <button
          type="button"
          className={s.addBtn}
          onClick={() => onChange(addRecess(rows, Math.min(2, maxPeriod(rows)), lengths.recess))}
          title={`중간놀이, 아침활동처럼 수업이 아닌 시간 구간을 ${lengths.recess}분으로 넣습니다`}
        >
          <Icon name="plus" size={15} /> 중간놀이 추가 ({lengths.recess}분)
        </button>
      </div>

      {sorted.some((r) => r.kind !== 'PERIOD') && (
        <p className={s.hint}>
          점심·중간놀이 왼쪽의 <strong>▲ ▼</strong> 로 앞뒤 교시와 자리를 바꿀 수 있습니다. 넣거나
          지우면 뒤 교시 시각이 자동으로 따라 움직입니다.
        </p>
      )}
    </div>
  )
}
