# Cloudflare Pages 部署指南

本文档说明如何将官网文件部署到 Cloudflare Pages，配置方式与 Vercel 类似。

## 部署方式对比

### Vercel 配置（当前）
- 配置文件：`vercel.json`
- 构建命令：`npm run build:home`
- 输出目录：`dist`
- 路由重写：通过 `vercel.json` 的 `rewrites` 配置

### Cloudflare Pages 配置
- 配置文件：`wrangler.toml`（可选，用于 CLI 部署）
- 路由重写：通过 `_redirects` 文件（类似 Vercel 的 rewrites）
- 构建命令：`npm run build:home`
- 输出目录：`dist`

## 部署方法

### 方法 1：通过 Cloudflare Dashboard（推荐，类似 Vercel）

1. **登录 Cloudflare Dashboard**
   - 访问 https://dash.cloudflare.com/
   - 进入 "Pages" 部分

2. **连接 GitHub 仓库**
   - 点击 "Create a project"
   - 选择 "Connect to Git"
   - 授权并选择你的 GitHub 仓库

3. **配置构建设置**
   - **项目名称**：`sunshine-control-panel`（或自定义）
   - **生产分支**：`master`（或你的主分支）
   - **框架预设**：`Vite`（Cloudflare 会自动检测）
   - **构建命令**：`npm run build:home`
   - **构建输出目录**：`dist`
   - **根目录**：`src_assets/common/sunshine-control-panel`（如果仓库根目录不是项目根目录）

4. **环境变量**（如果需要）
   - 在构建设置中添加环境变量
   - 例如：`NODE_ENV=production`

5. **部署**
   - 点击 "Save and Deploy"
   - Cloudflare 会自动构建并部署

### 方法 2：通过 Wrangler CLI

1. **安装 Wrangler CLI**
   ```bash
   npm install -g wrangler
   # 或
   npm install --save-dev wrangler
   ```

2. **登录 Cloudflare**
   ```bash
   wrangler login
   ```

3. **构建项目**
   ```bash
   cd src_assets/common/sunshine-control-panel
   npm install
   npm run build:home
   ```

4. **部署到 Cloudflare Pages**
   ```bash
   # 方式 1：直接部署 dist 目录
   wrangler pages deploy dist --project-name=sunshine-control-panel

   # 方式 2：使用 wrangler.toml 配置
   wrangler pages deploy dist
   ```

### 方法 3：通过 GitHub Actions 自动部署

创建 `.github/workflows/deploy-cloudflare-pages.yml`：

```yaml
name: Deploy to Cloudflare Pages

on:
  push:
    branches:
      - master
    paths:
      - 'src_assets/common/sunshine-control-panel/**'
  workflow_dispatch:

jobs:
  deploy:
    runs-on: ubuntu-latest
    name: Deploy to Cloudflare Pages
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '18'
          cache: 'npm'
          cache-dependency-path: src_assets/common/sunshine-control-panel/package-lock.json
      
      - name: Install dependencies
        working-directory: src_assets/common/sunshine-control-panel
        run: npm ci
      
      - name: Build
        working-directory: src_assets/common/sunshine-control-panel
        run: npm run build:home
      
      - name: Deploy to Cloudflare Pages
        uses: cloudflare/pages-action@v1
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          accountId: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
          projectName: sunshine-control-panel
          directory: src_assets/common/sunshine-control-panel/dist
          gitHubToken: ${{ secrets.GITHUB_TOKEN }}
```

**需要的 Secrets：**
- `CLOUDFLARE_API_TOKEN`：在 Cloudflare Dashboard > My Profile > API Tokens 创建
- `CLOUDFLARE_ACCOUNT_ID`：在 Cloudflare Dashboard 右侧边栏可以找到

## 配置文件说明

### `_redirects` 文件
- 位置：`src_assets/common/sunshine-control-panel/_redirects`
- 作用：处理 SPA 路由，将所有请求重定向到 `index.html`
- 构建后会自动复制到 `dist` 目录

### `wrangler.toml` 文件
- 位置：`src_assets/common/sunshine-control-panel/wrangler.toml`
- 作用：Wrangler CLI 的配置文件（可选）
- 如果使用 Dashboard 部署，此文件不是必需的

## 缓存配置

Cloudflare Pages 默认会缓存静态资源。如果需要自定义缓存策略，可以在 Cloudflare Dashboard 中配置：

1. 进入 Pages 项目设置
2. 在 "Custom domains" 部分配置缓存规则
3. 或使用 Cloudflare Workers 添加自定义 headers

## 自定义域名

1. 在 Cloudflare Dashboard 中进入 Pages 项目
2. 点击 "Custom domains"
3. 添加你的域名
4. 按照提示配置 DNS 记录

## 与 Vercel 的主要区别

| 特性 | Vercel | Cloudflare Pages |
|------|--------|------------------|
| 配置文件 | `vercel.json` | `wrangler.toml`（可选） |
| 路由重写 | `vercel.json` 中的 `rewrites` | `_redirects` 文件 |
| 构建配置 | `vercel.json` 中的 `buildCommand` | Dashboard 或 `wrangler.toml` |
| 缓存控制 | `vercel.json` 中的 `headers` | Dashboard 或 Workers |
| 自动部署 | 连接 GitHub 后自动 | 连接 GitHub 后自动 |

## 注意事项

1. **`_redirects` 文件位置**：确保 `_redirects` 文件在构建输出目录（`dist`）中。如果 Vite 没有自动复制，需要在 `vite.home.config.js` 中配置。

2. **构建输出目录**：确保构建输出到 `dist` 目录，与 `vercel.json` 中的配置一致。

3. **环境变量**：如果项目需要环境变量，在 Cloudflare Dashboard 的构建设置中添加。

4. **Node.js 版本**：Cloudflare Pages 默认使用 Node.js 18，如果需要其他版本，在构建设置中指定。

## 验证部署

部署完成后，访问 Cloudflare 提供的预览 URL 或自定义域名，确认：
- 首页正常加载
- 路由跳转正常（SPA 路由）
- 静态资源（CSS、JS、图片）正常加载
- 缓存策略生效
