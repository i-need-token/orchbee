# Desktop SSH Runtime Release

## 范围与已交付边界

Slice 2.3B 为 SSH Environment Runtime 交付一条离线且仅用于 release 的组合路径。Runtime workspace
准备一份参数固定的双目标 build，外部 signer 对精确 canonical payload 进行认证，Runtime workspace
finalize 一份 signed bundle，而 Desktop 在调用 Electron Packager 之前，使用独立提供的 public key
验证该 bundle。

普通 `desktop:package` 命令仍然是不包含 SSH Runtime release 的开发 package。新命令
`desktop:package:release` 是唯一准入 signed bundle 的路径。本 slice 不连接 Environment、不组合
`SshEnvironmentSupervisor`、不协商已安装 artifact、不发布 feed、不对 Electron 签名或 notarize，也
不添加任何 Runtime service。

## 已接受决策 D37-D46

| 决策 | 已接受契约                                                                                                                                                                                                                                 |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D37  | `@orchbee/ssh-environment-protocol` 拥有共享 release-manifest 与 trust-descriptor contract；Runtime 拥有 build 和离线 bundle prepare/finalize；Desktop 拥有独立验证与 package composition。                                                |
| D38  | 精确目标为 `aarch64-macos` 与 `x86_64-linux-gnu`，两者使用同一个显式 Runtime semantic version、Zig `0.16.0`、`ReleaseSafe` 和 stripping。Release identity 绝不来自 Git state。                                                             |
| D39  | Prepared、finalized 与 packaged bundle 都有固定 layout 和 canonical JSON。Unknown field、额外 entry、symlink、非普通文件、size drift、digest drift 和 platform-matrix drift 全部 fail closed。                                             |
| D40  | Repository 代码绝不读取 production private key。工作流是 prepare → 外部或 HSM-backed raw Ed25519 signature → 使用 64-byte signature 与独立 public key finalize。                                                                           |
| D41  | 一份 Desktop release 只信任一个 Ed25519 key 与一份精确 manifest。其 descriptor 存储 canonical SPKI、`ed25519-spki-sha256:<digest>` 和精确 final manifest bytes 的 SHA-256。                                                                |
| D42  | Trust descriptor 被 staging 到 `app.asar` 内；manifest 与 artifact 被 staging 到 Electron `Resources` 下。Desktop 在 packaging 之前把所有准入 bytes 读取、验证并缓冲到私有 staging 区。                                                    |
| D43  | `desktop:package` 保持现有开发行为。`desktop:package:release` 要求绝对 finalized-bundle path 与独立 public-key path；输入缺失或无效时，在 Electron Packager 之前失败。                                                                     |
| D44  | 本 slice 不提供 multi-key trust、feed、自动 upgrade 或 downgrade policy。Signing-key 或 Runtime 变化会产生新的 Desktop package；应用 rollback 携带它原有的 bundle 与 descriptor。                                                          |
| D45  | 本 slice 不组合 SSH connection、installation、supervisor 或 installed-artifact fast-path behavior，也不拥有 Electron code signing 或 notarization。                                                                                        |
| D46  | 确定性证据使用临时 Ed25519 key、完整 prepare/sign/finalize/verify flow、mutation 与 canonical-layout case、双目标编译、平台原生 Runtime test 与 repository gate。Production key 不进入测试；该有限顺序工作流不需要 TLA+ 或 Lean artifact。 |

## 建模记录

