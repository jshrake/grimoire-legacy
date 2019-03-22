const float GLYPHS_PER_UV = 16.;
const float FONT_TEX_BIAS = 127./255.;

vec2 font_from_screen(vec2 tpos, float font_size, vec2 char_pos) {    
    return (tpos/font_size + char_pos + 0.5)/GLYPHS_PER_UV;
}

vec3 sample_grad_dist(vec2 uv, float font_size) {
    
    vec3 grad_dist = (textureLod(iChannel0, uv, 0.).yzw - FONT_TEX_BIAS) * font_size;

    grad_dist.y = -grad_dist.y;
    grad_dist.xy = normalize(grad_dist.xy + 1e-5);
    
    return grad_dist;
    
}

const vec2 TABLE_RES = vec2(16, 16);
const vec2 CELL_DIMS = vec2(1.5, 1);
const vec2 TABLE_DIMS = TABLE_RES * CELL_DIMS;

float MARGIN = 2.0;

void mainImage( out vec4 fragColor, in vec2 fragCoord ) {

    vec3 color = vec3(1);
    
    float scl = 1.0 / floor( iResolution.y / (TABLE_RES.y + MARGIN) );    
    
    vec2 p = (fragCoord - 0.5 - 0.5*iResolution.xy)*scl + 0.5*TABLE_DIMS;
    
    vec2 b = abs(p - 0.5*TABLE_DIMS) - 0.5*TABLE_DIMS;
    float dbox = max(b.x, b.y);
    
    if (dbox < 0.) {
        
        vec2 cell = floor(p/CELL_DIMS);
        
        int keycode = int(cell.x) + int((15.-cell.y)*16.);
        
        bool hit = false;
        
        bvec3 ktex;
        for (int i=0; i<3; ++i) {
            ktex[i] = texelFetch(iChannel1, ivec2(keycode, i), 0).x > 0.;
        }
        
        color = ktex[0] ? vec3(1, 0.25, 0.25) : vec3(0.8);
                       
        float dtext = 1e5;

        const int place[3] = int[3]( 100, 10, 1 );
        bool nonzero = false;
        
        float i0 = (keycode >= 100 ? 1.0 : keycode >= 10 ? 1.5 : 2.0);

        for (int i=0; i<3; ++i) {
            
            int digit = keycode / place[i];
            keycode -= digit * place[i];
            
            if (digit > 0 || nonzero || i == 2) {

                vec2 p0 = (cell + vec2(0.5 + (float(i)-i0)*0.3, 0.5))*CELL_DIMS;
                vec2 uv = font_from_screen((p - p0), 1.0, vec2(digit, 12));
                vec2 dbox = abs(p - p0) - 0.5;
                dtext = min(dtext, max(max(dbox.x, dbox.y), sample_grad_dist(uv, 1.0).z));
                nonzero = true;
                
            }

        }
        
        vec3 textcolor = ktex[2] ? vec3(0) : (ktex[1] || ktex[0]) ? vec3(1) : vec3(0.9);
        
        if (ktex[1]) {
            vec2 q = (p/CELL_DIMS - cell);
            float b = pow( 24.0*q.x*q.y*(1.0-q.x)*(1.0-q.y), 0.5 );
            color = mix(color, vec3(1.0, 1.0, 0.25), 1.0-b);
        }
        
        color = mix(color, textcolor, smoothstep(0.5*scl, -0.5*scl, dtext));        
                    
        vec2 p0 = floor(p/CELL_DIMS + 0.5)*CELL_DIMS;
        vec2 dp0 = abs(p - p0);
        dbox = min(abs(dbox), min(abs(dp0.x), abs(dp0.y)));
        
    }
    
    color *= smoothstep(0., scl, abs(dbox));
 
    fragColor = vec4(color, 1);
    
    
}

out vec4 fragColor;
void main() { mainImage(fragColor, gl_FragCoord.xy); }
