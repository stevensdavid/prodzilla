/* eslint-disable @typescript-eslint/no-explicit-any */
import React from 'react'

interface EditorProps {
  value?: string
  onChange?: (value: string) => void
  onMount?: (editor: any, monaco: any) => void
  beforeMount?: (monaco: any) => void
}

const Editor = ({ value, onChange, onMount }: EditorProps) => {
  React.useEffect(() => {
    if (onMount) {
      const mockEditor = {
        getModel: () => ({ uri: 'mock-uri' }),
        onDidDispose: () => ({ dispose: () => {} }),
      }
      const mockMonaco = {
        languages: {
          register: () => {},
          setMonarchTokensProvider: () => {},
          setLanguageConfiguration: () => {},
          registerCompletionItemProvider: () => ({ dispose: () => {} }),
          registerHoverProvider: () => ({ dispose: () => {} }),
          CompletionItemKind: {
            Function: 1,
            Property: 9,
            Text: 0,
          },
          CompletionItemInsertTextRule: { InsertAsSnippet: 4 },
        },
        editor: {
          setModelMarkers: () => {},
        },
        MarkerSeverity: { Error: 8, Warning: 4, Info: 2, Hint: 1 },
      }
      onMount(mockEditor, mockMonaco)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <textarea
      data-testid="script-editor"
      value={value ?? ''}
      onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) =>
        onChange?.(e.target.value)
      }
    />
  )
}

export default Editor
