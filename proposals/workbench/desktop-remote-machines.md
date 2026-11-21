# Remote Machines in the Desktop Window

| Field              | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Accepted for building on 2026-11-20, except the window design, which the owner reviews before slices 6 to 9, and open question 4, which the owner decides. Slice 1 is the Contracts section; nothing a person can reach changes before the picker's slice                                                                                                                                                                                                        |
| Owner              | Desktop (`apps/desktop`), the Web Workbench (`apps/web`), the native Client's machine modules in the workbench Client crate, and the machine hub maintainers                                                                                                                                                                                                                                                                                                     |
| Current authority  | [`machines.md`](../../apps/orchbee/docs/machines.md) for the hub and the TUI, [`schemas/machine`](../../schemas/machine/README.md) for the hub contract, the [Desktop guide](../../apps/desktop/README.md) for the window's processes and ports, and [`INTERFACE-DESIGN.md`](../../docs/INTERFACE-DESIGN.md) for what both Clients share                                                                                                                         |
| Decision authority | The product owner decided on 2026-11-20 that the desktop window gets remote machines now, without waiting for browser transport, and that the TUI's design leads: the window reuses its logic, catalog, flows and words. The architecture and the first-release cut were accepted on 2026-11-20, and the recommended answers to the [open questions](#open-questions-for-the-owner) stand as vetoable picks; the window design and question 4 remain the owner's |
| Exit condition     | Build the accepted slices, move each maintained fact into its owning guide, schema and tests, then delete this proposal                                                                                                                                                                                                                                                                                                                                          |

## Goal and scope

