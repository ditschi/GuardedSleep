#!/usr/bin/env python3
"""Generates mockup.html. Edit frames here, then run: python3 gen.py"""
import os, re
from gen_css import CSS
from gen_table import TABLE

HERE = os.path.dirname(os.path.abspath(__file__))
sprite = open(os.path.join(HERE, "sprite-r0.txt")).read()
# replace logo, add new icons
sprite = re.sub(r'<symbol id="i-logo".*?</symbol>', '', sprite, flags=re.S)
EXTRA = """
<symbol id="i-logo" viewBox="0 0 24 24"><path d="M13.6 3.4A8 8 0 1 0 20.4 15 6.4 6.4 0 0 1 13.6 3.4z" fill="currentColor" stroke="none"/><path d="M1.5 20.2h21" stroke-width="2"/></symbol>
<symbol id="i-night" viewBox="0 0 24 24"><path d="M15 15.5a6.5 6.5 0 0 1-8.5-8.5 7.5 7.5 0 1 0 8.5 8.5z"/><path d="M17.5 3.5v3M16 5h3M20.5 9v2M19.5 10h2"/></symbol>
<symbol id="i-belloff" viewBox="0 0 24 24"><path d="M8.5 4.8A6 6 0 0 1 18 10v4.5"/><path d="M6 10.5V16l-1.5 2h12"/><path d="M10 20.5a2 2 0 0 0 4 0"/><path d="M3.5 3.5l17 17"/></symbol>
<symbol id="i-pausec" viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M10 8.5v7M14 8.5v7"/></symbol>
<symbol id="i-stopsq" viewBox="0 0 24 24"><rect x="6.5" y="6.5" width="11" height="11" rx="2" fill="currentColor" stroke="none"/></symbol>
<symbol id="i-x" viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18"/></symbol>
<symbol id="i-pause" viewBox="0 0 24 24"><path d="M7 4v16M17 4v16" stroke-width="3"/></symbol>
<symbol id="i-volx" viewBox="0 0 24 24"><path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z"/><path d="M16 9.5l5 5M21 9.5l-5 5"/></symbol>
<symbol id="i-micoff" viewBox="0 0 24 24"><path d="M15 10.5V6a3 3 0 0 0-5.6-1.5"/><path d="M9 9v2a3 3 0 0 0 4.6 2.5"/><path d="M5.5 11a6.5 6.5 0 0 0 10.3 5.3M18.5 11a6.4 6.4 0 0 1-.5 2.4"/><path d="M12 17.5V21"/><path d="M3.5 3.5l17 17"/></symbol>
<symbol id="i-battlow" viewBox="0 0 24 24"><rect x="2.5" y="7" width="17" height="10" rx="2.5"/><path d="M22 10.5v3"/><rect x="5" y="9.5" width="3" height="5" rx="1" fill="currentColor" stroke="none"/></symbol>
<symbol id="i-download" viewBox="0 0 24 24"><path d="M12 4v11M7 10.5l5 5 5-5M5 20h14"/></symbol>
<symbol id="i-lock" viewBox="0 0 24 24"><rect x="5" y="10.5" width="14" height="10" rx="2.5"/><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5"/></symbol>
<symbol id="i-cover" viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="3" fill="currentColor" fill-opacity=".35"/><circle cx="15.5" cy="13.5" r="1.4" fill="currentColor" stroke="none"/></symbol>
<symbol id="i-qr" viewBox="0 0 24 24"><rect x="3.5" y="3.5" width="6.5" height="6.5" rx="1"/><rect x="14" y="3.5" width="6.5" height="6.5" rx="1"/><rect x="3.5" y="14" width="6.5" height="6.5" rx="1"/><path d="M14 14h2.5v2.5H14zM18 18h2.5v2.5H18zM14 18.5v2M18 14h2.5"/></symbol>
<symbol id="i-hand" viewBox="0 0 24 24"><path d="M8 13V5.5a1.5 1.5 0 0 1 3 0V11m0-1.5V4.5a1.5 1.5 0 0 1 3 0V11m0-5.5a1.5 1.5 0 0 1 3 0V12m0-3.5a1.5 1.5 0 0 1 3 0V15a6 6 0 0 1-6 6h-1.5a6 6 0 0 1-4.6-2.1L4.5 15.5a1.6 1.6 0 0 1 2.3-2.2L8 14.5"/></symbol>
"""
sprite = sprite.replace("</defs>", EXTRA + "</defs>")

def I(n, c=""):
    return f'<svg class="i {c}"><use href="#i-{n}"/></svg>'

def sysb(t, bat=True):
    return f'<div class="sys"><span>{t}</span><span class="r">{I("signal","xs")}{I("batt","xs")}</span></div>'

def webbar(title="GuardedSleep"):
    return f'<div class="webbar">{I("lock","xs")}<div class="url">{title}</div>{I("dots","s")}</div>'

def phone(theme, inner):
    return f'<div class="phone {theme}"><div class="screen">{inner}<div class="gbar"></div></div></div>'

FRAMES = []
def frame(num, title, cap, body, tag=None, section=None):
    FRAMES.append((section, num, title, cap, body, tag))

# ---------- parent status bar ----------
def bar_calm(state="Connected", name="Nursery", meta="82%", icon="link", bolt=True):
    m = f'{I("bolt" if bolt else "batt","xs")}{meta}' if meta else ""
    return (f'<div class="sbar calm"><span class="ic">{I(icon)}</span><span class="okdot"></span>'
            f'<div class="grow">{state} <span>· {name}</span></div><span class="meta" title="Baby device battery">{m}</span></div>'
            '<div class="hline" style="margin-top:6px"></div>')

def bar(kind, icon, title, sub=None, meta=None):
    s = f"<small>{sub}</small>" if sub else ""
    m = f'<span class="meta">{meta}</span>' if meta else ""
    return f'<div class="sbar {kind}">{I(icon)}<div class="grow">{title}{s}</div>{m}</div>'

def tools(alert="Sound", night="off", web=False, icon_only=False, de=False):
    lbl = "Meldung" if de else "Alert"
    val = {"Sound": "Ton" if de else "Sound", "Vibrate only": "Nur Vibration" if de else "Vibrate only"}[alert]
    ic = "speaker" if alert == "Sound" else "vib"
    if web:
        a = ""
    elif icon_only:  # the banner states the mode; the chip only offers the change
        a = f'<span class="chip on" aria-label="{lbl}: {val}">{I(ic,"s")}{I("chevd","xs")}</span>'
    else:
        a = f'<span class="chip shrink">{I(ic,"s")}<span class="ell">{lbl}: <b>{val}</b></span>{I("chevd","xs")}</span>'
    nm = "Nachtmodus" if de else "Night mode"
    if night == "off":
        n = f'<span class="ibtn t" aria-label="{nm}: off">{I("night")}</span>'
    else:
        st = ("An" if de else "On") if night == "on" else "Auto"
        n = f'<span class="chip on" aria-label="{nm}: {st}">{I("night","s")}{st}</span>'
    extra = f'<span class="chip">{I("cover","s")}Black screen</span>' if web else ""
    return f'<div class="tools">{a}<span class="sp"></span>{extra}{n}<span class="ibtn t" aria-label="Settings">{I("gear")}</span></div>'

def hero(h, p=None):
    pp = f"<p>{p}</p>" if p else ""
    hh = f"<h2>{h}</h2>" if h else ""
    return f'<div class="hero">{hh}{pp}</div>'

def levels(kind):
    import math
    n = 44
    out = []
    for k in range(n):
        v = 8 + 6 * abs(math.sin(k * 1.7)) + 4 * abs(math.sin(k * 0.37))
        if kind == "some" and 24 <= k <= 31:
            v = [26, 40, 58, 66, 61, 44, 30, 20][k - 24]
        out.append(v)
    return out

def hz(kind="quiet", msg=None, thr=70, hatch=False, label="A sound above this line sends an alert", caps=("Last minute","now")):
    cls = "hz"
    bars = ""
    if kind in ("quiet", "some"):
        lv = levels(kind)
        for k, v in enumerate(lv):
            c = "up" if v > thr else ("r" if k >= len(lv) - 5 else "")
            op = "" if c else f";opacity:{.45+.55*k/len(lv):.2f}"
            bars += f'<i class="{c}" style="height:{v}%{op}"></i>'
    else:
        cls += " off"
        for k in range(44):
            bars += '<i style="height:3%"></i>'
    if hatch:
        cls += " hatch"
    m = f'<div class="msg">{msg}</div>' if msg else ""
    thrline = f'<div class="thr" style="bottom:{40+124*thr/100:.0f}px"><b>{label}</b></div>' if kind in ("quiet","some") else ""
    return (f'<div class="{cls}" aria-label="Sound in the last minute"><div class="bars">{bars}</div>{thrline}'
            f'<div class="base"></div>{m}<div class="cap"><span>{caps[0]}</span><span>{caps[1]}</span></div></div>')

def info_line(items):
    parts = "".join(f'<span>{I(ic,"xs")}{t}</span>' for ic, t in items)
    return f'<div class="meta-line">{parts}</div>'

