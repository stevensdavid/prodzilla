import { useEffect, useRef } from 'react'
import Editor, { type Monaco } from '@monaco-editor/react'
import type { editor as monacoEditor } from 'monaco-editor'
import {
  RHAI_LANGUAGE_ID,
  rhaiLanguageConfig,
  rhaiTokenProvider,
} from './rhai-language'
import { registerCompletionProvider, registerHoverProvider } from './providers'
import { useDiagnostics } from './useDiagnostics'
import type { ScriptDiagnostic } from '../../types'

interface ScriptEditorProps {
  value: string
  onChange: (value: string) => void
}

let languageRegistered = false

function setMarkers(
  monaco: Monaco,
  editorInstance: monacoEditor.IStandaloneCodeEditor,
  diagnostics: ScriptDiagnostic[]
) {
  const model = editorInstance.getModel()
  if (!model) return

  const markers = diagnostics.map((d) => ({
    startLineNumber: d.start_line,
    startColumn: d.start_column,
    endLineNumber: d.end_line,
    endColumn: d.end_column,
    message: d.message,
    severity:
      d.severity === 8
        ? monaco.MarkerSeverity.Error
        : d.severity === 4
          ? monaco.MarkerSeverity.Warning
          : monaco.MarkerSeverity.Info,
  }))

  monaco.editor.setModelMarkers(model, 'rhai', markers)
}

export default function ScriptEditor({ value, onChange }: ScriptEditorProps) {
  const monacoRef = useRef<Monaco | null>(null)
  const editorRef = useRef<monacoEditor.IStandaloneCodeEditor | null>(null)
  const diagnostics = useDiagnostics(value)

  useEffect(() => {
    if (monacoRef.current && editorRef.current) {
      setMarkers(monacoRef.current, editorRef.current, diagnostics)
    }
  }, [diagnostics])

  function handleEditorWillMount(monaco: Monaco) {
    if (!languageRegistered) {
      monaco.languages.register({ id: RHAI_LANGUAGE_ID })
      monaco.languages.setMonarchTokensProvider(
        RHAI_LANGUAGE_ID,
        rhaiTokenProvider
      )
      monaco.languages.setLanguageConfiguration(
        RHAI_LANGUAGE_ID,
        rhaiLanguageConfig
      )
      languageRegistered = true
    }
  }

  function handleEditorDidMount(
    editor: monacoEditor.IStandaloneCodeEditor,
    monaco: Monaco
  ) {
    monacoRef.current = monaco
    editorRef.current = editor

    const d1 = registerCompletionProvider(monaco)
    const d2 = registerHoverProvider(monaco)

    editor.onDidDispose(() => {
      d1.dispose()
      d2.dispose()
    })

    // Set initial markers if diagnostics already available
    setMarkers(monaco, editor, diagnostics)
  }

  return (
    <Editor
      height="400px"
      language={RHAI_LANGUAGE_ID}
      theme="vs-dark"
      value={value}
      onChange={(v) => onChange(v ?? '')}
      beforeMount={handleEditorWillMount}
      onMount={handleEditorDidMount}
      options={{
        minimap: { enabled: false },
        lineNumbers: 'on',
        bracketPairColorization: { enabled: true },
        fixedOverflowWidgets: true,
        scrollBeyondLastLine: false,
        fontSize: 14,
        tabSize: 4,
      }}
    />
  )
}
