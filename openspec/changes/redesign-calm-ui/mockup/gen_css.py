CSS = r"""
/* ================= Page chrome (not part of the design) ================= */
:root{
  --pg:#0e0e11; --pg2:#17171c; --pg-line:#2a2a31; --pg-ink:#ececf1; --pg-dim:#a4a4ae; --pg-acc:#c9bceb;
  --font:"Plus Jakarta Sans",system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;
}
*{box-sizing:border-box}
html,body{margin:0;background:var(--pg);color:var(--pg-ink);font-family:var(--font);-webkit-font-smoothing:antialiased}
body{padding:40px 16px 96px}
.pg-wrap{max-width:1680px;margin:0 auto}
.pg-head h1{font-size:40px;line-height:1.1;margin:0;font-weight:800;letter-spacing:-.02em}
.pg-head h1 span{color:var(--pg-acc)}
.pg-head p{margin:10px 0 0;color:var(--pg-dim);max-width:960px;font-size:15px;line-height:1.55}
.pg-tags{display:flex;gap:8px;flex-wrap:wrap;margin-top:14px}
.pg-tag{font-size:12px;font-weight:600;padding:6px 12px;border-radius:999px;border:1px solid var(--pg-line);color:var(--pg-dim)}
h2.pg-sec{font-size:13px;letter-spacing:.14em;text-transform:uppercase;color:var(--pg-dim);margin:56px 0 16px;font-weight:700}
p.pg-note{margin:-6px 0 18px;color:var(--pg-dim);font-size:14px;line-height:1.55;max-width:1000px}
.tk-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:16px}
.tk-col{border-radius:22px;padding:18px;border:1px solid var(--pg-line)}
.tk-col h3{margin:0 0 2px;font-size:17px}
.tk-col .tk-sub{font-size:12px;opacity:.8;margin-bottom:12px;line-height:1.45}
.tk-sw{display:grid;grid-template-columns:34px 1fr auto;gap:10px;align-items:center;padding:4px 0;font-size:12px}
.tk-sw i{width:34px;height:26px;border-radius:9px;display:block;border:1px solid rgba(127,127,127,.28)}
.tk-sw b{font-weight:600}
.tk-sw code{font-family:ui-monospace,Menlo,monospace;font-size:11px;opacity:.85}
.tk-safe{margin-top:12px;padding-top:12px;border-top:1px dashed rgba(127,127,127,.35)}
.tk-lbl{font-size:11px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;opacity:.75;margin-bottom:8px}
.tk-chips{display:flex;flex-wrap:wrap;gap:6px}
.tk-chip{display:inline-flex;align-items:center;gap:6px;font-size:12px;font-weight:600;padding:6px 10px;border-radius:10px}
.tk-chip svg{width:14px;height:14px}
.tk-scales{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:16px;margin-top:16px}
.tk-box{background:var(--pg2);border:1px solid var(--pg-line);border-radius:22px;padding:18px}
.tk-box h3{margin:0 0 12px;font-size:15px}
.tk-box p{font-size:12px;color:var(--pg-dim);margin:10px 0 0;line-height:1.5}
.tk-ty{display:flex;align-items:baseline;justify-content:space-between;gap:12px;border-bottom:1px solid var(--pg-line);padding:6px 0}
.tk-ty small{font-size:11px;color:var(--pg-dim);white-space:nowrap}
.tk-rad{display:flex;gap:10px;flex-wrap:wrap;align-items:flex-end}
.tk-rad div{width:54px;height:54px;background:#3a3354;display:flex;align-items:flex-end;justify-content:center;font-size:10px;padding-bottom:4px;color:#e7e0fa}
.tk-fixed{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:10px}
.tk-fixed div{border-radius:14px;padding:12px;font-size:12px;line-height:1.45}
.tk-fixed b{display:block;font-size:13px;margin-bottom:2px}
.pg-frames{display:grid;grid-template-columns:repeat(auto-fill,minmax(404px,1fr));gap:56px 24px;justify-items:center}
.pg-frame{display:flex;flex-direction:column;align-items:center;gap:16px;max-width:404px}
.pg-cap{width:380px;max-width:100%}
.pg-cap h3{margin:0 0 4px;font-size:16px;line-height:1.35}
.pg-cap h3 small{font-weight:700;color:var(--pg-acc);margin-right:6px;font-variant-numeric:tabular-nums}
.pg-cap h3 em{font-style:normal;font-size:11px;font-weight:700;letter-spacing:.06em;color:#0e0e11;background:var(--pg-acc);padding:2px 6px;border-radius:5px;margin-left:6px;vertical-align:2px}
.pg-cap p{margin:0;font-size:13px;line-height:1.55;color:var(--pg-dim)}
.pg-cap code{font-size:11.5px;color:#d7d0e8}
.pg-tl{display:grid;grid-template-columns:repeat(5,1fr);gap:6px;margin:0 0 28px;font-size:12.5px}
.pg-tl div{border-radius:12px;padding:10px 12px;line-height:1.4}
.pg-tl b{display:block;font-size:13px}
.pg-tl span{opacity:.85}
@media (max-width:900px){.pg-tl{grid-template-columns:1fr 1fr}}
.pg-tablewrap{overflow-x:auto;border:1px solid var(--pg-line);border-radius:18px}
table.pg-copy{border-collapse:collapse;width:100%;min-width:1280px;table-layout:fixed;font-size:13px;line-height:1.45}
.pg-copy th,.pg-copy td{padding:10px 12px;text-align:left;vertical-align:top;border-bottom:1px solid var(--pg-line)}
.pg-copy th{background:var(--pg2);font-size:11.5px;letter-spacing:.06em;text-transform:uppercase;color:var(--pg-dim)}
.pg-copy td:first-child{font-weight:700}
.pg-copy td.t{color:var(--pg-dim)}
.pg-copy tr.grp td{background:#131317;color:var(--pg-acc);font-size:11.5px;letter-spacing:.1em;text-transform:uppercase;font-weight:700}
.pg-copy .de{color:#d4d0e4}
.pg-copy td div + div{margin-top:4px}
.dot{display:inline-block;width:10px;height:10px;border-radius:3px;margin-right:6px;vertical-align:-1px}
@media (max-width:440px){ .phone{zoom:.84} .pg-cap{width:320px} .pg-frames{grid-template-columns:1fr} }

/* ================= Design tokens per scheme ================= */
.t-dusk{
  --bg:#141218; --surf-lo:#1a1820; --surf-c:#211e27; --surf-hi:#29262f; --surf-hst:#322e39;
  --on:#ece6ef; --on-v:#beb6c4; --outline:#8b8393; --outline-v:#3b3642;
  --pri:#c9bceb; --on-pri:#2a2148; --pri-c:#3a3354; --on-pri-c:#e7e0fa;
  --sec-c:#3b3446; --on-sec-c:#e4dcef; --brand:#d9b49a; --horizon:#c9bceb;
  --ok:#86cfa6; --ok-c:#1b2f26; --info:#a9c6ea; --info-c:#1e2a38;
  --warn:#f0a774; --warn-c:#3e2719; --alert:#ff948a; --alert-c:#4a1b1c; --caution:#e3c978; --caution-c:#363019;
  --m-lo:#4d4660; --m-mid:#8f84b3; --m-top:#c9bceb; --scrim:rgba(8,6,12,.62); --snack:#e7e0fa; --on-snack:#211e27;
}
.t-day{
  --bg:#faf7f4; --surf-lo:#f3eeee; --surf-c:#ffffff; --surf-hi:#efe9ec; --surf-hst:#e5dee4;
  --on:#1f1b23; --on-v:#4f4756; --outline:#7f7588; --outline-v:#ddd3dd;
  --pri:#5a4893; --on-pri:#ffffff; --pri-c:#e6def8; --on-pri-c:#21184a;
  --sec-c:#ece6f3; --on-sec-c:#2e2540; --brand:#8a5a3c; --horizon:#5a4893;
  --ok:#1c6b45; --ok-c:#dcefe3; --info:#275d92; --info-c:#dde8f5;
  --warn:#9a4510; --warn-c:#fbe3d2; --alert:#b3261e; --alert-c:#fbddd9; --caution:#6e5600; --caution-c:#f6ecc4;
  --m-lo:#d8d0ea; --m-mid:#8a7cc0; --m-top:#5a4893; --scrim:rgba(31,27,35,.32); --snack:#2f2a36; --on-snack:#f3eef6;
}
.t-ember{ /* Night mode: red-shifted, ultra-dim; contrast held, brightness lowered by the backlight */
  --bg:#000000; --surf-lo:#0a0505; --surf-c:#110807; --surf-hi:#180b0a; --surf-hst:#200f0d;
  --on:#c98a7a; --on-v:#a87365; --outline:#9a5e50; --outline-v:#2e1714;
  --pri:#d08a6c; --on-pri:#1a0703; --pri-c:#2e140d; --on-pri-c:#e8b19a;
  --sec-c:#22100c; --on-sec-c:#dda08e; --brand:#b98a6c; --horizon:#a8604c;
  --ok:#7aa883; --ok-c:#0a140d; --info:#8a9cb8; --info-c:#0c0f14;
  --warn:#d07a44; --warn-c:#1a0c05; --alert:#e06a5e; --alert-c:#230807; --caution:#bf9c46; --caution-c:#151004;
  --m-lo:#2e1510; --m-mid:#8f4c3a; --m-top:#a8604c; --scrim:rgba(0,0,0,.7); --snack:#2e140d; --on-snack:#e8b19a;
}
/* Fixed safety palettes: never themed, never Ember, never dynamic colour */
.t-noise{ --bg:#250a10; --surf-lo:#2e0d15; --surf-c:#35111a; --surf-hi:#451722; --on:#fff0ee; --on-v:#f2c6c0; --outline:#c98880; --outline-v:#6a2a31;
  --alert:#ff948a; --slab:#7a1e30; --slab-on:#ffeae7; --slab-line:#ff948a; }
.t-lost{ --bg:#2a0709; --surf-lo:#330a0d; --surf-c:#3b0b0e; --surf-hi:#4d0f13; --on:#fff0ee; --on-v:#f7c6c0; --outline:#d68a82; --outline-v:#7a2a2e;
  --alert:#ff948a; --slab:#8c1d24; --slab-on:#ffe9e6; --slab-line:#ff948a; }
.t-alarm{ --bg:#3a080b; --surf-lo:#440a0e; --surf-c:#4d0c10; --surf-hi:#5e1116; --on:#fff3f1; --on-v:#ffcfc9; --outline:#e0968e; --outline-v:#86303a;
  --alert:#ffa198; --slab:#951e27; --slab-on:#fff1ef; --slab-line:#ffb4ab; }
.t-kiosk{ --bg:#000; --on:#8a837e; --on-v:#7d7671; --dim:#3a3633; --k-ok:#5a8f72; --k-warn:#a8794e; --k-alert:#b0605a; --k-talk:#6f9690; }
.t-web{ --chrome:#202024; --chrome-on:#c9c6cf; }

/* ================= Phone shell ================= */
.phone{width:380px;height:800px;border-radius:46px;position:relative;overflow:hidden;flex:none;
  background:var(--bg);color:var(--on);font-family:var(--font);
  box-shadow:0 0 0 9px #050507,0 0 0 10px #2b2b33,0 30px 60px -20px rgba(0,0,0,.7);
  font-variant-numeric:tabular-nums}
.screen{position:absolute;inset:0;display:flex;flex-direction:column}
.sys{height:40px;flex:none;display:flex;align-items:center;justify-content:space-between;padding:6px 28px 0;font-size:13px;font-weight:700;color:var(--on)}
.sys .r{display:flex;gap:6px;align-items:center;opacity:.85}
.gbar{position:absolute;left:50%;bottom:8px;width:120px;height:4px;border-radius:2px;background:var(--on);opacity:.3;transform:translateX(-50%)}
svg.i{width:24px;height:24px;fill:none;stroke:currentColor;stroke-width:1.8;stroke-linecap:round;stroke-linejoin:round;flex:none}
svg.i.s{width:20px;height:20px}
svg.i.xs{width:16px;height:16px}
svg.i.l{width:30px;height:30px}
svg.i.xl{width:40px;height:40px}
.webbar{height:44px;flex:none;display:flex;align-items:center;gap:10px;padding:0 14px;background:var(--chrome);color:var(--chrome-on);font-size:13px;font-weight:600}
.webbar .url{flex:1;height:30px;border-radius:999px;background:rgba(255,255,255,.07);display:flex;align-items:center;gap:6px;padding:0 12px;font-weight:500}

/* ---------- Status bar: always on top of every parent screen while monitoring ---------- */
.sbar{margin:4px 12px 0;border-radius:16px;min-height:52px;padding:7px 14px;display:flex;align-items:center;gap:11px;font-size:15px;font-weight:700;flex:none;position:relative}
.sbar .grow{flex:1;min-width:0;line-height:1.25}
.sbar .grow span{font-weight:500;opacity:.9}
.sbar small{display:block;font-weight:500;font-size:13.5px;opacity:.92;margin-top:1px}
.sbar .meta{display:flex;align-items:center;gap:4px;font-size:14px;font-weight:600;white-space:nowrap}
.sbar.calm{background:var(--surf-c);color:var(--on);font-weight:600}
.sbar.calm .okdot{width:10px;height:10px;border-radius:99px;background:var(--ok);flex:none}
.sbar.calm .ic{color:var(--ok)}
.sbar.calm .meta{color:var(--on-v)}
.sbar.info{background:var(--info-c);color:var(--info)}
.sbar.warn{background:var(--warn-c);color:var(--warn);box-shadow:inset 0 0 0 2px var(--warn)}
.sbar.warn-fixed{background:#3e2719;color:#f0a774;box-shadow:inset 0 0 0 2px #f0a774}
.sbar.grey{background:var(--surf-hst);color:var(--on)}
.sbar.needs{background:var(--alert-c);color:var(--alert)}
/* on full-screen alerts: a flat information strip, never a pill that looks tappable */
.sbar.flat{margin:0;border-radius:0;background:transparent;color:var(--on);border-bottom:1px solid var(--outline-v);padding:8px 22px;min-height:48px;font-weight:600}
.sbar.flat .ic{color:var(--alert)}
.sbar.flat.accent{border-left:5px solid var(--alert);padding-left:17px}
.hline{height:1px;margin:0 12px;background:linear-gradient(90deg,transparent,color-mix(in srgb,var(--horizon) 45%,transparent),transparent);flex:none}

/* ---------- Generic components ---------- */
.tools{display:flex;align-items:center;gap:8px;padding:10px 14px 0 16px;flex:none}
.tools .sp{flex:1}
.ibtn{width:48px;height:48px;border-radius:999px;display:grid;place-items:center;color:var(--on-v);flex:none}
.ibtn.t{background:var(--surf-c)}
.chip{display:inline-flex;align-items:center;gap:6px;height:48px;padding:0 14px;border-radius:16px;font-size:14px;font-weight:600;background:var(--surf-c);color:var(--on);white-space:nowrap}
.chip.on{background:var(--pri-c);color:var(--on-pri-c)}
.chip b{font-weight:700}
.banner{margin:8px 12px 0;border-radius:18px;padding:10px 10px 10px 14px;display:flex;align-items:center;gap:12px;font-size:15px;font-weight:700;min-height:62px;flex:none}
.banner .grow{flex:1;line-height:1.3}
.banner small{display:block;font-weight:500;font-size:13.5px;opacity:.92;margin-top:2px}
.banner .act{font-size:14px;font-weight:700;padding:0 14px;height:48px;display:flex;align-items:center;border-radius:999px;border:1.5px solid currentColor;white-space:nowrap}
.banner.vib{background:var(--sec-c);color:var(--on-sec-c)}
.banner.pause{background:var(--surf-hst);color:var(--on)}
.banner.warn{background:var(--warn-c);color:var(--warn)}
.banner.alert{background:var(--alert-c);color:var(--alert)}
.banner.info{background:var(--info-c);color:var(--info)}
.label{font-size:12px;font-weight:700;letter-spacing:.09em;text-transform:uppercase;color:var(--pri)}
.btn{display:flex;align-items:center;justify-content:center;gap:10px;border-radius:999px;font-weight:700;font-size:15.5px;min-height:52px;padding:0 20px;text-align:center}
.btn.fill{background:var(--pri);color:var(--on-pri)}
.btn.ton{background:var(--pri-c);color:var(--on-pri-c)}
.btn.out{border:1.5px solid var(--outline);color:var(--on)}
.btn.txt{color:var(--pri);min-height:48px}
.btn.danger{color:var(--alert);border:1.5px solid var(--outline)}
.btn.sm{min-height:48px;font-size:14.5px;padding:0 16px}

/* hero line + horizon strip (the owned motif: a horizon line with the alert line above it) */
.hero{padding:16px 22px 0;flex:none}
.hero h2{margin:0;font-size:26px;line-height:1.2;font-weight:700;letter-spacing:-.01em}
.hero p{margin:4px 0 0;font-size:15px;color:var(--on-v);line-height:1.45}
.hz{margin:14px 12px 0;height:204px;border-radius:24px;background:var(--surf-c);position:relative;overflow:hidden;flex:none}
.hz .bars{position:absolute;left:16px;right:16px;bottom:40px;height:124px;display:flex;align-items:flex-end;gap:2px}
.hz .bars i{flex:1;border-radius:3px 3px 0 0;background:var(--m-lo);min-height:3px}
.hz .bars i.r{background:var(--m-top)}
.hz .bars i.up{background:var(--m-top)}
.hz .base{position:absolute;left:0;right:0;bottom:39px;height:2px;background:linear-gradient(90deg,transparent,var(--horizon),transparent);opacity:.75}
.hz .thr{position:absolute;left:16px;right:16px;border-top:2px dashed var(--on-v);opacity:.85}
.hz .thr b{position:absolute;right:0;top:-12px;font-size:13.5px;font-weight:600;color:var(--on-v);background:var(--surf-c);padding:1px 6px;border-radius:6px}
.hz .cap{position:absolute;left:16px;right:16px;bottom:10px;display:flex;justify-content:space-between;font-size:14px;color:var(--on-v);font-weight:500}
.hz .msg{position:absolute;inset:0 0 40px;display:flex;align-items:center;justify-content:center;gap:8px;font-size:15px;font-weight:600;color:var(--on-v)}
.hz.off .bars i{background:var(--surf-hst)}
.hz.hatch{background:repeating-linear-gradient(-45deg,var(--surf-c) 0 10px,var(--surf-hi) 10px 20px)}
.info-line{display:flex;flex-wrap:wrap;gap:8px;padding:12px 14px 0}
.chip.flat{height:40px;font-size:13.5px;font-weight:600;background:transparent;border:1px solid var(--outline-v);color:var(--on-v);border-radius:12px;padding:0 12px}

/* dock */
.dock{margin-top:auto;background:var(--surf-lo);border-radius:32px 32px 0 0;padding:12px 16px 28px;border-top:1px solid var(--outline-v);flex:none;position:relative}
.listen{min-height:80px;border-radius:28px;background:var(--pri-c);color:var(--on-pri-c);display:flex;align-items:center;gap:14px;padding:0 20px}
.listen .tx{flex:1}
.listen b{display:block;font-size:19px}
.listen small{font-size:14px;opacity:.88;font-weight:500}
.listen .knob{width:52px;height:52px;border-radius:999px;background:var(--pri);color:var(--on-pri);display:grid;place-items:center;flex:none}
.listen.on{background:var(--pri);color:var(--on-pri)}
.listen.on .knob{background:var(--on-pri);color:var(--pri)}
.listen.off{background:var(--surf-hi);color:var(--on-v)}
.listen.off .knob{background:var(--surf-hst);color:var(--on-v)}
.row3{display:grid;grid-template-columns:1.2fr 1fr 1fr;gap:8px;margin-top:8px;grid-auto-rows:minmax(72px,auto)}
.qbtn{min-height:66px;border-radius:22px;background:var(--surf-hi);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:3px;font-size:13px;font-weight:700;color:var(--on);text-align:center;line-height:1.2;padding:6px 4px}
.qbtn.act{background:var(--pri-c);color:var(--on-pri-c)}
.qbtn.dis{color:var(--on-v);background:transparent;border:1.5px dashed var(--outline-v)}
.qbtn.stat{background:transparent;color:var(--on);border:1.5px solid var(--outline-v)}
.qbtn.talk-on{background:var(--pri);color:var(--on-pri)}
.snack{position:absolute;left:12px;right:12px;bottom:calc(100% + 8px);min-height:52px;border-radius:14px;background:var(--snack);color:var(--on-snack);display:flex;align-items:center;gap:10px;padding:0 6px 0 16px;font-size:14.5px;font-weight:600;box-shadow:0 8px 24px rgba(0,0,0,.35)}
.snack .grow{flex:1}
.snack .act{height:48px;display:flex;align-items:center;padding:0 14px;font-weight:800;color:var(--on-snack);text-decoration:underline;text-underline-offset:3px}
.talkpanel{border-radius:28px;background:var(--pri);color:var(--on-pri);padding:16px 18px;display:flex;align-items:center;gap:14px;min-height:96px}
.talkpanel b{display:block;font-size:20px}
.talkpanel small{font-size:14px;font-weight:600;opacity:.9}
.talkpanel .tm{font-size:30px;font-weight:700;margin-left:auto}
.progress{height:6px;border-radius:3px;background:color-mix(in srgb,var(--on-pri) 25%,transparent);margin-top:8px;position:relative;overflow:hidden}
.progress i{position:absolute;left:0;top:0;bottom:0;background:var(--on-pri);border-radius:3px}

/* lists / settings */
.topbar{display:flex;align-items:center;gap:6px;padding:6px 12px 2px 6px;flex:none}
.topbar .title{flex:1;font-size:20px;font-weight:700}
.scroll{flex:1;overflow:hidden;padding:0 12px;position:relative}
.fadeb{position:absolute;left:0;right:0;bottom:0;height:60px;background:linear-gradient(transparent,var(--bg));pointer-events:none}
.gh{font-size:13.5px;font-weight:700;color:var(--on-v);margin:18px 8px 8px;letter-spacing:.02em;display:flex;justify-content:space-between;gap:8px}
.gh span{font-weight:500}
.list{background:var(--surf-c);border-radius:24px;overflow:hidden}
.t-day .list,.t-day .hz,.t-day .sbar.calm,.t-day .card{box-shadow:0 0 0 1px #e0d8df}
.li{display:flex;align-items:center;gap:14px;padding:12px 16px;min-height:62px}
.li + .li{border-top:1px solid var(--outline-v)}
.li .tx{flex:1;min-width:0}
.li b{display:block;font-size:15.5px;font-weight:600;line-height:1.3}
.li small{display:block;font-size:14px;color:var(--on-v);margin-top:2px;line-height:1.4}
.li .ic{width:38px;height:38px;border-radius:13px;display:grid;place-items:center;background:var(--surf-hi);color:var(--pri);flex:none}
.li .val{font-size:14px;color:var(--on-v);font-weight:500;text-align:right}
.li .play{min-width:48px;height:48px;border-radius:999px;display:flex;align-items:center;justify-content:center;gap:6px;padding:0 12px;background:var(--surf-hi);color:var(--pri);flex:none;font-size:13.5px;font-weight:700}
.li .play.on{background:var(--pri);color:var(--on-pri)}
.li.sel{background:var(--surf-hi)}
.li.sub{padding-left:24px}
.li.ro{}
.pill{display:inline-flex;align-items:center;gap:4px;font-size:11px;font-weight:600;height:20px;padding:0 7px;border-radius:6px;background:var(--surf-hst);color:var(--on-v);vertical-align:1px}
.pill.ok{background:var(--ok-c);color:var(--ok)}
.pill.new{background:var(--pri-c);color:var(--on-pri-c)}
.radio{width:22px;height:22px;border-radius:999px;border:2px solid var(--outline);flex:none;display:grid;place-items:center}
.radio.on{border-color:var(--pri)}
.radio.on:after{content:"";width:11px;height:11px;border-radius:999px;background:var(--pri)}
.seg{display:grid;grid-auto-flow:column;grid-auto-columns:1fr;background:var(--surf-hi);border-radius:999px;padding:4px;gap:4px}
.seg div{min-height:48px;border-radius:999px;display:flex;align-items:center;justify-content:center;gap:6px;font-size:14px;font-weight:700;color:var(--on-v);text-align:center;line-height:1.15;padding:0 6px}
.seg .on{background:var(--pri-c);color:var(--on-pri-c)}
.seg .on:before{content:"✓";font-weight:800}
.note{font-size:14px;color:var(--on-v);line-height:1.5;padding:8px 8px 0}
.slider{height:6px;border-radius:3px;background:var(--surf-hst);position:relative;margin:12px 0 4px}
.slider i{position:absolute;left:0;top:0;bottom:0;border-radius:3px;background:var(--pri)}
.slider b{position:absolute;top:-8px;width:22px;height:22px;border-radius:999px;background:var(--pri)}
.timechip{display:inline-flex;align-items:center;height:48px;padding:0 14px;border-radius:14px;background:var(--surf-hst);font-size:15px;font-weight:700;color:var(--on)}
.card{background:var(--surf-c);border-radius:24px;padding:18px;margin:14px 12px 0;flex:none}
.card h3{margin:0;font-size:20px;line-height:1.25}
.card p{margin:6px 0 0;font-size:15px;color:var(--on-v);line-height:1.45}

/* sheets */
.scrim{position:absolute;left:0;right:0;bottom:0;top:96px;background:var(--scrim)}
.sheet{position:absolute;left:0;right:0;bottom:0;background:var(--surf-lo);border-radius:32px 32px 0 0;padding:8px 16px 28px;border-top:1px solid var(--outline-v)}
.sheet .sh{display:flex;align-items:center;gap:8px;margin:0 -6px 2px 6px}
.sheet .sh h4{flex:1;margin:0;font-size:20px}
.sheet .lead{margin:4px 6px 12px;font-size:15px;color:var(--on-v);line-height:1.5}
.sheet .lbl2{font-size:13.5px;font-weight:700;color:var(--on-v);margin:12px 6px 6px}
.handle{width:36px;height:4px;border-radius:2px;background:var(--outline);margin:4px auto 6px;opacity:.6}
.two{display:grid;grid-template-columns:1fr 1fr;gap:8px}

/* vertical step list (full-screen stages only) */
.vsteps{margin-top:16px;border-radius:18px;background:var(--surf-c);padding:10px 14px}
.vs{display:grid;grid-template-columns:24px 1fr auto;gap:10px;align-items:center;min-height:34px;font-size:14.5px;color:var(--on-v);position:relative}
.vs .mk{width:16px;height:16px;border-radius:99px;border:2px solid var(--outline);justify-self:center}
.vs.done .mk{background:var(--outline);border-color:var(--outline)}
.vs.cur{color:var(--on);font-weight:700}
.vs.cur .mk{width:20px;height:20px;background:var(--alert);border-color:var(--on);box-shadow:0 0 0 3px color-mix(in srgb,var(--alert) 30%,transparent)}
.vs .when{font-size:13.5px;font-weight:500}
.vs.cur .when{font-weight:800;color:var(--on)}

/* full-screen alert template: scrolling content + pinned actions */
.al-body{flex:1;overflow:hidden;position:relative;padding:12px 22px 0}
.al-body .kick{display:flex;align-items:center;gap:8px;font-size:12.5px;font-weight:800;letter-spacing:.1em;text-transform:uppercase;color:var(--alert)}
.al-body h1{margin:8px 0 0;font-size:28px;line-height:1.2;font-weight:700;letter-spacing:-.01em}
.al-body .fig{display:flex;align-items:baseline;gap:12px;margin-top:12px}
.al-body .fig b{font-size:52px;font-weight:600;line-height:1;letter-spacing:-.02em}
.al-body .fig span{font-size:15px;font-weight:600;color:var(--on-v);line-height:1.3}
.al-body p{margin:10px 0 0;font-size:15.5px;line-height:1.5;color:var(--on-v)}
.al-body .fb{display:flex;align-items:center;gap:10px;margin-top:12px;padding:10px 12px;border-radius:14px;background:var(--surf-c);font-size:14.5px;font-weight:600;line-height:1.4}
.al-body .fadeb{background:linear-gradient(transparent,var(--bg))}
.trbl{margin-top:12px;border-radius:18px;background:var(--surf-c);padding:14px}
.trbl .cause{font-size:15.5px;font-weight:700;line-height:1.35}
.trbl .cause span{display:block;font-size:12.5px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;color:var(--on-v);margin-bottom:4px}
.trbl ol{margin:8px 0 10px;padding-left:20px;font-size:14.5px;line-height:1.5;color:var(--on-v)}
.trbl .two .btn{border-color:var(--outline);color:var(--on)}
.al-foot{flex:none;padding:12px 22px 30px;display:flex;flex-direction:column;gap:8px;border-top:1px solid var(--outline-v);background:var(--bg)}
.slab{min-height:88px;border-radius:30px;background:var(--slab);color:var(--slab-on);border:2px solid var(--slab-line);display:flex;align-items:center;justify-content:center;gap:14px;font-size:21px;font-weight:800;text-align:left;padding:8px 22px}
.slab small{display:block;font-size:14px;font-weight:600;opacity:.92}
.alrow{display:grid;grid-template-columns:1.2fr 1fr 1fr;gap:8px;margin-top:4px}
.alrow div{min-height:60px;border-radius:20px;background:var(--surf-hi);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:2px;font-size:13.5px;font-weight:700;color:var(--on);text-align:center;line-height:1.2}
.hazard{height:10px;background:repeating-linear-gradient(-45deg,#ff8f86 0 10px,transparent 10px 20px);opacity:.6;flex:none}
.pulse{animation:pulse 2s ease-in-out infinite}
@keyframes pulse{0%,100%{opacity:1}50%{opacity:.6}}
@media (prefers-reduced-motion:reduce){.pulse,.edge-pulse{animation:none}}

/* baby kiosk */
.kedge{position:absolute;inset:0;border-radius:46px;pointer-events:none}
.edge-pulse{animation:ep 4s ease-in-out infinite}
@keyframes ep{0%,100%{opacity:.45}50%{opacity:.9}}
.k-wrap{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;padding:0 32px;color:var(--on)}
.k-wrap b{font-size:20px;font-weight:600;line-height:1.35}
.k-wrap p{font-size:15px;color:var(--on-v);margin:8px 0 0;line-height:1.5}
.k-status{flex:none;margin:0 20px 44px;display:flex;align-items:center;justify-content:center;gap:10px;font-size:16px;font-weight:600;color:var(--on-v)}
.k-rows{width:100%;text-align:left;margin-top:18px;border-top:1px solid var(--dim)}
.k-rows div{display:flex;justify-content:space-between;gap:10px;padding:10px 0;border-bottom:1px solid var(--dim);font-size:15px;color:var(--on-v)}
.k-rows div b{font-size:15px;font-weight:600;color:var(--on)}
.k-slide{margin:14px auto 0;width:240px;height:56px;border-radius:999px;border:1.5px solid var(--dim);display:flex;align-items:center;padding:0 5px;color:var(--on-v);font-size:14px;gap:10px}
.k-slide i{width:44px;height:44px;border-radius:999px;background:#1a1817;display:grid;place-items:center}

/* home */
.hhead{display:flex;align-items:center;gap:10px;padding:8px 12px 0 20px;flex:none}
.hhead .name{flex:1;font-size:18px;font-weight:700}
.logo{width:30px;height:30px;color:var(--brand)}
.hzhead{position:relative;padding:14px 22px 22px;flex:none}
.hzhead h1{margin:6px 0 0;font-size:28px;line-height:1.2;font-weight:700;letter-spacing:-.015em}
.hzhead p{margin:6px 0 0;font-size:15px;color:var(--on-v);line-height:1.5}
.hzhead:after{content:"";position:absolute;left:0;right:0;bottom:0;height:2px;background:linear-gradient(90deg,transparent,var(--horizon),transparent);opacity:.55}
.rolehead{display:flex;align-items:center;gap:10px;margin:16px 22px 8px;font-size:15px;font-weight:700}
.rolehead small{font-weight:500;color:var(--on-v);font-size:14px}
.rolehead svg{color:var(--pri)}
.conn{display:flex;align-items:center;gap:12px;padding:12px 12px 12px 16px;min-height:76px}
.conn + .conn{border-top:1px solid var(--outline-v)}
.conn .tx{flex:1;min-width:0}
.conn b{display:block;font-size:16px;font-weight:700}
.conn small{display:block;font-size:14px;color:var(--on-v);line-height:1.4}
.firstcard{display:flex;align-items:center;gap:16px;background:var(--surf-c);border-radius:28px;padding:20px;margin:0 16px 12px;min-height:108px}
.firstcard .ic{width:56px;height:56px;border-radius:18px;display:grid;place-items:center;background:var(--pri-c);color:var(--on-pri-c)}
.firstcard b{display:block;font-size:19px}
.firstcard small{font-size:15px;color:var(--on-v)}
.firstcard .tx{flex:1}
/* web night cover */
.drift{position:absolute;width:10px;height:10px;border-radius:99px;background:#2a2725}
"""
CSS += r"""
.tgl{min-width:64px;height:36px;border-radius:999px;background:var(--surf-hst);border:2px solid var(--outline);position:relative;flex:none;display:flex;align-items:center;justify-content:flex-end;padding:0 9px;font-size:12px;font-weight:800;color:var(--on-v)}
.tgl:after{content:"";position:absolute;top:7px;left:7px;width:18px;height:18px;border-radius:999px;background:var(--outline)}
.tgl.on{background:var(--pri);border-color:var(--pri);color:var(--on-pri);justify-content:flex-start}
.tgl.on:after{left:auto;right:3px;top:3px;width:26px;height:26px;background:var(--on-pri)}
"""

