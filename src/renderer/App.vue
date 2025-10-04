<template>
  <div id="app">
    <div class="container">
      <header class="header">
        <h1>Sunshine Control Panel</h1>
        <nav class="nav">
          <router-link to="/" class="nav-link">Dashboard</router-link>
          <router-link to="/settings" class="nav-link">Settings</router-link>
          <router-link to="/home" class="nav-link">Website</router-link>
        </nav>
      </header>
      
      <main class="main">
        <router-view />
      </main>
    </div>
  </div>
</template>

<script setup>
// 简单的 Electron 主应用入口
</script>

<style lang="less" scoped>
.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 20px;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 30px;
  padding-bottom: 20px;
  border-bottom: 1px solid #e2e8f0;
  
  h1 {
    color: #1e293b;
    margin: 0;
  }
}

.nav {
  display: flex;
  gap: 20px;
}

.nav-link {
  color: #64748b;
  text-decoration: none;
  padding: 8px 16px;
  border-radius: 6px;
  transition: all 0.3s ease;
  
  &:hover {
    color: #ff6b6b;
    background-color: #f8fafc;
  }
  
  &.router-link-active {
    color: #ff6b6b;
    background-color: #fef2f2;
  }
}

.main {
  min-height: 400px;
}
</style>