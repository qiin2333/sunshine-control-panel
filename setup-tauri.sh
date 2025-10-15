#!/bin/bash
# Tauri 项目初始化脚本（Linux/macOS 版本）

echo "🚀 Sunshine GUI - Tauri 项目初始化"
echo "================================="
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 检查 Rust 是否已安装
echo -e "${YELLOW}1️⃣ 检查 Rust 环境...${NC}"
if command -v cargo &> /dev/null; then
    RUST_VERSION=$(cargo --version)
    echo -e "   ${GREEN}✅ Rust 已安装: $RUST_VERSION${NC}"
else
    echo -e "   ${RED}❌ Rust 未安装！${NC}"
    echo -e "   ${RED}请访问 https://rustup.rs/ 安装 Rust${NC}"
    echo -e "   ${YELLOW}或运行: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${NC}"
    exit 1
fi

# 检查 Node.js 是否已安装
echo ""
echo -e "${YELLOW}2️⃣ 检查 Node.js 环境...${NC}"
if command -v node &> /dev/null; then
    NODE_VERSION=$(node --version)
    echo -e "   ${GREEN}✅ Node.js 已安装: $NODE_VERSION${NC}"
else
    echo -e "   ${RED}❌ Node.js 未安装！${NC}"
    exit 1
fi

# 创建图标目录
echo ""
echo -e "${YELLOW}3️⃣ 准备图标文件...${NC}"
ICON_DIR="src-tauri/icons"
if [ ! -d "$ICON_DIR" ]; then
    mkdir -p "$ICON_DIR"
    echo -e "   ${GREEN}✅ 已创建图标目录: $ICON_DIR${NC}"
else
    echo -e "   ${GREEN}✅ 图标目录已存在${NC}"
fi

# 复制图标文件
SOURCE_ICON="src/main/static/gura.ico"
DEST_ICON="$ICON_DIR/icon.ico"

if [ -f "$SOURCE_ICON" ]; then
    cp "$SOURCE_ICON" "$DEST_ICON"
    echo -e "   ${GREEN}✅ 已复制图标: gura.ico -> icon.ico${NC}"
elif [ -f "src/assets/sunshine.ico" ]; then
    cp "src/assets/sunshine.ico" "$DEST_ICON"
    echo -e "   ${GREEN}✅ 已复制图标: sunshine.ico -> icon.ico${NC}"
else
    echo -e "   ${YELLOW}⚠️  未找到图标文件，将使用默认图标${NC}"
    echo -e "   ${YELLOW}请手动放置图标到: $DEST_ICON${NC}"
fi

# 生成其他尺寸的图标（需要 ImageMagick，可选）
echo ""
echo -e "${YELLOW}4️⃣ 检查 ImageMagick（可选）...${NC}"
if command -v convert &> /dev/null; then
    echo -e "   ${GREEN}✅ ImageMagick 已安装${NC}"
    
    if [ -f "$DEST_ICON" ]; then
        echo -e "   ${CYAN}生成不同尺寸的图标...${NC}"
        
        # 生成 PNG 图标
        convert "$DEST_ICON" -resize 32x32 "$ICON_DIR/32x32.png" 2>/dev/null
        convert "$DEST_ICON" -resize 128x128 "$ICON_DIR/128x128.png" 2>/dev/null
        convert "$DEST_ICON" -resize 256x256 "$ICON_DIR/128x128@2x.png" 2>/dev/null
        
        echo -e "   ${GREEN}✅ 已生成 PNG 图标文件${NC}"
    fi
else
    echo -e "   ${YELLOW}⏭️  ImageMagick 未安装（跳过）${NC}"
    echo -e "   ${YELLOW}如需自动生成不同尺寸图标，请安装 ImageMagick${NC}"
fi

# 安装 npm 依赖
echo ""
echo -e "${YELLOW}5️⃣ 安装 npm 依赖...${NC}"
if [ ! -d "node_modules" ]; then
    echo -e "   ${CYAN}正在运行 npm install...${NC}"
    npm install
    if [ $? -eq 0 ]; then
        echo -e "   ${GREEN}✅ npm 依赖安装完成${NC}"
    else
        echo -e "   ${RED}❌ npm 依赖安装失败${NC}"
        exit 1
    fi
else
    echo -e "   ${GREEN}✅ node_modules 已存在，跳过安装${NC}"
    echo -e "   如需重新安装，请运行: npm install"
fi

# 检查 Tauri CLI
echo ""
echo -e "${YELLOW}6️⃣ 检查 Tauri CLI...${NC}"
if npx tauri --version &> /dev/null; then
    echo -e "   ${GREEN}✅ Tauri CLI 可用${NC}"
else
    echo -e "   ${RED}❌ Tauri CLI 不可用${NC}"
fi

# 完成
echo ""
echo "================================="
echo -e "${GREEN}✅ Tauri 项目初始化完成！${NC}"
echo ""
echo -e "${CYAN}下一步:${NC}"
echo -e "  • 开发模式: npm run dev"
echo -e "  • 生产构建: npm run build"
echo -e "  • 仅前端开发: npm run dev:renderer"
echo ""
echo -e "${CYAN}📚 查看文档:${NC}"
echo -e "  • 迁移指南: TAURI_MIGRATION_GUIDE.md"
echo -e "  • 组件更新示例: COMPONENT_UPDATE_EXAMPLE.md"
echo ""
echo -e "${GREEN}祝开发顺利！ 🎉${NC}"


