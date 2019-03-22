//=== original link for citation: https://www.shadertoy.com/view/llySRh

// --- printing chars, integers and floats ---------------------------

// --- access to the image of ascii code c

// 2 implementations.
// Use #if 1 for extensive text use (with no appearance change)
//           Also correct the windows bug ( light framing of chars. )

#if 0 // immediate draw. (allows for superimposition and appearance change).

vec4 char_(vec2 p, int c) {
    if (p.x<.0|| p.x>1. || p.y<0.|| p.y>1.) return vec4(0,0,0,1e5);
    //if (p.x<.25|| p.x>.75 || p.y<0.|| p.y>1.) return vec4(0,0,0,1e5); // strange bug with an old driver
	return textureGrad( iChannel0, p/16. + fract( vec2(c, 15-c/16) / 16. ), 
                        dFdx(p/16.), dFdy(p/16.) );
  // variants:
  //float l = log2(length(fwidth(p/16.*iResolution.xy)));
  //return textureLod( iChannel0, p/16. + fract( vec2(c, 15-c/16) / 16. ), l);
                   // manual MIPmap to avoid border artifact. Should be textureLod, but don't available everywhere
}
#  define draw_char() vec4(0)  // stub the final call function is used

#else // Deferred draw (call draw_char() ). Run and compiles faster.
      //     First only determine the valid char id at cur pixel 
      //     then call the draw char once at the end.

int char_id = -1; vec2 char_pos; 

vec4 char_(vec2 p, int c) {
 // if ( p.x>.25&& p.x<.75 && p.y>.0&& p.y<1. )  // normal char box
    if ( p.x>.25&& p.x<.75 && p.y>.1&& p.y<.85 ) // thighly y-clamped to allow dense text
        char_id = c, char_pos = p;
    return vec4(0);
}
vec4 draw_char() {
    int c = char_id; vec2 p = char_pos;
    return c < 0 
        ? vec4(0,0,0,1e5)
        : textureGrad( iChannel0, p/16. + fract( vec2(c, 15-c/16) / 16. ), 
                       dFdx(p/16.),dFdy(p/16.) );
}
#endif

// --- display int4
vec4 pInt(vec2 p, float n) {
    vec4 v = vec4(0);
    if (n < 0.) 
        v += char_(p - vec2(-.5,0), 45 ),
        n = -n;

    for (float i = 3.; i>=0.; i--) 
        n /=  9.999999, // 10., // for windows :-(
        v += char_(p - .5*vec2(i,0), 48+ int(fract(n)*10.) );
    return v;
}

// --- display float4.4
vec4 pFloat(vec2 p, float n) {
    vec4 v = vec4(0);
    if (n < 0.) v += char_(p - vec2(-.5,0), 45 ), n = -n;
    v += pInt(p,floor(n)); p.x -= 2.;
    v += char_(p, 46);      p.x -= .5;
    v += pInt(p,fract(n)*1e4);
    return v;
}

// --- chars
int CAPS=0;
#define low CAPS=32;
#define caps CAPS=0;
#define spc  U.x-=.5;
#define C(c) spc O+= char_(U,64+CAPS+c);

// --- key toggles -----------------------------------------------------
// FYI: LEFT:37  UP:38  RIGHT:39  DOWN:40   PAGEUP:33  PAGEDOWN:34  END : 35  HOME: 36
// Modifiers: SHIFT: 16 CTRL: 17 ALT: 18
// Advice:  Mode: keyToggle(key)  Action: keydown(key)+keyclick(modifier)
#define keyToggle(ascii)  ( texelFetch(iChannel3,ivec2(ascii,2),0).x > 0.)
#define keyDown(ascii)    ( texelFetch(iChannel3,ivec2(ascii,1),0).x > 0.)
#define keyClick(ascii)   ( texelFetch(iChannel3,ivec2(ascii,0),0).x > 0.)

