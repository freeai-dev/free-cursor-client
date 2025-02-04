# Free Cursor Client

[English](./README.md) | [中文](./README_zh.md)

Free Cursor Client is a tool for managing Free Cursor accounts.

## Usage (Windows)

Option 1: Using PowerShell installer (Recommended)

```powershell
powershell -ExecutionPolicy Bypass -Command "Invoke-Command -ScriptBlock ([scriptblock]::Create((iwr -Uri 'https://raw.githubusercontent.com/freeai-dev/free-cursor-client/main/install.ps1' -UseBasicParsing).Content)) -ArgumentList 'order'"
```

Option 2: Manual installation

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

If you have already paid but the command line window was accidentally closed, you can recover the installation using the following command:

Option 1: Using PowerShell installer
```powershell
powershell -ExecutionPolicy Bypass -Command "Invoke-Command -ScriptBlock ([scriptblock]::Create((iwr -Uri 'https://raw.githubusercontent.com/freeai-dev/free-cursor-client/main/install.ps1' -UseBasicParsing).Content)) -ArgumentList 'install'"
```

Option 2: Manual installation
```cmd
D:\apps\free-cursor-client.exe install
```

## Usage (macOS)

Execute the order command:

```bash
bash <(curl -L https://raw.githubusercontent.com/freeai-dev/free-cursor-client/refs/heads/main/install.sh) order
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
