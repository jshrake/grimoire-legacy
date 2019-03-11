#include "common.glsl"

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = 3.0 * (fragCoord.xy - 0.5 * iResolution.xy) / iResolution.xx;
  vec3 dir = normalize(rotY(iTime * 0.02) * vec3(uv, 1.0));
  fragColor = texture(iChannel0, dir);
}

out vec4 fragColor;
void main() { mainImage(fragColor, gl_FragCoord.xy); }