def dock(listen="idle", talk="idle", pause=None, snack=None, listen_sub=None, panel=None):
    sn = f'<div class="snack"><span class="grow">{snack}</span><span class="act">Undo</span></div>' if snack else ""
    if panel:
        top = panel
    elif listen == "idle":
        top = f'<div class="listen"><span class="knob">{I("head")}</span><div class="tx"><b>Listen</b><small>{listen_sub or "Tap to hear the room"}</small></div></div>'
    elif listen == "on":
        top = f'<div class="listen on"><span class="knob">{I("stopsq")}</span><div class="tx"><b>Listening · tap to stop</b><small>Room sound on the speaker</small></div></div>'
    else:
        top = f'<div class="listen off"><span class="knob">{I("head")}</span><div class="tx"><b>Listen when connected</b><small>{listen_sub or "Not available right now"}</small></div></div>'
    if talk == "idle":
        t = f'<div class="qbtn">{I("mic")}Hold to talk</div>'
    elif talk == "off":
        t = f'<div class="qbtn dis">{I("mic")}Talk when connected</div>'
    elif talk.startswith("again"):
        t = f'<div class="qbtn stat" role="status">{I("clock")}Talk again<br>at {talk[6:]}</div>'
    else:
        t = f'<div class="qbtn talk-on">{I("mic")}{talk}</div>'
    p = (f'<div class="qbtn act" aria-label="Noise alerts paused until {pause}. Double-tap to resume.">{I("pausec")}Paused</div>' if pause
         else f'<div class="qbtn">{I("pausec")}Pause noise alerts</div>')
    return f'<div class="dock">{sn}{top}<div class="row3">{t}{p}<div class="qbtn">{I("more")}More</div></div></div>'

def vsteps(rows):
    out = '<div class="vsteps">'
    for st, label, when in rows:
        out += f'<div class="vs {st}"><span class="mk"></span><span>{label}</span><span class="when">{when}</span></div>'
    return out + "</div>"

def prog(cur, label, total=4):
    segs = "".join(f'<i class="{"d" if k < cur else ("c" if k == cur else "")}"></i>' for k in range(total))
    return f'<div class="prog" aria-hidden="true"><span class="segs">{segs}</span><span>{label}</span></div>'

def cause(line, steps, wrong=False, note=""):
    w = ' <span class="wrong">Wrong connection? Stop monitoring below and pick another.</span>' if wrong else ""
    first = f'<span class="wrong">{steps[0]}</span>' if steps else ""
    ol = "".join(f"<li>{x}</li>" for x in steps[1:])
    more = (f'<span class="disc" role="button" aria-expanded="false" aria-controls="steps">More steps {I("chevd","xs")}</span>' if len(steps) > 1 else "")
    nt = f'<div class="cnote">{note}</div>' if note else ""
    return (f'<div class="cause2"><div class="cl"><span>Likely cause</span>{line}{first}{w}</div>'
            f'<div class="crow"><span class="btn out sm">{I("refresh","s")}Try now</span>{more}</div>'
            f'<ol hidden>{ol}</ol>{nt}</div>')

QUIET_STOP = '<div class="btn txt quiet">Stop monitoring</div>'

def red_body(kick_icon, kick, h1, counter, counter_aria, counter_lbl, sentence, progress, cause_html, end=QUIET_STOP):
    return (f'<div class="kick" aria-hidden="true">{I(kick_icon,"s")}{kick}</div><h1 tabindex="-1">{h1}</h1>'
            f'<div class="fig2"><b aria-label="{counter_aria}">{counter}</b><span>{counter_lbl}</span></div>'
            f'<p>{sentence}</p>{progress}{cause_html}{end}')

def alert(theme, t, strip, body, foot, band=False):
    hz_ = '<div class="hazard"></div>' if band else ""
    return phone(theme, f'{sysb(t)}{hz_}{strip}<div class="al-body">{body}<div class="fadeb"></div></div><div class="al-foot">{foot}</div>')

def strip_flat(icon, txt, meta, accent=False, ok=False):
    c = " accent" if accent else ""
    st = ' style="color:#9fe0bb"' if ok else ""
    # inside a full-screen alert the strip is not a live region; focus on the headline speaks instead
    return f'<div class="sbar flat{c}" aria-hidden="true"><span class="ic"{st}>{I(icon)}</span><div class="grow">{txt}</div><span class="meta">{meta}</span></div>'

def slab(icon, big, small):
    return f'<div class="slab">{I(icon,"l")}<span>{big}<small>{small}</small></span></div>'

STEPS_RECON = ["Reconnecting", "No connection (still trying)", "Vibrates + alert screen", "Loud alarm"]

# ================= HOME / MONITOR =================
S1 = "Home, monitor, night mode"

def home(theme):
    return phone(theme, f"""{sysb("21:58")}
  <div class="hhead"><svg class="i logo"><use href="#i-logo"/></svg><span class="name">GuardedSleep</span><span class="ibtn" title="Settings">{I("gear")}</span></div>
  <div class="hzhead"><div class="label">Good evening</div><h1>Start monitoring</h1><p>Choose what this device does tonight.</p></div>
  <div class="rolehead">{I("phone","s")}Parent device <small>· stays with you</small></div>
  <div class="list" style="margin:0 12px">
    <div class="conn"><div class="tx"><b>Nursery</b><small>With Nursery tablet · last used today, 21:40</small></div><span class="btn fill sm">Start</span></div>
    <div class="conn"><div class="tx"><b>Grandma's flat</b><small>With Grandma's old phone · Sun</small></div><span class="btn ton sm">Start</span></div>
  </div>
  <div class="rolehead">{I("babydev","s")}Baby device <small>· stays in the child's room</small></div>
  <div class="list" style="margin:0 12px">
    <div class="conn"><div class="tx"><b>Travel cot</b><small>For Anna's phone · 12 Oct</small></div><span class="btn ton sm">Start</span></div>
  </div>
  <div style="padding:16px 12px 0"><div class="btn ton">{I("plus","s")}Set up new devices</div></div>
  <div class="btn txt" style="margin-top:4px;font-size:14.5px">Open at start: Ask every time · Change</div>""")

frame("01", "Home · returning user", "Role is a section header, connections sit directly on one card (no nested cards). One verb, <b>Start</b>, everywhere; each row names the other device and when it was last used, and the last-used row is tagged and filled. \"Open at start\" is one setting with one name, shown here as a 48 dp text button.", home("t-dusk"), section=S1)

frame("02", "First start", "Two large choices; the \"Use as default\" toggle is gone (it lives in Settings › Open at start). Privacy line corrected: sound may pass a relay, so it promises encryption, not a direct path.", phone("t-dusk", f"""{sysb("18:05")}
  <div class="hhead"><svg class="i logo"><use href="#i-logo"/></svg><span class="name">GuardedSleep</span><span class="ibtn" title="Language">{I("globe")}</span></div>
  <div class="hzhead" style="padding-bottom:28px"><div class="label">Welcome</div><h1>Which device is this?</h1><p>You need two devices: one stays with the child, one stays with you. Set up the baby device first.</p></div>
  <div style="height:16px"></div>
  <div class="firstcard"><span class="ic">{I("babydev","l")}</span><div class="tx"><b>Baby device</b><small>Stays in the child's room and listens.</small></div>{I("chev")}</div>
  <div class="firstcard"><span class="ic">{I("phone","l")}</span><div class="tx"><b>Parent device</b><small>Stays with you and alerts you.</small></div>{I("chev")}</div>
  <div style="margin:auto 22px 44px;display:flex;gap:12px;font-size:14.5px;color:var(--on-v);line-height:1.5">{I("shield","s")}<span>Sound is encrypted between your devices. No account, no cloud recording.</span></div>"""), section=S1)

def monitor(theme, t, top_extra="", h="All quiet since 22:14", p="Monitoring for 1 h 27 min", hzk="quiet", hzmsg=None, info=None,
            alert_="Sound", night="off", dk=None, barhtml=None, icon_only=False):
    info = info if info is not None else [("belloff", "Baby device in Quiet mode"), ("check", "alerts tested 21:30")]
    return phone(theme, f"""{sysb(t)}{barhtml or bar_calm()}{tools(alert_, night, icon_only=icon_only)}{top_extra}{hero(h,p)}{hz(hzk, hzmsg) if hzk else ""}{info_line(info) if info else ""}{dk or dock()}""")

frame("03", "Parent monitor · calm (Dusk)", "The connected bar is now neutral: surface colour, a green dot and link icon, words. A colour fill only ever means a change. One headline, then the <b>horizon strip</b>: the last minute of sound rising from a horizon line, with a labelled dashed line \"Alerts above this line\". No text inside the graphic, so nothing collides. Listen is one tap; Talk stays hold-to-talk.", monitor("t-dusk", "23:41"), section=S1)

frame("04", "Parent monitor · Day, listening", "Day theme with Listen switched on: the whole Listen bar fills and reads \"Listening · tap to stop\" with a stop square, so the on/off state is obvious from across the room. A short past sound is reported in words with a unit (\"8 s\").", monitor("t-day", "15:58", h="Sound at 15:52", p="Lasted 8 s · quiet again", hzk="some", dk=dock(listen="on"), barhtml=bar_calm(meta="64%", bolt=False)), section=S1)

frame("05", "Parent monitor · Ember night mode", "Night mode is on (chip \"Night on\", tap to switch off). Ember contrast fixed per a11y review (variant text 5.0:1 on cards, outlines 3.6:1); dimming comes from the screen brightness, not from lower contrast. The calm bar stays neutral, so nothing glows.", monitor("t-ember", "02:14", h="All quiet since 00:52", p="Monitoring for 3 h 47 min", night="on", info=[("belloff","Baby device in Quiet mode")], barhtml=bar_calm(meta="79%")), section=S1)

frame("06", "Monitor · noise alerts paused + Vibrate only", "Each state is said once: the banners are the only statement. The alert chip shrinks to its icon (it still opens the choice), the dock button shows only \"Paused\" in the tinted on-state, and the hero keeps its normal line. Banner order: Pause above Vibrate only (concept §4). While paused, the Vibrate-only line says what happens <i>after</i> the pause. Undo snackbar is transient (8 s).",
      monitor("t-dusk", "20:25", top_extra=
        f'<div class="banner pause">{I("pausec")}<div class="grow">Noise alerts paused until 20:55<small>Connection and battery alerts still come</small></div><span class="act">Resume</span></div>'
        f'<div class="banner vib">{I("vib")}<div class="grow">Vibrate only<small>After the pause: vibrate first, sound after 2 min</small></div><span class="act">Turn sound on</span></div>',
        h="All quiet since 20:02", p="Monitoring for 25 min", alert_="Vibrate only", info=[], hzk=None, icon_only=True,
        dk=dock(pause="20:55", snack="Noise alerts paused for 30 min")), section=S1)

