#!/bin/bash

APP_SUPPORT_DIR="$HOME/Library/Application Support"
BASE_DIR="$APP_SUPPORT_DIR/dev.freeai.free-cursor-client"
BINARY_NAME="free-cursor-client"

# Get latest version from API
echo "正在获取最新版本信息..."
RESPONSE=$(curl -s -f "https://auth-server.freeai.dev/api/v1/client/download?plain=true")
if [ $? -ne 0 ]; then
    echo "从服务器获取下载信息失败"
    exit 1
fi

# Parse response using shell variable assignment
eval "$RESPONSE"

# Check for error message first
if [ ! -z "$ERROR" ]; then
    echo "服务器返回错误：$ERROR"
    exit 1
fi

if [ -z "$VERSION" ] || [ -z "$DOWNLOAD_URL" ]; then
    echo "获取版本信息失败"
    exit 1
fi
echo "最新版本：$VERSION"

INSTALL_DIR="$BASE_DIR/$VERSION"
BINARY_PATH="$INSTALL_DIR/$BINARY_NAME"
SYMLINK_PATH="$BASE_DIR/$BINARY_NAME"

# Check if the program needs to be installed or updated
check_installation() {
    # Check if binary exists
    if [ ! -f "$BINARY_PATH" ]; then
        return 1
    fi
    
    # Check if symlink exists and points to the correct version
    if [ -L "$SYMLINK_PATH" ]; then
        current_target=$(readlink "$SYMLINK_PATH")
        if [ "$current_target" = "$BINARY_PATH" ]; then
            return 0
        fi
    fi
    
    return 1
}

# Download and install the program
install_program() {
    echo "正在安装 Free Cursor Client $VERSION 版本..."
    
    # Create installation directory
    mkdir -p "$INSTALL_DIR"
    
    # Download the latest release
    echo "正在下载最新版本..."
    TEMP_DIR=$(mktemp -d)
    if ! curl -L -f "$DOWNLOAD_URL" -o "$TEMP_DIR/$BINARY_NAME"; then
        echo "下载发布版本失败"
        rm -rf "$TEMP_DIR"
        exit 1
    fi
    
    # Move binary to installation directory
    mv "$TEMP_DIR/$BINARY_NAME" "$BINARY_PATH"
    chmod +x "$BINARY_PATH"
    
    # Create symlink
    if [ -L "$SYMLINK_PATH" ]; then
        echo "正在删除已存在的符号链接..."
        rm "$SYMLINK_PATH"
    elif [ -e "$SYMLINK_PATH" ]; then
        echo "错误：$SYMLINK_PATH 已存在但不是符号链接"
        rm -rf "$TEMP_DIR"
        exit 1
    fi
    
    echo "正在创建符号链接..."
    ln -s "$BINARY_PATH" "$SYMLINK_PATH"
    
    # Cleanup
    rm -rf "$TEMP_DIR"
    
    echo "安装完成！"
}

# Main script
main() {
    if ! check_installation; then
        install_program
    else
        echo "Free Cursor Client $VERSION 版本已安装且为最新版本。"
    fi
    
    if [ ! -x "$SYMLINK_PATH" ]; then
        echo "错误：$SYMLINK_PATH 不是可执行文件"
        exit 1
    fi
    
    echo "正在启动 Free Cursor Client..."
    exec "$SYMLINK_PATH" "$@"
}

main "$@"
