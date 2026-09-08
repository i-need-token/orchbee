# Desktop Environment Registry

## 范围与已交付边界

Slice 2.3A 交付了第一个持久 Environment identity owner。Environment registry 的语义由 Hub 拥有；在
headless Hub 出现之前，Electron Main 是共址实现和唯一 writer。当前 store 把 SSH Environment intent
写入 `environment-registry-v2.json`，可以通过下述有界 migration 读取旧版
`environment-registry-v1.json`，在打开本地 Terminal Host 之前加载，并且只暴露进程内 create、list、
get 与显式 reload 方法。

本切片没有 renderer、Browser、CLI、Project、Workspace、Checkout、terminal、filesystem、Git、Agent
或 remote-service API。创建记录不会解析 SSH target、启动 OpenSSH、安装 Runtime 或修改远程机器。测试
只使用内存和本地临时目录；不会连接真实 SSH server。

## 已接受决策 D27-D36

| 决策 | 已接受合约                                                                                                                                                                                                                                                                                                  |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D27  | Environment registry 语义由 Hub 拥有。Desktop Main 是第一个共址 owner；Workbench 与推测性共享 package 不拥有或复制它。                                                                                                                                                                                      |
| D28  | `EnvironmentId` 与 SSH `runtimeInstanceId` 是相互独立、不可变的随机 128-bit identity。两者都不从 SSH address 或另一个 identity 推导。                                                                                                                                                                       |
| D29  | 版本 1 只准入 SSH record，并持久化有界 display name、显式 SSH target intent 与 Runtime identity。Secret、host key、已解析 target observation、连接状态、Runtime epoch 与 diagnostic 永不进入 registry。                                                                                                     |
| D30  | Slice 2.3A 只提供 create、list 与 get。Record 保持插入顺序；精确 SSH target 唯一；允许重复 display name。调用方预分配的相同 record 可幂等 replay，而相同 identity 或 target 携带不同 intent 时产生 conflict。Edit、repoint、delete 与远程 cleanup 均不可用。                                                |
| D31  | 有界版本化文件使用严格解析、owner-only directory 与 file、串行原子替换、file synchronization 和 parent-directory synchronization。加载时 missing 表示 empty；invalid 或 unreadable data 会阻断；rename 后的 durability failure 属于 indeterminate，必须显式 reload 后才能继续操作。不存在旧格式 migration。 |
| D32  | Slice 2.3B 把一份精确的离线双平台 Runtime release bundle 与 Desktop 组合。远程 bootstrap 不通过网络下载。                                                                                                                                                                                                   |
| D33  | Slice 2.3B 把 Ed25519 signing-key identity 与精确 manifest trust descriptor 绑定到仅用于 release 的 Desktop packaging。生产 private key 位于 repository 与普通 CI 之外。                                                                                                                                    |
| D34  | 不兼容 version 或 digest 的存活远程 Runtime 会阻断；Desktop 不会杀死或静默升级它。                                                                                                                                                                                                                          |
| D35  | Slice 2.3C 将在 Hello 前增加一个有界、同一 child 的 installed-artifact negotiation，且不会把远端 status token 当成 trust decision。                                                                                                                                                                         |
| D36  | Slice 2.3A、2.3B 与 2.3C 分别授权且可独立回滚。本切片只交付 durable registry 及其 evidence。                                                                                                                                                                                                                |

## 已接受扩展 D192

| 决策 | 已接受合约                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D192 | Registry 版本 2 用规范化、有界且经过评审的连接 option 列表扩展精确 SSH target intent。Desktop 优先读取版本 2。只有版本 2 文件不存在时，才能在内存中验证并迁移版本 1；之后首次 commit 写入版本 2，同时保持版本 1 的每个 byte 不变。版本 2 文件无效或不可读时会阻断，而不是 fallback。旧版 Desktop 因而可以回滚到未修改的版本 1 snapshot，但无法观察首次由版本 2 准入的 Environment。本决策只针对当前 schema 及其显式 migration 取代 D29 与 D31。 |

## Authority 与 identity model

`EnvironmentId` 是 provider-neutral 产品 identity，canonical form 为
`environment:<32 lowercase hex characters>`。`runtimeInstanceId` 是现有 SSH Runtime protocol identity，
恰好包含 32 个小写十六进制字符。调用方在提交 create 前分配两者，在 indeterminate result 后保留它们，
并且绝不根据 destination、alias、configuration path、resolved hostname、user 或 port 重新计算。

当前 record 的 canonical shape 如下：

