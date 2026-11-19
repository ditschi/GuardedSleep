from PIL import Image, ImageDraw
from brand import *
from rend import *
OUT=".."
WS=[5.2,5.6,6.0,6.6]
T=380
W=len(WS)*(T+40)+40
img=Image.new("RGBA",(W,T+600),(233,236,240,255))
label(img,"A2b+ wave weight study  (cream #F6D79A, 2 dp gap, wave 3 dp past the moon)",(40,18),26,bold=True)
for i,sw in enumerate(WS):
    full=svg(f'<rect width="108" height="108" fill="{NIGHT}"/>\n'+mark(sw,SAGE,CREAM))
    x=40+i*(T+40)
    big=launcher(full,512).resize((T,T),Image.LANCZOS)
    img.alpha_composite(big,(x,70))
    label(img,f"{sw} dp" + ("  (A2b original)" if sw==6.6 else ""),(x,70+T+8),22,bold=True)
    l48=launcher(full,48)
    s=home_strip(l48,"dark",48,count=3)
    img.alpha_composite(s,(x,70+T+44))
    # 4x nearest-neighbour zoom of the actual 48 px icon
    z=l48.resize((192,192),Image.NEAREST)
    bg=Image.new("RGBA",(208,208),(28,28,40,255)); bg.alpha_composite(z,(8,8))
    img.alpha_composite(bg,(x,70+T+44+s.height+12))
    label(img,"48 px at 4x (nearest)",(x+216,70+T+44+s.height+20),15)
    sl=home_strip(l48,"light",48,labels=False,count=3)
    img.alpha_composite(sl,(x,70+T+44+s.height+232))
    label(img,"48 px light wallpaper",(x,70+T+44+s.height+232+sl.height+4),15)
img.convert("RGB").save(f"{OUT}/weight-study.png")
