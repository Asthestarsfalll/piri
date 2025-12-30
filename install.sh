#!/bin/bash

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 获取脚本所在目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo -e "${GREEN}开始安装 piri...${NC}"

# 检查 Rust 是否安装
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}错误: 未找到 cargo，请先安装 Rust${NC}"
    echo "访问 https://rustup.rs/ 安装 Rust"
    exit 1
fi

# 检查是否以 root 运行（用于系统级安装）
if [ "$EUID" -eq 0 ]; then
    INSTALL_PREFIX="/usr/local"
    BIN_DIR="$INSTALL_PREFIX/bin"
    CONFIG_DIR="/etc/niri"
    USE_SUDO=""
else
    INSTALL_PREFIX="$HOME/.local"
    BIN_DIR="$INSTALL_PREFIX/bin"
    CONFIG_DIR="$HOME/.config/niri"
    USE_SUDO=""
fi

echo -e "${YELLOW}安装路径:${NC}"
echo "  二进制文件: $BIN_DIR/piri"
echo "  配置文件: $CONFIG_DIR/piri.toml"

# 构建项目
echo -e "${GREEN}正在构建项目...${NC}"
if ! cargo build --release; then
    echo -e "${RED}错误: 构建失败${NC}"
    exit 1
fi

# 创建必要的目录
echo -e "${GREEN}创建安装目录...${NC}"
mkdir -p "$BIN_DIR"
mkdir -p "$CONFIG_DIR"

# 复制二进制文件
echo -e "${GREEN}安装二进制文件...${NC}"
if [ -f "target/release/piri" ]; then
    sudo rm -f "$BIN_DIR/piri"
    cp target/release/piri "$BIN_DIR/piri"
    chmod +x "$BIN_DIR/piri"
    echo -e "${GREEN}✓ 二进制文件已安装到 $BIN_DIR/piri${NC}"
else
    echo -e "${RED}错误: 未找到构建产物 target/release/piri${NC}"
    exit 1
fi

# 复制配置文件（如果不存在）
if [ ! -f "$CONFIG_DIR/piri.toml" ]; then
    if [ -f "config.example.toml" ]; then
        cp config.example.toml "$CONFIG_DIR/piri.toml"
        echo -e "${GREEN}✓ 配置文件已复制到 $CONFIG_DIR/piri.toml${NC}"
        echo -e "${YELLOW}请根据需要编辑配置文件${NC}"
    else
        echo -e "${YELLOW}警告: 未找到 config.example.toml${NC}"
    fi
else
    echo -e "${YELLOW}配置文件已存在，跳过复制${NC}"
fi

# 检查 PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo -e "${YELLOW}警告: $BIN_DIR 不在 PATH 中${NC}"
    echo "请将以下内容添加到 ~/.bashrc 或 ~/.zshrc:"
    echo -e "${GREEN}export PATH=\"\$PATH:$BIN_DIR\"${NC}"
fi

# 验证安装
if command -v piri &> /dev/null; then
    echo -e "${GREEN}✓ 安装成功！${NC}"
    echo ""
    echo "使用以下命令验证安装:"
    echo "  piri --help"
else
    echo -e "${YELLOW}安装完成，但 piri 命令不可用${NC}"
    echo "请确保 $BIN_DIR 在 PATH 中，或使用完整路径: $BIN_DIR/piri"
fi

echo ""
echo -e "${GREEN}安装完成！${NC}"