void mainImage( out vec4 O,  vec2 uv )
{
    O -= O;
    uv /= iResolution.y;    
    vec2 U;
    int lod = int(mod(iTime,10.));
    
    U = ( uv - vec2(.0,.9) ) * 16.;  caps C(18) low C(5)C(19)C(15)C(12) caps C(-6)  // "Resol"
                             U.x-=1.; low C(19)C(3)C(18)C(5)C(5)C(14)               // "screen"
    U = ( uv - vec2(.6,.9) ) * 16.;   low C(20)C(5)C(24)C(20)                       // "text"
    U = ( uv - vec2(.85,.9) ) * 16.;  low C(12)C(15)C(4) spc C(-48+lod)             // "lod"
    U = ( uv - vec2(1.15,.9) ) * 16.;  low C(19)C(15)C(21)C(14)C(4)                 // "sound"

    U = ( uv - vec2(.0,.6) ) * 16.;  caps C(13) low C(15)C(21)C(19)C(5) caps C(-6)  // "mouse"
    U = ( uv - vec2(.5,.6) ) * 16.;  caps C(20) low C(9)C(13)C(5) caps C(-6)        // "Time"
    U = ( uv - vec2(1.45,.55) ) * 16.;  caps C(11) low C(5)C(25) caps C(-6)         // "Key"

    
    U = ( uv - vec2(.1,.8) ) * 8.;        // --- column 1
    O += pInt(U, iResolution.x);  U.y += .8;   // window resolution
    O += pInt(U, iResolution.y);  U.y += .8;
    O += pFloat((U-vec2(-1,.35))*1.5, iResolution.x/iResolution.y);  U.y += .8;
  //O += pInt(U, iResolution.z);  U.y += .8;
    U.y += .8;
    O += pInt(U, iMouse.x);  U.y += .8;        // mouse location
    O += pInt(U, iMouse.y);  U.y += .8;
    U.y += .4;
    O += pInt(U, iMouse.z);  U.y += .8;        // last mouse-click location 
    O += pInt(U, iMouse.w);  U.y += .8;
    
    U = ( uv - vec2(.5,.8) ) * 8.;        // --- column 2

    O += pInt(U, iChannel1_Resolution.x);  U.y += .8; // texture ( video )
    O += pInt(U, iChannel1_Resolution.y);  U.y += .8; // see LOD in column 2b
    //O += pInt(U, iChannelResolution[1].z);  U.y += .8;
    U.y += .8;

    O += pFloat(U, iTime);         U.y += .8;  // time
    O += pInt(U, float(iFrame));   U.y += .8;  // iFrame
    O += pFloat(U, 1./iTimeDelta); U.y += .8;  // FPS
    
    U.y += .8;

    O += pInt(U, iDate.w/3600.);          U.x -= 2.5;
    O += pInt(U, mod(iDate.w/60.,60.));   U.x -= 2.5;
    O += pFloat(U, mod(iDate.w,60.));  

    U = ( uv - vec2(.8,.8) ) * 8.;        // --- column 2b

    ivec2 S = textureSize(iChannel1,lod);
    O += pInt(U, float(S.x));  U.y += .8; // texture LOD
    O += pInt(U, float(S.y));  U.y += .8;

    U = ( uv - vec2(.6,.2) ) * 16.;  caps C(8) low C(15)C(21)C(18)  // "Hour"
    U = ( uv - vec2(.95,.2) ) * 16.;  caps C(13) low C(9)C(14)      // "Min"
    U = ( uv - vec2(1.25,.2) ) * 16.;  caps C(19) low C(5)C(3)      // "Sec"

    U = ( uv - vec2(1.1,.8) ) * 8.;        // --- column 3

    O += pInt(U, iChannel2_Resolution.x);  U.y += .8; // sound texture
    O += pInt(U, iChannel2_Resolution.y);  U.y += .8;
    // O += pInt(U, iChannelResolution[2].z);  U.y += .8;

    //O += pInt(U, iSampleRate/1e4);          U.x -= 2.; // iSampleRate
    //O += pInt(U, mod(iSampleRate,1e4)); 

    U = ( uv - vec2(1.4,.45) ) * 8.;       // --- column 4
    
    bool b = false;
    for (int i=0; i<256; i++)
        if (keyClick(i) )  O += pInt(U, float(i)),  // keypressed ascii 
                           b=true, U.y += .1 *8.;
    if (b==false) O += pInt(U, -1.);
        
    O += draw_char();
    O = O.xxxx;
}
out vec4 fragColor;
void main() { mainImage(fragColor, gl_FragCoord.xy); }
