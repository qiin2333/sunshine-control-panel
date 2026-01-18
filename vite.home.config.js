import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import vuePlugin from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'
import { copyFileSync } from 'fs'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

/**
 * Vite config for home website deployment
 */
export default defineConfig(({ mode }) => ({
  root: 'src/renderer/home',
  publicDir: '../../public',
  base: mode === 'production' ? '/' : '/',
  server: {
    port: 3000,
    host: true
  },
  build: {
    outDir: '../../../dist',
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: resolve('src/renderer/home', 'index.html'),
      },
    },
    sourcemap: mode === 'development'
  },
  plugins: [
    vuePlugin(),
    // 复制 _redirects 文件到输出目录（用于 Cloudflare Pages）
    {
      name: 'copy-redirects',
      closeBundle() {
        if (mode === 'production') {
          const redirectsSource = resolve(__dirname, '_redirects')
          const redirectsDest = resolve(__dirname, '../../../dist', '_redirects')
          try {
            copyFileSync(redirectsSource, redirectsDest)
            console.log('✓ Copied _redirects file for Cloudflare Pages')
          } catch (err) {
            console.warn('⚠ Could not copy _redirects file:', err.message)
          }
        }
      }
    }
  ],
  resolve: {
    alias: {
      '@': resolve('src/renderer'),
    },
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
