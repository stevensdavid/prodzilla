import type { Monaco } from '@monaco-editor/react'
import type { IDisposable, editor, Position } from 'monaco-editor'
import { getScriptingCompletions } from '../../api'
import type { ScriptingCompletionItem } from '../../types'
import { RHAI_LANGUAGE_ID } from './rhai-language'

let cachedItems: ScriptingCompletionItem[] | null = null

async function fetchCompletions(): Promise<ScriptingCompletionItem[]> {
  if (cachedItems) return cachedItems
  try {
    const resp = await getScriptingCompletions()
    cachedItems = resp.items
    return cachedItems
  } catch {
    return []
  }
}

function mapKind(
  monaco: Monaco,
  kind: string
): number {
  switch (kind) {
    case 'function':
      return monaco.languages.CompletionItemKind.Function
    case 'property':
      return monaco.languages.CompletionItemKind.Property
    default:
      return monaco.languages.CompletionItemKind.Text
  }
}

export function registerCompletionProvider(monaco: Monaco): IDisposable {
  return monaco.languages.registerCompletionItemProvider(RHAI_LANGUAGE_ID, {
    provideCompletionItems: async (model: editor.ITextModel, position: Position) => {
      const items = await fetchCompletions()
      const word = model.getWordUntilPosition(position)
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      }

      return {
        suggestions: items.map((item) => ({
          label: item.label,
          kind: mapKind(monaco, item.kind),
          detail: item.detail,
          documentation: item.documentation
            ? { value: item.documentation, isTrusted: true }
            : undefined,
          insertText: item.insert_text ?? item.label,
          insertTextRules: item.insert_text_rules
            ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
            : undefined,
          range,
        })),
      }
    },
  })
}

export function registerHoverProvider(monaco: Monaco): IDisposable {
  return monaco.languages.registerHoverProvider(RHAI_LANGUAGE_ID, {
    provideHover: async (model: editor.ITextModel, position: Position) => {
      const word = model.getWordAtPosition(position)
      if (!word) return null

      const items = await fetchCompletions()
      const match = items.find(
        (item) => item.label === word.word && item.kind === 'function'
      )
      if (!match) return null

      const contents = [
        { value: `**${match.detail}**` },
        ...(match.documentation ? [{ value: match.documentation }] : []),
      ]

      return {
        range: {
          startLineNumber: position.lineNumber,
          endLineNumber: position.lineNumber,
          startColumn: word.startColumn,
          endColumn: word.endColumn,
        },
        contents,
      }
    },
  })
}
