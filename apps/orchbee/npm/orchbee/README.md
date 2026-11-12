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

To remove Orchbee, run `orchbee uninstall` first, then `npm uninstall --global orchbee`, which
removes only this package.
