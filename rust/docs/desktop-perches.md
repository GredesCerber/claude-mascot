# Desktop icon perches

On Windows, `DesktopMonitor` uses the public Microsoft UI Automation (UIA) tree
for Explorer's `SHELLDLL_DefView` / `SysListView32` desktop view. It reads list
item bounding rectangles and runtime IDs only. It never reads desktop labels,
file paths, shortcut contents, or remote process memory, and it does not send
input or alter icon positions.

The provider runs UIA on a dedicated MTA worker because Explorer/UIA calls can
stall. The application thread clones a cached snapshot of at most 128 icons;
UIA filters to list items before applying that cap. Snapshots contain opaque
high-bit IDs and physical-pixel rectangles. Failed UIA reads clear the cache,
and snapshots older than three seconds are treated as empty so Explorer stalls
cannot leave ghost perches. Setting the provider disabled clears its cache and
stops further UIA polling. Drop signals
the worker to exit without blocking the UI thread on a potentially hung COM call.

Geometry outside 32–512 physical pixels per edge, non-finite/invalid rectangles, and
rectangles outside visible monitor work areas are discarded or clipped. Icons
are inserted at the bottom of `World.windows` so normal windows occlude them in
the existing bottom-to-top stacking order.

The upper bound accommodates high-DPI icons and their labels (a normal item
was measured at 150 by 157 physical pixels at 200%). The item-count cap and
desktop-host/list-item filters remain independent of this geometry allowance.

QA may inspect `DesktopMonitor::icon_rectangles()` and report the count and
rectangles only. Do not log UIA names, paths, or element properties.
