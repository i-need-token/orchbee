# orchbee

Orchbee is a terminal that runs and orchestrates AI agents. This package installs its command,
`orchbee`, which starts the terminal interface.

```sh
npm install --global orchbee@next
orchbee
```

The first `orchbee` installs the program this package carries into your Orchbee folder and then
starts it; later starts run the installed program directly. A newer version of this package
installs its program the same way on its first start.

Each `orchbee` command still starts Node before the program, which adds to its startup time. Where
scripts or agents run many commands, install with the
[installer](https://github.com/i-need-token/orchbee#install) instead: its `orchbee` starts without
Node. `npm uninstall --global orchbee` then removes only this package and keeps the installed
program.

To remove Orchbee, run `orchbee uninstall` first, then `npm uninstall --global orchbee`, which
removes only this package.
