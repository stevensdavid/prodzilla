import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Routes, Route } from 'react-router'
import MonitorDetail from '../pages/MonitorDetail'
import * as api from '../api'

vi.mock('../api')

const mockedApi = vi.mocked(api)

function renderWithRoute(name: string) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[`/monitors/${name}`]}>
        <Routes>
          <Route path="/monitors/:name" element={<MonitorDetail />} />
          <Route path="/" element={<div>Home</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  )
}

beforeEach(() => {
  vi.resetAllMocks()
})

describe('MonitorDetail', () => {
  const mockMonitorData = {
    monitor: {
      name: 'test-mon',
      monitor: {
        name: 'test-mon',
        url: 'https://example.com',
        http_method: 'GET',
        schedule: { initial_delay: 0, interval: 60 },
        expectations: [{ field: 'StatusCode' as const, operation: 'Equals' as const, value: '200' }],
        tags: { team: 'backend' },
      },
      version: 1,
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
    },
    version: 1,
  }

  it('renders monitor config summary', async () => {
    mockedApi.getMonitor.mockResolvedValue(mockMonitorData)
    mockedApi.getMonitorResults.mockResolvedValue([])

    renderWithRoute('test-mon')

    await waitFor(() => {
      expect(screen.getByText('test-mon')).toBeInTheDocument()
    })

    expect(screen.getByText('Single-step')).toBeInTheDocument()
    expect(screen.getByText(/every 60s/)).toBeInTheDocument()
    expect(screen.getByText('GET https://example.com')).toBeInTheDocument()
    expect(screen.getByText('team:backend')).toBeInTheDocument()
  })

  it('renders results timeline', async () => {
    mockedApi.getMonitor.mockResolvedValue(mockMonitorData)
    mockedApi.getMonitorResults.mockResolvedValue([
      {
        monitor_name: 'test-mon',
        timestamp_started: '2024-06-15T10:30:00Z',
        success: true,
        step_results: [
          {
            step_name: 'test-mon',
            timestamp_started: '2024-06-15T10:30:00Z',
            success: true,
            response: {
              timestamp_received: '2024-06-15T10:30:01Z',
              status_code: 200,
              body: 'OK',
              sensitive: false,
            },
          },
        ],
      },
      {
        monitor_name: 'test-mon',
        timestamp_started: '2024-06-15T10:29:00Z',
        success: false,
        step_results: [
          {
            step_name: 'test-mon',
            timestamp_started: '2024-06-15T10:29:00Z',
            success: false,
            error_message: 'Connection timeout',
          },
        ],
      },
    ])

    renderWithRoute('test-mon')

    await waitFor(() => {
      expect(screen.getByText('Pass')).toBeInTheDocument()
      expect(screen.getByText('Fail')).toBeInTheDocument()
    })
  })

  it('triggers monitor on button click', async () => {
    mockedApi.getMonitor.mockResolvedValue(mockMonitorData)
    mockedApi.getMonitorResults.mockResolvedValue([])
    mockedApi.triggerMonitor.mockResolvedValue({
      monitor_name: 'test-mon',
      timestamp_started: '2024-06-15T10:30:00Z',
      success: true,
      step_results: [],
    })

    renderWithRoute('test-mon')
    const user = userEvent.setup()

    await waitFor(() => {
      expect(screen.getByText('Trigger Now')).toBeInTheDocument()
    })

    await user.click(screen.getByText('Trigger Now'))

    expect(mockedApi.triggerMonitor).toHaveBeenCalledWith('test-mon')
  })

  it('shows delete confirmation dialog', async () => {
    mockedApi.getMonitor.mockResolvedValue(mockMonitorData)
    mockedApi.getMonitorResults.mockResolvedValue([])

    renderWithRoute('test-mon')
    const user = userEvent.setup()

    await waitFor(() => {
      expect(screen.getByText('Delete')).toBeInTheDocument()
    })

    await user.click(screen.getByText('Delete'))
    expect(screen.getByText(/Are you sure you want to delete/)).toBeInTheDocument()

    // Cancel
    await user.click(screen.getByText('Cancel'))
    expect(screen.queryByText(/Are you sure/)).not.toBeInTheDocument()
  })

  it('deletes monitor and navigates home', async () => {
    mockedApi.getMonitor.mockResolvedValue(mockMonitorData)
    mockedApi.getMonitorResults.mockResolvedValue([])
    mockedApi.deleteMonitor.mockResolvedValue(undefined)

    renderWithRoute('test-mon')
    const user = userEvent.setup()

    await waitFor(() => {
      expect(screen.getByText('Delete')).toBeInTheDocument()
    })

    await user.click(screen.getByText('Delete'))
    await user.click(screen.getAllByText('Delete')[1]) // The confirm button

    await waitFor(() => {
      expect(mockedApi.deleteMonitor).toHaveBeenCalledWith('test-mon')
    })
  })

  it('shows edit link', async () => {
    mockedApi.getMonitor.mockResolvedValue(mockMonitorData)
    mockedApi.getMonitorResults.mockResolvedValue([])

    renderWithRoute('test-mon')

    await waitFor(() => {
      expect(screen.getByText('Edit')).toBeInTheDocument()
    })

    expect(screen.getByText('Edit').closest('a')).toHaveAttribute(
      'href',
      '/monitors/test-mon/edit'
    )
  })

  it('shows loading state', () => {
    mockedApi.getMonitor.mockReturnValue(new Promise(() => {}))
    mockedApi.getMonitorResults.mockReturnValue(new Promise(() => {}))

    renderWithRoute('test-mon')
    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  it('shows error state', async () => {
    mockedApi.getMonitor.mockRejectedValue(new Error('Not found'))
    mockedApi.getMonitorResults.mockResolvedValue([])

    renderWithRoute('test-mon')

    await waitFor(() => {
      expect(screen.getByText(/Failed to load monitor/)).toBeInTheDocument()
    })
  })

  it('displays Scripted type for scripted monitor', async () => {
    const scriptedData = {
      monitor: {
        name: 'scripted-mon',
        monitor: {
          name: 'scripted-mon',
          script: 'let x = 1;\nassert(true, "ok");',
          script_timeout_seconds: 45,
          schedule: { initial_delay: 0, interval: 60 },
        },
        version: 1,
        created_at: '2024-01-01T00:00:00Z',
        updated_at: '2024-01-01T00:00:00Z',
      },
      version: 1,
    }
    mockedApi.getMonitor.mockResolvedValue(scriptedData)
    mockedApi.getMonitorResults.mockResolvedValue([])

    renderWithRoute('scripted-mon')

    await waitFor(() => {
      expect(screen.getByText('scripted-mon')).toBeInTheDocument()
    })
    expect(screen.getByText('Scripted')).toBeInTheDocument()
  })

  it('displays script timeout for scripted monitor', async () => {
    const scriptedData = {
      monitor: {
        name: 'scripted-mon',
        monitor: {
          name: 'scripted-mon',
          script: 'let x = 1;',
          script_timeout_seconds: 45,
          schedule: { initial_delay: 0, interval: 60 },
        },
        version: 1,
        created_at: '2024-01-01T00:00:00Z',
        updated_at: '2024-01-01T00:00:00Z',
      },
      version: 1,
    }
    mockedApi.getMonitor.mockResolvedValue(scriptedData)
    mockedApi.getMonitorResults.mockResolvedValue([])

    renderWithRoute('scripted-mon')

    await waitFor(() => {
      expect(screen.getByText(/Timeout: 45s/)).toBeInTheDocument()
    })
  })
})
