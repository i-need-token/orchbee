# Desktop SSH Environment Release

## 范围

Slice 2.3B 交付了冻结的 schema-version-1 Runtime-only release。Slice 3.1A 新增
schema-version-2 SSH Environment release，其中为每个已接受平台分别包含一个 SSH Runtime 与
一个 Terminal Host。仓库自动化构建并组装该 cohort，外部 authority 对其精确 canonical
payload 签名，Desktop 在 Electron Packager 运行前验证并私下暂存每一个组件。

`desktop:package:dev` 与 `desktop:package:local` 在结构上不带 release；
`desktop:package:release` 必须使用 v2。Slice 3.1B 现在会消费该 verified cohort，用于 atomic remote
installation 与精确 Terminal Host relay negotiation。这个 release boundary 仍不创建 remote Workspace 或
terminal、不发布 feed、不负责 Electron 签名或 notarization，也不定义 update 与 cleanup policy。

## 已接受决策 D37-D46

| 决策 | 已接受契约                                                                                                                                                                                                                                                                  |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D37  | `@orchbee/ssh-environment-protocol` 拥有共享 release manifest 与 trust descriptor contract；Runtime 拥有构建和离线 bundle prepare/finalize；Desktop 拥有独立验证与 package composition。                                                                                    |
| D38  | 精确 target 为 `aarch64-macos` 与 `x86_64-linux-gnu`，二者都使用一个显式 semantic Runtime version、Zig `0.16.0`、`ReleaseSafe` 与 stripping。Release identity 绝不从 Git state 推断。                                                                                       |
| D39  | Prepared、finalized 与 packaged bundle 都有固定布局与 canonical JSON。Unknown field、entry、symlink、non-regular file、size drift、digest drift 与 platform-matrix drift 都 fail closed。                                                                                   |
| D40  | 仓库代码绝不读取 production private key。流程为 prepare → 外部或 HSM-backed raw Ed25519 signature → 使用 64-byte signature 与独立 public key finalize。                                                                                                                     |
| D41  | 一个 Desktop release 信任一个 Ed25519 key 与一个精确 manifest。其 descriptor 存储 canonical SPKI、`ed25519-spki-sha256:<digest>` 与精确 final manifest bytes 的 SHA-256。                                                                                                   |
| D42  | Trust descriptor 暂存在 `app.asar` 内；manifest 与 artifact 暂存在 Electron `Resources` 下。Desktop 在 packaging 前把所有已接受 bytes 读取、验证并缓冲进 private staging area。                                                                                             |
| D43  | 显式 Dev 与 Local packaging 排除 release payload。`desktop:package:release` 要求 absolute finalized-bundle path 与独立 public-key path；输入缺失或无效时在 Electron Packager 前失败。                                                                                       |
| D44  | 本 slice 没有 multi-key trust、feed、automatic upgrade 或 downgrade policy。Signing key 或 Runtime 变化会生成新的 Desktop package；application rollback 携带其原有 bundle 与 descriptor。                                                                                   |
| D45  | 本 slice 不组合 SSH connection、installation、supervisor 或 installed-artifact fast path，也不拥有 Electron code signing 或 notarization。                                                                                                                                  |
| D46  | Deterministic evidence 使用临时 Ed25519 key、完整 prepare/sign/finalize/verify 流程、mutation 与 canonical-layout case、双 target compilation、platform-native Runtime test 和 repository gate。测试不进入 production key；这个有限顺序工作流不需要 TLA+ 或 Lean artifact。 |

## 已接受决策 D77-D86