def moresheet(theme):
    return phone(theme, f"""{sysb("23:42")}{bar_calm()}
  <div class="hero"><h2 style="opacity:.5">All quiet since 22:14</h2></div>
  <div class="scrim"></div>
  <div class="sheet"><div class="handle"></div>
    <div class="sh"><h4>Monitor options</h4><span class="ibtn" title="Close">{I("x")}</span></div>
    <div class="lbl2">Alerts for sounds</div>
    <div class="seg"><div class="on">Sound</div><div>Vibrate only</div></div>
    <div class="lbl2">Pause noise alerts</div>
    <div class="two"><div class="btn ton sm">10 min</div><div class="btn ton sm">30 min</div></div>
    <div class="note" style="padding-top:6px">Connection and battery alerts still come.</div>
    <div class="lbl2">Night mode</div>
    <div class="seg"><div>Off</div><div class="on">On</div><div>Automatic</div></div>
    <div class="list" style="margin-top:12px">
      <div class="li"><span class="ic">{I("flask","s")}</span><div class="tx"><b>Test alerts</b><small>Last test 21:30 · reached this phone</small></div>{I("chev","s")}</div>
      <div class="li"><span class="ic">{I("shield","s")}</span><div class="tx"><b>How is this private?</b></div>{I("chev","s")}</div>
    </div>
    <div class="list" style="margin-top:12px"><div class="li"><span class="ic" style="color:var(--alert)">{I("power","s")}</span><div class="tx"><b>Stop monitoring</b><small>Asks you to confirm</small></div></div></div>
  </div>""")

frame("07", "Monitor · More sheet", "Visible Close button (48 dp) plus drag handle. No \"Switch connection\" any more (spec: no switch while monitoring). Night mode uses the same Off · On · Automatic control as Settings. Stop monitoring sits apart, below a gap, and always confirms (frame 23). The scrim leaves the status bar readable (custom sheet; see concept §7).", moresheet("t-dusk"), section=S1)

frame("08", "Noise alert · shared alert template", "Same template as every full-screen alert: flat status strip, caps label, headline, big figure, feedback line, actions pinned at the bottom (content scrolls at 200 % font size). The slab is a dimmed but saturated wine with a light border (7.8:1 text), so it does not light the room. Listen comes first in the second row; 12 dp gap to the slab; Undo after any pause.", alert("t-noise", "23:47",
    strip_flat("link", "Connected · Nursery", f'{I("bolt","xs")}82%', ok=True),
    f"""<div class="kick" aria-hidden="true">{I("wave","s")}Sound in Nursery</div><h1 tabindex="-1">The child is making sound</h1>
    <div class="fig2"><b aria-label="12 seconds">0:12</b><span>since 23:47</span></div>
    <div style="display:flex;gap:5px;align-items:flex-end;height:56px;margin-top:16px">{''.join(f'<span style="flex:1;border-radius:3px;background:#ff948a;opacity:{.45+.5*(k%5)/5:.2f};height:{20+70*abs(((k*37)%11)/11-.5)*2:.0f}%"></span>' for k in range(28))}</div>
    <div class="fb">{I("speaker")}The room sound is playing on the speaker.</div>""",
    f"""{slab("volx","Stop sound","Stops the alert sound, keeps monitoring")}<div class="alrow" style="margin-top:12px"><div>{I("head","s")}Listen</div><div>{I("pausec","s")}Pause 10 min</div><div>{I("pausec","s")}Pause 30 min</div></div>"""), section=S1)

# ================= CONNECTION STATES =================
S2 = "Connection states"

frame("09", "First connection · neutral (first 20 s only)", "Never connected yet and less than 20 s since start. Blue, refresh symbol, no promise of \"a few seconds\". \"Wrong connection?\" is a plain line, and Stop monitoring is a quiet text button at the end of the content (confirm, frame 23), never a question inside a button.",
  monitor("t-dusk", "21:59", barhtml=bar("info","refresh","Connecting · Nursery","Waiting for the baby device"),
          h="Waiting for the baby device", p="Monitoring starts when Nursery tablet answers.", hzk="off", hzmsg=f'{I("refresh","s")}Nothing to hear yet', info=[],
          dk=f'<div class="wrongline">Wrong connection? You can stop monitoring and pick another on the start screen.</div>{QUIET_STOP.replace("btn txt quiet","btn txt quiet pad")}' + dock(listen="off", talk="off", listen_sub="Available once connected")), section=S2)

frame("10", "Short interruption · 0–20 s · neutral", "Was connected, lost contact less than 20 s ago. Same calm blue, different words. No timer, no steps. Listening resumes by itself (spec).",
  monitor("t-dusk", "01:03", barhtml=bar("info","refresh","Reconnecting · Nursery","Short drops are normal"),
          h="Back in a moment", p="The connection dropped briefly and usually comes back by itself. If it takes longer than 20 s, this bar turns orange.", hzk="off", hzmsg=f'{I("refresh","s")}Waiting for sound', info=[],
          dk=dock(listen="off", talk="off", listen_sub="Listening resumes by itself")), section=S2)

def stage11(theme, fixed=False, t="01:04", night="off"):
    b = bar("warn-fixed" if fixed else "warn", "linkoff", "Can't reach Nursery", "Still trying", f'<span aria-label="34 seconds">0:34</span>')
    line = f'<div style="padding:10px 22px 0;font-size:15.5px;line-height:1.45;font-weight:600">You may not hear the child right now. If it isn\'t back after 1 minute, this phone vibrates and shows an alert.</div>'
    return phone(theme, f'{sysb(t)}{b}{line}{tools("Sound", night)}{hz("off", I("linkoff","s")+"No sound coming in")}{dock(listen="off", talk="off", listen_sub="Not connected right now")}')

frame("11", "No connection, still trying · 20–60 s · lightweight", "Only the orange bar (outline, broken-link glyph, words, timer) plus one line that says what it means and what happens next. No card, no step bar, no big screen. The horizon strip goes flat with \"No sound coming in\" so nothing says \"All quiet\". Visual only plus a quiet notification.", stage11("t-dusk"), section=S2)

def stage12():
    return alert("t-lost", "01:05", strip_flat("warn", "No connection · Nursery", '<span aria-label="1 minute 12 seconds">1:12</span>'),
      red_body("vib", "Your phone is vibrating", "Please check on the child", "1:12", "1 minute 12 seconds", "without connection",
               "You can't hear the child. Loud alarm in 2 min (01:07) if still no connection.",
               prog(2, "Phone vibrating now"),
               cause("The baby device isn't online.", ["Is Nursery tablet on, plugged in and showing GuardedSleep?", "Is it on Wi-Fi? If not, open GuardedSleep on it again."])),
      slab("checkc", "I'm checking", "Silences sound and vibration for 5 min"))
frame("12", "No connection · from 1 min · full screen", "Simplified per round 2: strip, label, headline, big counter, <b>one sentence</b> with the next step and its time, a one-line progress (\"3 of 4\"), and one compact cause card: cause line, secondary Try now, \"Show steps\" folded by default. \"I'm checking\" is the only filled, pinned action. Stop monitoring is a quiet text button at the end of the content (scroll), with the confirm.", stage12(), section=S2)

frame("13", "Loud alarm · from 3 min", "Same simplified template. Hazard band and accent edge mark the loudest stage. No tone name here (settings only). The DND note sits under the cause card and is conditional: without the bypass grant it reads \"Do Not Disturb may block this alarm\" with a settings link.", alert("t-alarm", "01:07",
    strip_flat("speaker", "Loud alarm · Nursery", '<span aria-label="3 minutes 24 seconds">3:24</span>', accent=True),
    red_body("speaker", "Loud alarm sounding", "Still no connection to Nursery", "3:24", "3 minutes 24 seconds", "without connection",
             "Go to the child now. The alarm sounds until you tap \"I'm checking\".",
             prog(3, "Loud alarm now"),
             cause("This phone has no internet.", ["Check Wi-Fi or mobile data on this phone, and that flight mode is off.", "Move closer to the router."], note="The alarm also sounds in Do Not Disturb.")),
    slab("checkc", "I'm checking", "Stops the alarm for 5 min"), band=True), section=S2)

frame("14", "Connected again", "One element only (the bar is neutral again, the card carries the message). Duration in words, the missed-sound result in plain words, and a full-width OK in the thumb zone. The card stays until OK, never auto-dismisses (WCAG 2.2.1); the same text goes to a notification.", phone("t-dusk", f"""{sysb("01:08")}{bar_calm(meta="81%")}{tools()}
  <div class="card" style="background:var(--ok-c)"><div style="display:flex;gap:12px"><span style="color:var(--ok)">{I("checkc","l")}</span><div><h3>Connected again</h3><p>Monitoring was interrupted for <b style="color:var(--on)">1 min 48 s</b>.</p><p>The baby device heard no sound during that time.</p></div></div>
  <div class="btn out" style="margin-top:14px;border-color:var(--ok);color:var(--on)">OK</div></div>
  {hero("All quiet since 01:08", "Monitoring again")}{dock()}"""), section=S2)

# ================= BABY =================
S3 = "Baby unit"

def kstatus(icon, txt):
    return f'<div class="k-status">{I(icon,"s")}{txt}</div>'

frame("15", "Baby unit · dark nursery, connected", "Black kiosk, but an always-visible status line \"Listening · Travel cot\" at #7D7671 (4.7:1 on black; the screen brightness keeps it dim). State is coded by <b>shape</b> as well as words: short solid bar = all fine; dashed top edge = still trying (20–60 s); full frame with slow pulse = no connection. Never sound, vibration or haptics.", phone("t-kiosk", f"""
  <div style="position:absolute;top:22px;left:50%;transform:translateX(-50%);width:44px;height:6px;border-radius:3px;background:var(--k-ok)"></div>
  <div class="k-wrap"></div>{kstatus("link","Listening · Travel cot")}<div style="text-align:center;font-size:14px;color:var(--on-v);margin:-34px 0 40px">Touch for details</div>"""), section=S3)

