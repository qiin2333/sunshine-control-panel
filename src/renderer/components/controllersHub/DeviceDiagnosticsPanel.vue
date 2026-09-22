<template>
  <section class="chub-panel">
    <div class="chub-section">
      <div class="chub-section-head">
        <span class="chub-section-label">◈ {{ t.deviceHub.tabs.tools }}</span>
        <span class="chub-section-rule"></span>
        <el-button size="small" :loading="refreshing" @click="refreshAll">{{ t.deviceHub.refresh }}</el-button>
      </div>

      <div v-if="!initialized" class="chub-cards" aria-live="polite">
        <article v-for="index in 2" :key="index" class="chub-card chub-card-placeholder">
          <el-skeleton :rows="3" animated />
        </article>
      </div>
      <div v-else class="chub-cards">
        <!-- ControllerMeta -->
        <article class="chub-card">
          <div class="chub-card-head">
            <strong>{{ t.controllersHub.peripherals.meta.title }}</strong>
            <el-tag
              size="small"
              :type="!probeFailed.meta && metaStatus.installed ? 'success' : 'info'"
              effect="plain"
            >{{ probeFailed.meta ? t.deviceHub.probeUnavailable : (metaStatus.installed
              ? (metaStatus.version ? `v${metaStatus.version}` : t.controllersHub.peripherals.installed)
              : t.controllersHub.peripherals.notInstalled) }}</el-tag>
          </div>
          <p class="chub-hint">{{ t.controllersHub.peripherals.meta.hint }}</p>
          <div class="chub-card-actions">
            <el-button
              size="small"
              type="primary"
              :disabled="refreshing || probeFailed.meta"
              @click="emit('open-controller-meta')"
            >{{ t.controllersHub.peripherals.meta.launch }}</el-button>
          </div>
        </article>

        <!-- 手写笔输入检测 -->
        <article class="chub-card">
          <div class="chub-card-head">
            <strong>{{ t.controllersHub.peripherals.stylus.title }}</strong>
          </div>
          <p class="chub-hint">{{ t.controllersHub.peripherals.stylus.hint }}</p>
          <div class="chub-card-actions">
            <el-button
              size="small"
              type="primary"
              :disabled="refreshing"
              @click="emit('open-stylus-input-probe')"
            >{{ t.controllersHub.peripherals.stylus.launch }}</el-button>
          </div>
        </article>
      </div>
    </div>
  </section>
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { controllerMeta } from '../../tauri-adapter.js'
import { useI18n } from '../../desktop/i18n/index.js'

const emit = defineEmits(['open-controller-meta', 'open-stylus-input-probe'])
const { t } = useI18n()
const metaStatus = reactive({ installed: false, version: '' })
const probeFailed = reactive({ meta: false })
const initialized = ref(false)
const refreshing = ref(false)

async function refreshAll() {
  if (refreshing.value) return
  refreshing.value = true
  try {
    const result = await controllerMeta.probeStatus()
    probeFailed.meta = !result?.success
    if (result?.success) Object.assign(metaStatus, result.data)
  } catch {
    probeFailed.meta = true
  } finally {
    initialized.value = true
    refreshing.value = false
  }
}
onMounted(refreshAll)
</script>