A person in the desktop window reaches their saved Machines as the TUI does: the same list, state
words, logins and management, and the same way of opening a Machine's terminals. Only presentation
differs, as [`INTERFACE-DESIGN.md`](../../docs/INTERFACE-DESIGN.md#common-feature-contract) requires.

The scope is the Desktop window on Windows and macOS, where the `SavedMachines` host capability
holds. A browser tab needs browser transport first; Linux has no saved machines; remote
installation is missing from every Client and stays out.

## The reference: the TUI today

- **One hub.** Every connection to a Machine belongs to the [machine hub](../../apps/orchbee/docs/machines.md#machine-hub)
  of the data root. The TUI is one of its clients: a background holder attaches with `presence`,
  `asks` and `watch` ([the TUI as a hub client](../../apps/orchbee/docs/machines.md#the-tui-as-a-hub-client)).
- **Picker.** `Ctrl+b Shift+M`, or its row in the global actions menu, opens a centered picker with
  Machines, Workspaces and Agents modes. Machines lists Local, then each saved Machine with its name,
  address and state word; the selected row adds its details, its action on Enter and Actions on F2;
  a fixed "Add machine…" row ends the list ([managing machines](../../apps/orchbee/docs/machines.md#managing-machines-in-the-tui)).
- **Logins.** The person's own selection asks in the login overlay; a login the hub offers for a
  waiting command shows first as a notice guarded against typing; one at a time
  ([login questions](../../apps/orchbee/docs/machines.md#login-questions)).
- **Words.** `machine_status` chooses each row's word, tone, details and action, and its
  localization words them ([state words](../../apps/orchbee/docs/machines.md#state-words),
  [states and details](../../apps/orchbee/docs/machines.md#states-and-details)).
- **Surfaces.** One Session's surface is on screen. Local stays connected and at most four inactive
  remote surfaces are cached. A switch commits only a drawable candidate, and failure or Escape
  keeps the current surface ([persistent connections](../../apps/orchbee/docs/machines.md#persistent-tui-connections),
  [selection](../../apps/orchbee/docs/machines.md#selection)).
- **Sidebar.** The sidebar lists the Workspaces of every enabled Machine; while several are enabled,
  each group heading and row starts with the Machine's name and `/`. Activating a remote Workspace
  switches to it ([cross-machine navigation](../../apps/orchbee/docs/machines.md#cross-machine-workspace-navigation)).
- **Session bytes.** A remote surface's ordinary Client connection runs on a private local endpoint
  that copies each connection onto a hub `session` channel, and the remote Session admits it as it
  would a local Client ([Session carrier](../../apps/orchbee/docs/machines.md#session-carrier)).

## The window today

- One window shows one document at a time. Each document gets one `orchbee server connect` process
  and four ports: the local Session's Client bytes, folder acquisition, background status and window
  update ([server connection](../../apps/desktop/README.md#server-connection)).
- Main runs `orchbee server start` for each document and announces what only this computer knows:
  its home folder, host name, login shell and the negotiated capabilities.
- Nothing in the window attaches to the hub or names a Machine. The
  [attachment contract](../../schemas/machine/README.md#attachment) already states what Desktop's
  bundled Client will be to the hub, `presence` for the Desktop's whole process life and `asks` only
  while a window exists; `machines.md` leaves it to a later slice. This proposal is that slice.

## Responsibilities

| Responsibility                                                                                                                                                                        | Owner                                                                                                                                                                                                                                                                                                                                                          | Rejected                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Desktop's attachment to the hub, catalog reads and edits, the state of each Machine, login episodes, add checks, management, details and actions                                      | The **machines process**, a long-lived internal entry of the bundled Client (`orchbee --desktop-machines-v1`) that Main starts and supervises. It is built from the workbench Client crate's interface-free modules, the same ones the TUI's holder, picker and overlays use, and is the Desktop's one client of the hub                                       | A TypeScript hub client in Main: a second client owner of the hub and a second copy of the catalog, login and add rules. The page: sandboxed, without local IPC, and a second business writer in a renderer. The local Session's Server: the hub belongs to the data root and outlives Sessions, and a Server would carry other machines' login secrets through the Session; it stays the path for browser clients. `server connect`: it lives per document and relays opaque bytes, while `presence` must last the Desktop's life                                                                          |
| A remote Session's Client bytes for the page                                                                                                                                          | A **session relay** per connection, an internal entry of the bundled Client (`orchbee --machine-session-v1 <machine>`): it joins the running hub, opens one `session` channel to the Machine, performs the admission the TUI's Client performs through its carrier, announces the outcome on its first output line, then copies bytes as `server connect` does | The machines process multiplexing Session bytes on its own pipes: Main would split a Client protocol it never speaks, and one slow surface would share a pipe with the machine list. `orchbee --machine <m> server connect`: `--machine` routes bounded command packets, and a byte stream through it would change the public [routing contract](../../apps/orchbee/docs/machines.md#command-routing-contract). The machines process opening the TUI's local carrier endpoints for `server connect` to dial: two processes coupled through a private endpoint name, for nothing one channel per relay lacks |
| Processes, ports and the window's presence                                                                                                                                            | **Desktop Main**: starts and restarts the machines process, relays the page's machine records to it in an envelope it never opens, tells it when a window exists, starts a session relay for each remote connection the page asks for, reads only that relay's first line, and relays the rest with the existing `ServerConnectionRelay` bounds                | Main reading machine records: it would become a third interpreter of the view, and of the secrets inside answers                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Presentation: the Machines picker, login and notice Dialogs, the Add form, the Actions menu, confirmations, the sidebar's machine prefix, status notices; focus, highlight and scroll | **The page** (`apps/web`), with copy in a new `machines` domain of `@orchbee/i18n` whose en-US and zh-CN entries are the TUI's words, checked against shared examples                                                                                                                                                                                          | The machines process sending worded text: it would bypass `@orchbee/i18n`, the frontend's only product localization system, and tie a language switch to a native process                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Holding more than one Session in the page                                                                                                                                             | **The Desktop composition** in `apps/web` (`desktop-client`): it keeps the local Session's runtime for the page's life and, in the first release, one remote runtime at a time; nothing below it knows that more than one Session exists                                                                                                                       | Switching by reloading the document onto another Session: failure could not keep the current surface; Quit All, the notification-area counts and Agent notifications would follow whichever Session is shown, so a Quit All in a remote document would stop the remote Session; every return to this computer would reload. One window per Machine: several windows are outside the [window lifecycle](../../apps/desktop/docs/window-lifecycle.md) contract                                                                                                                                                |

```text
page (sandboxed renderer)
 ├─ port "server-connection" ─► Main ─ pipes ─► orchbee server connect ───────────► local Session
 ├─ port "machines" ──────────► Main ─ pipes ─► orchbee --desktop-machines-v1 ─────► machine hub ═ssh═► Machines
 └─ one port per remote ──────► Main ─ pipes ─► orchbee --machine-session-v1 <m> ─► machine hub ═session channel═► remote Session
```

## Multi-machine display

| Aspect             | TUI                                                                                    | Window, first release                                                                                                                             | Window, later                     |
| ------------------ | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| On screen          | One Session's surface                                                                  | The same                                                                                                                                          | The same                          |
| Kept connected     | Local, plus at most four inactive remote surfaces                                      | Local, plus the remote surface on screen                                                                                                          | Local, plus four inactive remote  |
| Sidebar            | Workspaces of every enabled Machine, prefixed `<machine> / ` while several are enabled | Workspaces of the Session on screen, with the same prefix rule                                                                                    | Every enabled Machine, as the TUI |
| Picker modes       | Machines, Workspaces, Agents                                                           | Machines                                                                                                                                          | All three                         |
| Switching          | A drawable candidate commits; failure, expiry or Escape keeps the current surface      | The same                                                                                                                                          | The same                          |
| Window-wide duties | Settings and update act on this computer                                               | Settings, Update, Quit All, notification-area counts and Agent notifications follow this computer's Session, also while a remote surface is shown | The same                          |

A remote surface carries only its Workspaces, Tabs and Panels. It never receives Quit All's Session
stop, and it offers no native folder picker, whose paths name this computer; folder choice there
falls back to what the Workbench offers where no picker is injected, as in a browser.

Returning to Local is immediate, because its runtime stays connected. A switch between two remote
Machines connects the new one first and ends the old one once the new surface is drawable.

## First release

In the first release:

1. The Machines picker with Local and every saved Machine: name, address, state word, the selected
   row's details, its action on Enter, the Actions menu with all six items, and Add machine.
2. Choosing a Machine runs the TUI's selection: a Machine the hub reads `online` opens without
   connecting again, and any other is connected with origin `user`; the TUI's 60 seconds bound the
   wait, a login's waiting not counted, and only a drawable surface replaces the one on screen.
3. The login Dialog for the person's own selection, and the command-login notice for logins the hub
   offers on behalf of waiting commands, with the TUI's queue and deferral.
4. The remote surface: its lost link, its automatic return to the Workspace it showed, the handshake
   refusals the TUI names, and a return to Local when the Machine on screen is removed or disabled.
5. The status notices for a stopped, unavailable or failed hub.
6. The sidebar of the Session on screen, prefixed as in the TUI.

Later:

- Workspaces and Agents modes, and other Machines' Workspaces and Agents in the sidebar. They need
  an observer per `online` Machine in the machines process and a bounded projection of their
  summaries to the page.
- A cache of four inactive remote surfaces.

Until the later slices land, the window states its limit honestly: the picker shows only the
Machines mode, and the [Desktop guide](../../apps/desktop/README.md) records the missing counterpart,
its owner and its completion condition, as staged delivery requires.

## Window design

Every surface below is the TUI's, drawn with the GUI's interaction vocabulary: the picker, the
login question, the notice and the Add form use the
[compact Desktop Dialog shell](../../docs/frontend/dialogs-and-forms.md#compact-desktop-dialog-shell),
the Remove and stop step uses the Alert Dialog's rules, and the Actions menu is the reviewed action
menu. The words are the TUI's, from the localization modules named in [Copy](#copy). Names,
addresses and counts in the drawings are examples.

### Entry

- `Mod+Shift+M` (`⌘⇧M` on macOS, `Ctrl+Shift+M` on Windows) opens the picker: the TUI's
  `Ctrl+b Shift+M` translated by the [keyboard transfer rules](../../docs/INTERFACE-DESIGN.md#keyboard-transfer-rules).
  It is the Workbench Action `workbench.machines.open`, so it appears wherever Workbench Actions do.
- In Desktop today that is only the Actions search `Mod+Shift+P` opens: Search and its sidebar
  button are withheld until Search is finished, and neither the application menu nor the
  notification-area menu lists Workbench Actions. Without the entry of
  [open question 4](#open-questions-for-the-owner), a person who uses only the mouse cannot open the
  picker in the first release.
- The Action's label is the TUI's "Browse machines, Workspaces and Agents" (浏览机器、工作区和智能体)
  once all three modes ship. Until then it reads "Browse machines" (浏览机器), a documented short
  variant, so it promises no mode the window lacks.

### Picker

```text
┌ Machines ────────────────────────────────────────────────── × ┐
│  Local                                               Current  │
│▐ devbox      dev@10.0.0.8                             Online  │
│    Orchbee 0.1.0-beta.22                                      │
│    Selected: 2 projects · 5 workspaces · 9 tabs               │
│      api                                                      │
│      web                                                      │
│                                       [ Actions ]  [ Open ↵ ] │
│  nas         admin@nas.local                         Offline  │
│  mac-mini    me@mac-mini                         Needs login  │
│  Add machine…                                                 │
├───────────────────────────────────────────────────────────────┤
│ Esc Close · ↑/↓ Choose · Enter Open · F2 Actions              │
└───────────────────────────────────────────────────────────────┘
```

```text
┌ Machines ─────────────────────────────────────────── × ┐
│▐ Local                                       Current   │
│  Add machine…                                          │
│  Connect another computer over SSH to use its          │
│  terminals and Agents here.                            │
├────────────────────────────────────────────────────────┤
│ Esc Close · ↑/↓ Choose · Enter Open                    │
└────────────────────────────────────────────────────────┘
```

- Rows keep the TUI's order and content: Local first, each saved Machine with its name, its address
  in the secondary color and its state word in the word's tone, then the fixed "Add machine…" row.
  The row of the surface on screen says "Current" (当前).
- The selected remote row opens in place to its details and two buttons, Actions and its action,
  the action last. Enter or a click on the action performs it; F2, the Actions button or a
  right-click opens the Actions menu. Local and "Add machine…" show neither.
- The title is the mode's name. The TUI's `{mode} {i}/{n}` position counter is a terminal density
  aid; the window's highlight and scroll carry it.
- The bottom row is the TUI's: its key hints, or one status sentence that replaces them while it
  applies. Mode controls appear only when Workspaces and Agents ship.

| TUI state                         | Window                                         | Copy (en-US / zh-CN)                                                                                                                                                                              |
| --------------------------------- | ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Row words and tones               | The row's word in its tone token               | The state word table: Online / 在线 … Unavailable / 不可用; Disabled / 已停用                                                                                                                     |
| Selected row's details and action | Lines under the row, action button             | `machine_status` details and actions: Open / 打开, Retry / 重试, Retry now / 立即重试, Reconnect / 重新连接, Enable / 启用, Log in / 登录, Log in again / 重新登录, Review and trust / 查看并信任 |
| Only Local                        | The line under the list                        | Connect another computer over SSH to use its terminals and Agents here. / 通过 SSH 连接另一台电脑，在这里使用它的终端和智能体。                                                                   |
| Selection running                 | Bottom row; the row shows its in-progress word | Switching… Esc cancels selection; background discovery continues. / 正在切换… Esc 取消选机，后台发现继续。                                                                                        |
| Selection failed                  | Bottom row, picker stays open                  | Connection failed. Check SSH readiness, then choose to retry. / 连接失败。请检查 SSH 就绪状态，然后重新选择以重试。 Or how the login ended, or the refusal (below)                                |
| Machine removed while chosen      | Bottom row                                     | Machine removed or disabled. Choose Local or another machine. / 机器已删除或停用，请选择本地或其他机器。                                                                                          |
| Hub stopped                       | Rows read Stopped; bottom row                  | Machine connections stopped. Choose a machine to start them. / 机器连接已停止，选择机器即可重新开始。                                                                                             |
| Hub unavailable                   | Rows read Unavailable; bottom row              | Machine connections are unavailable. Local remains available. / 机器连接不可用，仍可使用本地。                                                                                                    |
| Hub cannot start                  | Rows read Unavailable, error tone; bottom row  | Machine connections cannot start; the machine's details say why. / 机器连接无法开始，原因见机器详情。 The selected row's details say why and what to do, naming the held version                  |
| Catalog unreadable                | Bottom row; Local stays                        | Machine catalog unavailable. Local remains available. / 机器目录不可用，仍可选择本地。                                                                                                            |
| Handshake refusal                 | Row word, bottom row                           | The refusal sentences, such as The remote Orchbee belongs to another environment. / 远端 Orchbee 属于另一个环境。 with their second lines                                                         |
| A login waits behind the keyboard | Bottom row, and the window's status notice     | devbox needs login / devbox 需要登录                                                                                                                                                              |

### Login question

```text
┌ Log in to machine ───────────────────────────────── × ┐
│ devbox · dev@10.0.0.8                                 │
│ (ssh's prompt, as the TUI shows it)                   │
│ Password for dev@10.0.0.8:                            │
│ [ ••••••••                                          ] │
│ [ ] Remember password                                 │
│ Goes only to ssh. Orchbee does not keep it.           │
│                          [ Cancel Esc ]  [ Log in ↵ ] │
└───────────────────────────────────────────────────────┘
```

```text
┌ Log in to machine ───────────────────────────────── × ┐
│ devbox · dev@10.0.0.8                                 │
│ New host dev@10.0.0.8, ED25519 key:                   │
│ SHA256:(the whole fingerprint, wrapped, never cut)    │
│ Type yes to trust it:                                 │
│ [ yes                                               ] │
│              [ Cancel Esc ]  [ Trust and continue ↵ ] │
└───────────────────────────────────────────────────────┘
```

- It opens only for the selection the person waits on, or for a command login the person accepted
  from its notice; any other login is dismissed on arrival, as the TUI's "awaited only" rule says.
- The secret field masks each character, except that a new host's field shows what is typed while it
  begins `yes`. The field clears on submission, cancellation, failure and unmount, and the Dialog
  states where the answer goes and whether it is kept, as the
  [field contract](../../docs/frontend/dialogs-and-forms.md#field-contract) requires and the TUI's
  privacy line already words.
- A refused answer asks again with an empty field and the repeated line; several questions of one
  login follow in the same Dialog. A login that ends without the person keeps the Dialog, without
  the field, until Close.

| TUI state                   | Copy (en-US / zh-CN)                                                                                                                                                                                                                                                                                     |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Title and machine line      | Log in to machine / 登录机器; `{name} · {target}`                                                                                                                                                                                                                                                        |
| Password, passphrase, other | Password for {target}: / {target} 的密码：; Key passphrase for {target}: / {target} 的密钥口令：; Answer for ssh above, connecting to {target}: / 回答上面 ssh 的提问（正在连接 {target}）：                                                                                                             |
| New host                    | New host {target}, {algorithm} key: / 新主机 {target}，{algorithm} 密钥：; Type yes to trust it: / 输入 yes 以信任：; Type yes to trust this host. / 输入 yes 以信任这台主机。                                                                                                                           |
| Remember box and privacy    | Remember password / 记住密码 (or its until-restart form); Goes only to ssh. Orchbee does not keep it. / 只交给 ssh，Orchbee 不保存。; Goes to ssh, then to the system password store once logged in. / 交给 ssh，登录成功后存入系统的密码存放处。; the two no-store lines                                |
| Asked again                 | Wrong password. Try again. / 密码不对，请重试。; Wrong passphrase. Try again. / 口令不对，请重试。; Not accepted. Try again. / 未被接受，请重试。; The saved password no longer works. / 保存的密码已失效。                                                                                              |
| Sending                     | Logging in… / 正在登录…                                                                                                                                                                                                                                                                                  |
| Buttons                     | Cancel / 取消; Log in / 登录; Trust and continue / 信任并继续; Close / 关闭                                                                                                                                                                                                                              |
| Ended alone                 | Logged in. / 已登录。; Login failed. Choose the machine to try again. / 登录失败。重新选择这台机器可再试一次。; The answer did not reach ssh, so signing in stops. / 回答未能送达 ssh，登录已停止。; The login was not completed. Choose the machine to log in. / 登录未完成。重新选择这台机器即可登录。 |

### Command-login notice

```text
┌ Log in to machine ───────────────────────────────────┐
│ devbox · dev@10.0.0.8                                │
│ 2 commands are waiting to log in to this machine.    │
│ If you don't log in now, these commands will fail.   │
│                        [ Not now Esc ]  [ Log in ↵ ] │
└──────────────────────────────────────────────────────┘
```

- It opens when the hub offers this window a login on behalf of waiting commands. Like the TUI's
  notice it takes the keyboard, discards every key and paste that would otherwise reach a terminal,
  and lets Enter, Escape or its buttons act only after it has been fully visible for one second
  without a discarded key or paste; each discarded key, each paste and each resize starts that
  second again.
- Log in claims the login, and the login question follows. Not now dismisses it, which fails every
  waiting command and mutes the Machine's command logins as the TUI does.
- While another Dialog, the picker, a menu or the Settings page holds the keyboard, the notice
  waits, and "devbox needs login" shows in the picker's bottom row while it is open, in the
  Settings page's status row while that page is open, and otherwise in the window's status notice,
  until the notice can open.
- One notice or login question at a time, in the TUI's order. A notice another interface took, or
  whose login ended, after the person pressed Log in says why in a few words and closes on Close,
  Enter or Escape under the same one-second rule; one nobody touched closes by itself.

Copy: Log in to machine / 登录机器; 1 command is waiting to log in to this machine. / 有 1 条命令在等待登录这台机器。
and its plural; If you don't log in now, this command will fail. / 暂不登录的话，这条命令会失败。 and its plural;
Not now / 暂不登录; Log in / 登录; Logging in… / 正在登录…; the ended lines, such as Another Orchbee window took this
login. / 另一个 Orchbee 窗口接手了这次登录。

### Add machine

```text
┌ Add machine ───────────────────────────────────────── × ┐
│ Address                                                 │
│ [ user@10.0.0.8                                       ] │
│ Hosts in SSH config:  devbox  nas  mac-mini             │
│ Name (optional)                                         │
│ [                                                     ] │
│ Leave blank to use the host name.                       │
│ Checking 3 of 8: host key…                              │
│                               [ Cancel Esc ]  [ Add ↵ ] │
└─────────────────────────────────────────────────────────┘
```

- The picker turns into the form in place, and Escape or Cancel returns to the list, as in the TUI
  and as a multistep Dialog's inner step requires.
- Enter runs the eight checks and names the one running; a login question opens over the form. A
  failed check names itself, shows the state's details, and the primary button takes that state's
  action, except that a changed host key or a system Orchbee does not run on offers none until the
  address changes. A failed check, or Escape while the checks run, gives the add up. Once every
  check passes, the record is saved and the list shows the new row selected.
- The GUI field contract appends "(optional)" (可选) to an optional field's label. Slice 8 adds it to
  the TUI's Name label too, so both Clients keep one wording.

Copy: Add machine / 添加机器; Address / 地址; Hosts in SSH config / SSH 配置里的主机; Name / 名称; Leave blank to use
the host name. / 留空则使用主机名。; the eight check names (OpenSSH on this computer / 本机的 OpenSSH … remote
Session / 远端会话); Checking {position} of 8: {check}… / 检查第 {position}/8 步：{check}…; Check {position} of 8
failed: {check}. / 第 {position}/8 步未通过：{check}。; Every check passed. Saving… / 全部通过，正在保存…; the
address, name and save problems; Cancel / 取消; Add / 添加.

### Actions menu

```text
[ Actions ] ▾
  Rename
  Disable
  Disconnect
  Forget password
  Remove
  Remove and stop remote Session          (dimmed)
  Can't reach devbox, so its Session can't be stopped.
```

- The items, their order and their availability are the TUI's, from the machines process's view.
  An unavailable item is dimmed, and the highlighted one's reason shows under the menu.
- Rename edits the name in place in the row: Enter saves, Escape keeps the old one.
- Disable saves the Machine disabled and then disconnects it; Enable saves it enabled. Disconnect
  ends the link without asking. Forget password forgets its remembered answers. Remove removes the
  record at once and says so in the bottom row.
- Removing or disabling the Machine on screen returns the window to Local once the change is saved.

Copy: Rename / 重命名; Disable / 停用; Enable / 启用; Disconnect / 断开; Forget password / 忘记密码; Remove / 移除;
Remove and stop remote Session / 移除并停止远端会话; Can't reach {name}, so its Session can't be stopped. /
连不上 {name}，无法停止它的会话。; Removed {name} · remote Session keeps running / 已移除 {name} · 远端会话继续运行;
the three Forget password results.

### Remove and stop

```text
┌ Remove and stop remote Session? ─────────────────────────┐
│ devbox                                                   │
│ Ends every terminal on devbox.                           │
│ Orchbee and files on the remote stay.                    │
│                      [ Cancel Esc ]  [ Remove and stop ] │
└──────────────────────────────────────────────────────────┘
```

- The picker turns into this step in place, as the TUI's page does. Following the Alert Dialog's
  standard, the step has no close control and focuses Cancel, because a mistake ends every terminal
  on the Machine; this is one of the three details
  [`INTERFACE-DESIGN.md`](../../docs/INTERFACE-DESIGN.md#recurring-interaction-patterns) lets each
  Client keep.
- Remove and stop asks the hub for `stop-session` and keeps the Machine listed while it runs, under
  "Stopping the Session on devbox…", for up to the TUI's 120 seconds. The Machine on screen returns to
  Local as the stop begins. Success or failure returns to the list with the TUI's notice.

Copy: Remove and stop remote Session? / 移除并停止远端会话？; Ends {count} terminals on {name}. and its forms /
结束 {name} 上的 {count} 个终端。; Orchbee and files on the remote stay. / 远端的 Orchbee 和文件保留。; Stopping the
Session on {name}… / 正在停止 {name} 上的会话…; Cancel / 取消; Remove and stop / 移除并停止; Removed {name} · remote
Session stopped / 已移除 {name} · 远端会话已停止; The Session on {name} couldn't be stopped; the machine stays. /
没能停止 {name} 上的会话，机器保留。

### A remote surface

```text
┌ Sidebar ─────────────────┬ [ build ] [ logs ] ─────────────────────────┐
│ devbox / Projects        │                                             │
│   devbox / api           │  (devbox's terminal)                        │
│   devbox / web           │                                             │
│ devbox / Recents         │                                             │
│   devbox / scratch       │  Connection lost        [ Log in again ↵ ]  │
│                          │                                             │
│ Settings                 │                                             │
└──────────────────────────┴─────────────────────────────────────────────┘
```

- The sidebar applies the TUI's prefix rule: while more than one Machine is enabled, Local
  included, each group heading starts with the Machine's name and `/`, and each Workspace row with
  the name cut to eight cells and `/`.
- When the hub needs a login again for the Machine on screen, the surface carries the TUI's lost-link
  line, "connection lost · Enter to log in again" (连接已断开 · Enter 重新登录). The window splits it into its
  sentence, "Connection lost" (连接已断开), and a Log in again (重新登录) button whose key hint is Enter.
- A lost link without a Disconnect returns by itself to the Workspace it showed once the Machine is
  `online` again; after a Disconnect the surface waits for the Machine to be chosen again.

## Contracts

The page, the preload, Main and the bundled Client ship as one build, so the channels below are
internal pairings without negotiation: every JSON record carries `"version": 1`, and an incompatible
change renames the entry's `-v1` suffix. Members are camelCase and tokens kebab-case, as in the hub
contract, and a Machine is named by its catalog id, 32 lowercase hexadecimal digits. A record that
is malformed, oversized, of another version or of an unknown kind is dropped with a diagnostic by
whichever side reads it, and never ends that side. Desktop has one window, so at most one document
is attached at a time.

### Machines channel: lines between Main and the machines process

Each document receives a fifth transferred port, `machines`, after the four it has today. Main
starts `orchbee --desktop-machines-v1` with pipes once Desktop runs. The process reads standard
input and writes standard output, one UTF-8 JSON object per line, ended by LF. Main's lines are at
most 512 KiB and the process's at most 8 MiB, which any escaped record fits; a longer line is
discarded up to its LF. Standard error carries diagnostics only, never a secret. Main ends the
process by closing its standard input; the process then leaves the hub and exits within one second,
and Main ends one that has not.

| From    | `kind`    | Members              | Meaning                                                                                                                                                        |
| ------- | --------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Process | `started` | `available`          | Its first line. `false` where the host lacks `SavedMachines`: the process exits, and Main closes every `machines` port and does not start it again in its life |
| Main    | `window`  | `exists`             | Whether Desktop has a window, sent after `started` and at each change. The holder attaches with `asks` only while it is true                                   |
| Main    | `attach`  | `document`, a UUID   | That document's `machines` port is open. It replaces an attached document, and the process sends it a whole view                                               |
| Main    | `detach`  | `document`           | That document ended. The process forgets its unanswered intents and its unacknowledged view, and keeps every other state for the next document                 |
| Main    | `page`    | `document`, `record` | The page's record text, wrapped unopened. The process drops one from a document that is not attached, so a reload cannot deliver what the last document sent   |
| Process | `page`    | `document`, `record` | A record text for the page, at most 1 MiB of UTF-8. Main posts it as it is to that document's port while the document is attached, and drops it otherwise      |

The page posts each record to its port as one string of at most 65,536 UTF-16 code units; Main drops
anything else with a diagnostic. Main reads only the envelope and never parses a `record`.

### Machines channel: records between the page and the machines process

| From    | `kind`    | Members              | Meaning                                                                                                                                                              |
| ------- | --------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Process | `view`    | `sequence`, `view`   | The whole view, never a difference. `sequence` grows by one with each view in the process's life                                                                     |
| Page    | `ack`     | `sequence`           | The page took that view. The process sends the next only after it, and changes meanwhile coalesce into it, so a slow page holds one view                             |
| Page    | `intent`  | `request`, `intent`  | One intent below; `request` is a positive safe integer the page chooses, unique within its document                                                                  |
| Process | `outcome` | `request`, `outcome` | What became of intent `request`, sent at once rather than behind an unacknowledged view. At most 16 intents of a document are unanswered; the next is refused `busy` |

**The view.** Values, never words: the page words them through `@orchbee/i18n`. Each member is the
TUI value named beside it, so the process serializes what the TUI already decides.

| Member        | Value                                                                                                                                                                                                                                                                                          | TUI source                                     |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `hub`         | `null` while the holder is attached or no Machine needs a hub; otherwise `{"status"}` with `unknown`, `stopped` or `unavailable`, or `blocked` with a `cause`: `held` with the holder's `version`, `environment` with the other `environment`, `denied`, `elevated`, `protocol` or `data-root` | Discovery's hub status and `Blocked`           |
| `catalog`     | `read` or `unreadable`                                                                                                                                                                                                                                                                         | The machines store's last read                 |
| `rows`        | The saved Machines in catalog order, at most 64; Local is the page's own row                                                                                                                                                                                                                   | The picker's remote rows and `row_explanation` |
| `selection`   | `null`, or `{"request", "machine"}` while a choice runs                                                                                                                                                                                                                                        | The running selection                          |
| `login`       | `null`, the open login question, or the last question of a login that ended without the person                                                                                                                                                                                                 | `machine_login::Presented`                     |
| `notice`      | `null`, or the command-login notice                                                                                                                                                                                                                                                            | `machine_login::Offered`                       |
| `add`         | `null`, or how the add stands                                                                                                                                                                                                                                                                  | The Add form's `Check`                         |
| `suggestions` | `null` until read, then at most 32 host names                                                                                                                                                                                                                                                  | The SSH host suggestions                       |
| `stopping`    | `null`, or the Machine whose Remove and stop runs                                                                                                                                                                                                                                              | The manage page's stop                         |

A row is `machine`, `label`, `address` (its `ssh` target as the person named it, or `null`),
`enabled`, `supported`, `word`, `tone`, `lines`, `action` and `menu`:

- `word` is the picker message key of its word, from `Observation::key`: `online`, `opening`,
  `reconnecting`, `offline`, `disabled`, `needs-login`, `unresponsive`, `untrusted`,
  `not-installed`, `needs-update`, `local-needs-update`, `login-failed`, `key-changed`,
  `unsupported`, `incompatible`, `unknown`, `hub-stopped` or `hub-unavailable`. A handshake refusal
  the process keeps sets it by the [refusal table](../../apps/orchbee/docs/machines.md#session-carrier).
  `tone` is `success`, `progress`, `neutral`, `warning` or `error`.
- `lines` are `machine_status::Line` values as `{"line", ...members}`: `version` (`version`),
  `contents` (`projects`, `workspaces`, `tabs`), `label` (`label`), `unreachable` (`host`, and
  `why`: `timed-out`, `refused`, `unresolved` or `other`), `ssh-failed` (`host`), `next-attempt`
  and `last-attempt` (`at`, milliseconds since the Unix epoch), `unsupported` (`platform` and
  `architecture`, each `null` when unknown), `remote-too-old` (`version` or `null`), `login`
  (`state`, below), `hub` (the members of a non-null `hub`), and, without members, `no-ssh`,
  `install-ssh`, `remote-protocol`, `not-started`, `link-lost`, `unresponsive`, `installing`,
  `not-installed`, `supported`, `session-too-old`, `update-remote`, `local-too-old`,
  `update-local` and `other-environment`.
- A login `state` is `{"state": "needs-login", "cause"}`, with `cause` `null`, `auth-required`,
  `remembered-rejected`, `login-dismissed` or `disconnected`; `{"state": "login-failed", "cause"}`,
  with `null`, `password-rejected` or `key-rejected`; `{"state": "untrusted", "hostKey"}`; or
  `{"state": "key-changed", "hostKey", "host"}`, where `hostKey` is `null` when unknown and `host`
  is the name `ssh-keygen -R` forgets. A host key is `{"algorithm", "fingerprint"}`.
- `action` is `null`, `open`, `retry`, `retry-now`, `reconnect`, `enable`, `log-in`,
  `log-in-again` or `trust`.
- `menu` lists the Actions items in the TUI's order as `{"item", "available"}`: `rename`; `disable`
  or `enable` for a supported Machine; `disconnect` for an enabled, supported one;
  `forget-password` where answers can be remembered; `remove`; and `remove-and-stop`, available only
  while the Machine is online.

A login is `login` and `question` (the hub's ids), `machine` and `label` (for an add, the id it is
checked under and the name typed), `target`, `kind` (`password`, `key-passphrase`, `host-key` or
`other`, as this client reads it against the target), `hostKey` (for `host-key`, else `null`),
`prompt` (`ssh`'s prompt text), `notice` (`null`, `repeated`, `inadmissible` or
`remembered-rejected`), `sending`, `ending` and `remember`. `ending` is `null` while the question is
open, and otherwise how the login ended without the person: the hub's `succeeded`, `failed`,
`dismissed`, `abandoned`, `unavailable` or `unknown`, or this client's `dismissed-here`,
`undelivered` or `lost`. `remember` is `{"box": "offered", "ticked", "untilRestart"}`,
`{"box": "unavailable"}`, `{"box": "needs-newer-ssh"}` or `{"box": "absent"}`. The view never
carries an answer.

A notice is `login`, `machine`, `label`, `target`, `waiters`, `accepted` (the person pressed Log in)
and `ended`: `null`, `taken` (another interface claimed it, or this claim did not take it), or a
login `ending`.

An add is `request` (its `add` intent's), `label` (the name it will save) and `state`:
`{"state": "starting"}`, `{"state": "checking", "stage"}`,
`{"state": "failed", "stage", "lines", "action", "retry"}` or `{"state": "saving"}`. `stage` is
`ssh`, `reach`, `trust`, `login`, `system`, `orchbee`, `compatibility` or `session`, the eight
checks in order, or `null` in a failure no check names; `retry` is false where the TUI offers no way
past a changed host key or an unsupported system until the address changes.

**Intents.** An `intent` is an object whose `act` names it, each the TUI's act beside it.

| `act`            | Members                                                                                     | The TUI's act                                                                                                            |
| ---------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `choose`         | `machine`                                                                                   | Enter on a row whose action is not Enable: the selection, which also clears a kept refusal                               |
| `cancel-choice`  | none                                                                                        | Escape, or choosing Local, while a selection runs                                                                        |
| `refused`        | `machine`, `refusal`                                                                        | Discovery keeping a handshake refusal: the page reports the one its relay met                                            |
| `manage`         | `machine`, `item`, and `label` for `rename`                                                 | An Actions item; `enable` also serves Enter on a row whose action is Enable                                              |
| `add`            | `address` and `name`, at most 1,024 and 128 Unicode scalars, `name` empty for the host name | Enter on the Add form, or its failed check's action                                                                      |
| `cancel-add`     | none                                                                                        | Escape or Cancel on the form: gives up a running add and clears `add`                                                    |
| `suggestions`    | none                                                                                        | The form's one read of the SSH host suggestions                                                                          |
| `answer`         | `login`, `question`, `answer`: at most 1,024 UTF-8 bytes                                    | Enter on a password, passphrase or other question                                                                        |
| `remember`       | `login`, `question`, `ticked`                                                               | The remember box, which holds for the login's later questions                                                            |
| `trust`          | `login`, `question`                                                                         | Trust and continue, once the field reads `yes`; Orchbee answers `yes` itself                                             |
| `cancel-login`   | `login`, `question`                                                                         | Cancel, which ends the login                                                                                             |
| `close-login`    | `login`, `question`                                                                         | Close on a login that ended without the person                                                                           |
| `accept-notice`  | `login`                                                                                     | Log in on the notice: the claim. On an ended notice, Close                                                               |
| `decline-notice` | `login`                                                                                     | Not now: the dismissal that fails the waiting commands and mutes the Machine's command logins. On an ended notice, Close |

`refusal` is the TUI's `Refusal`: `other-environment`, `client-too-old`, `session-too-old` or
`development-build-mismatch`.

**Outcomes.** An `outcome` is an object whose `result` names it.

| `result`       | Members   | When                                                                                                                                                                                                                                                                                                                                      |
| -------------- | --------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `chosen`       | `machine` | `choose`: the hub reads the Machine `online` for this choice, so the page may ask for its relay                                                                                                                                                                                                                                           |
| `failed`       | `message` | `choose` or `refused`: the picker message the TUI shows, `failed`, `removed`, `catalog`, `hub`, `hub-message-stopped`, `hub-message-blocked`, `login-undelivered`, `login-failed-message`, `login-key-changed-message`, `login-unanswered-message`, `refused-environment`, `refused-client-old`, `refused-session-old` or `refused-build` |
| `check-failed` | none      | `add`: a check failed, and the view's `add` says which and why                                                                                                                                                                                                                                                                            |
| `problem`      | `problem` | `add` or `manage`: the TUI's `address`, `name` or `save` problem                                                                                                                                                                                                                                                                          |
| `added`        | `machine` | `add`: every check passed and the record is saved                                                                                                                                                                                                                                                                                         |
| `done`         | `notice`  | Any other intent carried out. `notice` is `null`, the TUI's manage notice `{"notice", "label"}` with `removed`, `removed-stopped`, `stop-failed` or `not-online`, or its forget result `{"notice"}` with `forgotten`, `nothing-saved` or `forget-failed`                                                                                  |
| `cancelled`    | none      | A `choose` or `add` that `cancel-choice` or `cancel-add` gave up, or a `choose` another replaced                                                                                                                                                                                                                                          |
| `refused`      | `reason`  | Not carried out: `busy`, `malformed`, or `stale`, for ids that no longer name an open question, notice or saved Machine                                                                                                                                                                                                                   |

The process keeps an answer only in the TUI's fixed buffer, delivers it as the TUI's answer path
does, writes it to the system password store only when the box is ticked and the login succeeds,
and wipes every copy it made.

### Remote Session connection

- **Request.** The page posts an `orchbee.desktop.machine-session-request` message to its own
  window with `request` and `machine`, a Machine the machines process just answered `chosen` for.
  The preload forwards a parsed request to Main on the IPC channel of the same name, adding its
  document's identity, which the page never sees. Main parses it again and refuses `unavailable`
  while that document holds two relays.
- **Relay.** Main starts `orchbee --machine-session-v1 <machine>` with pipes. The relay joins the
  running hub, never starting or replacing one, opens one `session` channel to the Machine, admits
  this build's own Client offer through it as the TUI's carrier does, and writes one LF-ended JSON
  line of at most 64 KiB: `{"version": 1, "outcome": "admitted", "capabilities"}` with at most 256
  negotiated `client-server` capability names, or `{"version": 1, "outcome": "refused", "reason"}`.
  After `admitted` every byte both ways is the page's, unread. The relay's later failures go to
  standard error and its exit status.
- **Reasons.** The TUI carrier's `Refusal` for the handshake's answer, `other-environment`,
  `client-too-old`, `session-too-old` (also for a Session older than the handshake) and
  `development-build-mismatch`, which the page reports to the machines process unchanged; the
  transient `capacity`, `stopping` and `timeout`; `not-online` and `not-found` from the channel's
  `open`; `hub-unavailable` when no hub can be joined; and `link-lost` or `protocol` when the
  channel ends, or anything answers outside its contract, before admission.
- **Reply.** Main reads the first line within 30 seconds; a relay that writes nothing in time,
  exits first or writes anything else is ended and refused `timeout` or `protocol`. Main answers on
  `orchbee.desktop.machine-session` with `document`, `request` and `outcome`: `admitted` with one
  transferred port and `bootstrap` `{"machine", "negotiatedCapabilities"}`, or `refused` with
  `reason`, Main's own being `unavailable`. The preload drops an answer for another document and
  posts the rest to the window as `orchbee.desktop.machine-session-ready`. The page derives the rest
  of a remote Session's bootstrap: no first Workspace of its own, no Quit All, no folder picker and
  no window update.
- **Bounds.** The relay's bytes use the existing relay bounds, 4 MiB unread and 64 MiB of pending
  input. A document holds at most two relays in the first release, the surface on screen and one
  candidate, and at most six once four inactive surfaces are cached. Closing the port, or the
  document's end, ends the relay; ending a relay never stops the remote Session.
- **What only the far computer knows.** The page's runtime needs the home folder of the Session's
  computer, for terminals started without a directory and for `~` in paths. No contract carries a
  remote home today. The relay slice decides between the far link helper's answer and a
  `client-server` capability labelled `addedIn` its first release; until one is negotiated, the
  remote surface leaves the directory to its Server and shows full paths.

### Release gate

Until the slice that ships the picker, nothing a person can reach changes. Main's composition reads
one code constant, never a setting. While it is false, Main starts no machines process, closes its
end of each document's `machines` port at once, as for a host without `SavedMachines`, and refuses
every remote connection request `unavailable`; Desktop therefore holds no hub attachment and no SSH
connection for anyone. The two internal entries exist, but no shipped path starts them. The picker's
slice flips the constant. With it on, a person without a saved Machine still gets no hub and no SSH
connection, because the holder starts only as the TUI's does, at the first enabled Machine of a
known kind or the first add.

### Native entries and the hub

- **Machines process.** Pipes only, refused on a console as `server connect` is. Its holder follows
  the TUI's rule: it attaches when the catalog first holds an enabled Machine of a known kind, or at
  the first add, with `presence` and `watch` for the Desktop's whole life, and with `asks` while Main
  says a window exists, sending `attach` again at each change, since each one replaces the last. It
  reads `machines.json` as the TUI does and asks the hub to `reload` a revision the hub has not
  taken. It writes nothing to the console and never logs a secret.
- **Session relay.** Opens with `join` and sends no `attach`, so its connection holds no hub and is
  offered no login; the machines process's `presence` keeps the hub running. `join` waits for a hub
  somebody else starts and fails `closing` ten seconds after the endpoint is gone, which the relay
  reports as `hub-unavailable`.
- **Compatibility.** Both entries use the existing `machine-hub` generation. Desktop's bundled Client
  states its product version in its offer, so the hub's [`asks` gate](../../schemas/machine/README.md#attachment)
  counts its `asks`. The relay's admission negotiates the `client-server` generation with the remote
  Session exactly as the TUI's carrier does: a Machine of another product version opens whenever the
  generations overlap, and the page gates actions on that Session's negotiated capabilities as it
  does for the local one.
- **Records.** Nothing persisted changes: `machines.json`, the remembered answers and the window
  state file keep their shapes.

### Copy

The `machines` domain of `@orchbee/i18n` holds every word the window shows for Machines, in en-US
and zh-CN, word for word the TUI's. The TUI's words live in the workbench Client crate's
localization modules `machines`, `machine_status`, `machine_login`, `machine_remember`,
`machine_waiting` and `machine_manage`, and the Action's label in `action_menu`. One shared fixture
of examples, each a message with its
arguments and its expected text in both languages, is checked by the Rust localization tests and by
the TypeScript catalog tests, so neither side can drift alone. Terms follow the
[Glossary](../../docs/GLOSSARY.md).

## Failure handling

| Failure                                       | What happens                                                                                                                                                                                                                                                                                          |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The machines process exits or stops answering | Main starts it again, at most three times a minute, then waits for the next window. Meanwhile rows read Unavailable and the status notice says machine connections are unavailable. Remote surfaces keep their own relays and attachments, and the restarted process's `presence` holds the hub again |
| A session relay exits or its channel is lost  | Only that surface's connection ends. It reconnects by itself to the Workspace it showed once the hub reads the Machine `online` again, unless the person disconnected it; a Machine that needs a login again shows the TUI's lost-link line, whose action logs in again                               |
| The hub stops, crashes or cannot be used      | Rows read Stopped or Unavailable as in the TUI, the status notice says which, and choosing a Machine starts a hub as the TUI's selection does                                                                                                                                                         |
| A handshake refusal from the remote Session   | The relay's `refused` reason maps to the TUI's refusal table: the row's word, the picker's message and the sidebar notice                                                                                                                                                                             |
| A malformed, oversized or stale record        | Dropped with a diagnostic by whichever side reads it; a request with an identity gets a refused outcome. No record ends a process                                                                                                                                                                     |
| The page crashes or reloads                   | The machines process keeps its state; the next document receives a fresh view, and an open question shows again with an empty field                                                                                                                                                                   |
| The window closes while Desktop keeps running | Main says no window exists; the process stops asking and dismisses the logins this window held, so waiting commands fail at once instead of waiting out the question                                                                                                                                  |
| The local Session fails                       | The machines process and remote surfaces are unaffected, and the local surface follows its existing recovery                                                                                                                                                                                          |

## Accepted limits

- A secret passes through the page, Main and the machines process's input. None logs it; the page
  clears its field on submission, cancellation, failure and unmount, and the machines process wipes
  its copy after delivery as the TUI does. JavaScript strings cannot be wiped, so copies in the page
  and in Main stay until garbage collection, as with any Electron password field.
- Without a window, Desktop answers no command login: its `asks` is false, so unless another
  interface asks, a waiting command fails `auth-required` at once rather than opening the window.

## Slices

Each slice merges with its owning documents changed in the same commit. "Haiku" marks a slice whose
brief can specify it exactly; an Opus reviewer checks and commits its diff. Slice 1 is the
[Contracts](#contracts) section as written, before any code: repository standards and package
guides state only what runs, and a package export ships with its first consumer, so each later slice
moves the part of the contract it builds into its owning guide, schema and tests, and deletes it
here.

| #   | Slice                                                                                                                                                                                                 | Rung                              |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| 1   | Contracts: the wire records, bounds and refusals of both channels, the remote connection and the release gate, written here                                                                           | Opus, high                        |
| 2   | The machines process: holder, view, intents, logins, Add checks and management from the existing modules; Windows and macOS; a Desktop window section in `machines.md`                                | Opus, xhigh                       |
| 3   | The session relay: `join`, `session` channel, admission, first line, copying; the remote home decision; its part of that section                                                                      | Opus, xhigh                       |
| 4   | Desktop Main: supervising the machines process, the fifth port and its envelope, window presence, remote connection requests and relays, the release gate; the Desktop guide and the protocol schemas | Opus, high                        |
| 5   | The Desktop composition: the retained local runtime, one remote runtime, switching on a drawable candidate, window-wide duties kept local                                                             | Opus, high                        |
| 6   | The Machines picker, the Add form, the Actions menu, rename and the Remove and stop confirmation, from the wireframes and the view's types                                                            | Haiku, max                        |
| 7   | The login Dialog and the command-login notice: secret handling, the one-second guard, the queue and deferral                                                                                          | Opus, high                        |
| 8   | The `machines` copy domain in both languages and the shared examples                                                                                                                                  | Haiku, max                        |
| 9   | The sidebar prefix, the status notices, and the `workbench.machines.open` Action with its binding                                                                                                     | Haiku, max                        |
| 10  | A scripted real-window run on Windows and macOS against the hub's stand-in far end and one real Machine                                                                                               | Haiku, max; Opus checks each FAIL |
| 11  | Later: observers in the machines process, Workspaces and Agents modes, other Machines in the sidebar                                                                                                  | Opus, xhigh                       |
| 12  | Later: four cached inactive remote surfaces                                                                                                                                                           | Opus, high                        |

Slices 2, 3 and 8 can start together after slice 1; 4 needs 2 and 3; 5 needs 4; 6, 7 and 9 need 5's
types and 8's copy.

## Open questions for the owner

Each question names the recommended answer first.

1. **Management in the first release.** Recommended: the window adds and manages Machines in the
   first release. With it, a person presses Add machine…, types `dev@devbox` and watches eight checks;
   without it, the window lists saved Machines only and adding one takes
   `orchbee machine add dev@devbox` in a terminal, or the TUI.
2. **Switching between two remote Machines.** Recommended: accept a reconnect for now. In the first
   release only Local and the remote on screen stay connected, so devbox → nas → devbox waits for each
   remote connection, while a return to Local is instant; the TUI's four cached surfaces come later.
3. **The sidebar in the first release.** Recommended: accept the Session on screen only. On Local the
   sidebar does not list devbox's Workspaces, and the person goes through `Ctrl+Shift+M`, devbox,
   Open; the TUI's sidebar already lists `devbox / Projects` beside Local's.
4. **A permanent Machines entry.** Recommended: none, as in the TUI, which has no permanent indicator.
   The alternative is a Machines entry beside Settings at the bottom of the sidebar in both Clients,
   reading `devbox · Online` like a remote indicator; it is a new feature point for both. Without it
   the window's picker has no mouse path in the first release, as [Entry](#entry) explains. The
   slices build without it and open the picker through one path whatever invoked it, so the entry
   can be added later without changing the picker.
5. **Connections while only the notification-area icon remains.** Recommended: keep them, as the
   attachment contract says. After the window closes, devbox stays connected over SSH, reopening shows
   it Online at once, and `orchbee --machine devbox …` in another terminal reuses that connection; the
   cost is an SSH connection held in the background.
6. **Command logins without a window.** Recommended: fail at once. With the window closed,
   `orchbee --machine devbox agent list` that needs a password ends `auth-required` instead of opening
   the Orchbee window.
7. **This computer's notifications while a remote surface is shown.** Recommended: keep them. While the
   person works on devbox, an Agent on this computer that needs them still raises a notification and
   counts in the notification-area icon. The TUI's hidden surfaces, Local among them, withdraw their
   notifications, but its sidebar keeps listing every Machine's Workspaces; the window's first-release
   sidebar lists only the Session on screen, so a withdrawn notification would leave no sign at all.
