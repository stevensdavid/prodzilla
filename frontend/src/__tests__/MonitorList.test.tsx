import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'
import MonitorList from '../pages/MonitorList'
import * as api from '../api'

vi.mock('../api')

const mockedApi = vi.mocked(api)

function renderWithProviders(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>
  )
}

beforeEach(() => {
  vi.resetAllMocks()
})

describe('MonitorList', () => {
  it('shows loading state', () => {
    mockedApi.listMonitors.mockReturnValue(new Promise(() => {})) // never resolves
    mockedApi.listMonitorSummaries.mockReturnValue(new Promise(() => {}))

    renderWithProviders(<MonitorList />)
    expect(screen.getByText('Loading monitors...')).toBeInTheDocument()
  })

  it('renders monitor table with data', async () => {
    mockedApi.listMonitors.mockResolvedValue({
      monitors: [
        {
          name: 'health-check',
          monitor: {
            name: 'health-check',
            url: 'https://example.com',
            http_method: 'GET',
            schedule: { initial_delay: 0, interval: 30 },
            tags: { team: 'backend', tier: 'sev0' },
          },
          version: 1,
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
    })
    mockedApi.listMonitorSummaries.mockResolvedValue([
      { name: 'health-check', status: 'OK', last_probed: '2024-06-15T10:30:00Z' },
    ])

    renderWithProviders(<MonitorList />)

    await waitFor(() => {
      expect(screen.getByText('health-check')).toBeInTheDocument()
    })

    expect(screen.getByText('OK')).toBeInTheDocument()
    expect(screen.getByText('Single-step')).toBeInTheDocument()
    expect(screen.getByText('30s')).toBeInTheDocument()
    // Tags appear in both the filter bar and the table
    expect(screen.getAllByText('team:backend').length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText('tier:sev0').length).toBeGreaterThanOrEqual(1)
  })

  it('shows Create Monitor link', async () => {
    mockedApi.listMonitors.mockResolvedValue({ monitors: [] })
    mockedApi.listMonitorSummaries.mockResolvedValue([])

    renderWithProviders(<MonitorList />)

    await waitFor(() => {
      expect(screen.getByText('Create Monitor')).toBeInTheDocument()
    })
    expect(screen.getByText('Create Monitor').closest('a')).toHaveAttribute(
      'href',
      '/monitors/new'
    )
  })

  it('filters monitors by tag', async () => {
    mockedApi.listMonitors.mockResolvedValue({
      monitors: [
        {
          name: 'mon-a',
          monitor: {
            name: 'mon-a',
            url: 'https://a.com',
            http_method: 'GET',
            schedule: { initial_delay: 0, interval: 60 },
            tags: { team: 'alpha' },
          },
          version: 1,
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
        {
          name: 'mon-b',
          monitor: {
            name: 'mon-b',
            url: 'https://b.com',
            http_method: 'GET',
            schedule: { initial_delay: 0, interval: 60 },
            tags: { team: 'beta' },
          },
          version: 1,
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
    })
    mockedApi.listMonitorSummaries.mockResolvedValue([])

    renderWithProviders(<MonitorList />)

    await waitFor(() => {
      expect(screen.getByText('mon-a')).toBeInTheDocument()
      expect(screen.getByText('mon-b')).toBeInTheDocument()
    })

    // Click the tag filter for team:alpha (first occurrence is the filter chip)
    const user = userEvent.setup()
    const alphaChips = screen.getAllByText('team:alpha')
    await user.click(alphaChips[0])

    // mon-b should be filtered out
    expect(screen.getByText('mon-a')).toBeInTheDocument()
    expect(screen.queryByText('mon-b')).not.toBeInTheDocument()

    // Click clear
    await user.click(screen.getByText('Clear'))
    expect(screen.getByText('mon-b')).toBeInTheDocument()
  })

  it('shows error state', async () => {
    mockedApi.listMonitors.mockRejectedValue(new Error('Network error'))
    mockedApi.listMonitorSummaries.mockResolvedValue([])

    renderWithProviders(<MonitorList />)

    await waitFor(() => {
      expect(screen.getByText(/Failed to load monitors/)).toBeInTheDocument()
    })
  })

  it('shows scripted monitors in list', async () => {
    mockedApi.listMonitors.mockResolvedValue({
      monitors: [
        {
          name: 'scripted-mon',
          monitor: {
            name: 'scripted-mon',
            script: 'let x = 1;',
            script_timeout_seconds: 30,
            schedule: { initial_delay: 0, interval: 60 },
          },
          version: 1,
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
    })
    mockedApi.listMonitorSummaries.mockResolvedValue([
      { name: 'scripted-mon', status: 'OK', last_probed: '2024-06-15T10:30:00Z' },
    ])

    renderWithProviders(<MonitorList />)

    await waitFor(() => {
      expect(screen.getByText('scripted-mon')).toBeInTheDocument()
    })
    expect(screen.getByText('Scripted')).toBeInTheDocument()
  })
})
