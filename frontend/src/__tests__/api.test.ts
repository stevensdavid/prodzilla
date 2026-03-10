import { describe, it, expect, vi, beforeEach } from 'vitest'
import {
  listMonitors,
  listMonitorSummaries,
  getMonitor,
  createMonitor,
  updateMonitor,
  deleteMonitor,
  getMonitorResults,
  triggerMonitor,
} from '../api'
import { ApiRequestError } from '../types'
import type { Monitor, MonitorListResponse, MonitorApiResponse } from '../types'

const mockFetch = vi.fn()
vi.stubGlobal('fetch', mockFetch)

function jsonResponse(data: unknown, status = 200, headers: Record<string, string> = {}) {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(data),
    headers: new Headers(headers),
  }
}

function testMonitor(name: string): Monitor {
  return {
    name,
    url: 'https://example.com',
    http_method: 'GET',
    schedule: { initial_delay: 0, interval: 60 },
  }
}

function testMonitorApiResponse(name: string): MonitorApiResponse {
  return {
    name,
    monitor: testMonitor(name),
    version: 1,
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
  }
}

beforeEach(() => {
  mockFetch.mockReset()
})

describe('listMonitors', () => {
  it('fetches and returns monitor list', async () => {
    const data: MonitorListResponse = {
      monitors: [testMonitorApiResponse('test-1')],
    }
    mockFetch.mockResolvedValue(jsonResponse(data))

    const result = await listMonitors()
    expect(result.monitors).toHaveLength(1)
    expect(result.monitors[0].name).toBe('test-1')
    expect(mockFetch).toHaveBeenCalledWith('/api/v1/monitors', undefined)
  })
})

describe('listMonitorSummaries', () => {
  it('fetches summaries', async () => {
    const data = [{ name: 'test-1', status: 'OK', last_probed: '2024-01-01T00:00:00Z' }]
    mockFetch.mockResolvedValue(jsonResponse(data))

    const result = await listMonitorSummaries()
    expect(result).toHaveLength(1)
    expect(result[0].status).toBe('OK')
    expect(mockFetch).toHaveBeenCalledWith('/api/v1/monitors/summary', undefined)
  })
})

describe('getMonitor', () => {
  it('fetches monitor and extracts version from etag', async () => {
    const data = testMonitorApiResponse('test-1')
    mockFetch.mockResolvedValue(jsonResponse(data, 200, { etag: '3' }))

    const result = await getMonitor('test-1')
    expect(result.monitor.name).toBe('test-1')
    expect(result.version).toBe(3)
  })

  it('URL-encodes the monitor name', async () => {
    const data = testMonitorApiResponse('my monitor')
    mockFetch.mockResolvedValue(jsonResponse(data, 200, { etag: '1' }))

    await getMonitor('my monitor')
    expect(mockFetch).toHaveBeenCalledWith('/api/v1/monitors/my%20monitor')
  })

  it('throws ApiRequestError on 404', async () => {
    mockFetch.mockResolvedValue(
      jsonResponse({ error: 'not_found', message: 'Monitor not found: x' }, 404)
    )

    await expect(getMonitor('x')).rejects.toThrow(ApiRequestError)
    await expect(getMonitor('x')).rejects.toMatchObject({ status: 404 })
  })
})

describe('createMonitor', () => {
  it('sends POST with monitor JSON', async () => {
    const monitor = testMonitor('new-mon')
    const response = testMonitorApiResponse('new-mon')
    mockFetch.mockResolvedValue(jsonResponse(response, 201))

    const result = await createMonitor(monitor)
    expect(result.name).toBe('new-mon')

    const [url, options] = mockFetch.mock.calls[0]
    expect(url).toBe('/api/v1/monitors')
    expect(options.method).toBe('POST')
    expect(JSON.parse(options.body)).toEqual(monitor)
  })

  it('throws on 409 conflict', async () => {
    const monitor = testMonitor('dup-mon')
    mockFetch.mockResolvedValue(
      jsonResponse({ error: 'already_exists', message: 'Monitor already exists: dup-mon' }, 409)
    )

    await expect(createMonitor(monitor)).rejects.toThrow(ApiRequestError)
  })
})

describe('updateMonitor', () => {
  it('sends PUT with monitor and version', async () => {
    const monitor = testMonitor('upd-mon')
    const response = { ...testMonitorApiResponse('upd-mon'), version: 2 }
    mockFetch.mockResolvedValue(jsonResponse(response))

    const result = await updateMonitor('upd-mon', monitor, 1)
    expect(result.version).toBe(2)

    const [url, options] = mockFetch.mock.calls[0]
    expect(url).toBe('/api/v1/monitors/upd-mon')
    expect(options.method).toBe('PUT')
    expect(JSON.parse(options.body)).toEqual({ monitor, version: 1 })
  })
})

describe('deleteMonitor', () => {
  it('sends DELETE request', async () => {
    mockFetch.mockResolvedValue({ ok: true, status: 204, json: () => Promise.resolve(undefined) })

    await deleteMonitor('del-mon')
    const [url, options] = mockFetch.mock.calls[0]
    expect(url).toBe('/api/v1/monitors/del-mon')
    expect(options.method).toBe('DELETE')
  })
})

describe('getMonitorResults', () => {
  it('fetches results without show_response by default', async () => {
    mockFetch.mockResolvedValue(jsonResponse([]))

    await getMonitorResults('test-mon')
    expect(mockFetch).toHaveBeenCalledWith(
      '/api/v1/monitors/test-mon/results',
      undefined
    )
  })

  it('adds show_response param when true', async () => {
    mockFetch.mockResolvedValue(jsonResponse([]))

    await getMonitorResults('test-mon', true)
    expect(mockFetch).toHaveBeenCalledWith(
      '/api/v1/monitors/test-mon/results?show_response=true',
      undefined
    )
  })
})

describe('triggerMonitor', () => {
  it('sends POST to trigger endpoint', async () => {
    const result = {
      monitor_name: 'test-mon',
      timestamp_started: '2024-01-01T00:00:00Z',
      success: true,
      step_results: [],
    }
    mockFetch.mockResolvedValue(jsonResponse(result))

    const res = await triggerMonitor('test-mon')
    expect(res.success).toBe(true)

    const [url, options] = mockFetch.mock.calls[0]
    expect(url).toBe('/api/v1/monitors/test-mon/trigger')
    expect(options.method).toBe('POST')
  })
})