frame("16", "Baby unit · talk-back incoming", "Static dim microphone ring, \"Parent is talking\" with which parent device. No brightness boost. During a talk the status line and edge mark are hidden: only the talk indicator shows (talk-back spec).", phone("t-kiosk", f"""
  <div class="k-wrap"><div style="width:132px;height:132px;border-radius:50%;border:2px solid var(--k-talk);display:grid;place-items:center;color:var(--k-talk)">{I("mic","xl")}</div>
  <b style="margin-top:24px">Parent is talking</b><p>Anna's phone · ends by itself after 30 s</p></div><div style="height:80px"></div>"""), section=S3)

frame("17", "Baby unit · connection lost (touch-wake)", "No connection for more than 1 min: full edge frame, slow pulse (shape, works without hue). Touch-wake text at 4.7:1 names the connection, the parent device and the last talk (spec). Silent by design, and it says so.", phone("t-kiosk", f"""
  <div class="kedge edge-pulse" style="box-shadow:inset 0 0 0 5px var(--k-alert)"></div>
  {sysb("01:07")}
  <div class="k-wrap" style="justify-content:flex-start;padding-top:40px"><span style="color:var(--k-alert)">{I("linkoff","xl")}</span>
  <b style="margin-top:14px">Parent device not responding — still trying</b><p>No connection for 3 min 24 s. This device stays silent and reconnects by itself.</p>
  <div class="k-rows"><div>Connection<b>Travel cot · baby device</b></div><div>Parent device<b>Anna's phone</b></div><div>Last talk<b>00:41 · 12 s</b></div><div>This device<b>charging · 64%</b></div></div></div>
  <div style="padding:0 0 40px"><div class="k-slide"><i>{I("chev","s")}</i>Hold and slide to unlock</div></div>"""), section=S3)

# ================= SETTINGS =================
S4 = "Settings and connections"

def settings(theme, monitoring=True):
    top = bar_calm() if monitoring else ""
    ro = monitoring
    rows = [("phone","Nursery","Parent device · with Nursery tablet","In use"),("phone","Grandma's flat","Parent device · with Grandma's old phone",None),("babydev","Travel cot","Baby device · for Anna's phone",None)]
    r = ""
    for ic, n, s, u in rows:
        tag = f' <span class="pill ok">● {u}</span>' if u else ""
        tail = "" if ro else I("chev","s")
        r += f'<div class="li{" ro" if ro else ""}"><span class="ic">{I(ic,"s")}</span><div class="tx"><b>{n}{tag}</b><small>{s}</small></div>{tail}</div>'
    note = f'<div class="note">{I("lock","xs")} Stop monitoring to change connections.</div>' if ro else ""
    return phone(theme, f"""{sysb("23:45")}{top}
  <div class="topbar"><span class="ibtn" title="Back">{I("back")}</span><span class="title">Settings</span></div>
  <div class="scroll">
    <div class="gh" style="margin-top:6px">Connections <span>parent 2 of 2 · baby 1 of 2</span></div>
    <div class="list">{r}</div>{note}
    <div class="gh">This device</div>
    <div class="list">
      <div class="li"><span class="ic">{I("bell","s")}</span><div class="tx"><b>Alerts</b><small>Sound · alarm tone Classic</small></div>{I("chev","s")}</div>
      <div class="li"><span class="ic">{I("wave","s")}</span><div class="tx"><b>Sound detection</b><small>Medium sensitivity · at least 2 s</small></div>{I("chev","s")}</div>
      <div class="li"><span class="ic">{I("display","s")}</span><div class="tx"><b>Display</b><small>{"Light" if theme=="t-day" else "Dark"} · Night mode off</small></div>{I("chev","s")}</div>
      <div class="li"><span class="ic">{I("power","s")}</span><div class="tx"><b>Open at start</b><small>Ask every time · or open one connection directly</small></div>{I("chev","s")}</div>
      <div class="li"><span class="ic">{I("shield","s")}</span><div class="tx"><b>Check this device's setup</b><small>All set — alerts can reach you</small></div>{I("chev","s")}</div>
      <div class="li"><span class="ic">{I("globe","s")}</span><div class="tx"><b>Language</b><small>English</small></div>{I("chev","s")}</div>
    </div>
    <div class="fadeb"></div></div>""")

frame("18", "Settings · overview (parent, while monitoring)", "Status bar stays on top. While monitoring the connection rows are read-only (no chevron, lock note \"Stop monitoring to change connections\"); the one in use carries a dot plus the word. \"Open at start\" is the single place for the default, with its meaning in the sub-line.", settings("t-dusk"), section=S4)

frame("19", "Settings › Alerts", "\"Vibrate only\" replaces Discreet. Grouped by situation in plain words. The gain row says it covers noise alerts. Connection alarm tone shows the chosen tone.", phone("t-dusk", f"""{sysb("23:46")}{bar_calm()}
  <div class="topbar"><span class="ibtn">{I("back")}</span><span class="title">Alerts</span></div>
  <div class="scroll">
    <div class="gh" style="margin-top:6px">When the child makes sound</div>
    <div class="list">
      <div class="li" style="flex-direction:column;align-items:stretch;gap:10px"><div><b>Alerts for sounds</b><small>Sound: you hear the room with each alert</small></div><div class="seg"><div class="on">Sound</div><div>Vibrate only</div></div></div>
      <div class="li"><div class="tx"><b>If nobody reacts to Vibrate only</b><small>Switch to sound after 2 minutes (recommended)</small></div>{I("chev","s")}</div>
      <div class="li"><div class="tx"><b>Alert sound plays on</b><small>Headphones if connected, otherwise speaker</small></div>{I("chev","s")}</div>
      <div class="li" style="flex-direction:column;align-items:stretch"><div style="display:flex;justify-content:space-between"><b>Extra loud noise alerts</b><span class="val">0%</span></div><div class="slider"><i style="width:4%"></i><b style="left:0"></b></div></div>
    </div>
    <div class="gh">When the connection is lost</div>
    <div class="list"><div class="li"><span class="ic">{I("bell","s")}</span><div class="tx"><b>Connection alarm tone <span class="pill new">New</span></b><small>Classic · two alternating beeps</small></div>{I("chev","s")}</div></div>
    <div class="note">For sounds you always hear the room itself. The alarm tone is only for a lost connection.</div>
    <div class="list" style="margin-top:10px"><div class="li"><span class="ic">{I("flask","s")}</span><div class="tx"><b>Test alerts</b><small>Last test 21:30 · reached this phone</small></div><span class="btn ton sm">Test</span></div></div>
    <div class="fadeb"></div></div>"""), section=S4)

TONES = [("Classic","Two alternating beeps, high and low. Today's sound.","Default"),("Low","Deep, slow beeps. Best if high sounds are hard to hear.","Hard of hearing"),
         ("Beacon","Three quick high beeps, then a pause.",None),("Rising","Four notes stepping upward, then a pause.",None),
         ("Chime","Three soft bell notes, one after another.",None)]
def tones(theme):
    r = ""
    for k, (n, d, tag) in enumerate(TONES):
        sel = k == 0
        playing = k == 2
        pb = (f'<span class="play on" aria-label="Stop preview of {n}">{I("stopsq","xs")}Stop</span>' if playing
              else f'<span class="play" aria-label="Play {n}, soft then loud">{I("play","xs")}Play</span>')
        t = f' <span class="pill">{tag}</span>' if tag else ""
        sub = "Playing soft… then loud" if playing else d
        r += f'<div class="li{" sel" if sel else ""}"><span class="radio{" on" if sel else ""}"></span><div class="tx"><b>{n}{t}</b><small>{sub}</small></div>{pb}</div>'
    return phone(theme, f"""{sysb("23:47")}{bar_calm()}
  <div class="topbar"><span class="ibtn">{I("back")}</span><span class="title">Connection alarm tone</span></div>
  <div class="scroll">
    <div class="note" style="padding:2px 8px 10px">Plays when the connection is lost: a soft reminder after 1 minute, a loud alarm after 3 minutes. Play shows both, soft then loud, at reduced volume. <b style="color:var(--on)">Hard of hearing? Choose Low.</b></div>
    <div class="list">{r}</div>
    <div class="btn out" style="margin-top:12px">{I("bell","s")}Test connection alarm</div>
    <div class="note">{I("speaker","xs")} Plays at full alarm volume for about 8 s, on this phone only.</div>
    <div class="fadeb"></div></div>""")

frame("20", "Alarm tone picker", "Tone set aligned with the OpenSpec change, with Low (replaces Pulse; delta) directly under Classic, tagged \"Hard of hearing\", and the line \"Hard of hearing? Choose Low.\" Then Beacon, Rising (stepped notes, no glide), Chime. Descriptions say what each sounds like. The soft/loud switch is gone: Play plays soft then loud at reduced volume and turns into Stop while playing. The test warns that it is full volume.", tones("t-dusk"), section=S4)

