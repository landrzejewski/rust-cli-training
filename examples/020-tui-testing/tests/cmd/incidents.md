# platctl-incidents – command reference (executable documentation)

This file is run by `trycmd` (`tests/trycmd.rs`): every `console` block is executed and its
output compared. Docs and behavior can't drift apart.

```console
$ platctl-incidents list
#101 billing        critical  open          Payment provider returns 503
#102 search         warning   open          p95 latency above 1s
#103 notifications  warning   acknowledged  E-mail queue growing

$ platctl-incidents ack 101
acknowledged #101

$ platctl-incidents ack 999
? 1
error: incident #999 not found

$ platctl-incidents --version
platctl-incidents 0.1.0

```