| 字段     | Slice 2.3B 记录                                                                                                                                                                                                                                                                                                                                |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题     | Release engineer 必须把精确的受支持远程 Runtime bytes 绑定到一份 Desktop package，同时不把 production private key 放入 repository、不要求远程机器下载代码，也不允许 package-time substitution。                                                                                                                                                |
| 输入     | 受信 intent 是一个显式 Runtime semantic version、一个由外部治理的 Ed25519 signer，以及一个独立选择的 public key。Build output、prepared directory、signature、manifest、artifact、filesystem kind、package environment variable 以及任何必需输入的缺失，在验证前都不受信。                                                                     |
| 数据     | Schema-version-1 unsigned 与 signed manifest 包含有序双平台 artifact matrix。Schema-version-1 trust descriptor 包含 canonical SPKI、由其派生的 signing-key identity，以及精确 final-manifest digest。Prepared 与 final bundle 使用固定 path，不存在可选 entry。                                                                                |
| 输出     | Prepare 要么原子发布 signing payload 与 artifact，要么保持所请求 output 不存在。Finalize 要么原子发布 signed manifest 与 artifact，要么保持 output 不存在。Release packaging 要么使用已验证缓冲 resource 调用 Electron Packager，要么先失败。普通 packaging 有意不包含 SSH Runtime release。                                                   |
| 不变量   | 两个 artifact 共享一个显式 Runtime version；每个 signed size 与 digest 都匹配缓冲 bytes；manifest 与 descriptor 都是 canonical；descriptor 同时固定 signer identity 与精确 manifest bytes；production private key 绝不是输入；被 staging 的是已验证 bytes，而不是稍后再次读取的 source path；未知 filesystem content 不授予 authority。        |
| 决策     | D37-D46 固定 ownership、target、toolchain mode、canonical format、private-key exclusion、trust binding、package placement、command separation、rollback、non-goal 与 evidence。本 slice 内没有 runtime policy switch。                                                                                                                         |
| 模型     | 一个有限 workflow 拥有 `prepareSshRuntimeRelease`、外部 signing、`finalizeSshRuntimeRelease`、`readVerifiedSshRuntimeRelease` 与 `stageSshRuntimeReleasePackageResources`。Canonical schema function 是确定性 data-contract operation。                                                                                                        |
| 转换     | 每个 repository-owned workflow 从 output 不存在开始，验证并缓冲全部输入，只向私有 candidate 或 staging directory 写入，并且只在所有检查通过后才发布或调用下一边界。它没有 background state、retry、partial-success result 或自动 recovery。                                                                                                    |
| 依赖     | Zig `0.16.0`、Node.js cryptography 与 filesystem API、外部 Ed25519 signer 或 HSM、owner-controlled release workspace，以及 Electron Packager。Network access 与 production signing credential 不是 repository capability。                                                                                                                     |
| 所有权   | Protocol package 拥有 encoding schema；Runtime workspace 拥有 build parameter 与 prepared/final bundle construction；外部 release authority 拥有 private key 与 signature act；Desktop 拥有 public-key normalization、trust-descriptor construction、verification policy、private staging 与 package placement。                               |
| 验证     | Contract unit test 覆盖 canonical encoding 与 strict field。Runtime Node test 覆盖固定 argument、prepare/finalize、signature、mutation 与 layout。Desktop verifier 和 package test 覆盖精确 trust、错误 key、changed bytes、noncanonical manifest、extra entry、symlink 与 buffered staging。Zig 编译两个目标并在受支持 CI host 上运行。       |
| 正确性   | Claim 是下述 trust assumption 下的精确 acceptance/rejection 与 phase ordering。这是一个没有 concurrent state、retry、cancellation 或关键无界 transformation 的有界顺序 workflow，因此选择 example 与 integration test 作为证据；TLA+ 与 Lean 不会增强本 slice 声明的边界。                                                                     |
| 参考范围 | Contract 覆盖 release schema `1`、trust schema `1`、Zig `0.16.0`、Node.js `>=22`、macOS arm64 与 glibc Linux x86_64 Runtime artifact，以及 Electron Packager resource placement。它不声明跨机器 bit-for-bit reproducibility、Electron signing/notarization、owner-controlled workspace 的敌对并发 mutation，或其他目标上的 Runtime execution。 |

## Canonical artifact 与 placement

Prepare 精确输出：

```text
prepared-release/
  artifacts/
    darwin-arm64/orchbee-ssh-environment-runtime
    linux-x64/orchbee-ssh-environment-runtime
  manifest-signing-payload.json
```

Signing payload 没有 trailing newline。Finalize 只接受一个 raw 64-byte Ed25519 signature 与一个
匹配 public key，用它们验证这些 payload bytes，再次检查两个 artifact，并精确输出：

```text
final-release/
  artifacts/
    darwin-arm64/orchbee-ssh-environment-runtime
    linux-x64/orchbee-ssh-environment-runtime
  manifest.json
```

Final manifest 与 trust descriptor 都使用各自 canonical property order 和一个 trailing newline。
Release packaging 创建以下拆分：

