# Free Cursor Client

[English](./README.md) | [中文](./README_zh.md)

Free Cursor Client 是一个管理 Free Cursor 账户的工具。

## 使用方法（Windows）

从[这里](https://github.com/freeai-dev/free-cursor-client/releases)下载最新版本。

假设您下载的文件保存在 `D:\apps\free-cursor-client.exe`，请按以下步骤操作：

1. 打开命令提示符（CMD）
2. 切换到程序所在目录：
   ```cmd
   cd D:\apps
   ```
3. 执行下单命令：
   ```cmd
   .\free-cursor-client.exe order
   ```

或者，您也可以直接使用完整路径执行：

```cmd
D:\apps\free-cursor-client.exe order
```

支付购买后，需重启 Cursor。

## 使用方法（macOS）

执行下单命令：

```bash
curl https://raw.githubusercontent.com/freeai-dev/free-cursor-client/refs/heads/master/install.sh | bash -s -- order
```

支付购买后，需重启 Cursor。

## 邀请计划

当您邀请新用户，且被邀请的用户成功付费订阅后，您将获得一个月的额外使用时长作为奖励。

生成邀请码（需要在本地创建过支付订单）：

```cmd
.\free-cursor-client.exe invite
```

## 问题咨询

联系邮箱 `customer@freeai.dev`
