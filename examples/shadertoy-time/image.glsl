// Created by inigo quilez - iq/2013
// License Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License.


// See also:
//
// Input - Keyboard    : https://www.shadertoy.com/view/lsXGzf
// Input - Microphone  : https://www.shadertoy.com/view/llSGDh
// Input - Mouse       : https://www.shadertoy.com/view/Mss3zH
// Input - Sound       : https://www.shadertoy.com/view/Xds3Rr
// Input - SoundCloud  : https://www.shadertoy.com/view/MsdGzn
// Input - Time        : https://www.shadertoy.com/view/lsXGz8
// Input - TimeDelta   : https://www.shadertoy.com/view/lsKGWV
// Inout - 3D Texture  : https://www.shadertoy.com/view/4llcR4


float sdLine( in vec2 p, in vec2 a, in vec2 b )
{
	vec2 pa = p-a, ba = b-a;
	float h = clamp( dot(pa,ba)/dot(ba,ba), 0.0, 1.0 );
	return length( pa - ba*h );
}

vec3 line( in vec3 buf, in vec2 a, in vec2 b, in vec2 p, in vec2 w, in vec4 col )
{
   float f = sdLine( p, a, b );
   float g = fwidth(f)*w.y;
   return mix( buf, col.xyz, col.w*(1.0-smoothstep(w.x-g, w.x+g, f)) );
}

vec3 hash3( float n ) { return fract(sin(vec3(n,n+1.0,n+2.0))*43758.5453123); }

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // get time
    float mils = fract(iDate.w);
	float secs = mod( floor(iDate.w),        60.0 );
	float mins = mod( floor(iDate.w/60.0),   60.0 );
	float hors = mod( floor(iDate.w/3600.0), 24.0 );
    
    // enable this for subsecond resolution
    //secs += mils;

	vec2 uv = (2.0*fragCoord.xy-iResolution.xy)/min(iResolution.y,iResolution.x);

	float r = length( uv );
	float a = atan( uv.y, uv.x )+3.1415926;
    
	// background color
	vec3 nightColor = vec3( 0.2, 0.2, 0.2 ) + 0.1*uv.y;
	vec3 dayColor   = vec3( 0.5, 0.6, 0.7 ) + 0.2*uv.y;
	vec3 col = mix( nightColor, dayColor, smoothstep( 5.0, 7.0, hors ) - 
				                          smoothstep(19.0,21.0, hors ) );

    // inner watch body	
	col = mix( col, vec3(0.9-0.4*pow(r,4.0)), 1.0-smoothstep(0.94,0.95,r) );

    // 5 minute marks	
	float f = abs(2.0*fract(0.5+a*60.0/6.2831)-1.0);
	float g = 1.0-smoothstep( 0.0, 0.1, abs(2.0*fract(0.5+a*12.0/6.2831)-1.0) );
	float w = fwidth(f);
	f = 1.0 - smoothstep( 0.1*g+0.05-w, 0.1*g+0.05+w, f );
	f *= smoothstep( 0.85, 0.86, r+0.05*g ) - smoothstep( 0.94, 0.95, r );
	col = mix( col, vec3(0.0), f );

	// seconds hand
	vec2 dir;
	dir = vec2( sin(6.2831*secs/60.0), cos(6.2831*secs/60.0) );
    col = line( col, vec2(0.0), dir*0.9, uv+0.05, vec2(0.005,4.0), vec4(0.0,0.0,0.0,0.2) );
    col = line( col, vec2(0.0), dir*0.0, uv+0.05, vec2(0.055,4.0), vec4(0.0,0.0,0.0,0.2) ); 
    col = line( col, vec2(0.0), dir*0.9, uv,      vec2(0.005,1.0), vec4(0.5,0.0,0.0,1.0) );

	// minutes hand
	dir = vec2( sin(6.2831*mins/60.0), cos(6.2831*mins/60.0) );
    col = line( col, vec2(0.0), dir*0.7, uv+0.05, vec2(0.015,4.0), vec4(0.0,0.0,0.0,0.2) );
    col = line( col, vec2(0.0), dir*0.7, uv,      vec2(0.015,1.0), vec4(0.0,0.0,0.0,1.0) );

    // hours hand
	dir = vec2( sin(6.2831*hors/12.0), cos(6.2831*hors/12.0) );
    col = line( col, vec2(0.0), dir*0.4, uv+0.05, vec2(0.015,4.0), vec4(0.0,0.0,0.0,0.2) );
    col = line( col, vec2(0.0), dir*0.4, uv,      vec2(0.015,1.0), vec4(0.0,0.0,0.0,1.0) );

    // center mini circle	
	col = mix( col, vec3(0.5), 1.0-smoothstep(0.050,0.055,r) );
	col = mix( col, vec3(0.0), 1.0-smoothstep(0.005,0.01,abs(r-0.055)) );

    // border of watch
	col = mix( col, vec3(0.0), 1.0-smoothstep(0.01,0.02,abs(r-0.95)) );

    // dithering    
    col += (1.0/255.0)*hash3(uv.x+13.0*uv.y);

	fragColor = vec4( col,1.0 );
}
out vec4 fragColor;
void main() { mainImage(fragColor, gl_FragCoord.xy); }
