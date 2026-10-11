import type { VisualizerPresetId } from './presets'

// Each preset compiles separately. A complex preset cannot add work to a simple one.
// All geometry is procedural; no texture has a fixed resolution.
// Each audio signal has one visible job per scene, so instruments read as different motions:
// bass sets scale, mids set shape, treble sets fine detail, and kick, snare and hat pulses
// trigger distinct events.
const scenes: Record<VisualizerPresetId, string> = {
  spectrum: `
    float x = clamp(uv.x, 0., .9999) * 16.;
    int band = int(x);
    float base = -.72 + kick*.05;
    float y = p.y - base;
    float height = min(1.58, amp(band) * 1.35);
    float shape = 1. - smoothstep(.31, .38, abs(fract(x)-.5));
    float bar = shape * (1.-smoothstep(height, height+.014, y)) * step(0., y);
    vec3 ink = spectral(uv.x*.7 + .28 + swell*.25);
    float crack = snare * smoothstep(height-.3, height, y);
    c += mix(ink, vec3(1.), crack*.7) * bar * (1.1 + uv.y + crack*1.5);
    c += ink * glow(y, .008) * (.22 + kick*1.2);
    c += ink * exp(-abs(y)*13.) * step(y, 0.) * amp(band) * (.5 + kick);
    c += ink * glow(y-height, .008) * step(.13,fract(x)) * step(fract(x),.87) * .7;
    float cap = min(1.62, peak(band) * 1.35);
    c += mix(ink, vec3(1.), .6) * shape * glow(y-cap-.03, .006) * 1.4;
    float above = y - cap - .05;
    float spark = hash(vec2(floor(x), floor(above*16.) + floor(time*24.)));
    c += vec3(1.) * hat * step(.84, spark) * step(0., above) * exp(-above*5.) * shape * 1.4;
  `,
  flow: `
    for(int i=0;i<6;i++){
      float f=float(i);
      float y=p.y + (.1+bass*.48)*sin(p.x*1.35 + time*.26+f*.22)
        + (.05+highMid*.22)*sin(p.x*2.6-time*.2+f*.36)
        + hat*.035*sin(p.x*40.+f*3.+time*9.) + (f-2.5)*(.04+lowMid*.13);
      c += spectral(f*.085+.48+snare*.25) * glow(y,.009+amp(i+4)*.03+kick*.015)
        * (.25+amp(i+4)*1.25+kick*.6);
    }
    c *= exp(-p.x*p.x*.09);
  `,
  aurora: `
    for(int i=0;i<5;i++){
      float f=float(i);
      float wave=(.16+lowMid*.32)*sin(p.x*1.5+f*.55+time*.19+snare*.7)+.15*sin(p.x*3.3-time*.24);
      float curtain=p.y-wave+(f-2.)*.105+.22-bass*.38;
      float filament=.45+.55*pow(.5+.5*sin(p.x*47.+noise(vec2(p.x*5.,time*.2))*8.+f+hat*3.),3.);
      float veil=exp(-max(curtain,0.)*(6.5-highMid*4.5))*smoothstep(-.04,.05,curtain);
      vec3 tint=mix(vec3(.1,1.,.48),vec3(.52,.18,1.),clamp(f*.19+swell-.5,0.,1.));
      c+=tint*(veil*.3*filament*(.7+treble*1.2)+glow(curtain,.014)*(.4+kick*1.4))*(.15+amp(i+2)+bass*.6);
    }
  `,
  plasma: `
    vec2 q=p*(1.35+lowMid*.9);
    for(int i=0;i<4;i++){
      float f=float(i);
      q += (.18+bass*.42)*sin(q.yx*(1.4+f*.16)+vec2(time*.2,-time*.17)+f*1.7);
    }
    float field=sin(q.x*2.+time*.14)+sin(q.y*2.7-time*.2)+sin(length(q)*3.-mids*3.);
    c=spectral(field*.19+time*.018+snare*.3)*(.3+.45*sin(field*2.8+bass*3.)+kick*.4);
    c+=vec3(1.,.3,.65)*pow(.5+.5*cos(field*3.3),18.-hat*11.)*(.1+treble*1.3);
  `,
  solar: `
    float angle=atan(p.y,p.x);
    float radius=.24+bass*.28;
    float flame=noise(vec2(angle*3.5,time*.3))*noise(vec2(angle*9.-time*.2,r*5.));
    float spikes=pow(.5+.5*sin(angle*23.+time*.5),12.)*(treble+hat);
    float edge=r-radius-flame*(.04+lowMid*.32)-spikes*.12;
    vec3 hot=mix(vec3(1.,.26,.025),vec3(1.,.85,.6),snare*.75);
    c+=hot*glow(edge,.04)*2.;
    c+=vec3(1.,.66,.13)*glow(edge,.008)*(1.4+hat);
    c+=vec3(1.,.075,.015)*exp(-abs(edge)*6.)*(.2+flame);
    float interior=(1.-smoothstep(radius-.03,radius+.01,r));
    c+=interior*mix(vec3(1.,.22,.035),vec3(1.,.7,.4),snare*.5)*(.6+.4*fbm(p*9.+time*.15));
    c+=interior*vec3(1.,.8,.3)*pow(noise(p*18.-time*.2),5.);
    float wave=r-radius-(1.-kick)*.75;
    c+=vec3(1.,.5,.12)*glow(wave,.02)*kick*1.4*(.6+flame);
  `,
  eclipse: `
    vec2 q=rotate(.28+snare*.14)*p;
    float tilt=3.6-mids*1.5;
    float disk=length(vec2(q.x,q.y*tilt));
    float orbit=.48+lowMid*.2;
    float streak=pow(.5+.5*sin(atan(q.y*tilt,q.x)*18.+time*.7+disk*25.),1.+treble*4.);
    c+=vec3(1.,.49,.16)*glow(disk-orbit,.03+bass*.05)*(.45+streak)*1.3;
    c+=vec3(.45,.63,1.)*glow(disk-orbit-.11,.012)*(.25+highMid*1.6);
    float hole=.2+bass*.15;
    c*=smoothstep(hole-.012,hole+.012,r);
    c+=mix(vec3(1.,.6,.24),vec3(.6,.77,1.),smoothstep(-.3,.3,p.y))*glow(r-hole,.012)*(1.3+kick*2.);
    c+=vec3(.6,.28,.13)*glow(r-hole,.085)*(.2+kick*.6);
  `,
  tunnel: `
    for(int i=0;i<12;i++){
      float z=fract(float(i)/12.+time*.055);
      float size=(.08+z*z*2.6)*(1.-kick*.14*z);
      vec2 q=rotate(z*.85+time*.065+snare*.45)*p;
      float d=mix(max(abs(q.x),abs(q.y)),length(q)*.82,clamp(lowMid*1.3,0.,1.))-size;
      float level=amp(int(z*15.));
      c+=spectral(z*.65+time*.025)*glow(d,.003+z*.008+bass*.014+kick*.01)*(.2+z)
        *(1.-smoothstep(.88,1.,z))*(.06+level*1.8);
      c+=vec3(1.)*hat*glow(d,.002)*step(.92,hash(vec2(floor(atan(q.y,q.x)*12.),float(i))))*z;
    }
    c+=vec3(.3,.1,.8)*exp(-r*3.)*(.05+bass*.9);
  `,
  starfield: `
    vec2 q=rotate(time*.018)*p;
    for(int i=0;i<5;i++){
      float z=fract(float(i)/5.+time*.045);
      float scale=mix(16.,3.,z);
      vec2 grid=q*scale+float(i)*13.7;
      vec2 cell=floor(grid), local=fract(grid)-.5;
      vec2 center=vec2(hash(cell),hash(cell+41.))-.5;
      vec2 delta=local-center*.72;
      float seed=hash(cell+7.);
      float star=exp(-length(delta)/(.004+z*.01+treble*.01));
      float rays=(glow(delta.x,.006)*glow(delta.y,.12)+glow(delta.y,.006)*glow(delta.x,.12))
        *(hat*1.6+kick*.5);
      float fade=smoothstep(0.,.12,z)*(1.-smoothstep(.75,1.,z));
      c+=mix(vec3(.35,.65,1.),vec3(1.,.72,.5),seed)*(star+rays)*fade*(.3+amp(int(seed*16.))*1.3);
    }
    c+=mix(vec3(.15,.25,1.),vec3(.7,.2,1.),snare)*exp(-r*r*2.5)*(kick+snare)*.25;
    c+=mix(vec3(.08,.2,.9),vec3(.55,.12,.85),fbm(p*1.5-time*.03))*fbm(p*2.5+time*.05)
      *bass*.4*exp(-r*r*.6);
  `,
  nova: `
    float angle=atan(p.y,p.x);
    float rays=pow(.5+.5*sin(angle*31.+sin(angle*11.+time*.18)*3.),9.-treble*6.);
    float radius=.07+bass*.33;
    c+=spectral(r*.35+swell*.2-.08)*exp(-abs(r-radius)*5.)*(.2+rays*(.5+treble*2.6));
    c+=vec3(1.,.75,.52)*glow(r-radius,.017)*1.3;
    c+=vec3(1.,.85,.8)*exp(-r*r/(.006+bass*.032));
    c+=vec3(.32,.15,.75)*glow(r-.62-highMid*.28,.025)*(.25+highMid*1.3);
    float shock=r-radius-(1.-kick)*1.1;
    c+=spectral(angle*.08+time*.02)*glow(shock,.018)*kick*kick*1.8*(.6+rays);
    c+=vec3(.6,.85,1.)*glow(r-radius-(1.-snare)*.55,.008)*snare*1.4;
  `,
  prism: `
    for(int i=0;i<8;i++){
      float f=float(i);
      vec2 q=rotate(time*.06+f*.13+snare*(f-3.5)*.09)*p;
      float a=atan(q.y,q.x)+PI*.5;
      float triangle=cos(floor(.5+a/(TAU/3.))*(TAU/3.)-a)*length(q);
      float size=(.1+f*.085)*(1.+kick*.12)+amp(i+3)*.14;
      c+=spectral(f*.11+.47+swell*.2-.1)*glow(triangle-size,.004+hat*.008)
        *(.3+amp(i+3)*1.4)*(1.-f*.06);
    }
  `,
  kaleidoscope: `
    float angle=atan(p.y,p.x)+time*.07+snare*.3;
    angle=abs(mod(angle,TAU/8.)-PI/8.);
    vec2 q=vec2(cos(angle),sin(angle))*r*(1.-kick*.15);
    for(int i=0;i<5;i++){
      float f=float(i);
      q=abs(q)-vec2(.17+bass*.2,.08+mids*.16);
      q=rotate(.65+f*.13+time*.035)*q;
      float d=abs(length(q)-(.09+amp(i+4)*.22));
      c+=spectral(f*.15+r*.28+time*.012)*glow(d,.008+hat*.007)*(.2+amp(i+4)*1.1+f*.05);
    }
  `,
  mandala: `
    float a=atan(p.y,p.x)+time*.035+snare*.2;
    for(int i=0;i<8;i++){
      float f=float(i);
      float petals=.045*sin(a*(8.+2.*f)+f*.8+time*.09);
      float edge=r-(.12+f*.085)*(.78+bass*.36+kick*.1)-petals*(.3+amp(i)*4.);
      c+=mix(vec3(1.,.64,.2),vec3(.95,.22,.55),f/9.)*glow(edge,.004)*(.3+amp(i)*1.4);
    }
    c+=vec3(1.,.85,.48)*pow(.5+.5*cos(a*32.),28.)*glow(r-.85,.015)*(.15+treble*1.4+hat*1.6);
  `,
  grid: `
    float horizon=-.04-kick*.02;
    float sky=p.y-horizon;
    c+=mix(vec3(.02,.01,.07),vec3(.3,.03,.32),exp(-max(sky,0.)*2.4))*.7;
    vec2 starCell=floor(p*46.);
    float twinkle=step(.965,hash(starCell))*glow(length(fract(p*46.)-.5),.06);
    c+=vec3(.85,.8,1.)*twinkle*(.15+hat*1.6)*(.6+.4*sin(time*3.+hash(starCell+3.)*TAU));

    // Striped sun. Gaps widen toward the bottom and scroll down; snares jump them.
    vec2 sunCenter=vec2(0.,horizon+.36);
    float sunSize=.34+bass*.09;
    float sunDistance=length(p-sunCenter);
    float sunHeight=clamp((p.y-sunCenter.y+sunSize)/(2.*sunSize),0.,1.);
    float stripe=fract((p.y-sunCenter.y)*24.+time*.25+snare*.5);
    float stripes=step(mix(.55,0.,smoothstep(0.,.55,sunHeight)),stripe);
    vec3 sunInk=mix(vec3(1.,.07,.45),vec3(1.,.88,.1),smoothstep(.15,.85,sunHeight));
    c+=sunInk*(1.-smoothstep(sunSize-.004,sunSize,sunDistance))*stripes*(1.1+lowMid*.6);
    c+=mix(vec3(1.,.12,.3),vec3(1.,.45,.08),sunHeight)
      *exp(-max(sunDistance-sunSize,0.)*(5.5-bass*2.5))*step(sunSize,sunDistance)*(.3+bass*.6);

    // Low-poly mountains, far pairs first. Outer peaks follow bass; inner peaks follow treble.
    for(int i=0;i<14;i++){
      float f=float(i), k=floor(f*.5), side=mod(f,2.)*2.-1.;
      float seed=hash(vec2(f,7.));
      float x0=side*(.34+k*.25+seed*.08);
      float h=(.07+k*.085+seed*.05)*(.75+amp(int((6.-k)*2.)+i%2)*.5);
      float w=h*(1.1+hash(vec2(f,2.))*.6);
      float x=(p.x-x0)/w, y=(p.y-horizon)/h;
      float edge=(1.-abs(x)-y)*h;
      float inside=smoothstep(-.002,.002,edge)*step(0.,y);
      // Straight ridges from the apex and zigzag contours give flat low-poly facets.
      float slope=x/max(1.-y,.02);
      float zigzag=abs(fract(slope*1.5+seed)-.5)*.5;
      float facets=max(wire(slope*2.+seed,1.2),wire(y*3.5+zigzag,1.2));
      vec3 body=mix(vec3(.09,.03,.27),vec3(.02,.03,.13),y);
      body+=vec3(.25,.45,1.)*facets*(.3+treble*1.2)*(1.-y*.3);
      c=mix(c,body,inside);
      c+=vec3(1.,.2,.85)*glow(edge,.0035)*step(0.,y)*step(abs(x),1.)*(.6+kick*1.2+highMid*.5);
    }

    // Grid floor rushing toward the camera.
    if(sky<0.){
      float depth=.35/max(-sky,.002);
      vec2 road=vec2(p.x*depth,depth+time*1.2);
      // Distant lines merge, so they fade into the horizon haze before they saturate.
      float fade=exp(-depth*.2);
      float rails=wire(road.x*12.,1.6), ties=wire(road.y*4.,1.4);
      rails*=1.+step(abs(road.x*12.),.5)*1.5;
      c=mix(vec3(.24,.05,.46),vec3(.05,.02,.18),clamp(-sky*1.8,0.,1.))*(.6+lowMid*.6);
      c+=(vec3(1.,.2,.85)*rails+vec3(.45,.3,1.)*ties)*fade*(.55+bass*.9+kick*1.2);
      // Each kick sends a band of light down the road toward the camera.
      c+=vec3(1.,.45,1.)*glow(depth-.4-kick*14.,.35+depth*.05)*kick*(.4+rails+ties)*1.4;
    }
    c+=vec3(1.,.4,.9)*glow(sky,.005)*(.45+kick*1.6);
    c+=vec3(1.,.85,1.)*exp(-length(vec2(p.x*.6,sky*3.))*14.)*(.7+kick);
  `,
  helix: `
    float phase=p.x*3.+time*.45+snare*.6;
    float height=.18+bass*.42;
    float a=sin(phase)*height,b=sin(phase+PI)*height;
    c+=vec3(.1,.85,1.)*glow(p.y-a,.01+kick*.012)*(.4+lowMid*1.3);
    c+=vec3(1.,.2,.65)*glow(p.y-b,.01+kick*.012)*(.4+highMid*1.3);
    float rung=abs(fract((p.x+time*.15)*5.)-.5);
    float between=step(min(a,b),p.y)*step(p.y,max(a,b));
    c+=spectral(p.x*.12+.5)*glow(rung,.025)*between*(.25+mids*.9);
    float strand=min(abs(p.y-a),abs(p.y-b));
    c+=vec3(1.)*hat*step(.9,hash(floor(p*60.)+floor(time*15.)))*glow(strand,.03)*2.;
    c*=exp(-p.x*p.x*.12);
  `,
  rings: `
    for(int i=0;i<7;i++){
      float f=float(i);
      vec2 q=rotate(f*.43+time*.06+snare*.25*(f-3.))*p;
      q.y*=1.5+.5*sin(f*.9+time*.1);
      float radius=.2+f*.09+amp(i*2)*.12;
      c+=spectral(f*.105+.06)*glow(length(q)-radius,.005)*(.3+amp(i*2)*1.4);
      float angle=time*(.12+f*.015)+f;
      vec2 point=vec2(cos(angle),sin(angle))*radius;
      c+=spectral(f*.105+.06)*glow(length(q-point),.008+treble*.03)*(.6+treble+hat*step(4.,f)*1.5);
    }
    c+=vec3(1.,.72,.32)*exp(-r*r*(100.-kick*60.))*(.35+bass+kick);
  `,
  silk: `
    for(int i=0;i<14;i++){
      float f=float(i);
      float level=amp(i+1);
      float wave=sin(p.x*(1.4+f*.018)+time*.23+f*.12)*(.18+bass*.42);
      wave+=sin(p.x*3.-time*.16+f*.23)*(.03+level*.2);
      wave+=hat*.02*sin(p.x*30.+f*1.7);
      wave+=(f-6.5)*(.016+mids*.022);
      c+=spectral(f*.036+.5+snare*.2)*glow(p.y-wave,.003+kick*.003)*(.18+level*1.1);
    }
    c*=exp(-p.x*p.x*.08);
  `,
  vortex: `
    float a=atan(p.y,p.x);
    float spiral=a*5.+log(r+.06)*(6.+mids*8.)-time*.4;
    float arm=pow(.5+.5*sin(spiral+noise(p*3.)*2.),12.-bass*8.);
    c+=spectral(log(r+.1)*.21+time*.012+snare*.2)*arm*(.35+bass*1.2+kick*.8)
      *smoothstep(.04,.3,r)*exp(-r*.5);
    c+=vec3(.1,.5,1.)*glow(r-.12-bass*.15,.012)*(1.+kick*1.5);
    c+=vec3(.3,.15,1.)*exp(-r*4.)*.25;
    c+=vec3(.8,.9,1.)*hat*arm*step(.9,hash(floor(p*50.)+floor(time*12.)))*2.;
  `,
  sparks: `
    for(int i=0;i<4;i++){
      float f=float(i);
      vec2 grid=p*(3.+f*2.)+vec2(sin(time*.12+f)*.4,time*.08*(f+1.));
      vec2 cell=floor(grid), local=fract(grid)-.5;
      float seed=hash(cell+f*19.);
      vec2 center=vec2(sin(seed*TAU+time*.13),cos(seed*21.+time*.12))*.27;
      float point=length(local-center);
      float light=(.15+.6*pow(.5+.5*sin(seed*30.+time*.45),3.))*(.15+amp(int(seed*16.))*1.6)
        +kick*step(.75,seed)*1.2+hat*step(.9,fract(seed*7.))*1.6;
      c+=mix(vec3(.3,1.,.35),vec3(1.,.65,.1),seed)
        *(glow(point,.012)*1.8+glow(point,.06+bass*.08)*(.08+bass*.25))*light;
    }
    c+=vec3(.02,.15,.06)*exp(-r*r)*(.5+bass);
  `,
  terrain: `
    for(int i=0;i<10;i++){
      float f=float(9-i);
      float x=(p.x+3.)*2.+f*.7+time*.12;
      int index=int(mod(floor(x),16.));
      float audio=mix(amp(index),amp((index+1)%16),smoothstep(0.,1.,fract(x)));
      float mountain=.2*sin(p.x*2.+f*.65)+.07*sin(p.x*5.4-f*.8)-audio*.6;
      float edge=p.y+.8-f*.145-mountain-step(f,.5)*kick*.07;
      vec3 ink=spectral(f*.068+.4+swell*.2);
      c=mix(c,ink*.025,1.-smoothstep(-.025,0.,edge));
      c+=ink*glow(edge,.005)*(.25+audio*1.4+hat*.6*step(6.5,f)+snare*.4*step(2.5,f)*step(f,6.5));
    }
  `,
  chrome: `
    vec2 q=p*(1.6+lowMid*.8);
    for(int i=0;i<4;i++){
      float f=float(i);
      q+=sin(q.yx*(1.4+f*.17)+time*.12+f*2.)*(.2+bass*.36);
    }
    float surface=sin(q.x*2.+q.y)+cos(q.y*2.3-q.x*.4);
    float shine=pow(.5+.5*sin(surface*3.+mids*2.),12.-treble*6.);
    vec3 metal=mix(vec3(.025,.08,.13),vec3(.62,.87,1.),.5+.5*sin(surface*2.));
    c=metal*(.2+.7*shine)+vec3(1.,.38,.58)*pow(.5+.5*cos(surface*2.+1.+snare*2.),8.)*(.35+snare*.8);
    c+=vec3(.9,1.,1.)*shine*(.2+treble+kick*.6);
  `,
}

