import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Modal } from '@/components/Modal'
import { Button, Notice } from '@/components/ui'
import { assignApi, type AssignSaved } from '@/ipc/assign'
import { DAY_LABEL, minToHm } from '@/ipc/bell'
import type { Candidate, FindResult } from '@/ipc/find'
import { errorDetail, errorMessage } from '@/ipc/invoke'
import s from './AssignConfirmModal.module.css'

interface Props {
  open: boolean
  result: FindResult
  candidate: Candidate
  onClose: () => void
  onSaved: (saved: AssignSaved) => void
}

/**
 * 단일 보결 배정 확인.
 *
 * 저장 버튼을 누르는 순간 백엔드가 후보 판정 엔진을 **다시** 돌린다.
 * 조회한 뒤 다른 보결이 먼저 들어갔거나 결근·시간표가 바뀌었다면
 * 저장하지 않고 이유를 알려 준다.
 */
export function AssignConfirmModal({ open, result, candidate, onClose, onSaved }: Props) {
  const qc = useQueryClient()
  const slot = result.slot

  // 이 시간을 원래 맡은 선생님이 곧 '대신 들어가 주는 대상'이다.
  // 따로 고르게 하지 않고 시간표에서 그대로 가져와 기록에 남긴다.
  const covered = slot.inCharge

  const save = useMutation({
    mutationFn: () =>
      assignApi.create({
        date: slot.date,
        classId: slot.classId,
        slotType: slot.slotType,
        periodNo: slot.periodNo,
        absentTeacherId: covered.teacherId,
        subTeacherId: candidate.teacherId,
      }),
    onSuccess: async (saved) => {
      await Promise.all([
        qc.invalidateQueries({ queryKey: ['assign-history'] }),
        qc.invalidateQueries({ queryKey: ['absences'] }),
      ])
      onSaved(saved)
    },
  })

  return (
    <Modal
      open={open}
      title="보결 배정 확인"
      description="아래 내용으로 배정합니다. 저장하는 순간 배정 가능 여부를 한 번 더 확인합니다."
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            취소
          </Button>
          <Button
            variant="primary"
            icon="check"
            disabled={save.isPending}
            onClick={() => save.mutate()}
          >
            {save.isPending ? '확인하는 중…' : '보결 배정'}
          </Button>
        </>
      }
    >
      {save.error && (
        <Notice tone="danger">
          <div>
            <strong>배정하지 못했습니다</strong>
            <p className={s.errText}>{errorMessage(save.error)}</p>
            {errorDetail(save.error) && (
              <details className={s.detail}>
                <summary>자세히</summary>
                <pre className="selectable">{errorDetail(save.error)}</pre>
              </details>
            )}
          </div>
        </Notice>
      )}

      {result.notice && (
        <div className={s.notice}>
          <p className={s.noticeTitle}>{result.notice.title}</p>
          <p className={s.noticeBody}>{result.notice.body}</p>
        </div>
      )}

      <dl className={s.sheet}>
        <dt>날짜</dt>
        <dd>
          <span className="num">{slot.date}</span>
          <span className={s.sub}>{DAY_LABEL[slot.dayOfWeek]}요일</span>
        </dd>

        <dt>대상 학급</dt>
        <dd>
          <strong>{slot.classFullLabel}</strong>
        </dd>

        <dt>시간</dt>
        <dd>
          <strong>{slot.slotLabel}</strong>
          <span className={`${s.sub} num`}>
            {minToHm(slot.startMin)} ~ {minToHm(slot.endMin)}
          </span>
        </dd>

        <dt>대신 들어감</dt>
        <dd>
          {covered.name ? (
            <>
              <strong>{covered.name}</strong>
              <span className={s.sub}>
                {[covered.roleLabel, covered.subjectName].filter(Boolean).join(' · ')} — 원래 이
                시간 담당
              </span>
            </>
          ) : (
            <span className={s.none}>담임이 지정되지 않은 학급입니다</span>
          )}
        </dd>

        <dt>배정 교사</dt>
        <dd>
          <strong className={s.pick}>{candidate.name}</strong>
          <span className={s.sub}>
            {candidate.roleLabel}
            {candidate.duty ? ` · ${candidate.duty}` : ''}
          </span>
        </dd>

        <dt>추천 순위</dt>
        <dd>
          <span className={candidate.rank === 1 ? `${s.rank} ${s.rank1}` : s.rank}>
            {candidate.rank > 0 ? `${candidate.rank}순위` : '순위 없음'}
          </span>
          {candidate.reason && <span className={s.reason}>{candidate.reason}</span>}
        </dd>

        <dt>보결 횟수</dt>
        <dd className="num">
          오늘 {candidate.counts.today}회 · 이번 달 {candidate.counts.month}회 · 누적{' '}
          {candidate.counts.total}회
        </dd>
      </dl>
    </Modal>
  )
}
