import { invoke as tauriInvoke } from '@tauri-apps/api/core'

/** Rust `AppError`와 1:1 대응 */
export interface AppError {
  code: string
  userMessage: string
  detail?: string | null
}

export function isAppError(e: unknown): e is AppError {
  return (
    typeof e === 'object' &&
    e !== null &&
    'code' in e &&
    'userMessage' in e &&
    typeof (e as AppError).userMessage === 'string'
  )
}

/** 화면에 그대로 보여줄 수 있는 문장을 뽑아낸다. */
export function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.userMessage
  if (e instanceof Error) return e.message
  return '예상하지 못한 문제가 발생했습니다.'
}

export function errorDetail(e: unknown): string | undefined {
  if (isAppError(e)) return e.detail ?? undefined
  if (e instanceof Error) return e.stack
  return undefined
}

/**
 * Tauri 명령 호출 래퍼.
 * Rust 쪽 오류는 항상 AppError 형태로 정규화해서 던진다.
 */
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args)
  } catch (e) {
    if (isAppError(e)) throw e
    const err: AppError = {
      code: 'UNKNOWN',
      userMessage: '프로그램에서 예상하지 못한 문제가 발생했습니다. 다시 시도해 주세요.',
      detail: typeof e === 'string' ? e : JSON.stringify(e),
    }
    throw err
  }
}
