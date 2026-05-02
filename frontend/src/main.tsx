import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter, Routes, Route } from 'react-router'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import './index.css'
import Layout from './components/Layout'
import MonitorList from './pages/MonitorList'
import MonitorDetail from './pages/MonitorDetail'
import MonitorCreate from './pages/MonitorCreate'
import MonitorEdit from './pages/MonitorEdit'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5000,
      retry: 1,
    },
  },
})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<MonitorList />} />
            <Route path="monitors/new" element={<MonitorCreate />} />
            <Route path="monitors/:name" element={<MonitorDetail />} />
            <Route path="monitors/:name/edit" element={<MonitorEdit />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  </StrictMode>
)
