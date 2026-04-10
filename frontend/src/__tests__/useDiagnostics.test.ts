import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useDiagnostics } from '../components/scripting/useDiagnostics'
import * as api from '../api'

vi.mock('../api')
const mockedApi = vi.mocked(api)

describe('useDiagnostics', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.resetAllMocks()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('returns empty diagnostics initially', () => {
    const { result } = renderHook(() => useDiagnostics(''))
    expect(result.current).toEqual([])
  })

  it('does not validate empty string', async () => {
    renderHook(() => useDiagnostics(''))
    await act(async () => {
      vi.advanceTimersByTime(500)
    })
    expect(mockedApi.validateScript).not.toHaveBeenCalled()
  })

  it('calls validate after 300ms debounce', async () => {
    mockedApi.validateScript.mockResolvedValue({
      valid: true,
      diagnostics: [],
    })

    renderHook(() => useDiagnostics('let x = 1;'))

    await act(async () => {
      vi.advanceTimersByTime(300)
    })

    expect(mockedApi.validateScript).toHaveBeenCalledTimes(1)
    expect(mockedApi.validateScript).toHaveBeenCalledWith('let x = 1;')
  })

  it('does not call validate before 300ms', async () => {
    mockedApi.validateScript.mockResolvedValue({
      valid: true,
      diagnostics: [],
    })

    renderHook(() => useDiagnostics('let x = 1;'))

    await act(async () => {
      vi.advanceTimersByTime(200)
    })

    expect(mockedApi.validateScript).not.toHaveBeenCalled()
  })

  it('cancels previous request on new input', async () => {
    mockedApi.validateScript.mockResolvedValue({
      valid: true,
      diagnostics: [],
    })

    const { rerender } = renderHook(({ script }) => useDiagnostics(script), {
      initialProps: { script: 'a' },
    })

    await act(async () => {
      vi.advanceTimersByTime(100)
    })

    rerender({ script: 'ab' })

    await act(async () => {
      vi.advanceTimersByTime(300)
    })

    // Only the second script value should have been validated
    expect(mockedApi.validateScript).toHaveBeenCalledTimes(1)
    expect(mockedApi.validateScript).toHaveBeenCalledWith('ab')
  })

  it('returns diagnostics from backend', async () => {
    const diag = {
      start_line: 1,
      start_column: 5,
      end_line: 1,
      end_column: 5,
      message: 'unexpected token',
      severity: 8,
    }
    mockedApi.validateScript.mockResolvedValue({
      valid: false,
      diagnostics: [diag],
    })

    const { result } = renderHook(() => useDiagnostics('let x = !'))

    await act(async () => {
      vi.advanceTimersByTime(300)
    })

    expect(result.current).toEqual([diag])
  })

  it('silently degrades on network error', async () => {
    mockedApi.validateScript.mockRejectedValue(new Error('Network error'))

    const { result } = renderHook(() => useDiagnostics('let x = 1;'))

    await act(async () => {
      vi.advanceTimersByTime(300)
    })

    expect(result.current).toEqual([])
  })
})
