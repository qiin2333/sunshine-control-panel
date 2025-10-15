<template>
  <div class="sunshine-loader">
    <div v-if="loading" class="loading-container">
      <div class="spinner"></div>
      <p>正在连接 Sunshine...</p>
      <p class="url-info">{{ sunshineUrl }}</p>
    </div>
    <iframe 
      v-else
      :src="sunshineUrl" 
      class="sunshine-frame"
      @load="onFrameLoad"
    ></iframe>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { sunshine } from '@/tauri-adapter.js'

const loading = ref(true)
const sunshineUrl = ref('')

onMounted(async () => {
  try {
    // 获取 Sunshine URL
    sunshineUrl.value = await sunshine.getUrl()
    
    // 给一点时间加载
    setTimeout(() => {
      loading.value = false
    }, 1000)
  } catch (error) {
    console.error('获取 Sunshine URL 失败:', error)
    sunshineUrl.value = 'https://localhost:47990/'
    loading.value = false
  }
})

const onFrameLoad = () => {
  console.log('Sunshine 页面加载完成')
}
</script>

<style scoped>
.sunshine-loader {
  width: 100%;
  height: 100vh;
  overflow: hidden;
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100vh;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
}

.spinner {
  width: 50px;
  height: 50px;
  border: 4px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin-bottom: 20px;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.url-info {
  margin-top: 10px;
  font-size: 14px;
  opacity: 0.8;
}

.sunshine-frame {
  width: 100%;
  height: 100vh;
  border: none;
}
</style>

