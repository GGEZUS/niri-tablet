# Issue: 4-finger gestures act as 3-finger ones (NixOS)

First external bug report, 2026-09-07. Reporter's video (with
`show-touch-points` on) shows 4 touch dots, so the compositor receives all
four touches, but the 3-finger action runs. Not reproducible on the
maintainer's machines (T480, Surface Go 2).

## Hypothesis (pending logs)

Recognition race: the tracker locks the finger count when the swipe
centroid crosses the 16px movement threshold. A 4th finger landing after
that moment is ignored, so the gesture runs with `fingers=3` and the
4-finger actions (`swipe-4-up`/`swipe-4-down`, `tap-4`) are never selected.
Staggered finger landing, hand pivot while landing, or digitizer contact
jitter can each cross the threshold before the 4th finger arrives.

Full analysis and the candidate fix (settle window + re-baseline on late
join) live in the maintainer's notes; evidence lands in this folder.

## What was shipped to diagnose it

Patch 0013: `gestures { debug-log }` logs the full gesture event stream
(per-finger down/up/motion with slots and timestamps, recognition
decisions, dispatched actions) at info level with a `gesture-debug:`
prefix. No behavior change.

## Reply template for the reporter

> Thanks for the video, it rules out the touches being dropped before the
> compositor. I've added a debug logging option to find out exactly what
> the gesture tracker sees. Could you:
>
> 1. Update to the next release (anything after v26.04.12), and update the
>    `rev` in your NixOS config to match.
> 2. Add this to the `gestures` block of your niri config and save (it
>    takes effect immediately, no re-login needed):
>
>    gestures {
>        debug-log
>    }
>
> 3. Reproduce the problem a few times (also try one intentional 3-finger
>    swipe for comparison).
> 4. Send me the log output:
>
>    journalctl --user -u niri.service --since -10m | grep gesture-debug
>
> Feel free to remove the option afterwards. If you'd rather run niri from
> a terminal, the same lines appear on its stderr.

## Interpreting the logs

- `down slot=N ... fingers=4` lines: each finger as the compositor sees it,
  with timestamps. Large gaps between the 3rd and 4th `down` support the
  staggered-landing theory.
- `recognized: swipe fingers=N`: the locked finger count. The bug is
  confirmed if this says `fingers=3` while four `down` lines preceded it.
- `down slot=N ... IGNORED: gesture already active with 3 fingers`: the
  smoking gun, the 4th finger arriving after recognition.
- `swipe dispatch: fingers=N ... discrete=...`: which action the dispatch
  picked and why.

`t480-known-good.log` is the maintainer's baseline capture on hardware
where 4-finger gestures work: first three fingers in one frame, 4th
joins 9-18ms later, recognition 27-100ms after that, every 4-finger
flick dispatched with `fingers=4`. Compare the reporter's capture
against that ordering.
