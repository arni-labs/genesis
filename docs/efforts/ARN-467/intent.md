# Intent

Genesis must work for agents. An agent should be able to clone a repository,
fetch from it, push to it with a GitToken, and install an app from it — without a
human intervening, and without the answer being a 504.

It was not working at all. Every git operation against Genesis production timed
out, and installing an app through Genesis (which is the source of truth for
TemperPaw and Katagami apps) failed at the bundle. The causes were split across
two repositories: Genesis's own Cedar policies and wire modules, and the kernel
it runs on.

This effort is the Genesis half. Its kernel half is
[ARN-499](https://linear.app/arni-build/issue/ARN-499) in `nerdsane/temper`,
PR #462, pinned here as the `temper` submodule.

Tracked as [ARN-467](https://linear.app/arni-build/issue/ARN-467).
