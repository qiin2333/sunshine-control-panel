const { resolve, join } = require('path')
const vuePlugin = require('@vitejs/plugin-vue')
const { defineConfig } = require('vite')

const rendererSrcPath = join(__dirname, 'src', 'renderer')

/**
 * https://vitejs.dev/config
 */
const config = defineConfig({
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
        iddSetting: resolve(rendererSrcPath, 'idd-setting.html'),
      },
    },
  },
  plugins: [vuePlugin()],
})

module.exports = config
