// Шлёт маскоту список видимых окон (снизу вверх по стопке) при любом изменении.
const SVC = "org.claude.Mascot";

function onCurrentDesktop(w) {
    if (w.onAllDesktops) return true;
    const cur = workspace.currentDesktop;
    for (const d of w.desktops) {
        if (d.id === cur.id) return true;
    }
    return false;
}

function send() {
    const out = [];
    const active = workspace.activeWindow;
    for (const w of workspace.stackingOrder) {
        if (!w.normalWindow || w.minimized || w.deleted || !onCurrentDesktop(w)) continue;
        const g = w.frameGeometry;
        out.push({
            id: String(w.internalId),
            x: g.x, y: g.y, w: g.width, h: g.height,
            cls: String(w.resourceClass || ""),
            df: String(w.desktopFileName || ""),
            cap: String(w.caption || ""),
            fs: !!w.fullScreen,
            act: w === active,
        });
    }
    const areas = [];
    for (const scr of workspace.screens) {
        const a = workspace.clientArea(KWin.MaximizeArea, scr, workspace.currentDesktop);
        areas.push({x: a.x, y: a.y, w: a.width, h: a.height});
    }
    callDBus(SVC, "/", SVC, "update", JSON.stringify({windows: out, areas: areas}));
}

function hook(w) {
    w.frameGeometryChanged.connect(send);
    w.minimizedChanged.connect(send);
    w.fullScreenChanged.connect(send);
    w.captionChanged.connect(send);
}

workspace.windowList().forEach(hook);
workspace.windowAdded.connect(function (w) { hook(w); send(); });
workspace.windowRemoved.connect(send);
workspace.windowActivated.connect(send);
workspace.currentDesktopChanged.connect(send);
if (workspace.screensChanged) workspace.screensChanged.connect(send);
if (workspace.stackingOrderChanged) workspace.stackingOrderChanged.connect(send);
send();
