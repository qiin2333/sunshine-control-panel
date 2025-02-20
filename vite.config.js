import { resolve, join } from 'path'
import vuePlugin from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'
import { dirname } from 'node:path'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

const rendererSrcPath = join(__dirname, 'src', 'renderer')

/**
 * https://vitejs.dev/config
 */
export default defineConfig(() => ({
  root: rendererSrcPath,
  publicDir: 'public',
  server: {
    port: 8080,
  },
  open: false,
  build: {
    outDir: join(__dirname, 'build', 'renderer'),
    emptyOutDir: true,
    rollupOptions: {
      input: {
        index: resolve(rendererSrcPath, 'index.html'),
        placeholder: resolve(rendererSrcPath, 'placeholder.html'),
        vdd: resolve(rendererSrcPath, 'vdd/index.html'),
        clock: resolve(rendererSrcPath, 'stop-clock-canvas/index.html'),
      },
    },
  },
  plugins: [vuePlugin()],
}))
