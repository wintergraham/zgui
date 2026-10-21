# Failed fresh-composition burst

This exploratory native Wayland run reached successful external model
replacement, normal editing recovery and fresh preedit `n`, then failed waiting
for `ni hao` after injecting the remaining characters as a burst. The exact
application and native protocol logs are retained. This supplements the earlier
[initial burst failure](../../wayland-ime/failed-initial/README.md).

The later functional harness sequences characters through observed preedit
updates. Its passing result does not resolve this burst-input failure.
The exploratory script revision was not separately frozen, so these logs are
preserved as diagnostic evidence rather than a reproducible source-matched run.
