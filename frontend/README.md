# Prodzilla Frontend

React + TypeScript web UI for Prodzilla synthetic monitoring.

## Development

Run the backend and frontend dev server in separate terminals:

```bash
# Terminal 1: Backend (port 3000)
cargo run

# Terminal 2: Frontend dev server (port 5173, proxies API to backend)
cd frontend
npm install
npm run dev
```

Open http://localhost:5173.

## Production Build

```bash
cd frontend
npm run build    # outputs to frontend/dist/
```

Then `cargo run` serves the frontend from `frontend/dist/` on port 3000.

## Tests

```bash
npm test          # single run
npm run test:watch # watch mode
```

## Stack

- React 19, TypeScript, Vite
- Tailwind CSS v4
- React Router v7
- TanStack Query (React Query)
- Vitest + React Testing Library
