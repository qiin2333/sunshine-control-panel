import { resolve } from 'path'
import vuePlugin from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

/**
 * Vite config for website deployment with LESS support
 */
export default defineConfig(({ mode }) => ({
  root: 'src/renderer',
  publicDir: 'public',
  base: mode === 'production' ? '/' : '/',
  server: {
    port: 3000,
    host: true
  },
  build: {
    outDir: '../../dist',
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: resolve('src/renderer', 'index.html'),
      },
    },
    sourcemap: mode === 'development'
  },
  plugins: [vuePlugin()],
  resolve: {
    alias: {
      '@': resolve('src/renderer')
    }
  },
  css: {
    preprocessorOptions: {
      less: {
        // LESS 配置选项
        math: 'always',
        relativeUrls: true,
        javascriptEnabled: true,
        // 全局变量和混入
        additionalData: `
          @import "@/styles/variables.less";
          @import "@/styles/mixins.less";
        `
      }
    }
  }
}))
