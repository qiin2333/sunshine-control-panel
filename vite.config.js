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
    strictPort: true,
    host: '127.0.0.1',
    // HMR 配置：确保 WebSocket 直接连接到 Vite 服务器
    hmr: {
      protocol: 'ws',
      host: '127.0.0.1',
      port: 8080,
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  open: false,
  build: {
    outDir: join(__dirname, 'dist'),
    emptyOutDir: true,
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      input: {
        index: resolve(rendererSrcPath, 'index.html'),
        placeholder: resolve(rendererSrcPath, 'placeholder.html'),
        sunshineFrame: resolve(rendererSrcPath, 'sunshine-frame.html'),
        vdd: resolve(rendererSrcPath, 'vdd/index.html'),
        console: resolve(rendererSrcPath, 'console/index.html'),
        clock: resolve(rendererSrcPath, 'stop-clock-canvas/index.html'),
        home: resolve(rendererSrcPath, 'home/index.html'),
        about: resolve(rendererSrcPath, 'about/index.html'),
        toolbar: resolve(rendererSrcPath, 'toolbar/index.html'),
        toolWindow: resolve(rendererSrcPath, 'tool-window/index.html'),
        desktop: resolve(rendererSrcPath, 'desktop/index.html'),
      },
    },
  },
  plugins: [vuePlugin()],
  resolve: {
    alias: {
      '@': rendererSrcPath,
    },
  },
  define: {
    // 根据构建模式设置环境变量
    __PROD__: JSON.stringify(process.env.NODE_ENV === 'production'),
    __DEV__: JSON.stringify(process.env.NODE_ENV === 'development'),
  },
  css: {
    preprocessorOptions: {
      less: {
        math: 'always',
        relativeUrls: true,
        javascriptEnabled: true,
        additionalData: `
          @import "@/styles/variables.less";
          @import "@/styles/mixins.less";
        `,
      },
    },
  },
  clearScreen: false,
}))
