<template>
  <div class="tool-container">
    <div class="tool-header">
      <h2>码率调整</h2>
      <button class="close-btn" @click="$emit('close')">×</button>
    </div>

    <div class="tool-content">
      <div class="dev-notice">
        <div class="icon">🚧</div>
        <h3>功能开发中</h3>
        <p>码率调整功能即将推出</p>
        <p class="subtitle">敬请期待！</p>
      </div>
    </div>
  </div>
</template>

<script setup>
defineEmits(['close']);
</script>

<style scoped>
.tool-container {
  width: 420px;
  color: white;
}

.tool-header {
  padding: 16px 24px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px);
  position: relative;
}

.tool-header h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  text-align: center;
}

.close-btn {
  position: absolute;
  top: 12px;
  right: 16px;
  width: 32px;
  height: 32px;
  border: none;
  background: rgba(255, 255, 255, 0.2);
  color: white;
  font-size: 28px;
  line-height: 1;
  border-radius: 50%;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: rotate(90deg);
}

.tool-content {
  padding: 40px 30px;
}

.dev-notice {
  text-align: center;
  padding: 40px 20px;
}

.dev-notice .icon {
  font-size: 64px;
  margin-bottom: 20px;
  animation: bounce 2s ease-in-out infinite;
}

@keyframes bounce {
  0%, 100% {
    transform: translateY(0);
  }
  50% {
    transform: translateY(-10px);
  }
}

.dev-notice h3 {
  font-size: 24px;
  margin-bottom: 12px;
  font-weight: 600;
}

.dev-notice p {
  font-size: 16px;
  opacity: 0.9;
  margin-bottom: 8px;
}

.dev-notice .subtitle {
  font-size: 14px;
  opacity: 0.7;
}
</style>