| 决策 | 已接受契约                                                                                                                                                                                                                                                                                                                                                                                               |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D77  | Phase 3 分 slice 实施。Slice 3.1A 只拥有 remote-execution component 构建、签名、验证与 Desktop packaging；它不创建 remote installation、process、Workspace 或 terminal effect。                                                                                                                                                                                                                          |
| D78  | 现有 `orchbee-terminal-host` 继续作为唯一 PTY、process-tree、Ghostty-state 与 terminal-recovery authority。SSH Runtime 在本 slice 不重做 terminal behavior，也不增加 generic execution、shell 或 network listener。                                                                                                                                                                                      |
| D79  | `ssh-environment-release-v2` 与冻结的 v1 相互独立，精确包含 `ssh-environment-runtime`、随后是 `terminal-host`；每个组件精确包含 `darwin-arm64`、随后是 `linux-x64`，合计四个 artifact。                                                                                                                                                                                                                  |
| D80  | Runtime 与 Terminal Host 保持不同的 semantic component version、size 与 SHA-256 digest。Signed manifest digest 标识 compatibility cohort；它不替代 Runtime build identity 或未来 Terminal Host epoch identity。                                                                                                                                                                                          |
| D81  | 两个组件都使用 Zig `0.16.0`、`ReleaseSafe`、stripping 与显式 version。精确 target 为 Apple Silicon macOS 与 x64 glibc Linux，artifact 名为 `orchbee-ssh-environment-runtime` 与 `orchbee-terminal-host`。                                                                                                                                                                                                |
| D82  | 每个 native application 拥有自己的 build recipe。仓库级 release automation 拥有跨 application prepare/finalize 并消费 installed build output；两个 application 都不导入对方 source tree。                                                                                                                                                                                                                |
| D83  | Prepare 为离线操作，finalize 接受一个外部 raw Ed25519 signature。一个 canonical signature 覆盖每个 component role、version、platform、size 与 digest。布局必须精确，symlink 和多余 entry fail closed，每个 artifact 上限 128 MiB，manifest 上限 64 KiB。Production private key 保持在仓库 tooling 之外。                                                                                                 |
| D84  | 显式 Desktop release packaging 在暂存前独立验证 signature、manifest、四个 artifact 与 public key。它把精确 trust descriptor 放在 `app.asar` 内，并把已缓冲的 manifest/artifact 放在 `Resources` 下；Dev 与 Local packaging 继续不带 release。                                                                                                                                                            |
| D85  | 冻结的 v1 artifact 只对已交付 Runtime 与 `project-root-v1` behavior 保持有效。未来 Terminal Host capability 必须要求已验证 v2，不能使用 v1 companion、remote `PATH`、remote build、network download、mixed cohort，或在 validation failure 后 downgrade。                                                                                                                                                |
| D86  | Evidence 覆盖 strict schema 与 canonical bytes、missing/reordered/expanded/oversized/tampered input、错误 key 与 signature、symlink、固定 build arguments、真实 target compilation、prepare/finalize/package round trip、documentation pairing 与 repository gate。真实 SSH、remote upload、production signing、Workspace、terminal execution 与 UI 不在 claim 内；这个有界顺序工作流不需要 TLA+ model。 |

## 版本化契约

| Contract | 组件                                    | Consumer                                                                                                               |
| -------- | --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| v1       | 两个 SSH Runtime artifact               | 为现有 Slice 2.3B 到 2.5 behavior 保留的冻结 legacy contract；新的 Desktop release packaging 不接受它                  |
| v2       | SSH Runtime 与 Terminal Host × 两个平台 | 当前 `desktop:package:release` 与 Slice 3.1B SSH activation；bootstrap 原子安装并 commit 精确 selected-platform cohort |

这些 schema 不是 permissive union。v2 path 绝不会把 v1 bytes 当作缺失 companion，也不会在 v2
validation 失败后 fallback。

## 建模记录