frame("21", "Settings › Display · night mode", "One control: <b>Off · On · Automatic</b>. The time window appears only under Automatic (owner decision: no light sensor in alpha, backlog). Toggles carry the word On/Off. The note states that night mode never dims alerts or connection problems.", phone("t-dusk", f"""{sysb("20:12")}{bar_calm(meta="91%")}
  <div class="topbar"><span class="ibtn">{I("back")}</span><span class="title">Display</span></div>
  <div class="scroll">
    <div class="gh" style="margin-top:6px">Colours</div>
    <div class="list"><div class="li"><span class="radio on"></span><div class="tx"><b>Dark (default)</b></div></div><div class="li"><span class="radio"></span><div class="tx"><b>Light</b></div></div><div class="li"><span class="radio"></span><div class="tx"><b>Match system</b></div></div></div>
    <div class="gh">Screen at night</div>
    <div class="list">
      <div class="li" style="flex-direction:column;align-items:stretch;gap:10px"><div><b>Night mode</b><small>Very dim, red-tinted screen for a dark room</small></div><div class="seg"><div>Off</div><div>On</div><div class="on">Automatic</div></div></div>
      <div class="li sub"><div class="tx"><b>Between</b></div><span class="timechip">21:00</span><span>–</span><span class="timechip">07:00</span></div>
    </div>
    <div class="note">Now off · turns on at 21:00. Alerts and connection problems always use their normal colours.</div>
    <div class="fadeb"></div></div>"""), section=S4)

def connections(theme):
    return phone(theme, f"""{sysb("19:20")}
  <div class="topbar" style="padding-top:8px"><span class="ibtn">{I("back")}</span><span class="title">Connections</span></div>
  <div class="scroll">
    <div class="gh" style="margin-top:6px">This device as parent device <span>2 of 2</span></div>
    <div class="list">
      <div class="li" style="flex-direction:column;align-items:stretch;gap:12px;padding:16px">
        <div style="display:flex;align-items:center;gap:12px"><span class="ic">{I("phone","s")}</span><div style="flex:1"><b>Nursery</b><small>With Nursery tablet · last used today, 21:40</small></div>{I("chevd","s")}</div>
        <div style="border:1.5px solid var(--outline);border-radius:14px;padding:8px 14px;min-height:56px"><small style="margin:0">Name · only on this device</small><div style="display:flex;align-items:center;justify-content:space-between;font-size:16px;font-weight:600;margin-top:2px">Nursery{I("pen","s")}</div></div>
        <div class="btn danger sm">{I("trash","s")}Forget this connection</div>
      </div>
      <div class="li"><span class="ic">{I("phone","s")}</span><div class="tx"><b>Grandma's flat</b><small>With Grandma's old phone · Sun</small></div>{I("chev","s")}</div>
    </div>
    <div class="gh">This device as baby device <span>1 of 2</span></div>
    <div class="list"><div class="li"><span class="ic">{I("babydev","s")}</span><div class="tx"><b>Travel cot</b><small>For Anna's phone · 12 Oct</small></div>{I("chev","s")}</div></div>
    <div class="note">Up to 2 connections for each. Forget one to add another. Which one opens at start: Settings › Open at start.</div>
    <div class="btn ton" style="margin-top:10px">{I("plus","s")}Set up new devices</div>
    <div class="fadeb"></div></div>""")
frame("22", "Connections · rename, forget", "Only while not monitoring. Expand inline: name (local only), Forget with a role-correct label (\"Forget this connection\") and a confirm dialog. No default toggle here: \"Open at start\" has one home, referenced in the note.", connections("t-dusk"), section=S4)

frame("23", "Stop monitoring · confirm", "Replaces \"Stop and switch\". The only way out of monitoring is an explicit Stop with this confirm; \"Keep monitoring\" is the filled default in the thumb position, back and scrim keep monitoring. Opened from a red screen, the confirm silences and acknowledges nothing: sound and vibration go on until Stop is confirmed, and Keep monitoring / back / scrim return to the alert unchanged. After stopping, Home (frame 01) is where role and connection are chosen; the other connection of the same role is listed first (\"Start Grandma\'s flat\"), but nothing starts without a tap and the role never changes silently.", phone("t-dusk", f"""{sysb("23:52")}{bar_calm()}
  <div class="hero"><h2 style="opacity:.5">All quiet since 22:14</h2></div>
  <div class="scrim"></div>
  <div class="sheet"><div class="handle"></div>
    <div class="sh"><h4>Stop monitoring Nursery?</h4><span class="ibtn" title="Close">{I("x")}</span></div>
    <p class="lead">This phone will give no alerts any more, also not for a lost connection. The baby device shows that you stopped.</p>
    <div class="btn danger" style="margin:8px 0">{I("power","s")}Stop monitoring</div>
    <div class="btn fill" style="min-height:60px">Keep monitoring</div>
  </div>"""), section=S4)

# ================= NEW IN ROUND 1 =================
S5 = "Added in round 1 · failure and edge states"

frame("24", "First connection failed · 20–60 s", "Level 1 from start. Orange bar \"Not connected yet\", one line with the likely cause, \"Wrong connection?\" as a line and Stop monitoring as a quiet text button (confirm).",
  phone("t-dusk", f'{sysb("22:00")}{bar("warn","linkoff","Not connected yet · Nursery","Still trying",'<span aria-label="34 seconds">0:34</span>')}'
        f'<div style="padding:10px 22px 0;font-size:15.5px;line-height:1.45;font-weight:600">Nursery tablet hasn\'t answered. Is monitoring started on it? Until then you can\'t hear the child.</div>'
        f'{tools()}{hz("off", I("linkoff","s")+"Nothing to hear yet")}'
        +f'<div class="wrongline">Wrong connection? You can stop monitoring and pick another on the start screen.</div>{QUIET_STOP.replace("btn txt quiet","btn txt quiet pad")}'+f'{dock(listen="off", talk="off", listen_sub="Available once connected")}'),
  tag="NEW", section=S5)

frame("25", "First connection failed · from 1 min", "Same simplified template as frame 12. The headline says monitoring never started, the progress reads \"Connecting\" stages, and \"Wrong connection?\" sits inside the cause line; Stop monitoring is the quiet text button at the end.", alert("t-lost", "22:01",
    strip_flat("warn", "Couldn't connect · Nursery", '<span aria-label="1 minute 20 seconds">1:20</span>'),
    red_body("vib", "Your phone is vibrating", "Couldn't connect to Nursery", "1:20", "1 minute 20 seconds", "since start",
             "Monitoring never started. Loud alarm in 2 min (22:03) if still no connection.",
             prog(2, "Phone vibrating now"),
             cause("Nursery tablet isn't online.", ["Start monitoring on Nursery tablet.", "Both devices need internet, or the same Wi-Fi."], wrong=True)),
    slab("checkc", "I'm checking", "Silences sound and vibration for 5 min")), tag="NEW", section=S5)

frame("26", "After \"I'm checking\"", "Stays red, same template. The sentence carries the comeback time computed from the acknowledgment (tapped 03:12 → 03:17). Pinned action is the calm \"Turn sound back on now\".", alert("t-lost", "03:13",
    strip_flat("warn", "No connection · Nursery", '<span aria-label="4 minutes 5 seconds">4:05</span>'),
    red_body("pausec", "Alarm paused", "Comes back at 03:17 if still no connection", "4:05", "4 minutes 5 seconds", "without connection",
             "You still can't hear the child.",
             prog(2, "Paused while you check"),
             cause("The connection is blocked on this network.", ["Guest or hotel Wi-Fi can block it. Switch this phone to mobile data.", "Then tap Try now."])),
    f'<div class="btn out" style="min-height:60px;border-color:var(--outline);color:var(--on)">{I("speaker","s")}Turn sound back on now</div>'), tag="NEW", section=S5)

frame("27", "Microphone hears nothing", "<code>mic_silent</code>: the link is fine (bar stays neutral) but the baby device's microphone delivers only silence. Warn banner with the fix, the strip goes hatched with \"Microphone silent\" and the headline says \"Can't hear the room\", never \"All quiet\". Announced assertively.",
  monitor("t-dusk", "23:58", top_extra=f'<div class="banner warn">{I("micoff")}<div class="grow">Nursery\'s microphone hears nothing<small>Check the baby device. Another app may be using its microphone.</small></div><span class="act">Help</span></div>',
          h="Can't hear the room", p="No sound has reached the microphone for 30 s. That is not the same as quiet.", hzk="off", hzmsg=f'{I("micoff","s")}Microphone silent', info=[]).replace('class="hz off"','class="hz off hatch"'),
  tag="NEW", section=S5)

frame("28", "Readiness problem on the monitor", "The worst failing readiness item sits in the banner slot, ranked above Vibrate only and Pause. Order: notifications off → full-screen alerts off → alarm volume 0 → Do Not Disturb bypass missing → battery optimisation on → this phone below 20 % → headphones connected → no test for 30 days. One \"Fix\" opens the right system setting.",
  monitor("t-dusk", "22:20", top_extra=f'<div class="banner alert">{I("volx")}<div class="grow">Alarm volume is off<small>You won\'t hear alerts on this phone</small></div><span class="act">Fix</span></div>',
          h="All quiet since 22:14", p="Monitoring for 6 min"),
  tag="NEW", section=S5)

frame("29", "Baby device stopped monitoring", "Grey bar with pause glyph (fixed safety colours in Ember). The strip and \"All quiet\" are hidden: nothing is monitored, so nothing may look calm. One alert was raised once (no escalation, spec). The card says what it means and that the screen updates by itself.", phone("t-dusk", f"""{sysb("06:40")}{bar("grey","pause","Stopped · Nursery","Monitoring was stopped on the baby device")}{tools()}
  <div class="card"><h3>Nobody is listening in Nursery</h3><p>Someone stopped monitoring on the baby device. You get no alerts until it is started there again. This screen updates by itself.</p>
  <p style="font-size:14.5px">Check who stopped it on the baby device.</p><div class="btn txt quiet">Close monitoring on this phone</div></div>
  {dock(listen="off", talk="off", listen_sub="Nothing to listen to")}"""), tag="NEW", section=S5)

frame("30", "Needs you · code didn't match", "Copy per critical review: \"no longer paired\" was misleading. Variants in the copy table: update needed (\"Update GuardedSleep on both devices\") and the calm security warning while retrying (\"Someone may be interfering with the connection. GuardedSleep keeps trying.\").", phone("t-dusk", f"""{sysb("21:05")}{bar("needs","linkoff","Needs you · Nursery","Monitoring can't start")}
  <div class="card" style="margin-top:20px"><span style="color:var(--alert)">{I("qr","l")}</span><h3 style="margin-top:10px">The code didn't match</h3><p>These two devices could not confirm each other. Scan the code on the baby device again.</p>
  <div class="btn fill" style="margin-top:16px">{I("qr","s")}Scan the code again</div><div class="btn txt" style="margin-top:4px">Stop monitoring</div></div>"""), tag="NEW", section=S5)

