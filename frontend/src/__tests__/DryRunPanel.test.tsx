import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import DryRunPanel from '../components/scripting/DryRunPanel'
import * as api from '../api'

vi.mock('../api')
const mockedApi = vi.mocked(api)

function renderWithProviders(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>
  )
}

const successResult = {
  result: {
    monitor_name: 'dry-run',
    timestamp_started: '2024-01-01T00:00:00Z',
    success: true,
    step_results: [
      {
        step_name: 'step-1',
        timestamp_started: '2024-01-01T00:00:00Z',
        success: true,
      },
    ],
  },
  logs: [
    { level: 'info', message: 'hello', timestamp: '2024-01-01T00:00:00Z' },
  ],
}

const failResult = {
  result: {
    monitor_name: 'dry-run',
    timestamp_started: '2024-01-01T00:00:00Z',
    success: false,
    step_results: [
      {
        step_name: 'fail-step',
        timestamp_started: '2024-01-01T00:00:00Z',
        success: false,
        error_message: 'assertion failed: boom',
      },
    ],
  },
  logs: [],
}

beforeEach(() => {
  vi.resetAllMocks()
  sessionStorage.clear()
})

describe('DryRunPanel', () => {
  it('renders Run Script button', () => {
    renderWithProviders(
      <DryRunPanel script="let x = 1;" timeoutSeconds={30} />
    )
    expect(screen.getByText('Run Script')).toBeInTheDocument()
  })

  it('disables button when script is empty', () => {
    renderWithProviders(<DryRunPanel script="   " timeoutSeconds={30} />)
    expect(screen.getByText('Run Script')).toBeDisabled()
  })

  it('shows warning on first click', async () => {
    const user = userEvent.setup()
    renderWithProviders(
      <DryRunPanel script="let x = 1;" timeoutSeconds={30} />
    )

    await user.click(screen.getByText('Run Script'))
    expect(
      screen.getByText(/Scripts execute real HTTP requests/)
    ).toBeInTheDocument()
  })

  it('executes script after confirming warning', async () => {
    mockedApi.executeScript.mockResolvedValue(successResult)
    const user = userEvent.setup()
    renderWithProviders(
      <DryRunPanel script="let x = 1;" timeoutSeconds={30} />
    )

    await user.click(screen.getByText('Run Script'))
    await user.click(screen.getByText('Continue'))

    await waitFor(() => {
      expect(mockedApi.executeScript).toHaveBeenCalledWith('let x = 1;', 30)
    })
  })

  it('skips warning on second run', async () => {
    mockedApi.executeScript.mockResolvedValue(successResult)
    const user = userEvent.setup()
    renderWithProviders(
      <DryRunPanel script="let x = 1;" timeoutSeconds={30} />
    )

    // First run: show warning, confirm
    await user.click(screen.getByText('Run Script'))
    await user.click(screen.getByText('Continue'))

    await waitFor(() => {
      expect(mockedApi.executeScript).toHaveBeenCalledTimes(1)
    })

    // Second run: no warning
    await user.click(screen.getByText('Run Script'))
    await waitFor(() => {
      expect(mockedApi.executeScript).toHaveBeenCalledTimes(2)
    })
  })

  it('displays step results on success', async () => {
    mockedApi.executeScript.mockResolvedValue(successResult)
    sessionStorage.setItem('prodzilla-dryrun-warned', '1')
    const user = userEvent.setup()
    renderWithProviders(
      <DryRunPanel script="let x = 1;" timeoutSeconds={30} />
    )

    await user.click(screen.getByText('Run Script'))

    await waitFor(() => {
      expect(screen.getByText('step-1')).toBeInTheDocument()
      expect(screen.getByText('Passed')).toBeInTheDocument()
    })
  })

  it('displays failed step with error message', async () => {
    mockedApi.executeScript.mockResolvedValue(failResult)
    sessionStorage.setItem('prodzilla-dryrun-warned', '1')
    const user = userEvent.setup()
    renderWithProviders(
      <DryRunPanel script="assert(false);" timeoutSeconds={30} />
    )

    await user.click(screen.getByText('Run Script'))

    await waitFor(() => {
      expect(screen.getByText('fail-step')).toBeInTheDocument()
      expect(screen.getByText('assertion failed: boom')).toBeInTheDocument()
      expect(screen.getByText('Failed')).toBeInTheDocument()
    })
  })

  it('displays captured logs', async () => {
    mockedApi.executeScript.mockResolvedValue(successResult)
    sessionStorage.setItem('prodzilla-dryrun-warned', '1')
    const user = userEvent.setup()
    renderWithProviders(
      <DryRunPanel script="log_info('hi');" timeoutSeconds={30} />
    )

    await user.click(screen.getByText('Run Script'))

    await waitFor(() => {
      expect(screen.getByText('info')).toBeInTheDocument()
      expect(screen.getByText('hello')).toBeInTheDocument()
    })
  })

  it('shows error state on network failure', async () => {
    mockedApi.executeScript.mockRejectedValue(new Error('Network error'))
    sessionStorage.setItem('prodzilla-dryrun-warned', '1')
    const user = userEvent.setup()
    renderWithProviders(
      <DryRunPanel script="let x = 1;" timeoutSeconds={30} />
    )

    await user.click(screen.getByText('Run Script'))

    await waitFor(() => {
      expect(screen.getByText(/Network error/)).toBeInTheDocument()
    })
  })

  it('output panel is collapsible', async () => {
    mockedApi.executeScript.mockResolvedValue(successResult)
    sessionStorage.setItem('prodzilla-dryrun-warned', '1')
    const user = userEvent.setup()
    renderWithProviders(
      <DryRunPanel script="let x = 1;" timeoutSeconds={30} />
    )

    await user.click(screen.getByText('Run Script'))

    await waitFor(() => {
      expect(screen.getByText('step-1')).toBeInTheDocument()
    })

    // Collapse
    await user.click(screen.getByText('Hide output'))
    expect(screen.queryByText('step-1')).not.toBeInTheDocument()

    // Expand
    await user.click(screen.getByText('Show output'))
    expect(screen.getByText('step-1')).toBeInTheDocument()
  })

  it('cancel dismisses warning', async () => {
    const user = userEvent.setup()
    renderWithProviders(
      <DryRunPanel script="let x = 1;" timeoutSeconds={30} />
    )

    await user.click(screen.getByText('Run Script'))
    expect(
      screen.getByText(/Scripts execute real HTTP requests/)
    ).toBeInTheDocument()

    await user.click(screen.getByText('Cancel'))
    expect(
      screen.queryByText(/Scripts execute real HTTP requests/)
    ).not.toBeInTheDocument()
  })
})