```text
app.asar:/dist/main/ssh-environment-runtime-release-trust-v1.json
Resources/ssh-environment-runtime-release-v1/
  artifacts/
    darwin-arm64/orchbee-ssh-environment-runtime
    linux-x64/orchbee-ssh-environment-runtime
  manifest.json
```

Descriptor 是 application trust state，因此保留在 `app.asar` 内。Signed manifest 与 executable bytes
必须是 `app.asar` 外部的真实文件，以便后续 bootstrap 读取并流式传输所选 artifact。Desktop 从独立
public-key file 派生 canonical DER SPKI 与 signing-key identity；final release bundle 不提供自己的 trust
root。

## 离线 release 工作流

所有 path 都必须是绝对路径，所有请求的 output directory 都必须不存在，而 production signer 必须运行
在 repository tooling 之外。从 repository root 执行：

```sh
pnpm --filter @orchbee/ssh-environment-runtime release:prepare --output /absolute/prepared-release --runtime-version 1.2.3

# 外部 release authority 对下列文件的精确 bytes 签名：
# /absolute/prepared-release/manifest-signing-payload.json
# 它写入 raw 64-byte Ed25519 signature，且不向此 process 暴露 private key。

pnpm --filter @orchbee/ssh-environment-runtime release:finalize --prepared /absolute/prepared-release --signature /absolute/signature.bin --public-key /absolute/release-public-key.pem --output /absolute/final-release

ORCHBEE_SSH_RUNTIME_RELEASE_DIRECTORY=/absolute/final-release \
ORCHBEE_SSH_RUNTIME_RELEASE_PUBLIC_KEY=/absolute/release-public-key.pem \
pnpm desktop:package:release
```

传给 Desktop 的 public key 是独立 release input，不是从 bundle 内部信任的文件。Variable 缺失、相对
path、错误 key、错误 signature、noncanonical JSON、changed bytes、不受支持 matrix 或扩展 layout，
都会让 release command 在 Electron Packager 之前停止。普通命令仍为：

```sh
pnpm desktop:package
```

它不读取这些 release variable，也不包含 Runtime bundle 或 descriptor。

## Trust、rollback 与 privacy

Production release authority 必须认证 prepared payload、保护 private key，并通过 access-controlled
release job 向 Desktop packaging 提供预期 public key。该 workflow 信任 pinned Zig 与 Node toolchain、
Node 的 SHA-256 与 Ed25519 implementation、owner-controlled local release workspace，以及 Electron
Packager 文档定义的 `asar` 与 `extraResource` placement。普通 CI 只用临时 key 证明机制；它绝不声明
production signer 已批准某个 release。

不存在可变的 `current` trust identity。一份 Desktop package 包含一个 descriptor 与一份匹配 bundle。
改变 Runtime bytes/version 或 signing key 都必须产生并发布另一份 Desktop package。回滚应用会一并恢复
旧 package 自己的 descriptor 与 bundle；本 slice 不执行远程 downgrade、cleanup、feed lookup 或 key
rollover。

Manifest 暴露 Runtime version、受支持 platform、size 与 hash。Descriptor 暴露 public key、由其 digest
派生的 identity，以及 manifest digest。这些是 release metadata，不是 secret。Private key、SSH
credential、Environment target、username、remote path 与 AskPass value 在结构上缺失，也不得加入任何
bundle。

## 验证与当前限制

从 repository root 运行聚焦证据：

```sh
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/desktop test:release-package
pnpm --filter @orchbee/desktop typecheck
```

macOS native CI leg 除 repository gate 执行的双目标编译外，还会运行 Runtime test。完整 release rehearsal
可以使用临时 Ed25519 key，但 production key 始终位于本地与普通 CI 证据之外。

Slice 2.3C 现在为 installed-artifact negotiation 与进程内 supervisor composition 消费这一精确 package
边界。单独的 release bundle 仍不能创建或连接 Environment、打开远程 terminal、浏览 file、执行 generic
command、操作 Project 或 Workspace、运行 Agent、安装 operating-system service，或跨 logout/reboot
存活。Electron application signing、notarization、installer creation、publication 与 update distribution
也仍是独立 release work。