CSS += r"""
/* ---- round 2 ---- */
.tools{padding:10px 16px 0 16px}
.chip.shrink{min-width:0;flex:0 1 auto}
.ell{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}
.meta-line{display:flex;flex-wrap:wrap;gap:4px 14px;padding:12px 22px 0;font-size:14px;color:var(--on-v);line-height:1.5}
.meta-line span{display:inline-flex;align-items:center;gap:6px}
.statusline{display:flex;align-items:center;gap:8px;padding:0 4px 10px;font-size:14.5px;font-weight:600;color:var(--on-v)}
.fig2{margin-top:12px}
.fig2 b{display:block;font-size:52px;font-weight:600;line-height:1;letter-spacing:-.02em}
.fig2 span{display:block;font-size:15px;font-weight:600;color:var(--on-v);margin-top:6px}
.prog{display:flex;align-items:center;gap:10px;margin-top:14px;font-size:14px;font-weight:600;color:var(--on-v)}
.prog .segs{display:flex;gap:4px}
.prog .segs i{width:22px;height:6px;border-radius:3px;background:var(--outline-v)}
.prog .segs i.d{background:var(--outline)}
.prog .segs i.c{background:var(--on);width:30px}
.cause2{margin-top:14px;border-radius:18px;background:var(--surf-c);padding:14px}
.cause2 .cl{font-size:15.5px;font-weight:700;line-height:1.4}
.cause2 .cl span:first-child{display:block;font-size:12px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;color:var(--on-v);margin-bottom:4px}
.cause2 .wrong{display:block;font-weight:500;color:var(--on-v);margin-top:4px;font-size:14.5px}
.cause2 .crow{display:flex;align-items:center;gap:8px;margin-top:10px;flex-wrap:wrap}
.cause2 .btn{border-color:var(--outline);color:var(--on);white-space:nowrap}
.disc{display:inline-flex;align-items:center;gap:6px;min-height:48px;padding:0 10px;font-size:14.5px;font-weight:700;color:var(--on)}
.btn.quiet{color:var(--on-v);font-weight:600;text-decoration:underline;text-underline-offset:3px;margin-top:8px;justify-content:flex-start;padding:0 2px}
.btn{white-space:nowrap}
.scrollhint{position:absolute;right:6px;top:10px;width:4px;height:120px;border-radius:2px;background:var(--outline);opacity:.6}
/* Ember: every warning banner and safety chip uses the fixed Dusk safety palette */
.t-ember .banner.warn,.banner.warn.fx{background:#3e2719;color:#f0a774}
.t-ember .banner.alert,.banner.alert.fx{background:#4a1b1c;color:#ff948a}
.t-ember .banner.info{background:#1e2a38;color:#a9c6ea}
.t-ember .sbar.grey{background:#322e39;color:#ece6ef}
.t-ember .sbar.needs{background:#4a1b1c;color:#ff948a}
.banner.more{min-height:48px;padding:6px 10px 6px 14px;font-size:14.5px;font-weight:700;background:transparent;border:1.5px solid var(--outline-v);color:var(--on)}
.t-batt{ --bg:#1f1209; --surf-lo:#26170c; --surf-c:#2e1c0f; --surf-hi:#3a2413; --on:#fff3ea; --on-v:#f5d2bb; --outline:#d39a74; --outline-v:#6a4126;
  --alert:#f0a774; --slab:#7a3d12; --slab-on:#ffe9d8; --slab-line:#f0a774; }
.talkpanel.dark{background:var(--pri-c);color:var(--on-pri-c)}
.talkpanel.dark .progress{background:color-mix(in srgb,var(--on-pri-c) 22%,transparent)}
.talkpanel.dark .progress i{background:var(--pri)}
/* 200 % font scale evidence frames */
.fs2 .sbar{font-size:24px;min-height:64px}
.fs2 .sbar small{font-size:20px}
.fs2 .sbar .meta{font-size:22px}
.fs2 .tools{flex-wrap:wrap;row-gap:8px}
.fs2 .tools .chip.shrink{flex:1 0 100%}
.fs2 .tools .sp{display:none}
.fs2 .tools > :nth-child(3){margin-left:auto}
.fs2 .chip{font-size:22px;height:auto;min-height:60px}
.fs2 .ibtn{width:60px;height:60px}
.fs2 .hero h2{font-size:40px}
.fs2 .hero p{font-size:28px}
.fs2 .dock .listen b{font-size:30px}
.fs2 .dock .listen small{font-size:24px}
.fs2 .row3{grid-template-columns:1fr 1fr}
.fs2 .row3 .qbtn:last-child{grid-column:span 2;flex-direction:row;gap:10px}
.fs2 .qbtn{font-size:22px;min-height:84px}
.fs2 .al-body .kick{font-size:20px}
.fs2 .al-body h1{font-size:44px}
.fs2 .fig2 b{font-size:44px}
.fs2 .fig2 span,.fs2 .al-body p{font-size:28px}
.fs2 .prog{font-size:24px}
.fs2 .slab{font-size:34px;min-height:120px}
.fs2 .slab small{font-size:22px}
.fs2 .mid{flex:1;overflow:hidden;position:relative}
/* baby set-up confirmation at wake brightness */
.k-confirm{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;padding:0 28px;color:#e2dcd6}
.k-confirm b{font-size:28px;line-height:1.3;font-weight:700}
.k-confirm p{font-size:18px;color:#c8c1bb;margin:10px 0 0;line-height:1.5}
.kbtn{background:var(--dim);color:#e2dcd6}
"""
CSS += r"""
.wrongline{padding:10px 22px 0;font-size:14.5px;color:var(--on-v);line-height:1.45}
.btn.quiet.pad{padding:0 22px;margin-top:0;min-height:48px}
.kmark-dash{position:absolute;top:18px;left:40px;right:40px;height:6px;background:repeating-linear-gradient(90deg,var(--k-warn) 0 16px,transparent 16px 26px)}
"""

CSS += r"""
.qbtn{font-size:14px}
.prog .segs i{background:color-mix(in srgb,var(--outline) 60%,transparent)}
.prog .segs i.d{background:var(--outline)}
.prog .segs i.c{background:var(--on)}
.cnote{margin-top:10px;font-size:14px;color:var(--on-v)}
.morelink{display:flex;align-items:center;justify-content:space-between;gap:8px;min-height:48px;margin:4px 12px 0;padding:0 8px 0 14px;font-size:14.5px;font-weight:600;color:var(--on-v);flex:none}
"""
