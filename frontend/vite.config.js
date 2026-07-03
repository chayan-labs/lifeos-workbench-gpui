import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
  ],
  // Force a single React instance. recharts (and any other dep with its own
  // React copy) must resolve to frontend/node_modules/react, otherwise hooks
  // run against a second React and crash with "Invalid hook call".
  resolve: {
    dedupe: ['react', 'react-dom'],
  },
})