const common = `#version 300 es
precision highp float;
uniform vec2 resolution, visualBounds;
uniform float bands[16], peaks[16];
uniform vec3 colorA, colorB;
uniform vec4 levels;
uniform vec3 hits;
uniform float time, intensity, swell;
out vec4 outputColor;
const float PI=3.14159265, TAU=6.2831853;
float amp(int i){return clamp(bands[i]*intensity,0.,1.4);}
float peak(int i){return clamp(peaks[i]*intensity,0.,1.4);}
float glow(float d,float width){
  d=abs(d); return exp(-d/width)+.22*exp(-d/(width*4.));
}
mat2 rotate(float a){float s=sin(a),c=cos(a);return mat2(c,-s,s,c);}
float hash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
float noise(vec2 p){
  vec2 i=floor(p),f=fract(p);f=f*f*(3.-2.*f);
  return mix(mix(hash(i),hash(i+vec2(1,0)),f.x),mix(hash(i+vec2(0,1)),hash(i+vec2(1,1)),f.x),f.y);
}
/** Anti-aliased line on each integer of v, about px pixels wide at 1080p. */
float wire(float v,float px){
  float d=abs(fract(v-.5)-.5)/max(fwidth(v),1e-5);
  return 1.-smoothstep(px*.5,px*.5+1.,d*1080./resolution.y);
}
float fbm(vec2 p){return noise(p)*.57+noise(p*2.03)*.28+noise(p*4.09)*.15;}
vec3 spectral(float t){
  vec3 vivid=.53+.47*cos(TAU*(t+vec3(0.,.33,.67)));
  return mix(vivid,mix(colorA,colorB,.5+.5*sin(t*TAU)),.18);
}
void main(){
  vec2 screen=gl_FragCoord.xy/resolution;
  vec2 uv=vec2(screen.x,(gl_FragCoord.y-visualBounds.x)/max(1.,visualBounds.y));
  vec2 p=(uv-.5)*2.*vec2(resolution.x/max(1.,visualBounds.y),1.);
  float r=length(p),energy=0.;
  for(int i=0;i<16;i++)energy+=amp(i)/16.;
  vec4 level=clamp(levels*intensity,0.,1.4);
  float bass=level.x,lowMid=level.y,highMid=level.z,treble=level.w,mids=(lowMid+highMid)*.5;
  float kick=hits.x,snare=hits.y,hat=hits.z;
  vec3 c=vec3(0.);
`

