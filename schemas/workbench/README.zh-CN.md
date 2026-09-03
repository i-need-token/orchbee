# Workbench 跨语言 Contract

## 权威

[`v1/workbench.schema.json`](v1/workbench.schema.json) 是 version-1 只读 Workbench projection
kernel 的语言中立权威。`@orchbee/workbench-client-protocol` 中的 TypeScript binding 与
`orchbee-workbench-protocol` 中的 Rust binding 是独立实现。任何 binding 都不能增加可接受值、
省略 invariant 或重新定义字段含义。

Version 1 使用由 JSON Schema 2020-12 约束的已解码 JSON value。每个 object 都是 closed，
每个 collection 与 string 都有边界，identity 是有类型且非零的 128-bit 小写十六进制字符串，
revision 是 canonical unsigned-64 十进制字符串。transport 必须在 JSON decoding 前拒绝超过
16 KiB 的 encoded intent，或超过 1 MiB 的 encoded snapshot 或 delta。Transport framing、
duplicate-key handling、authentication、authorization、redaction 与 version negotiation 不属于
本切片。

[`v1/workbench-conformance-v1.json`](v1/workbench-conformance-v1.json) 是依据 contract 编写的
跨语言 oracle。Valid 与 invalid document 覆盖 strict parsing 和 semantic invariant；transition
case 覆盖确切 delta result 与 typed rejection。实现必须消费每个 case，不能用自己的 parser 或
reducer 重写 expected result。

## Projection

`workbench.snapshot` 是一个 Site revision 的完整 authorized product projection。它包含：

- 有序 Project 及各自有序 Workspace；
- 恰好一个 standalone Terminal Context，以及每个 Workspace 各一个 Terminal Context；
- 有序 Tab group 与 Tab；
- 每个 Tab 的完整 binary Panel/Split topology；
- 每个 Panel 的一个 primary Terminal Presentation 与零个或多个 child Terminal Presentation；以及
- Presentation 可选的 opaque Worker/Terminal binding。

Projection 包含 label，但不包含 directory、path、terminal content、terminal lifecycle policy、
Environment、Checkout、Worktree、Agent、Launch Profile、Archive、SSH、lease、fence、Host epoch、
attachment 或 persistence fact。它也不包含 Client-local active location、active Tab、focus、
disclosure、selection、maximization、split weight、viewport、geometry、search、window、DOM、React
或 pixel state。Presentation 缺少 binding 只表示本 projection 没有暴露可 attach 的 Terminal
identity；Client 不能据此推断 lifecycle reason。

Project、Workspace 与 Terminal Context array 使用 identity 升序。Tab group 与 Tab 保留显式产品
顺序。Pinned Tab 构成一个前置且不分组的 prefix，每个 Tab group 的 member 都连续。每个被引用
context 恰好存在一次，每个 context 也恰好被引用一次。除 JSON Schema validation 外，identity
uniqueness、tree depth、node quantity、membership 与 reference invariant 还作用于完整 snapshot。

## Intent 与 delta

`workbench.snapshot.read` 是 version-1 唯一准入的 semantic intent。它请求完整 authorized
projection，不拥有 transport、response envelope、pagination、command-line grammar 或 mutation
behavior。

`workbench.delta` 命名一个确切 Site、base revision、更大的 result revision，以及一组无序且原子
的 aggregate change。一个 change 可以替换完整 navigation、upsert 一个完整 Terminal Context，
或移除一个完整 Terminal Context。至多允许一次 navigation replacement，且每个 Terminal Context
identity 至多一个 change。当 authorized Site fact 的变化位于本 projection 外时，空 change set
可以推进 projection revision。

`applyWorkbenchDelta` 无副作用地执行以下步骤：

1. 拒绝 Site mismatch、base-revision mismatch 或 non-increasing result revision；
2. 拒绝重复 navigation 或 Terminal Context target；
3. 暂存每个 aggregate replacement，并按 identity 排序 Terminal Context；
4. 校验完整 candidate snapshot；以及
5. 只有所有检查成功后才返回完整 next snapshot。

任何 rejection 都保持 caller 的 snapshot 不变。恢复时获取完整 snapshot；本切片不定义 replay
buffer、retry、persistence 或 transport request。

## 限制

| 值                        | Version-1 限制                 |
| ------------------------- | ------------------------------ |
| Encoded read intent       | 16 KiB                         |
| Encoded snapshot 或 delta | 1 MiB                          |
| Product label             | 512 个 Unicode code point      |
| Project                   | 每个 snapshot 256 个           |
| Workspace                 | 每个 Project 256 个            |
| Terminal Context          | 每个 snapshot 1,024 个         |
| Tab group                 | 每个 Terminal Context 64 个    |
| Tab                       | 每个 Terminal Context 256 个   |
| Child Presentation        | 每个 Panel 16 个               |
| Panel/Split depth         | 从 Tab root 开始 16 个 node    |
| Panel/Split node          | 每个 Terminal Context 4,096 个 |
| Delta change              | 每个 delta 1,024 个            |

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/workbench-client-protocol test
cargo test -p orchbee-workbench-protocol --locked
pnpm verify
```

这些检查只证明 schema/binding conformance 与 pure delta refinement。它们不证明 Server、
transport、authorization policy、persistence model、renderer、CLI command 或 mobile
implementation。