frame("31", "Baby device battery almost empty", "Below 5 %: shared alert template on the fixed <b>orange</b> palette, so red stays reserved for \"you can\'t hear the child\". Not snoozable. At 15 % (not charging) a warn banner appears instead: \"Nursery tablet battery low · 14 % · plug it in\"; unplugged while monitoring: info banner \"The baby device is no longer charging\". This phone below 20 % is a readiness item (frame 28).", alert("t-batt", "04:12",
    strip_flat("link", "Connected · Nursery", f'{I("battlow","xs")}4%', ok=True),
    f"""<div class="kick" aria-hidden="true">{I("battlow","s")}Baby device battery</div><h1 tabindex="-1">Nursery tablet is almost empty</h1>
    <div class="fig2"><b>4%</b><span>battery left</span></div>
    <p>Plug in the baby device now. When it runs out, monitoring stops.</p>
    <div class="fb">{I("info")}The baby device stays silent. Only this phone alerts.</div>""",
    slab("check", "OK, I'll plug it in", "Closes this screen")), tag="NEW", section=S5)

def talk(stage):
    if stage == "wait":
        panel = f'<div class="talkpanel" style="background:var(--pri-c);color:var(--on-pri-c)">{I("refresh","l")}<div><b>One moment…</b><small>Keep holding · asking the baby device</small></div></div>'
        top = ""
    elif stage == "speak":
        panel = (f'<div class="talkpanel dark">{I("mic","l")}<div style="flex:1"><b>Speak now</b><small>Release to stop</small>'
                 f'<div class="progress"><i style="width:40%"></i></div></div><span class="tm" aria-label="18 seconds left">0:18</span></div>')
        top = f'<div class="banner info">{I("pausec")}<div class="grow">Noise alerts paused while you talk<small>Sound detection restarts 2 s after you let go.</small></div></div>'
    return phone("t-dusk", f'{sysb("00:41")}{bar_calm()}{tools()}{top}{hero("All quiet since 22:14", None)}{hz("quiet", hatch=(stage=="speak"))}'
                 f'<div class="dock"><div class="statusline" role="status">{I("head","s")}Listening is paused while you talk</div>{panel}</div>')

frame("32", "Talk-back · one moment", "Hold to talk (the spec requires a hold). Pressing asks the baby device; until it grants, the panel says \"One moment… keep holding\". Accessibility action \"Talk for up to 30 s\" starts a timed talk with a visible Stop.", talk("wait"), tag="NEW", section=S5)
frame("33", "Talk-back · speak now", "Granted: \"Speak now · Release to stop\" with the time left (big counter) and a bar; haptic tick 5 s before the end. Banner: noise alerts are paused on the baby device while you talk (spec), and when they come back.", talk("speak"), tag="NEW", section=S5)
frame("34", "Talk-back · ended, available again at …", "After a talk the banner gives the reason (here the 30 s limit) and the clock time when talk is possible again (cooldown or 90 s budget). The Talk slot shows the same time as status (outlined, full contrast, role status), not as a disabled button. \"All quiet since\" restarts after the talk, because detection was paused during it. Other reasons (off on the baby device, can't play sound, other parent talking, no answer) are in the copy table.",
  phone("t-dusk", f'{sysb("00:42")}{bar_calm()}{tools()}<div class="banner info">{I("clock")}<div class="grow">Talk ended: time limit reached<small>You can talk again at 00:43. The baby device needs a short break so it can hear the child.</small></div></div>{hero("All quiet since 00:42", "Restarted after your talk")}{hz("quiet")}{dock(talk="again 00:43")}'),
  tag="NEW", section=S5)

frame("35", "Ember · no connection, still trying", "From the orange stage on, the status bar leaves Ember and uses the fixed Dusk safety colours (outline + glyph + words), so it cannot blend into the red-tinted night UI. Night mode itself stays on for the rest of the screen.", stage11("t-ember", fixed=True, t="02:31", night="on"), tag="NEW", section=S5)

# web
S6 = "Added in round 1 · browser parent"
def webphone(inner, theme="t-dusk"):
    return phone(f"{theme} t-web", f"{sysb('22:10')}{webbar()}{inner}")

frame("36", "Browser parent · tap to turn on alert sound", "Browsers block sound until a tap. The gate covers the monitor until tapped; the status bar stays visible. No Vibrate-only chip in the browser (spec: the browser parent keeps sound). The dock is the same as on Android, including Hold to talk (owner decision: full web talk-back per <code>add-talk-back</code>); the first tap on any control also unlocks sound.", webphone(f"""{bar_calm()}
  <div style="flex:1;display:flex;flex-direction:column;justify-content:center;padding:0 22px;gap:14px;text-align:center">
  <span style="margin:0 auto;color:var(--pri)">{I("speaker","xl")}</span><h2 style="margin:0;font-size:24px">Alert sound is off</h2>
  <p style="margin:0;color:var(--on-v);font-size:15.5px;line-height:1.5">Your browser needs one tap before GuardedSleep may play alerts.</p>
  <div class="btn fill" style="min-height:64px;font-size:17px">{I("hand","s")}Tap to turn on alert sound</div></div>{dock()}"""), tag="NEW", section=S6)

frame("37", "Browser parent · keep this page open", "Persistent banner with the wake lock held. Tools: \"Black screen\" (the web overlay, formerly \"Night screen\") next to \"Night\" (Ember colours). Two names, two jobs: Night mode = dim red colours; Black screen = a black cover that keeps the page awake, closed by an alert at once. Dock identical to Android: Listen, <b>Hold to talk</b> (pointer hold, or Space held; button \"Talk for up to 30 s\" as accessible alternative), Pause, More; talk states as frames 32–34.", webphone(f"""{bar_calm()}{tools(web=True)}
  <div class="banner info">{I("info")}<div class="grow">Keep this page open and the screen on<small>Alerts only work while this page is visible.</small></div></div>
  {hero("All quiet since 22:02", "Monitoring for 8 min")}{hz("quiet")}{dock()}"""), tag="NEW", section=S6)

frame("38", "Browser parent · back from the background", "After the page was hidden: warning with the duration in words and the missed-events result from the core. While hidden, a system notification said that alerts can't reach you. A talk that was running when the page went to the background ends at once (\"Talk stopped\").", webphone(f"""{bar_calm()}{tools(web=True)}
  <div class="banner warn">{I("warn")}<div class="grow">This page was in the background for 6 min<small>Alerts could not reach you then. The baby device heard no sound meanwhile.</small></div><span class="act">OK</span></div>
  {hero("All quiet since 22:02", None)}{hz("quiet")}{dock()}"""), tag="NEW", section=S6)

frame("39", "Browser · black screen (first tap shows status)", "Pure black cover, a slowly drifting dim dot. First tap shows the status for a few seconds, second tap closes. Any alert replaces it at once. Talk-back is not reachable under the cover; the first tap reveals the status, the second returns to the monitor with Hold to talk.", phone("t-kiosk t-web", f"""<div class="drift" style="left:250px;top:300px"></div>
  <div class="k-wrap"><b>Connected · Nursery</b><p>All quiet since 22:02</p><p style="margin-top:20px">Tap again to close the black screen</p></div>"""), tag="NEW", section=S6)

S7 = "Added in round 1 · Day variants and baby unlock"
frame("40", "Day · Home", "Day surface check: white cards with a 1 px border on warm paper (card vs page alone is only 1.08:1).", home("t-day"), tag="NEW", section=S7)
frame("41", "Day · Settings (while monitoring)", "Read-only connection rows, neutral status bar in Day.", settings("t-day"), tag="NEW", section=S7)
frame("42", "Day · alarm tone picker", "Preview playing state and the full-volume test warning in Day.", tones("t-day"), tag="NEW", section=S7)
frame("43", "Day · no connection, still trying", "Orange stage in Day. Full-screen alerts stay on their dark fixed palettes in Day too, so they never light the room more than needed.", stage11("t-day", t="14:22"), tag="NEW", section=S7)
frame("44", "Baby unit · unlocked", "After hold-and-slide: who is connected, last talk, Quiet mode, battery; \"Lock screen\" as the default, \"Stop monitoring\" with confirm. Using this device as the parent device: stop, then choose on Home.", phone("t-kiosk", f"""{sysb("21:30")}
  <div class="k-wrap" style="justify-content:flex-start;padding-top:24px;color:#bdb6b0"><span style="color:var(--k-ok)">{I("link","xl")}</span>
  <b style="margin-top:12px;color:#d8d2cc">Listening · Travel cot</b><p style="color:#a8a19b">This device is the baby device.</p>
  <div class="k-rows"><div>Parent device<b style="color:#d8d2cc">Anna's phone · connected</b></div><div>Last talk<b style="color:#d8d2cc">21:12 · 9 s</b></div><div>Quiet mode<b style="color:#d8d2cc">on</b></div><div>Battery<b style="color:#d8d2cc">charging · 64%</b></div></div></div>
  <div style="padding:0 22px 40px;display:flex;flex-direction:column;gap:10px">
  <div class="btn kbtn" style="min-height:56px">{I("lock","s")}Lock screen</div>
  <div class="btn" style="border:1.5px solid #5e5854;color:#e2a39b;min-height:52px">{I("power","s")}Stop monitoring</div>
  <div style="text-align:center;font-size:14px;color:#a8a19b;line-height:1.5">To use this device as the parent device, stop monitoring and choose on the start screen.</div></div>"""), tag="NEW", section=S7)