```json
{
	"environmentId": "environment:11111111111111111111111111111111",
	"name": "Build machine",
	"provider": {
		"kind": "ssh",
		"runtimeInstanceId": "22222222222222222222222222222222",
		"target": {
			"configurationFilePath": null,
			"destination": "build-host",
			"options": [
				{ "enabled": true, "kind": "forward-agent" },
				{ "kind": "port", "value": 2222 }
			]
		}
	}
}
```

SSH target key 是 nullable absolute configuration-file path、concrete destination 与规范化 structured
option 的精确 triple。Singleton option 无论输入顺序如何都采用一个稳定顺序；重复 identity-file option
保留相对顺序。一个 target 只有一个 Environment，因为未来多个 Project path 应位于该 Environment 下。
Target resolution 仍是 OpenSSH 拥有的瞬时 observation，不能静默改变产品 identity。不同 alias、
configuration path 或 option value 表示不同的显式 intent；发现或合并 physical-machine alias 仍是保留决策。

## Append-only lifecycle

严格的版本 2 document 存储 `schemaVersion`、`revision` 与 `environments`。由于该 owner 没有 update 或
deletion，`revision` 等于已准入 record 数量。

| 当前条件                             | 操作或结果 | 权威结果                                                                |
| ------------------------------------ | ---------- | ----------------------------------------------------------------------- |
| 版本 2 与版本 1 都 missing           | Load       | 以 revision `0` 和空 record 就绪。                                      |
| 有效版本 2 文件                      | Load       | 以精确的已验证 order 与 revision 就绪。                                 |
| 版本 2 missing 且存在有效版本 1      | Load       | 以内存中的版本 2 projection 就绪；不会写入任一文件。                    |
| 版本 2 无效，或 fallback 版本 1 无效 | Load       | Unavailable；不会创建替换文件。                                         |
| 未使用的 identity 与 target          | Create     | 只 append 一次，并在 durable commit 后推进 revision。                   |
| 相同 identity 与完全相同 record      | Create     | 返回 `existing`；不写入且不推进 revision。                              |
| 相同 identity 携带不同 intent        | Create     | 返回 `environment-id` conflict；保留 state。                            |
| 另一个 identity 使用相同精确 target  | Create     | 返回 `ssh-target` conflict；保留 state。                                |
| 达到 capacity                        | Create     | 不写入并拒绝。                                                          |
| Store revision 在 commit 前发生变化  | Create     | 刷新可信 snapshot 并返回 conflict；不静默重新排序或 retry request。     |
| Atomic rename 前失败                 | Create     | 返回 unavailable，同时保留可读的旧 snapshot 并允许调用方控制 retry。    |
| Rename 后确认 parent directory 失败  | Create     | 返回 indeterminate，丢弃可信内存 snapshot，并在显式 reload 前阻断操作。 |

Controller 按到达顺序串行化每次 load 与 operation。返回的 record 是 detached copy，因此调用方无法修改
权威 snapshot。没有代码路径会改变既有 target、Runtime identity、name 或 order，也没有产品路径会删除
registry file。

## 持久化与恢复

Desktop 把当前文件解析为 `<userData>/environment-registry-v2.json`。边界如下：

| 限制                     | 值                                                              |
| ------------------------ | --------------------------------------------------------------- |
| Schema version           | 恰好为 `2`                                                      |
| 最大 encoded JSON byte   | 1 MiB，磁盘上另加一个 trailing newline                          |
| 最大 Environment record  | 1,024                                                           |
| 最大 display-name length | 256 个 UTF-16 code unit，非空、已 trim 且不含 control character |
| 最大 SSH target option   | 32 个规范化且经过评审的 option                                  |
| Environment identity     | `environment:` 加 32 个小写十六进制字符                         |
| Runtime identity         | 32 个小写十六进制字符                                           |

在 POSIX 上，creation 确保 `0700` parent directory 与全新的 `0600` temporary file。Adapter 写入并同步
temporary file，以原子 rename 覆盖 registry，然后同步 parent directory。Temporary-name collision 或
其他 rename 前 error 会保持旧文件不变。如果 rename 已完成但 directory synchronization 失败，adapter
不会宣称成功。显式 reload 观察 filesystem 呈现的任一严格 document；调用方随后可以安全 replay 相同的
预分配 record。恢复不会保留隐藏的 pending request，也不会预留 caller ordering：rename 前失败后，或
indeterminate result 完成 reload 后，每个后续 create 都会根据恢复后的 canonical document 重新求值。
replay 相同 identity 是安全的，但不是必需的。

