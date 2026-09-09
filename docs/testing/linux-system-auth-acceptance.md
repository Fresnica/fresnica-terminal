# Fresnica Terminal Linux System Auth 实机验收

适用测试包：`x86_64` Linux，产品代码基线 `40c54873ad8543c68434c029958e5f4a39d5a348`。

## 目标

验证 Linux System Auth 的真实桌面用户在场认证链：Fresnica CLI -> 固定路径高信任 provider -> polkit `auth_self` -> 32-byte `WalletUnlockKey` -> Core 签名。

只使用 Stellar Testnet。不要向测试报告发送 Fresnica Passphrase、助记词、S-key 或其他私密材料。

## 环境要求

- x86_64 Linux 桌面系统，`uname -m` 应为 `x86_64`。
- 系统需要 `libudev.so.1`（常见桌面发行版通常已安装）。
- 当前会话必须是本地桌面登录会话，并有可工作的 polkit authentication agent。
- 普通用户可以使用 `sudo`，但只有安装步骤使用 `sudo`；所有 Fresnica 钱包和交易命令必须以普通用户执行。
- 准备一个受 Fresnica Passphrase 保护的 Testnet software wallet，以及另一个 Testnet 收款地址。

## 1. 校验测试包

在解压后的包目录执行：

```bash
sha256sum -c SHA256SUMS
./bin/fresnica --version
uname -m
```

所有 SHA256 必须为 `OK`，架构必须为 `x86_64`。
## 2. 安装高信任 provider

安装步骤使用 `sudo`：

```bash
sudo ./scripts/install-linux-system-auth-provider.sh \
  ./bin/fresnica-system-auth-provider
```

预期最后显示 provider、policy 路径，以及 `polkit auth_self`。

检查权限和 policy：

```bash
stat -c '%U:%G %a %n' /usr/libexec/fresnica-system-auth-provider
pkaction --verbose --action-id com.fresnica.system-auth.release
```

provider 必须是：

```text
root:root 4755 /usr/libexec/fresnica-system-auth-provider
```

polkit action 的 active policy 必须是 `auth_self`，不能是 `auth_self_keep`。

从此处开始不要再使用 `sudo` 运行 Fresnica。
## 3. Enable 与 status

先查看 Testnet 钱包：

```bash
BIN="$PWD/bin/fresnica"
"$BIN" --network testnet wallet list
```

对一个 software wallet 执行：

```bash
"$BIN" --network testnet wallet system-auth enable WALLET_NAME
```

预期：

1. CLI 要求输入 `Fresnica passphrase:`。
2. enable 成功。
3. **Linux enrollment 本身不应弹 polkit 认证框。** Fresnica Passphrase 已经完成 enrollment 授权；polkit 用于之后每次 release/signing。

检查状态：

```bash
"$BIN" --network testnet wallet system-auth status WALLET_NAME
```

预期：`System authentication: enabled`，并且 status 不弹系统认证。
## 4. Routine signing：系统认证成功

发送一笔极小 Testnet XLM：

```bash
"$BIN" --network testnet send 0.0000001 XLM \
  to GDESTINATION \
  --wallet WALLET_NAME
```

确认交易后，预期出现桌面 polkit 用户认证框。使用当前 Linux 用户的系统认证方式完成认证。

通过条件：

- polkit 认证出现并成功；
- 交易签名和提交成功；
- routine signing **不询问 Fresnica Passphrase**；
- 每次新的签名 release 都再次经过 polkit，不应因为上一笔认证而静默跳过。

如果桌面环境没有可用 authentication agent，provider 应返回 Passphrase fallback，而不是直接释放 unlock key。该无-agent 场景不是正常桌面验收的必测项。
## 5. Cancel 必须 Fail Close

再次发一笔极小 Testnet 交易，在 polkit 认证框出现后选择取消。

预期 CLI 终止交易，并显示：

```text
Error: System authentication cancelled
```

通过条件：取消后不得继续签名，也不得自动弹 Fresnica Passphrase 作为降级路径。

## 6. Lifecycle

```bash
"$BIN" --network testnet wallet system-auth disable WALLET_NAME
"$BIN" --network testnet wallet system-auth status WALLET_NAME
"$BIN" --network testnet wallet system-auth enable WALLET_NAME
```

预期：disable 要求 Fresnica Passphrase；status 显示 `disabled`；重新 enable 后恢复 `enabled`。随后再发一笔极小 Testnet 交易，polkit 认证并签名必须再次成功。
## 7. 报告结果

请记录：

```text
Linux distribution/version:
Desktop environment:
polkit authentication agent:
Package SHA256 verification: PASS/FAIL
Enable: PASS/FAIL
Status without auth prompt: PASS/FAIL
Routine system-auth signing: PASS/FAIL
Second signing requires fresh polkit auth: PASS/FAIL
Cancel -> fail closed: PASS/FAIL
Disable -> disabled -> re-enable -> sign: PASS/FAIL
Unexpected output/errors:
```

可以提供 Testnet transaction hash。不要提供 Passphrase、助记词或 S-key。

## 可选清理

在专用测试机上完成测试后，可先对 wallet 执行 `system-auth disable`。如还要卸载系统 provider，再执行：

```bash
sudo rm -f /usr/libexec/fresnica-system-auth-provider
sudo rm -f /usr/share/polkit-1/actions/com.fresnica.system-auth.policy
sudo rm -rf /var/lib/fresnica-system-auth
```