export function fragmentSource(preset: VisualizerPresetId): string {
  return (
    common +
    scenes[preset] +
    `
    float awake=smoothstep(0.,.065,energy);
    float vignette=1.-smoothstep(.3,1.3,length(screen-.5))*.55;
    c=max(c,vec3(0.))*awake*vignette;
    c*=1.+kick*.3;
    c=1.-exp(-c*1.45);
    outputColor=vec4(c,1.);
  }`
  )
}

export interface FeedbackStyle {
  /** Share of the previous frame that remains after one 60 fps frame. */
  decay: number
  /** Above 1 pushes trails outward; below 1 pulls them inward. */
  zoom: number
  /** Radians per 60 fps frame. */
  spin: number
  /** Screen fraction per 60 fps frame. */
  drift: [number, number]
}

// Trails give each scene a motion signature. The previous preset also fades through them.
export const feedbackStyles: Record<VisualizerPresetId, FeedbackStyle> = {
  spectrum: { decay: 0.8, zoom: 1, spin: 0, drift: [0, 0.0016] },
  flow: { decay: 0.86, zoom: 1, spin: 0, drift: [-0.0012, 0] },
  aurora: { decay: 0.88, zoom: 1, spin: 0, drift: [0, 0.0012] },
  plasma: { decay: 0.5, zoom: 1.002, spin: 0, drift: [0, 0] },
  solar: { decay: 0.86, zoom: 1.007, spin: 0.002, drift: [0, 0] },
  eclipse: { decay: 0.86, zoom: 1.004, spin: 0.004, drift: [0, 0] },
  tunnel: { decay: 0.84, zoom: 1.022, spin: 0.003, drift: [0, 0] },
  starfield: { decay: 0.9, zoom: 1.024, spin: 0, drift: [0, 0] },
  nova: { decay: 0.87, zoom: 1.012, spin: 0, drift: [0, 0] },
  prism: { decay: 0.85, zoom: 1.008, spin: -0.004, drift: [0, 0] },
  kaleidoscope: { decay: 0.82, zoom: 0.996, spin: 0.006, drift: [0, 0] },
  mandala: { decay: 0.83, zoom: 1.006, spin: 0.002, drift: [0, 0] },
  grid: { decay: 0.5, zoom: 1, spin: 0, drift: [0, 0] },
  helix: { decay: 0.82, zoom: 1, spin: 0, drift: [-0.0016, 0] },
  rings: { decay: 0.85, zoom: 1.003, spin: 0.004, drift: [0, 0] },
  silk: { decay: 0.86, zoom: 1, spin: 0, drift: [0, 0.0008] },
  vortex: { decay: 0.9, zoom: 0.985, spin: 0.012, drift: [0, 0] },
  sparks: { decay: 0.88, zoom: 1, spin: 0, drift: [0, 0.0012] },
  terrain: { decay: 0.7, zoom: 1, spin: 0, drift: [0, 0] },
  chrome: { decay: 0.55, zoom: 1, spin: 0, drift: [0, 0] },
}

