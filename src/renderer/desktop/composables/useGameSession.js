import { computed, ref } from 'vue'
import { tauriInvoke } from './useTauri.js'

/**
 * 受跟踪的游戏会话（前端侧）。
 *
 * 后端 `game_session.rs` 持有进程句柄并负责 prep-cmd undo、窗口让位和时长统计；
 * 这里只订阅它的事件、维护给 UI 用的派生状态。
 */

export const runningGame = ref(null)
export const gameStats = ref({})

/**
 * 启动过场状态。游戏加载常常要几十秒，这段时间必须有可见反馈，
 * 否则用户会以为没点中而反复按 A。
 */
export const launchState = ref(null)

const nowTick = ref(Date.now())

let tickerId = null
let listenersReady = false
let unlistenLaunched = null
let unlistenExited = null
let dismissTimer = null

export const elapsedSeconds = computed(() => {
  const game = runningGame.value
  if (!game) return 0
  const started = Number(game.startedAtMs) || 0
  if (!started) return Number(game.elapsedSeconds) || 0
  return Math.max(0, Math.floor((nowTick.value - started) / 1000))
})

export function formatDuration(totalSeconds) {
  const seconds = Math.max(0, Math.floor(Number(totalSeconds) || 0))
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  if (hours > 0) return `${hours}:${String(minutes).padStart(2, '0')}`
  return `${minutes}:${String(seconds % 60).padStart(2, '0')}`
}

function startTicker() {
  if (tickerId !== null) return
  tickerId = setInterval(() => {
    nowTick.value = Date.now()
  }, 1000)
}

function stopTicker() {
  if (tickerId === null) return
  clearInterval(tickerId)
  tickerId = null
}

function clearDismissTimer() {
  if (dismissTimer !== null) {
    clearTimeout(dismissTimer)
    dismissTimer = null
  }
}

export function dismissLaunchState() {
  clearDismissTimer()
  launchState.value = null
}

function autoDismiss(delayMs) {
  clearDismissTimer()
  dismissTimer = setTimeout(() => {
    dismissTimer = null
    launchState.value = null
  }, delayMs)
}

export async function refreshGameStats() {
  try {
    const stats = await tauriInvoke('get_game_stats')
    gameStats.value = stats?.games || {}
  } catch {
    // 非 Tauri 环境或首次运行还没有统计文件
  }
  return gameStats.value
}

export async function refreshRunningGame() {
  try {
    runningGame.value = (await tauriInvoke('get_running_game')) || null
  } catch {
    runningGame.value = null
  }
  if (runningGame.value) startTicker()
  else stopTicker()
  return runningGame.value
}

export function statFor(appName) {
  return gameStats.value?.[appName] || null
}

/**
 * 启动一个应用并跟踪它。
 *
 * `autoYield` 为真时后端会在启动成功后最小化大屏窗口，并在游戏退出后恢复，
 * 这样串流画面里看到的是游戏而不是压在上面的启动器。
 */
export async function launchTrackedGame(app, { autoYield = true, coverUrl = '' } = {}) {
  const appName = app?.name || ''
  clearDismissTimer()
  launchState.value = { appName, coverUrl, status: 'launching', message: '' }

  try {
    const result = await tauriInvoke('launch_game', {
      app,
      options: { autoYield },
    })

    if (result?.tracked) {
      runningGame.value = {
        appName: result.appName || appName,
        pid: result.pid || 0,
        startedAtMs: Date.now(),
        elapsedSeconds: 0,
        adopted: false,
      }
      startTicker()
      launchState.value = { appName, coverUrl, status: 'started', message: '' }
      autoDismiss(2500)
    } else {
      launchState.value = {
        appName,
        coverUrl,
        status: 'untracked',
        message: result?.untrackedReason || '',
      }
      autoDismiss(5000)
    }
    refreshGameStats()
    return result
  } catch (error) {
    const raw = String(error ?? '')
    const conflict = raw.startsWith('already-running:')
    launchState.value = {
      appName,
      coverUrl,
      status: conflict ? 'conflict' : 'error',
      message: conflict ? raw.slice('already-running:'.length) : raw,
    }
    autoDismiss(conflict ? 5000 : 8000)
    if (conflict) refreshRunningGame()
    throw error
  }
}

export async function stopTrackedGame() {
  try {
    return await tauriInvoke('stop_running_game')
  } catch (error) {
    console.error('Failed to stop the running game:', error)
    return false
  }
}

export async function initGameSession() {
  await Promise.all([refreshRunningGame(), refreshGameStats()])
  if (listenersReady) return

  try {
    const { listen } = await import('@tauri-apps/api/event')
    unlistenLaunched = await listen('game-launched', (event) => {
      const payload = event.payload
      if (!payload?.appName) return
      runningGame.value = payload
      nowTick.value = Date.now()
      startTicker()
    })
    unlistenExited = await listen('game-exited', (event) => {
      const payload = event.payload || {}
      // 只有当前跟踪的那一局退出才清空，避免旧监控迟到的事件误清
      if (!runningGame.value || runningGame.value.appName === payload.appName) {
        runningGame.value = null
        stopTicker()
      }
      refreshGameStats()
      clearDismissTimer()
      launchState.value = {
        appName: payload.appName || '',
        status: 'exited',
        message: '',
        seconds: Number(payload.seconds) || 0,
      }
      autoDismiss(4000)
    })
    listenersReady = true
  } catch {
    // 非 Tauri 环境，事件 API 不可用
  }
}

export function disposeGameSession() {
  stopTicker()
  clearDismissTimer()
  unlistenLaunched?.()
  unlistenExited?.()
  unlistenLaunched = null
  unlistenExited = null
  listenersReady = false
}
