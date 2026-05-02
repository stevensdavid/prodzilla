import type { Expectation } from '../types'

export interface StepFormState {
  name: string
  url: string
  httpMethod: string
  headers: Array<{ key: string; value: string }>
  body: string
  timeoutSeconds: string
  sensitive: boolean
  expectations: Expectation[]
}

export function emptyStepFormState(): StepFormState {
  return {
    name: '',
    url: '',
    httpMethod: 'GET',
    headers: [],
    body: '',
    timeoutSeconds: '',
    sensitive: false,
    expectations: [],
  }
}
