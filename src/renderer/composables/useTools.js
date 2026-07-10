import { ref } from 'vue'
import { ElMessage, ElMessageBox, ElLoading, ElNotification } from 'element-plus'
import { openExternalUrl, tools, vmouse, controllerMeta } from '@/tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'

// Module-scoped reactive flag so all sidebar instances share state.
const clipboardSyncEnabled = ref(false)
let clipboardSyncInitialised = false
let clipboardSyncPollTimer = null

/**
 * 工具操作 Composable
 */
export function useTools() {
  const { t } = useI18n()

  /**
   * 公共确认对话框操作
   * @param {string} message - 确认消息
   * @param {string} title - 对话框标题
   * @param {function} action - 执行的操作
   * @param {string} successMsg - 成功消息
   */
  const confirmAction = async (message, title, action, successMsg) => {
    try {
      await ElMessageBox.confirm(message, title, {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning',
      })
      await action()
      ElMessage.success(successMsg)
    } catch (error) {
      if (error !== 'cancel') {
        ElMessage.error(`操作失败: ${error}`)
      }
    }
  }

  /**
   * 卸载 VDD
   */
  const uninstallVdd = async () => {
    await confirmAction(
      '确定要卸载虚拟显示器驱动吗？此操作需要管理员权限。',
      '确认卸载',
      tools.uninstallVddDriver,
      '卸载请求已发送'
    )
  }

  /**
   * 重启显卡驱动
   */
  const restartDriver = async () => {
    await confirmAction(
      '确定要重启显卡驱动吗？这将暂时中断屏幕显示。',
      '确认重启',
      tools.restartGraphicsDriver,
      '重启请求已发送'
    )
  }

  /**
   * 重启 Sunshine 服务
   */
  const restartSunshine = async () => {
    await confirmAction(
      '确定要重启 Sunshine 服务吗？这将断开当前所有连接。\n\n如果弹出 UAC 提示，请点击"是"以确认。\nSunshine 服务将在几秒钟内重启。',
      '确认重启',
      tools.restartSunshineService,
      '重启请求已发送'
    )
  }

  /**
   * 以用户模式重启 Sunshine（非服务模式）
   */
  const restartSunshineInUserMode = async () => {
    await confirmAction(
      '确定要以用户模式重启 Sunshine 吗？\n\n这将：\n1. 停止 Sunshine 服务\n2. 关闭所有 Sunshine 进程\n3. 以用户模式启动 Sunshine \n\n这将断开当前所有连接。',
      '确认重启',
      tools.restartSunshineInUserMode,
      '用户模式重启请求已发送'
    )
  }

  /**
   * 打开串流计时器
   */
  const openTimer = async () => {
    await createWindow('/stop-clock-canvas/index.html', '串流计时器', {
      prefix: 'timer',
      width: 1080,
      height: 600,
    })
  }

  /**
   * 打开外部 URL
   * @param {string} url - 要打开的URL
   */
  const openUrl = async (url) => {
    await openExternalUrl(url)
  }

  /**
   * 清理无用的封面图片和临时文件
   */
  const cleanupCovers = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core')

      // 首先检查是否以管理员权限运行
      const isRunningAsAdmin = await invoke('is_running_as_admin')

      if (!isRunningAsAdmin) {
        // 不是管理员，提示重启
        await ElMessageBox.confirm('清理临时文件需要管理员权限。\n\n是否以管理员身份重启应用？', '需要管理员权限', {
          confirmButtonText: '以管理员重启',
          cancelButtonText: '取消',
          type: 'warning',
        })

        // 用户确认后，调用重启为管理员
        await restartAsAdmin()
        return
      }

      // 已经是管理员，继续执行清理
      await ElMessageBox.confirm(
        '此操作将删除：\n1. 未被应用使用的封面图片\n2. config 目录下的 temp_ 临时文件\n\n是否继续？',
        '清理无用文件',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
      )

      // 显示加载提示
      const loading = ElMessage({
        message: '正在清理无用文件...',
        type: 'info',
        duration: 0,
      })

      // 调用 Tauri 命令
      const result = await invoke('cleanup_unused_covers')

      loading.close()

      // 显示结果
      if (result.success) {
        if (result.deleted_count > 0) {
          ElMessageBox.alert(
            `${result.message}\n\n删除的文件数: ${result.deleted_count}\n释放的空间: ${(
              result.freed_space / 1024
            ).toFixed(2)} KB`,
            '清理完成',
            {
              confirmButtonText: '确定',
              type: 'success',
            }
          )
        } else {
          ElMessage.success(result.message)
        }
      } else {
        ElMessage.error('清理失败: ' + result.message)
      }
    } catch (error) {
      if (error !== 'cancel') {
        console.error('清理文件失败:', error)
        ElMessage.error('清理文件失败: ' + error)
      }
    }
  }

  /**
   * 以管理员权限重启 GUI
   */
  const restartAsAdmin = async () => {
    try {
      // 确认对话框
      await ElMessageBox.confirm('将以管理员权限重启应用，当前窗口会关闭。是否继续？', '提升权限', {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning',
      })

      // 显示提示
      ElMessage.info('正在请求管理员权限...')

      // 调用 Tauri 命令
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('restart_as_admin')

      // 如果到这里说明成功请求了重启
      ElMessage.success('正在以管理员权限重启...')
    } catch (error) {
      if (error !== 'cancel') {
        console.error('重启失败:', error)
        ElMessage.error('重启失败: ' + error)
      }
    }
  }

  /**
   * 检查更新，返回 UpdateInfo（包含 is_latest 标记）由调用方处理展示
   */
  const checkForUpdates = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core')

      ElMessage.info('正在检查更新...')

      const result = await invoke('check_for_updates')

      if (result) {
        return result // 返回更新信息（包含 is_latest 标记），让调用者处理
      }
      return null
    } catch (error) {
      console.error('检查更新失败:', error)
      ElMessage.error('检查更新失败: ' + error)
      return null
    }
  }

  /**
   * 公共窗口创建函数
   * @param {string} url - 窗口URL路径
   * @param {string} title - 窗口标题
   * @param {object} options - 窗口配置选项
   */
  const createWindow = async (url, title, options = {}) => {
    try {
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow')
      const baseUrl = window.location.origin
      const windowId = `${options.prefix || 'window'}_${Date.now()}`

      const newWindow = new WebviewWindow(windowId, {
        url: `${baseUrl}${url}`,
        title,
        width: options.width || 1080,
        height: options.height || 800,
        decorations: options.decorations !== false,
        center: true,
      })

      // 等待窗口创建完成后显示
      newWindow.once('tauri://created', async () => {
        console.log(`✅ ${title}窗口已创建`)
        await newWindow.show()
        await newWindow.setFocus()
        console.log(`✅ ${title}窗口已显示`)
      })

      newWindow.once('tauri://error', (e) => {
        console.error(`❌ ${title}窗口创建失败:`, e)
        ElMessage.error(`${title}窗口创建失败`)
      })
    } catch (error) {
      console.error(`❌ 打开${title}失败:`, error)
      ElMessage.error(`打开${title}失败: ${error.message}`)
    }
  }

  /**
   * 安装虚拟鼠标驱动
   */
  const installVmouse = async () => {
    await confirmAction(
      '将安装虚拟鼠标驱动，此操作需要管理员权限。\n安装后可能需要重启系统才能生效。',
      '确认安装',
      vmouse.install,
      '安装请求已发送'
    )
  }

  /**
   * 卸载虚拟鼠标驱动
   */
  const uninstallVmouse = async () => {
    await confirmAction(
      '确定要卸载虚拟鼠标驱动吗？此操作需要管理员权限。\nSunshine 将回退到 SendInput 方式。',
      '确认卸载',
      vmouse.uninstall,
      '卸载请求已发送'
    )
  }

  /**
   * 打开手柄测试工具（ControllerMeta）
   *
   * 流程：
   * 1. 已安装 → 直接启动
   * 2. 未安装 → 询问用户：下载并启动 / 打开网页版 / 取消
   * 3. 下载过程中通过消息提示进度
   */
  const openGamepadTest = async () => {
    const FALLBACK_WEB = 'https://hardwaretester.com/gamepad'
    const OFFICIAL_SITE = 'https://www.controllermeta.com/'

    ElMessage.info('正在准备手柄测试工具...')

    // 规范化版本号比较：忽略 v 前缀、按数字段比较
    const normalizeVersion = (v) => String(v || '').trim().replace(/^v/i, '')
    const compareVersion = (a, b) => {
      const pa = normalizeVersion(a).split('.').map((x) => parseInt(x, 10) || 0)
      const pb = normalizeVersion(b).split('.').map((x) => parseInt(x, 10) || 0)
      const len = Math.max(pa.length, pb.length)
      for (let i = 0; i < len; i++) {
        const d = (pa[i] || 0) - (pb[i] || 0)
        if (d !== 0) return d
      }
      return 0
    }

    // 执行下载 + 启动的完整流程（已有 loading 实例时复用）
    const downloadAndLaunch = async (release, loading) => {
      const sizeMb = release.download_size
        ? (release.download_size / 1024 / 1024).toFixed(1)
        : '?'
      loading.setText(`⏳ 正在下载 ControllerMeta ${release.version} (${sizeMb} MB)... 0%`)

      const { listen } = await import('@tauri-apps/api/event')
      const unlisten = await listen('controllermeta-download-progress', (event) => {
        const p = event.payload?.progress ?? 0
        const downloaded = ((event.payload?.downloaded ?? 0) / 1024 / 1024).toFixed(1)
        loading.setText(`⏳ 下载中 ${release.version} - ${downloaded}/${sizeMb} MB (${p}%)`)
      })

      try {
        await controllerMeta.download(release.download_url, release.version)
        unlisten()
        loading.close()
        ElMessage.success(`ControllerMeta ${release.version} 安装完成，正在启动...`)
        try {
          await controllerMeta.launch()
        } catch (err) {
          ElMessage.error(`启动失败: ${err}`)
        }
      } catch (err) {
        unlisten()
        loading.close()
        console.error('[ControllerMeta] download failed:', err)
        ElMessage.error(`下载失败: ${err}`)
      }
    }

    try {
      const status = await controllerMeta.getStatus()

      if (status.installed) {
        // 已安装：先启动（不阻塞），然后后台检查更新
        try {
          await controllerMeta.launch()
          ElMessage.success(
            `已启动 ControllerMeta${status.version ? ' ' + status.version : ''}`
          )
        } catch (err) {
          ElMessage.error(`启动失败: ${err}`)
          return
        }

        // 后台静默检查更新，不打扰用户
        ;(async () => {
          try {
            const release = await controllerMeta.checkRelease()
            if (
              release?.download_url &&
              status.version &&
              compareVersion(release.version, status.version) > 0
            ) {
              ElNotification({
                title: 'ControllerMeta 有新版本',
                message: `当前 ${status.version}，最新 ${release.version}。点击「更新」下载并重启。`,
                type: 'info',
                duration: 0,
                position: 'bottom-right',
                dangerouslyUseHTMLString: false,
                onClick: async () => {
                  const loading = ElLoading.service({
                    lock: true,
                    text: '准备更新...',
                    background: 'rgba(0, 0, 0, 0.5)',
                  })
                  await downloadAndLaunch(release, loading)
                },
              })
            }
          } catch (err) {
            console.warn('[ControllerMeta] 后台检查更新失败（静默）:', err)
          }
        })()

        return
      }

      // 未安装：询问用户操作
      let choice
      try {
        choice = await ElMessageBox({
          title: '手柄测试工具',
          message:
            'ControllerMeta 是一款高精度手柄分析工具（实时摇杆轨迹、8000Hz 回报率检测、震动测试等）。\n\n首次使用需要下载安装（约 17 MB，来自 GitHub Releases）。',
          showCancelButton: true,
          distinguishCancelAndClose: true,
          confirmButtonText: '下载并启动',
          cancelButtonText: '打开网页版',
          closeOnClickModal: false,
          type: 'info',
        })
      } catch (action) {
        if (action === 'cancel') {
          await openExternalUrl(FALLBACK_WEB)
        }
        return
      }

      // ElMessageBox 在当前 Element Plus 版本中 resolve 的是 action 字符串（如 'confirm'），
      // 不是 { action: 'confirm' } 对象。旧判断会导致点击「下载并启动」后直接返回，
      // loading 和下载进度监听都不会创建。
      if (choice !== 'confirm') return

      const loading = ElLoading.service({
        lock: true,
        text: '🔍 正在查询 ControllerMeta 最新版本...',
        background: 'rgba(0, 0, 0, 0.5)',
      })

      let release
      try {
        release = await controllerMeta.checkRelease()
      } catch (err) {
        loading.close()
        console.error('[ControllerMeta] checkRelease failed:', err)
        ElMessage.error(`查询版本失败（可能是网络问题）: ${err}`)
        await openExternalUrl(OFFICIAL_SITE)
        return
      }

      if (!release?.download_url) {
        loading.close()
        ElMessage.warning('未找到可下载的安装包，已打开官网')
        await openExternalUrl(OFFICIAL_SITE)
        return
      }

      await downloadAndLaunch(release, loading)
    } catch (err) {
      console.error('[ControllerMeta] openGamepadTest failed:', err)
      ElMessage.error(`手柄测试工具不可用: ${err}`)
      await openExternalUrl(FALLBACK_WEB)
    }
  }

  /**
   * Clipboard sync is enabled by default whenever the user-session agent is
   * alive. The sidebar button no longer toggles anything — it only reflects
   * status and reports it on click.
   */
  const refreshClipboardSyncStatus = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const status = await invoke('clipboard_sync_status')
      clipboardSyncEnabled.value = status?.transport_state === 'connected' && status?.service_allowed !== false
      return status
    } catch (err) {
      console.warn('[clipboard] status query failed:', err)
      return null
    }
  }

  const showClipboardSyncStatus = async () => {
    const status = await refreshClipboardSyncStatus()
    const msg = t.value.clipboardSync
    if (!status) {
      ElMessage.warning(msg.statusUnavailable)
      return
    }
    if (!status.agent_active || status.transport_state === 'stopped') {
      ElMessage.warning(msg.agentInactive)
    } else if (status.service_allowed === false) {
      ElMessage.warning(msg.serviceDisabled)
    } else if (status.transport_state === 'connected') {
      ElMessage.success(msg.active)
    } else if (status.transport_state === 'connecting') {
      ElMessage.info(msg.connecting)
    } else if (status.transport_state === 'disconnected') {
      const detail = status.last_error ? `: ${status.last_error}` : ''
      ElMessage.warning(`${msg.disconnected}${detail}`)
    } else {
      ElMessage.info(msg.inactive)
    }
  }

  /** Initial read on first sidebar render so the indicator dot is correct. */
  const initClipboardSyncStatus = async () => {
    if (clipboardSyncInitialised) return
    clipboardSyncInitialised = true
    await refreshClipboardSyncStatus()
    clipboardSyncPollTimer = window.setInterval(refreshClipboardSyncStatus, 5000)
    window.addEventListener('beforeunload', () => {
      if (clipboardSyncPollTimer) window.clearInterval(clipboardSyncPollTimer)
      clipboardSyncPollTimer = null
    }, { once: true })
  }

  return {
    confirmAction,
    uninstallVdd,
    restartDriver,
    restartSunshine,
    restartSunshineInUserMode,
    openTimer,
    openUrl,
    cleanupCovers,
    restartAsAdmin,
    checkForUpdates,
    createWindow,
    installVmouse,
    uninstallVmouse,
    openGamepadTest,
    showClipboardSyncStatus,
    initClipboardSyncStatus,
    clipboardSyncEnabled,
  }
}