| 字段         | Slice 3.1A 记录                                                                                                                                                                                                                              |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | 把精确 Runtime 与 Terminal Host compatibility cohort 绑定到一个 Desktop package，同时不接受 production private key、remote download、remote build、path lookup 或 component substitution。                                                   |
| Inputs       | Trusted intent 是两个显式 semantic version、一个由外部治理的 Ed25519 signer，以及一个独立提供的 public key。Build output、prepared/final directory、signature、manifest、filesystem kind、package variable 与缺失 input 在验证前都不可信。   |
| Data         | Schema-version-2 manifest 包含有序的双 component × 双 platform matrix。Trust schema `2` 绑定 canonical SPKI、其派生 signing-key identity 与精确 final-manifest digest。                                                                      |
| Outputs      | Prepare 与 finalize 向原本缺失的目标目录原子发布精确内容。Release packaging 要么以 private buffer 中已验证的完整 cohort 调用 Electron Packager，要么提前失败。Dev 与 Local packaging 不含 SSH Environment release。                          |
| Invariants   | 每个必需 component 精确存在一次，每个已签名 size 与 digest 匹配 buffered bytes，component version 保持独立，canonical manifest bytes 定义 cohort，trust 同时 pin signer 与 manifest，且 production private key 绝不是输入。                  |
| Ownership    | Native application 拥有 component build；仓库自动化拥有 cohort assembly；protocol package 拥有 data contract；外部 authority 拥有签名；Desktop 拥有 public-key normalization、trust construction、完整验证、buffering 与 package placement。 |
| Verification | Protocol 与 Desktop unit test、仓库自动化 test、两个 native target compile、documentation pairing 与 `pnpm verify`。该 workflow 有限且顺序执行，因此 deterministic transition 与 negative-path test 对这个 claim 比 concurrency model 更强。 |

## Canonical artifact

Prepare 精确输出：

```text
prepared-release/
  artifacts/
    darwin-arm64/
      orchbee-ssh-environment-runtime
      orchbee-terminal-host
    linux-x64/
      orchbee-ssh-environment-runtime
      orchbee-terminal-host
  manifest-signing-payload.json
```

Signing payload 没有 trailing newline。Finalize 接受一个匹配 public key 与一个 raw 64-byte
Ed25519 signature，重新检查四个 artifact，并以 canonical `manifest.json` 替换 payload。Final
manifest 与 trust descriptor 各有一个 trailing newline。

Release packaging 创建：

```text
app.asar:/dist/main/ssh-environment-release-trust-v2.json
Resources/ssh-environment-release-v2/
  artifacts/
    darwin-arm64/
      orchbee-ssh-environment-runtime
      orchbee-terminal-host
    linux-x64/
      orchbee-ssh-environment-runtime
      orchbee-terminal-host
  manifest.json
```

Descriptor 作为 application trust state 保持在 `app.asar` 内。Executable bytes 作为真实文件
保持在 `Resources` 下，供以后安装精确 artifact。Bundle 绝不提供自己的 trust root。

## 离线工作流

所有 path 必须为 absolute，且请求的 output directory 必须不存在。从仓库根目录运行：

```sh
pnpm release:ssh-environment:prepare --output /absolute/prepared-release --runtime-version 1.2.3 --terminal-host-version 4.5.6

# 外部 authority 对以下文件的精确 bytes 签名：
# /absolute/prepared-release/manifest-signing-payload.json
# 并写出一个 raw 64-byte Ed25519 signature。

pnpm release:ssh-environment:finalize --prepared /absolute/prepared-release --signature /absolute/signature.bin --public-key /absolute/release-public-key.pem --output /absolute/final-release

ORCHBEE_SSH_ENVIRONMENT_RELEASE_DIRECTORY=/absolute/final-release \
ORCHBEE_SSH_ENVIRONMENT_RELEASE_PUBLIC_KEY=/absolute/release-public-key.pem \
pnpm desktop:package:release
```

`pnpm desktop:package:dev` 与 `pnpm desktop:package:local` 会忽略这些 variable，并且不包含 release
cohort 或 trust descriptor。

## Trust 与当前限制

一个 Desktop package 包含一个 descriptor 与匹配 cohort。改变任一 component 或 signing key
都要求另一个 Desktop package；application rollback 会一起恢复其原有 descriptor 与 cohort。
不存在 mutable current pointer、feed、key rollover、remote downgrade、cleanup 或 component
replacement。

Manifest 暴露 component version、platform、size 与 hash。Descriptor 暴露 public key、派生 key
identity 与 manifest digest。SSH credential、Environment target、username、remote path、AskPass
value 与 private key 在结构上不存在。

Slice 3.1B 会远程安装精确 Terminal Host artifact，并通过一个 Runtime-owned opaque relay 消费它，但不改变
release signing 或 packaging ownership。Workspace admission、terminal creation/routing、Agent execution、
真实 SSH interoperability、Electron signing、notarization、publication、update distribution、component
cleanup 与 service installation 都需要以后单独接受的 slice。
