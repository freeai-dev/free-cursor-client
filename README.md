# Free Cursor Client

[English](./README.md) | [中文](./README_zh.md)

Free Cursor Client is a tool for managing Free Cursor accounts.

## Usage (Windows)

Download the latest release from [here](https://github.com/freeai-dev/free-cursor-client/releases).

For example, if you downloaded the file to `D:\apps\free-cursor-client.exe`, follow these steps:

1. Open Command Prompt (CMD)
2. Navigate to the program directory:
   ```cmd
   cd /d D:\apps
   ```
3. Execute the order command:
   ```cmd
   .\free-cursor-client.exe order
   ```

Alternatively, you can use the full path to execute:

```cmd
D:\apps\free-cursor-client.exe order
```

After successful payment, you'll need to restart Cursor.

## Usage (macOS)

Execute the order command:

```bash
curl https://raw.githubusercontent.com/freeai-dev/free-cursor-client/refs/heads/master/install.sh | bash -s -- order
```

After successful payment, you'll need to restart Cursor.

## Referral Program

When you invite new users and they successfully complete a paid subscription, you will receive one month of additional usage time as a reward.

To generate an invitation code (requires having created a payment order locally):

```cmd
.\free-cursor-client.exe invite
```

## Support

Contact email: `customer@freeai.dev`
