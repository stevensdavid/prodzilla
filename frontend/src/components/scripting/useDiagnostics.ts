import { useEffect, useRef, useState } from 'react'
import { validateScript } from '../../api'
import type { ScriptDiagnostic } from '../../types'

const DEBOUNCE_MS = 300

export function useDiagnostics(script: string): ScriptDiagnostic[] {
  const [diagnostics, setDiagnostics] = useState<ScriptDiagnostic[]>([])
  const abortRef = useRef<AbortController | null>(null)

  useEffect(() => {
    if (!script.trim()) {
      // Use a zero-delay timeout to avoid synchronous setState in effect body
      const t = setTimeout(() => setDiagnostics([]), 0)
      return () => clearTimeout(t)
    }

    const timer = setTimeout(async () => {
      // Cancel previous in-flight request
      abortRef.current?.abort()
      const controller = new AbortController()
      abortRef.current = controller

      try {
        const result = await validateScript(script)
        if (!controller.signal.aborted) {
          setDiagnostics(result.diagnostics)
        }
      } catch {
        if (!controller.signal.aborted) {
          setDiagnostics([])
        }
      }
    }, DEBOUNCE_MS)

    return () => {
      clearTimeout(timer)
    }
  }, [script])

  return diagnostics
}
