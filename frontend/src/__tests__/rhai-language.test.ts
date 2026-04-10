import { describe, it, expect } from 'vitest'
import {
  RHAI_LANGUAGE_ID,
  rhaiLanguageConfig,
  rhaiTokenProvider,
} from '../components/scripting/rhai-language'

describe('rhai-language', () => {
  it('exports language id as rhai', () => {
    expect(RHAI_LANGUAGE_ID).toBe('rhai')
  })

  it('token provider has root state', () => {
    expect(rhaiTokenProvider.tokenizer.root).toBeDefined()
    expect(Array.isArray(rhaiTokenProvider.tokenizer.root)).toBe(true)
    expect(rhaiTokenProvider.tokenizer.root.length).toBeGreaterThan(0)
  })

  it('keywords include all Rhai keywords', () => {
    const keywords = rhaiTokenProvider.keywords as string[]
    const expected = [
      'let',
      'if',
      'else',
      'fn',
      'return',
      'true',
      'false',
      'for',
      'while',
      'loop',
    ]
    for (const kw of expected) {
      expect(keywords).toContain(kw)
    }
  })

  it('language config has comment tokens', () => {
    expect(rhaiLanguageConfig.comments?.lineComment).toBe('//')
    expect(rhaiLanguageConfig.comments?.blockComment).toEqual(['/*', '*/'])
  })

  it('language config has bracket pairs', () => {
    const brackets = rhaiLanguageConfig.brackets as Array<[string, string]>
    expect(brackets).toContainEqual(['(', ')'])
    expect(brackets).toContainEqual(['{', '}'])
    expect(brackets).toContainEqual(['[', ']'])
  })

  it('language config has auto-closing pairs', () => {
    const pairs = rhaiLanguageConfig.autoClosingPairs as Array<{
      open: string
      close: string
    }>
    const opens = pairs.map((p) => p.open)
    expect(opens).toContain('{')
    expect(opens).toContain('[')
    expect(opens).toContain('(')
    expect(opens).toContain('"')
  })
})