S8 = "Added in round 2 · stress cases and baby states"
def de_dock():
    return (f'<div class="dock"><div class="listen"><span class="knob">{I("head")}</span><div class="tx"><b>Zuhören</b><small>Tippen, um den Raum zu hören</small></div></div>'
            f'<div class="row3"><div class="qbtn">{I("mic")}Zum Sprechen halten</div><div class="qbtn" aria-label="Geräusch-Alarme pausieren">{I("pausec")}Pausieren</div><div class="qbtn">{I("more")}Mehr</div></div></div>')
frame("45", "German · monitor, tool row check", "Longest German tool row with night mode on: \"Meldung: Ton\" plus \"An\" plus the gear, 16 px gutter kept. The settings button is laid out first and never shrinks; the alert chip ellipsizes before anything is cut; above 1.15× the night chip shows its icon only (state in <code>stateDescription</code>). While Vibrate only is active the chip is icon-only anyway (frame 06), so \"Nur Vibration\" never has to fit here.",
  phone("t-dusk", f'{sysb("23:41")}{bar_calm(state="Verbunden", name="Kinderzimmer")}{tools(night="on", de=True)}{hero("Alles ruhig seit 22:14","Überwachung seit 1 Std. 27 Min.")}'
        f'{hz("quiet", label="Darüber gibt es einen Alarm", caps=("Letzte Minute","jetzt"))}{info_line([("belloff","Babygerät im Ruhemodus"),("check","Alarme getestet 21:30")])}{de_dock()}'),
  tag="NEW", section=S8)

frame("46", "200 % font scale · red alert", "Rendered with every text size doubled. The body scrolls (scroll indicator at the right, content continues under the fade), the counter drops to Headline size, the progress stays one line, and \"I'm checking\" stays pinned and fully visible. Gate: Roborazzi + Playwright snapshot at fontScale 2.0 for frames 08, 12, 13, 26.",
  phone("t-lost fs2", f'{sysb("01:05")}{strip_flat("warn", "No connection", "1:12")}<div class="al-body">'
        + red_body("vib", "Your phone is vibrating", "Please check on the child", "1:12", "1 minute 12 seconds", "without connection",
                   "You can't hear the child. Loud alarm in 2 min (01:07) if still no connection.", prog(2, "Vibrating"),
                   cause("The baby device isn't online.", []))
        + '<div class="fadeb"></div><div class="scrollhint"></div></div><div class="al-foot">' + slab("checkc", "I'm checking", "5 min silence") + '</div>'),
  tag="NEW", section=S8)

frame("47", "200 % font scale · monitor", "Text doubled: the alert chip takes row 1, night and gear stay together right-aligned on row 2 (traversal order fixed: status, alert, night, settings), the middle scrolls between the pinned status bar and the pinned dock, and the dock becomes 2 + 1. The strip keeps its graphic, the headline carries the state.",
  phone("t-dusk fs2", f'{sysb("23:41")}{bar_calm()}<div class="mid">{tools()}{hero("All quiet since 22:14","Monitoring for 1 h 27 min")}{hz("quiet")}<div class="fadeb"></div><div class="scrollhint"></div></div>{dock()}'),
  tag="NEW", section=S8)

frame("48", "Several banners at once · order and limit", "Global order: security warning › microphone silent › readiness › baby battery critical / unplugged › weak connection › Pause › Vibrate only. At most two banners show; the rest collapse into one \"+N more\" row that opens the list. Each new banner of warn level or higher is announced once. Here: mic silent and alarm volume off, plus baby battery low and Vibrate only folded. When the Vibrate-only banner is folded, the alert chip returns to its labelled form, so the no-sound indicator stays visible (parent-alerts).",
  monitor("t-dusk", "23:58", top_extra=
          f'<div class="banner warn">{I("micoff")}<div class="grow">Nursery\'s microphone hears nothing<small>Check the baby device. Another app may be using it.</small></div><span class="act">Help</span></div>'
          f'<div class="banner alert">{I("volx")}<div class="grow">Alarm volume is off<small>You won\'t hear alerts on this phone</small></div><span class="act">Fix</span></div>'
          f'<div class="morelink" role="button" aria-label="2 more notices: baby battery low, Vibrate only. Opens the list.">+2 more · baby battery low, Vibrate only{I("chev","s")}</div>',
          h="Can't hear the room", p="No sound reaches the microphone. That is not the same as quiet.", hzk=None, info=[], alert_="Vibrate only", icon_only=False),
  tag="NEW", section=S8)

frame("49", "Ember · warning banner keeps safety colours", "With night mode on, every warning banner, the grey/red bar states and safety chips use the fixed Dusk safety palette, not the red-tinted night colours, so the only signal of a monitor that cannot work never blends in.",
  monitor("t-ember", "02:40", top_extra=f'<div class="banner warn">{I("micoff")}<div class="grow">Nursery\'s microphone hears nothing<small>Check the baby device. Another app may be using it.</small></div><span class="act">Help</span></div>',
          h="Can't hear the room", p="No sound reaches the microphone.", hzk="off", hzmsg=f'{I("micoff","s")}Microphone silent', info=[], night="on", barhtml=bar_calm(meta="79%")).replace('class="hz off"','class="hz off hatch"'),
  tag="NEW", section=S8)

frame("50", "Baby unit · 20–60 s, dashed edge", "Middle state drawn: a <b>dashed</b> line along the whole top edge (6 dp, 16 dp dashes, 10 dp gaps) versus the short solid bar (44 × 6 dp, centred) when fine and the full 5 dp frame with slow pulse from 1 min. Distinguishable by shape at arm's length without hue. Luminance cap: at minimum brightness no mark emits more than #5A1414 does at normal brightness (critical review).",
  phone("t-kiosk", f'<div class="kmark-dash"></div><div class="k-wrap"></div>{kstatus("linkoff","Can\'t reach Anna\'s phone · still trying")}<div style="text-align:center;font-size:14px;color:var(--on-v);margin:-34px 0 40px">Touch for details</div>'),
  tag="NEW", section=S8)

frame("51", "Baby unit · needs attention (battery or charger)", "A fourth mark shape: short bar plus a separate 10 dp square, and the words \"Needs attention\" in the status line (\"needs a hand\"), and a second status line that says what to do. Never sound or light beyond the kiosk level.",
  phone("t-kiosk", f'<div style="position:absolute;top:22px;left:50%;transform:translateX(-50%);display:flex;gap:8px"><span style="width:44px;height:6px;border-radius:3px;background:var(--k-warn)"></span><span style="width:10px;height:10px;border-radius:2px;background:var(--k-warn);margin-top:-2px"></span></div>'
        f'<div class="k-wrap"></div>{kstatus("link","Listening · Travel cot")}<div class="k-status" style="margin-top:-30px">{I("battlow","s")}Needs attention: not charging · 12%</div>'),
  tag="NEW", section=S8)

frame("52", "Baby unit · set-up confirmation (first 60 s)", "For the first 60 s after Start, and on every touch-wake, a larger, clearer confirmation at wake brightness (about 10 %): the parent connection name and that it is listening (\"Connected to the parent device\" if no name was given). The dim countdown is not a live region. Then it dims back to frame 15.",
  phone("t-kiosk", f'<div class="k-confirm"><span style="color:var(--k-ok)">{I("checkc","xl")}</span><b style="margin-top:16px">Connected to Anna\'s phone</b><p style="font-size:22px;color:#e2dcd6;margin-top:4px">Listening · Travel cot</p><p>This screen dims in 45 s. Touch it any time to see this again.</p></div><div style="height:60px"></div>'),
  tag="NEW", section=S8)

frame("53", "Settings › Open at start", "The single home of the default: one list across both roles, so only one connection can be chosen. This is per connection, beyond the role-based start mode of <code>alternate-roles-and-apk-size</code> (delta), and it never starts monitoring by itself.",
  phone("t-dusk", f"""{sysb("19:22")}
  <div class="topbar" style="padding-top:8px"><span class="ibtn">{I("back")}</span><span class="title">Open at start</span></div>
  <div class="scroll"><div class="note" style="padding:2px 8px 10px">What GuardedSleep shows when you open it.</div>
  <div class="list"><div class="li sel"><span class="radio on"></span><div class="tx"><b>Ask every time</b><small>Shows the start screen</small></div></div>
  <div class="li"><span class="radio"></span><div class="tx"><b>Nursery</b><small>This device as parent device</small></div></div>
  <div class="li"><span class="radio"></span><div class="tx"><b>Grandma's flat</b><small>This device as parent device</small></div></div>
  <div class="li"><span class="radio"></span><div class="tx"><b>Travel cot</b><small>This device as baby device</small></div></div></div>
  <div class="note">The chosen connection opens ready to start. Monitoring still starts only when you tap Start.</div></div>"""),
  tag="NEW", section=S8)

frame("54", "Day · More sheet", "Segmented controls, close button and scrim checked in Day.", moresheet("t-day"), tag="NEW", section=S8)

