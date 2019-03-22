// Created by inigo quilez - iq/2016
// License Creative Commons Attribution-NonCommercial-ShareAlike 3.0


// The old school demoscene effect, deformation feedback. An article from 2002
// describing it: http://iquilezles.org/www/articles/feedbackfx/feedbackfx.htm



const float th = 0.06;

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 q = fragCoord / iResolution.xy;
    vec3 col = texture( iChannel0, q ).xyz;

    if( abs(q.x-0.5)<(0.5-th) && abs(q.y-0.5)<(0.5-th) )
    {
        vec2 p = (-iResolution.xy + 2.0*fragCoord) / iResolution.y;

        // replace this with any cool plane deformation
        vec2 uv = p/dot(p,p) + cos( iTime + vec2(0.0,2.0));
        
        col = 0.7 * texture( iChannel0, fract(uv) ).xyz;
    }
    else
    {
        col = texture( iChannel1, 0.5*q ).xyz;
    }

	fragColor = vec4(col,1.0);
}

out vec4 fragColor;
void main() { mainImage(fragColor, gl_FragCoord.xy); }
