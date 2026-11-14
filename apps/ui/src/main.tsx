import './theme/fonts.css'
import './theme/base.css'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { initTauriBridge } from './api/tauri-bridge'

initTauriBridge()

const container = document.getElementById('root')
if (!container) {
  throw new Error('Elemen #root tidak ditemukan.')
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