# ================= PAGE =================
def tokens():
    def col(cls, title, sub, sw, extra=""):
        rows = "".join(f'<div class="tk-sw"><i style="background:{c}"></i><b>{n}</b><code>{v}</code></div>' for c, n, v in sw)
        chips = "".join(f'<span class="tk-chip" style="background:var(--{k}-c);color:var(--{k})">{I(ic)}{t}</span>' for k, ic, t in
                        [("info","refresh","Connecting"),("warn","linkoff","No connection"),("alert","warn","Needs you"),("caution","pausec","Caution")])
        calm = f'<span class="tk-chip" style="background:var(--surf-c);color:var(--on)"><span style="width:8px;height:8px;border-radius:9px;background:var(--ok)"></span>Connected</span>'
        return f'<div class="tk-col {cls}" style="background:var(--bg);color:var(--on)"><h3>{title}</h3><div class="tk-sub">{sub}</div>{rows}<div class="tk-safe"><div class="tk-lbl">Status roles</div><div class="tk-chips">{calm}{chips}</div></div>{extra}</div>'
    d = col("t-dusk","Dusk · dark, default","Warm plum-black, low chroma. Gold accent removed: no non-safety element sits within ±20° of a safety hue. Brand apricot only in the logo.",
        [("#141218","background","#141218"),("#1a1820","surfaceContainerLow · dock, sheet","#1A1820"),("#211e27","surfaceContainer · cards, calm bar","#211E27"),("#ece6ef","onSurface · 15.4:1","#ECE6EF"),("#beb6c4","onSurfaceVariant · 9.4:1","#BEB6C4"),("#c9bceb","primary / onPrimary","#C9BCEB / #2A2148"),("#3a3354","primaryContainer / on","#3A3354 / #E7E0FA"),("#c9bceb","horizon line (motif)","primary 45–75 %"),("#d9b49a","brand · logo only","#D9B49A")])
    y = col("t-day","Day · light, opt-in","Cards white with a 1 px #E0D8DF border on #FAF7F4 (surface contrast alone is 1.08:1).",
        [("#faf7f4","background","#FAF7F4"),("#f3eeee","surfaceContainerLow","#F3EEEE"),("#ffffff","surfaceContainer + border","#FFFFFF / #E0D8DF"),("#1f1b23","onSurface · 16:1","#1F1B23"),("#4f4756","onSurfaceVariant · 8.6:1","#4F4756"),("#5a4893","primary / onPrimary · 7.6:1","#5A4893 / #FFF"),("#e6def8","primaryContainer / on","#E6DEF8 / #21184A"),("#8a5a3c","brand · logo only","#8A5A3C")])
    e = col("t-ember","Ember · night mode, off by default","Off · On · Automatic. Contrast held (variant 5.0:1 on cards, outline 3.6:1); dimness comes from the backlight. From the orange stage the status bar uses the fixed Dusk safety colours.",
        [("#000","background","#000000"),("#110807","surfaceContainer","#110807"),("#c98a7a","onSurface · 7.4:1","#C98A7A"),("#a87365","onSurfaceVariant · 5.0:1 on cards","#A87365"),("#9a5e50","outline · 3.6:1","#9A5E50"),("#d08a6c","primary / onPrimary","#D08A6C / #1A0703"),("#2e140d","primaryContainer / on","#2E140D / #E8B19A")])
    fixed = """<div class="tk-box"><h3>Fixed safety palettes (outside ColorScheme)</h3><div class="tk-fixed">
<div style="background:#250a10;color:#fff0ee"><b>Noise alert</b>bg #250A10 · slab #7A1E30, text #FFEAE7 (7.8:1), border #FF948A</div>
<div style="background:#2a0709;color:#fff0ee"><b>No connection</b>bg #2A0709 · slab #8C1D24, text #FFE9E6 (7.8:1), border #FF948A</div>
<div style="background:#3a080b;color:#fff3f1"><b>Loud alarm</b>bg #3A080B + hazard band · slab #A3222B, border #FFB4AB</div>
<div style="background:#3e2719;color:#f0a774;box-shadow:inset 0 0 0 1.5px #f0a774"><b>Orange stage bar</b>fixed in Dusk, Day uses #FBE3D2/#9A4510; Ember borrows Dusk</div>
<div style="background:#000;color:#8a837e;border:1px solid #3a3633"><b>Baby unit</b>status text #7D7671 (4.7:1) · marks ok #5A8F72, trying #A8794E, lost #B0605A, all ≥ 3:1, plus shape</div>
</div><p>Slabs are dimmed but saturated: they no longer light the room. The status strip on red screens is flat (no fill), so the slab is the only filled control.</p></div>"""
    ty = """<div class="tk-box"><h3>Type · Plus Jakarta Sans, tnum on</h3>
<div class="tk-ty"><span style="font-size:52px;font-weight:600">1:12</span><small>Display 52/52 600 · big counters only</small></div>
<div class="tk-ty"><span style="font-size:28px;font-weight:700">No connection</span><small>Headline 28/34 700</small></div>
<div class="tk-ty"><span style="font-size:20px;font-weight:700">Monitor options</span><small>Title 20/26 700</small></div>
<div class="tk-ty"><span style="font-size:16px;font-weight:600">Connected · Nursery</span><small>Title M 16/22 600</small></div>
<div class="tk-ty"><span style="font-size:15px">You may not hear the child right now.</span><small>Body 15/22 · instructions</small></div>
<div class="tk-ty"><span style="font-size:14px">Last test 21:30 · reached this phone</span><small>Body S 14/20 · floor for sentences</small></div>
<div class="tk-ty"><span style="font-size:12px;font-weight:700;letter-spacing:.09em;text-transform:uppercase">Monitoring</span><small>Label 12/16 caps only</small></div>
<p>Sizes collapsed to 12 / 14 / 15 / 16 / 20 / 28 / 52. Durations in running text are words ("1 min 12 s"); m:ss only for big counters, with an aria label in words.</p></div>"""
    sh = """<div class="tk-box"><h3>Shape · motif · targets</h3><div class="tk-rad"><div style="border-radius:8px">8</div><div style="border-radius:12px">12</div><div style="border-radius:16px">16</div><div style="border-radius:24px">24</div><div style="border-radius:32px">32</div><div style="border-radius:999px">full</div></div>
<p>Owned motif: <b>the horizon line</b>, used in three places only: the logo (crescent above a line), the hairline under the status bar and Home header, and the horizon strip on the monitor (the alert line sits above it). The crescent is brand only; night mode uses a moon-with-stars glyph, Quiet mode a bell-off glyph.</p>
<p>Targets: 48 dp minimum everywhere (banner actions, play buttons, chips, links), dock 66, Listen 80, alert slabs 88+. Night-time actions in the bottom 40 %.</p></div>"""
    return f'<div class="tk-grid">{d}{y}{e}</div><div class="tk-scales">{fixed}{ty}{sh}</div>'

def build():
    out = []
    cur = None
    for section, num, title, cap, body, tag in FRAMES:
        if section != cur:
            if cur is not None:
                out.append("</div>")
            out.append(f'<h2 class="pg-sec">{section}</h2>')
            if section == S2:
                out.append('<p class="pg-note">Colour and feedback follow the core level only (0 s / 20 s / 60 s / 3 min, counted from the last contact, or from start if never connected). Blue only for the first 20 s. The 20–60 s stage is light: bar plus one line. The vertical step list appears only on full-screen stages and names what the phone does.</p>'
                           '<div class="pg-tl"><div style="background:#1e2a38;color:#a9c6ea"><b>09 / 10 · Connecting / Reconnecting</b><span>0–20 s · blue, bar only</span></div>'
                           '<div style="background:#3e2719;color:#f0a774"><b>11 / 24 · No connection (still trying)</b><span>20–60 s · orange bar + one line</span></div>'
                           '<div style="background:#2a0709;color:#ff948a"><b>12 / 25 · Vibrates + alert screen</b><span>from 1 min · full screen, soft tone every 10 s</span></div>'
                           '<div style="background:#3a080b;color:#ffa198"><b>13 · Loud alarm</b><span>from 3 min, or 5 min after "I\'m checking" (26)</span></div>'
                           '<div style="background:#1b2f26;color:#86cfa6"><b>14 · Connected again</b><span>card stays until OK</span></div></div>')
            out.append('<div class="pg-frames">')
            cur = section
        t = f"<em>{tag}</em>" if tag else ""
        out.append(f'<div class="pg-frame">{body}<div class="pg-cap"><h3><small>{num}</small>{title}{t}</h3><p>{cap}</p></div></div>')
    out.append("</div>")
    return "\n".join(out)

html = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>GuardedSleep — Combined UI</title>
<style>{CSS}</style></head><body>
{sprite}
<div class="pg-wrap">
<header class="pg-head"><h1>GuardedSleep — <span>Combined UI</span></h1>
<p>Round 2. Simplified red screens (one sentence with the next step, compact cause card with folded steps, Stop monitoring as a quiet text button), tool row that keeps its gutter in German, each state said once, banner order and limit, safety colours inside night mode, 200 % evidence, baby dashed / needs-attention / set-up states. Round 1 summary: Base C, calmed with A, status bar and steps from B. This round: neutral connected bar, horizon strip instead of the ring, one shared alert template with pinned actions and dimmed slabs, tap-to-listen, "Vibrate only" and "Pause noise alerts", no switching while monitoring, light 20–60 s stage, failure frames (first connect failed, after "I'm checking", microphone silent, readiness, stopped, needs you, battery), talk-back states, browser frames, Day variants, a readable baby screen. Frames 24–44 were added in round 1, 45–54 in round 2; 01–23 keep their numbers.</p>
<div class="pg-tags"><span class="pg-tag">Compose M3 + Svelte/CSS</span><span class="pg-tag">Dusk · Day · Ember</span><span class="pg-tag">Fixed safety palettes</span><span class="pg-tag">Neutral person: "the child"</span><span class="pg-tag">Generated by gen.py</span></div></header>
<h2 class="pg-sec">Tokens</h2>
{tokens()}
{build()}
<h2 class="pg-sec">Copy · every status and alert string</h2>
<p class="pg-note">English master and German (Sie). Neutral person ("the child" / "das Kind"). German never splices the connection name into a sentence: it uses the label pattern "Zustand · Name" (e.g. "Verbunden · Kinderzimmer"). Running-text durations in words; m:ss only in big counters. ★ = new key.</p>
{TABLE}
</div></body></html>"""
html = re.sub(r"(\d) (s|min|h|%)(?=[\s.,;:)<·])", "\\1&nbsp;\\2", html)
# Repository hygiene: no trailing whitespace, exactly one final newline.
html = "\n".join(l.rstrip() for l in html.splitlines()).rstrip("\n") + "\n"
open(os.path.join(HERE, "mockup.html"), "w").write(html)
print("frames:", len(FRAMES))
