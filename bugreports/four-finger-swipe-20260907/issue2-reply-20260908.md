Thanks, these logs settled it. Good news: the compositor side works perfectly on your machine, all four fingers included.

The log shows every 4th finger joining the tracker (`late join: slot=3 now tracking 4 fingers`, at most ~9 ms after the 3rd), the gestures recognized with the right count (`recognized: swipe fingers=4`, `tap: 4 fingers`), and the dispatch then asking the config what a 4-finger gesture should do:

```
gesture-debug: tap dispatch: fingers=4 action=Some(MaximizeColumn)
gesture-debug: swipe dispatch: fingers=4 hold=false direction=Vertical discrete=None
```

The config had no answer because no 4-finger actions are bound in it. When a 4-finger gesture has no binding, it falls back to the 3-finger one by design: the tap runs the regular `tap` action, the flick runs the animated workspace swipe. That is exactly the behavior in your video, and literally the issue title: they work as 3-finger gestures.

Which makes this a documentation failure on my side. The README's gesture table lists the 4-finger actions without saying they need to be bound first (unlike the edge/corner rows). I've fixed the table; sorry for the runaround.

To get the actions from the table, add these inside your `touchscreen-swipe` block. `config/gestures.kdl` in the repo has the full example:

```kdl
tap-4 { toggle-overview; }                      // or your launcher
swipe-4-down { close-window; }
swipe-4-up { spawn-sh "wvkbd-mobintl -H 260"; }  // any action; this toggles wvkbd
```

They apply live on save, no re-login needed. Feel free to remove the `debug-log` line again.

Could you add the binds and confirm the gestures now do their own thing? I'll keep the issue open until then. And thanks again for the logs: they also confirmed the recognition path handles late-joining fingers correctly on real hardware, which was the other suspect.