只有当前文件缺失且不存在有效的旧版版本 1 文件时，missing current file 才表示 empty。版本 2 缺失时，
Desktop 可以读取 `<userData>/environment-registry-v1.json`、严格验证它，并把相同 revision 与 record
投影为不含 option 的版本 2 shape。在后续 create 提交完整版本 2 document 前，migration 只读。一旦版本 2
存在，它就是唯一 authority；无效或不可读的版本 2 永不 fallback。如果 store 预期已经准入的非零
revision，但两个文件都消失，该 commit 会阻断，而不是重建 empty registry。版本 1 永不重写或删除，
因此旧版 Desktop 可以回滚到该 snapshot 且不会破坏它；它有意无法看到仅存在于版本 2 的 record。后续
schema change 仍必须拥有显式 migration 与 rollback contract。

## 隐私与信任边界

该文件包含 user-selected label、SSH destination 或 alias、可选 absolute SSH configuration path、
规范化且经过评审的 option 与 opaque identity。Option value 可能包含 user name、port、jump destination
或 identity-file path。这类 inventory 本身也可能敏感，因此它留在 owner-only Desktop data root 内，
不会作为 diagnostic、trace、telemetry field、renderer bootstrap 或 Browser payload 发出。显式 SSH
management UI 只能接收用于选择的有界 canonical command projection；它永不接收 registry document。

Password、passphrase、one-time code、private-key content、agent socket、`known_hosts` data、resolved
target observation、host-key lookup name、Runtime artifact、Runtime epoch、availability、retry state 与
prompt text 在结构上不存在。显式 identity-file path 是 metadata，绝不是 key material。OpenSSH 仍是
configuration、credential 与 host verification 的 authority。持久化一条 record 不会授予连接或远程
effect。

## 建模与验证

`EnvironmentRegistryCommit` 是针对 independently ordered create、atomic rename、pre-commit failure、
post-rename uncertainty、process restart、reload 与 caller replay 的有界 TLA+ model。它检查 canonical
identity 与 target uniqueness、稳定 append order、revision/count agreement、capacity、已保留成功 claim，
以及 reload fairness 下离开 indeterminate phase 的 progress。
Failure 与 reload 不会保留 request-owned gate；下一个 caller 可以提交任意 record，并按普通到达顺序对
恢复后的 document 进行检查。

| 声明                                                 | 确定性 evidence                                                  | 边界与排除项                                                          |
| ---------------------------------------------------- | ---------------------------------------------------------------- | --------------------------------------------------------------------- |
| Identity 与精确 target 保持唯一                      | Example test 加 fixed-seed stateful property trace               | 八个 generated identity 与 target；不会推断 physical SSH-host alias。 |
| Create 幂等且 insertion order 稳定                   | Pure transition test 与 controller test                          | 只有 create/list/get；没有 edit 或 delete。                           |
| Concurrent caller 不能复用 stale in-memory revision  | Controller serialization 与真实 file-store competing-commit test | 一个 Desktop process 拥有 Electron single-instance lock。             |
| Invalid 或 oversized data 永不变成 empty replacement | Strict-schema 与 real-file test                                  | Operating-system failure code 会缩减为有界 reason。                   |
| Rename 前 failure 保留旧文件                         | Temporary-file collision fault injection                         | 成功 sync 后的 filesystem 或 hardware behavior 仍由 platform 决定。   |
| Rename 后 sync failure 永不报告 durable commit       | Directory-sync fault injection 与 `EnvironmentRegistryCommit`    | 必须显式 reload；当前没有 UI 发起它。                                 |

从 repository root 运行聚焦边界：

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop exec vitest run src/main/environment/__tests__
node scripts/tla/run-models.mjs --models desktop-environment-registry-commit
```

## 当前限制与下一权威

Slice 2.3A 不向 renderer 或 remote Client 暴露 Environment management，不自动创建 record，不解析或连接
其 SSH target，不组合 `SshEnvironmentSupervisor`，不协商 installed artifact，不增加 Runtime service，也不
把 Project、Checkout、Directory、Workspace、terminal、Git operation 或 Agent 映射到 Environment。Slice
2.3B 现在独立 package signed Runtime release，并且不授予任何上述 registry 或 connection effect。

Slice 2.3C 单独组合了显式进程内 activation 与 installed-artifact fast path，且不会让 registry
load/create 自动连接。D192 随后引入上述有界 version-1-to-version-2 migration，但没有改变该非激活规则。
Environment rename、target edit、removal、remote cleanup、provider expansion、physical-host alias
reconciliation、任何后续 migration 与 Project placement 需要后续已接受决策；不能从这个 append-only
file 中推断它们。
