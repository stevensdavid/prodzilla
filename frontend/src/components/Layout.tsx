import { Link, Outlet, useLocation } from 'react-router'

export default function Layout() {
  const location = useLocation()

  return (
    <div className="min-h-screen bg-gray-50">
      <header className="bg-white border-b border-gray-200">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between h-14">
            <Link to="/" className="text-lg font-semibold text-gray-900">
              Prodzilla
            </Link>
            <nav className="flex gap-4">
              <Link
                to="/"
                className={`text-sm font-medium ${
                  location.pathname === '/'
                    ? 'text-indigo-600'
                    : 'text-gray-500 hover:text-gray-700'
                }`}
              >
                Monitors
              </Link>
              <Link
                to="/monitors/new"
                className={`text-sm font-medium ${
                  location.pathname === '/monitors/new'
                    ? 'text-indigo-600'
                    : 'text-gray-500 hover:text-gray-700'
                }`}
              >
                Create
              </Link>
            </nav>
          </div>
        </div>
      </header>
      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
        <Outlet />
      </main>
    </div>
  )
}
