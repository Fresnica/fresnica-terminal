# Fresnica Terminal Windows System Auth 实机验收

适用测试包：`x86_64` Windows。打包源树 `40c54873ad8543c68434c029958e5f4a39d5a348`；System Auth 产品代码 checkpoint `9dc54225846e9f192153c329b348613128ef0b5b`。

## 目标

验证 Windows System Auth 的真实用户在场认证链：Fresnica CLI -> 固定 Program Files provider -> Win32 WebAuthn / Windows Hello -> LocalSystem broker -> 32-byte `WalletUnlockKey` -> Core 签名。

只使用 Stellar Testnet。不要向测试报告发送 Fresnica Passphrase、助记词、S-key 或其他私密材料。

## 环境要求

- x86_64 Windows 10 1903+ 或 Windows 11。
- 必须是交互式桌面用户会话。
- Windows Hello PIN 已配置；如设备支持指纹/人脸，也可一并测试。
- 安装步骤需要 Administrator；钱包和交易验收必须回到普通用户 PowerShell/Terminal。
- 准备一个受 Fresnica Passphrase 保护的 Testnet software wallet，以及另一个 Testnet 收款地址。

## 1. 校验测试包

在包目录中查看：

```powershell
Get-Content .\SHA256SUMS.txt
Get-FileHash .\fresnica.exe -Algorithm SHA256
Get-FileHash .\fresnica-system-auth-provider.exe -Algorithm SHA256
Get-FileHash .\fresnica-system-auth-service.exe -Algorithm SHA256
```

三个二进制的 hash 必须与 `SHA256SUMS.txt` 一致。
## 2. 安装高信任 provider/service

打开 **Administrator PowerShell**，进入测试包目录：

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\install-windows-system-auth-development.ps1 `
  -ProviderBinary .\fresnica-system-auth-provider.exe `
  -ServiceBinary .\fresnica-system-auth-service.exe
```

安装成功后检查：

```powershell
Get-Service FresnicaSystemAuth
Get-ChildItem "$env:ProgramFiles\Fresnica\SystemAuth"
```

预期 service 为 `Running`，安装目录中存在 provider 与 service。

安装程序还会将状态目录限制为 SYSTEM/Administrators：

```powershell
icacls.exe "$env:ProgramData\Fresnica\SystemAuth"
```

完成安装后关闭 Administrator PowerShell，后续测试使用普通用户 PowerShell。
## 3. Enable 与 status

普通用户 PowerShell：

```powershell
$BIN = "$PWD\fresnica.exe"
& $BIN --network testnet wallet list
& $BIN --network testnet wallet system-auth enable WALLET_NAME
```

在一个没有现存 Fresnica Windows System Auth domain 的干净测试状态下，预期：

1. CLI 要求输入 `Fresnica passphrase:`。
2. Windows Security / Windows Hello 出现用户验证界面。
3. 使用 PIN、指纹或人脸完成认证。
4. CLI 报告 system authentication enabled。

如果同一 Windows 用户已有其他 Fresnica signer enrollment，新的 wallet enrollment 可以复用既有 domain，因此不一定再次创建 Hello credential。

状态检查：

```powershell
& $BIN --network testnet wallet system-auth status WALLET_NAME
```

预期：`System authentication: enabled`，并且 status 本身不弹 Windows Hello。
## 4. Routine signing：Windows Hello 成功

发送一笔极小 Testnet XLM：

```powershell
& $BIN --network testnet send 0.0000001 XLM `
  to GDESTINATION `
  --wallet WALLET_NAME
```

确认交易后，预期出现 Windows Hello/WebAuthn 用户验证。先使用 Windows Hello PIN 完成一次签名；如果设备支持指纹/人脸，再分别验证这些方式。

通过条件：

- Windows Hello 用户验证出现并成功；
- 交易签名和提交成功；
- routine signing **不询问 Fresnica Passphrase**；
- 每次新的 signing release 都产生新的 WebAuthn assertion，不得因为上一笔认证而静默释放 unlock key。

可以记录成功的 Testnet transaction hash。
## 5. Cancel 必须 Fail Close

再次发一笔极小 Testnet 交易，在 Windows Hello 界面出现后选择取消。

预期 CLI 终止交易，并显示：

```text
Error: System authentication cancelled
```

通过条件：取消后不得继续签名，也不得自动弹 Fresnica Passphrase 作为降级路径。

如果平台 authenticator 不可用，Fresnica 可以进入显式 Passphrase fallback；但用户主动取消属于不同语义，必须直接终止。

## 6. Lifecycle 与 domain 重建

```powershell
& $BIN --network testnet wallet system-auth disable WALLET_NAME
& $BIN --network testnet wallet system-auth status WALLET_NAME
& $BIN --network testnet wallet system-auth enable WALLET_NAME
```

如果这是该 Windows 用户最后一个 Fresnica signer enrollment，disable 会同时移除 Fresnica 的 domain metadata；重新 enable 应创建新的 WebAuthn/Hello credential。随后再签一笔极小 Testnet 交易，Windows Hello 认证并签名必须成功。
## 7. 报告结果

请记录：

```text
Windows version/build:
Windows Hello methods available: PIN / fingerprint / face
Package SHA256 verification: PASS/FAIL
Service installation/running: PASS/FAIL
First-domain enrollment with Hello: PASS/FAIL
Status without Hello prompt: PASS/FAIL
Routine PIN signing: PASS/FAIL
Fingerprint/face signing (if available): PASS/FAIL/N/A
Second signing requires fresh Hello assertion: PASS/FAIL
Cancel -> fail closed: PASS/FAIL
Disable -> disabled -> re-enable -> sign: PASS/FAIL
Unexpected output/errors:
```

可以提供 Testnet transaction hash。不要提供 Passphrase、助记词或 S-key。

## 可选清理

仅在专用测试机上执行。先对 wallet 执行 `system-auth disable`，然后在 Administrator PowerShell 中：

```powershell
Stop-Service FresnicaSystemAuth -ErrorAction SilentlyContinue
sc.exe delete FresnicaSystemAuth
Remove-Item -Recurse -Force "$env:ProgramFiles\Fresnica\SystemAuth" -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force "$env:ProgramData\Fresnica\SystemAuth" -ErrorAction SilentlyContinue
```