/** Blends the scene over warped history. Its output becomes the next history. */
export const feedbackSource = `#version 300 es
precision highp float;
uniform sampler2D scene, history;
uniform vec2 resolution, center, drift;
uniform float decay, zoom, spin, aberration;
out vec4 outputColor;
void main(){
  vec2 uv=gl_FragCoord.xy/resolution;
  float aspect=resolution.x/resolution.y;
  vec2 d=(uv-center)*vec2(aspect,1.);
  float s=sin(spin),k=cos(spin);
  d=mat2(k,-s,s,k)*d/zoom;
  vec2 back=d/vec2(aspect,1.)+center-drift;
  vec3 trail=texture(history,back).rgb*decay;
  // 8-bit history must fall to zero. Without this bias, dim pixels stall.
  trail=max(trail-1.5/255.,0.)*step(0.,back.x)*step(back.x,1.)*step(0.,back.y)*step(back.y,1.);
  vec2 split=(uv-center)*aberration;
  vec3 s0=vec3(texture(scene,uv+split).r,texture(scene,uv).g,texture(scene,uv-split).b);
  // Max keeps static light stable. Screen blending would amplify it every frame.
  outputColor=vec4(max(s0,trail),1.);
}`

/** Adds bloom from the scene mip chain, the backdrop or music video, and grain. */
export const presentSource = `#version 300 es
precision highp float;
uniform sampler2D scene, frame, video;
uniform vec2 resolution;
uniform vec3 hits;
uniform vec4 levels;
uniform float beat, grain, swell, videoOn, overlay, videoAspect;
out vec4 outputColor;
float hash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
// Kick punches in, snare splits color, bass saturates, and builds raise contrast.
vec3 videoLayer(vec2 uv){
  float screen=resolution.x/resolution.y;
  vec2 q=(uv-.5)*(screen>videoAspect?vec2(1.,videoAspect/screen):vec2(screen/videoAspect,1.));
  q/=1.+hits.x*.05;
  vec2 split=normalize(q+1e-4)*(.0012+hits.y*.009);
  vec3 v=vec3(texture(video,q+.5+split).r,texture(video,q+.5).g,texture(video,q+.5-split).b);
  float luma=dot(v,vec3(.299,.587,.114));
  v=mix(vec3(luma),v,.85+levels.x*.6+(swell-.5)*.5);
  v=(v-.5)*(1.+(swell-.5)*.3)+.5;
  v*=1.+hits.x*.3+hits.z*.06;
  v*=1.-smoothstep(.45,.95,length(uv-.5))*.45;
  return clamp(v,0.,1.);
}
void main(){
  vec2 uv=gl_FragCoord.xy/resolution;
  vec3 c=texture(frame,uv).rgb;
  vec3 bloom=textureLod(scene,uv,3.).rgb*.4+textureLod(scene,uv,5.).rgb*.4+textureLod(scene,uv,7.).rgb*.35;
  bloom=max(bloom-.1,0.)*(.75+beat*.9);
  c+=bloom*(1.-c);
  float noise=(hash(gl_FragCoord.xy+grain)-.5)*.022;
  vec3 color=vec3(.013,.02,.033)+c*.97;
  if(videoOn>.5){
    vec3 v=videoLayer(uv);
    c*=overlay;
    color=v+c*(1.-v);
  }
  outputColor=vec4(color+noise,1.);
}`
