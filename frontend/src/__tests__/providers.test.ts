/* eslint-disable @typescript-eslint/no-explicit-any */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import {
  registerCompletionProvider,
  registerHoverProvider,
} from '../components/scripting/providers'
import * as api from '../api'

vi.mock('../api')
const mockedApi = vi.mocked(api)

function createMockMonaco() {
  let completionProvider: any = null
  let hoverProvider: any = null

  return {
    languages: {
      registerCompletionItemProvider: vi.fn((_langId: string, provider: any) => {
        completionProvider = provider
        return { dispose: vi.fn() }
      }),
      registerHoverProvider: vi.fn((_langId: string, provider: any) => {
        hoverProvider = provider
        return { dispose: vi.fn() }
      }),
      CompletionItemKind: {
        Function: 1,
        Property: 9,
        Text: 0,
      },
      CompletionItemInsertTextRule: { InsertAsSnippet: 4 },
    },
    getCompletionProvider: () => completionProvider,
    getHoverProvider: () => hoverProvider,
  }
}

beforeEach(() => {
  vi.resetAllMocks()
})

describe('registerCompletionProvider', () => {
  it('registers with language id rhai', () => {
    const monaco = createMockMonaco()
    registerCompletionProvider(monaco as any)
    expect(
      monaco.languages.registerCompletionItemProvider
    ).toHaveBeenCalledWith('rhai', expect.any(Object))
  })

  it('returns a disposable', () => {
    const monaco = createMockMonaco()
    const disposable = registerCompletionProvider(monaco as any)
    expect(disposable).toHaveProperty('dispose')
  })

  it('maps API items to Monaco format', async () => {
    const mockItems = [
      {
        label: 'http_get',
        kind: 'function',
        detail: 'http_get(url: &str) -> ScriptResponse',
        documentation: 'Makes an HTTP GET request',
        insert_text: 'http_get("${1:url}")',
        insert_text_rules: 4,
      },
      {
        label: 'status',
        kind: 'property',
        detail: 'status: i64',
      },
    ]
    mockedApi.getScriptingCompletions.mockResolvedValue({ items: mockItems })

    const monaco = createMockMonaco()
    registerCompletionProvider(monaco as any)

    const provider = monaco.getCompletionProvider()
    const model = {
      getWordUntilPosition: () => ({ startColumn: 1, endColumn: 5 }),
    }
    const position = { lineNumber: 1, column: 5 }

    const result = await provider.provideCompletionItems(model, position)

    expect(result.suggestions).toHaveLength(2)
    expect(result.suggestions[0].label).toBe('http_get')
    expect(result.suggestions[0].kind).toBe(1) // Function
    expect(result.suggestions[0].insertText).toBe('http_get("${1:url}")')
    expect(result.suggestions[0].insertTextRules).toBe(4) // InsertAsSnippet
    expect(result.suggestions[1].label).toBe('status')
    expect(result.suggestions[1].kind).toBe(9) // Property
  })
})

describe('registerHoverProvider', () => {
  it('registers with language id rhai', () => {
    const monaco = createMockMonaco()
    registerHoverProvider(monaco as any)
    expect(monaco.languages.registerHoverProvider).toHaveBeenCalledWith(
      'rhai',
      expect.any(Object)
    )
  })

  it('returns a disposable', () => {
    const monaco = createMockMonaco()
    const disposable = registerHoverProvider(monaco as any)
    expect(disposable).toHaveProperty('dispose')
  })

  it('returns docs for known function', async () => {
    const mockItems = [
      {
        label: 'http_get',
        kind: 'function',
        detail: 'http_get(url: &str) -> ScriptResponse',
        documentation: 'Makes an HTTP GET request',
      },
    ]
    mockedApi.getScriptingCompletions.mockResolvedValue({ items: mockItems })

    const monaco = createMockMonaco()
    registerHoverProvider(monaco as any)

    const provider = monaco.getHoverProvider()
    const model = {
      getWordAtPosition: () => ({
        word: 'http_get',
        startColumn: 1,
        endColumn: 9,
      }),
    }
    const position = { lineNumber: 1, column: 5 }

    const result = await provider.provideHover(model, position)

    expect(result).not.toBeNull()
    expect(result.contents).toContainEqual({
      value: '**http_get(url: &str) -> ScriptResponse**',
    })
    expect(result.contents).toContainEqual({
      value: 'Makes an HTTP GET request',
    })
  })

  it('returns null for unknown word', async () => {
    mockedApi.getScriptingCompletions.mockResolvedValue({ items: [] })

    const monaco = createMockMonaco()
    registerHoverProvider(monaco as any)

    const provider = monaco.getHoverProvider()
    const model = {
      getWordAtPosition: () => ({
        word: 'foobar',
        startColumn: 1,
        endColumn: 7,
      }),
    }
    const position = { lineNumber: 1, column: 3 }

    const result = await provider.provideHover(model, position)
    expect(result).toBeNull()
  })

  it('returns null when no word at position', async () => {
    const monaco = createMockMonaco()
    registerHoverProvider(monaco as any)

    const provider = monaco.getHoverProvider()
    const model = {
      getWordAtPosition: () => null,
    }
    const position = { lineNumber: 1, column: 1 }

    const result = await provider.provideHover(model, position)
    expect(result).toBeNull()
  })
})
